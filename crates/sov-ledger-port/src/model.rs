//! Bounded exhaustive model checker over any cloneable [`LedgerPort`] (arc42 Appendix D, spike 1).
//!
//! Mirrors the TLC run: all accounts exist from the start, transfer ids and amounts are bounded,
//! and every enabled action is applied from every reachable state (breadth first). Each step is
//! checked for `LegalMove`, and each new state for I1 to I4.

use crate::invariants::{check_invariants, check_legal_move, Invariant, InvariantViolation};
use crate::{
    AccountId, AccountView, LedgerError, LedgerPort, NewTransfer, TransferId, TransferKind,
    TransferRecord,
};
use std::collections::{HashMap, HashSet, VecDeque};

/// The finite universe the checker explores.
#[derive(Clone, Debug)]
pub struct Bounds {
    /// Accounts `1..=accounts` exist from the start.
    pub accounts: u32,
    /// Transfer ids `1..=ids` may be used.
    pub ids: u64,
    /// Amounts `1..=max_amt` are tried.
    pub max_amt: u64,
    /// Which accounts carry the guard rule.
    pub guarded: Vec<u32>,
}

/// The constants of the recorded TLC run: 3 accounts, 3 transfer ids, max amount 2, a1 guarded.
#[must_use]
pub fn tlc_bounds() -> Bounds {
    Bounds {
        accounts: 3,
        ids: 3,
        max_amt: 2,
        guarded: vec![1],
    }
}

/// One step the checker may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Create an immediate transfer.
    CreatePosted {
        /// Transfer id.
        id: u64,
        /// Debit account.
        debit: u32,
        /// Credit account.
        credit: u32,
        /// Amount.
        amount: u64,
    },
    /// Create a two-phase transfer.
    CreatePending {
        /// Transfer id.
        id: u64,
        /// Debit account.
        debit: u32,
        /// Credit account.
        credit: u32,
        /// Amount.
        amount: u64,
    },
    /// Post the pending transfer with this id.
    Post(u64),
    /// Void the pending transfer with this id.
    Void(u64),
}

/// Every action in the bounded universe.
#[must_use]
pub fn actions(b: &Bounds) -> Vec<Action> {
    let mut v = Vec::new();
    for id in 1..=b.ids {
        for debit in 1..=b.accounts {
            for credit in 1..=b.accounts {
                for amount in 1..=b.max_amt {
                    v.push(Action::CreatePosted {
                        id,
                        debit,
                        credit,
                        amount,
                    });
                    v.push(Action::CreatePending {
                        id,
                        debit,
                        credit,
                        amount,
                    });
                }
            }
        }
        v.push(Action::Post(id));
        v.push(Action::Void(id));
    }
    v
}

/// Apply one action to a ledger.
///
/// # Errors
///
/// Whatever the ledger rejects the action with.
pub fn apply<L: LedgerPort>(l: &mut L, a: Action) -> Result<(), LedgerError> {
    let new = |id, debit, credit, amount, kind| NewTransfer {
        id: TransferId(id),
        debit: AccountId(debit),
        credit: AccountId(credit),
        amount,
        kind,
    };
    match a {
        Action::CreatePosted {
            id,
            debit,
            credit,
            amount,
        } => l.create_transfer(new(id, debit, credit, amount, TransferKind::Single)),
        Action::CreatePending {
            id,
            debit,
            credit,
            amount,
        } => l.create_transfer(new(id, debit, credit, amount, TransferKind::Pending)),
        Action::Post(id) => l.post_pending(TransferId(id)),
        Action::Void(id) => l.void_pending(TransferId(id)),
    }
}

/// Build the initial ledger of the model from an empty engine.
///
/// # Errors
///
/// If the engine is not empty (an account id is already taken).
pub fn open_accounts<L: LedgerPort>(mut l: L, b: &Bounds) -> Result<L, LedgerError> {
    for a in 1..=b.accounts {
        l.open_account(AccountId(a), b.guarded.contains(&a))?;
    }
    Ok(l)
}

type Key = (Vec<AccountView>, Vec<TransferRecord>);

fn key<L: LedgerPort>(l: &L) -> Key {
    (l.accounts(), l.transfers())
}

/// A counterexample: the broken invariant and a shortest action sequence reaching it.
#[derive(Debug)]
pub struct Found {
    /// What broke.
    pub violation: InvariantViolation,
    /// Actions from the initial state to the violation.
    pub trace: Vec<Action>,
}

/// The outcome of an exhaustive exploration.
// The four `saw_*` flags are independent reachability probes, not a state machine in disguise.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct Exploration {
    /// Initial state plus every enabled action application (TLC's "states generated").
    pub generated: usize,
    /// Distinct states reached.
    pub distinct: usize,
    /// Enabled `Post`/`Void` applications. TLC's count of the spec as written weights each of these
    /// by `accounts^2 * max_amt` (see the test), because of the way `Next` is laid out in Ledger.tla.
    pub post_void_applications: usize,
    /// Longest shortest-path, in steps. TLC's reported "depth" counts the initial state too (steps + 1).
    pub max_steps: usize,
    /// The first violation found, if any.
    pub violation: Option<Found>,
    /// Probe: some transfer was created pending.
    pub saw_pending: bool,
    /// Probe: some pending transfer was posted.
    pub saw_posted_from_pending: bool,
    /// Probe: some pending transfer was voided.
    pub saw_voided: bool,
    /// Probe: the guard rejected some transfer.
    pub saw_guard_rejection: bool,
}

/// Breadth-first, exhaustive exploration from `init`. Stops at the first violation and returns a
/// shortest counterexample. `init` must already hold the model's accounts (see [`open_accounts`]).
#[must_use]
pub fn explore<L: LedgerPort + Clone>(init: L, b: &Bounds) -> Exploration {
    let acts = actions(b);
    let mut seen: HashSet<Key> = HashSet::new();
    let mut steps: HashMap<Key, usize> = HashMap::new();
    let mut parent: HashMap<Key, (Key, Action)> = HashMap::new();
    let mut queue: VecDeque<(L, Key)> = VecDeque::new();

    let k0 = key(&init);
    seen.insert(k0.clone());
    steps.insert(k0.clone(), 0);
    queue.push_back((init, k0));
    let mut out = Exploration {
        generated: 1,
        distinct: 1,
        post_void_applications: 0,
        max_steps: 0,
        violation: None,
        saw_pending: false,
        saw_posted_from_pending: false,
        saw_voided: false,
        saw_guard_rejection: false,
    };

    while let Some((cur, cur_key)) = queue.pop_front() {
        let depth = steps[&cur_key];
        for &a in &acts {
            let mut next = cur.clone();
            if let Err(rejection) = apply(&mut next, a) {
                if matches!(rejection, LedgerError::GuardViolation(_)) {
                    out.saw_guard_rejection = true;
                }
                continue;
            }
            out.generated += 1;
            match a {
                Action::CreatePending { .. } => out.saw_pending = true,
                Action::Post(_) => out.saw_posted_from_pending = true,
                Action::Void(_) => out.saw_voided = true,
                Action::CreatePosted { .. } => {}
            }
            if matches!(a, Action::Post(_) | Action::Void(_)) {
                out.post_void_applications += 1;
            }
            let nk = key(&next);
            let bad = check_legal_move(&cur_key.1, &nk.1).and_then(|()| check_invariants(&next));
            if let Err(violation) = bad {
                let mut trace = vec![a];
                let mut at = cur_key.clone();
                while let Some((prev, act)) = parent.get(&at) {
                    trace.push(*act);
                    at = prev.clone();
                }
                trace.reverse();
                out.violation = Some(Found { violation, trace });
                return out;
            }
            if seen.insert(nk.clone()) {
                out.distinct += 1;
                out.max_steps = out.max_steps.max(depth + 1);
                steps.insert(nk.clone(), depth + 1);
                parent.insert(nk.clone(), (cur_key.clone(), a));
                queue.push_back((next, nk));
            }
        }
    }
    out
}

/// Which invariants a defect is expected to trip.
#[cfg(feature = "fault-injection")]
#[must_use]
pub fn expected(d: crate::Defect) -> &'static [Invariant] {
    match d {
        crate::Defect::VoidReleasesOnlyDebit | crate::Defect::PostForgetsCredit => {
            &[Invariant::I1Conservation]
        }
        crate::Defect::NoGuardCheck => &[Invariant::I2Guard],
        crate::Defect::VoidLeaksReservation | crate::Defect::PostLeaksPending => {
            &[Invariant::I3PendingExact]
        }
        crate::Defect::PostAfterVoid => &[Invariant::LegalMove],
    }
}
