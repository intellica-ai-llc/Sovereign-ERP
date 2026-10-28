//! Bounded exhaustive model checker over any cloneable [`LedgerPort`] (arc42 Appendix D, spike 1).
//!
//! Mirrors the TLC run: all accounts exist from the start, transfer ids and amounts are bounded,
//! and every enabled action is applied from every reachable state (breadth first). Each step is
//! checked for LegalMove, and each new state for I1 to I4.

use crate::invariants::{check_invariants, check_legal_move, Invariant, InvariantViolation};
use crate::*;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug)]
pub struct Bounds {
    pub accounts: u32,
    pub ids: u64,
    pub max_amt: u64,
    pub guarded: Vec<u32>,
}

/// The constants of the recorded TLC run: 3 accounts, 3 transfer ids, max amount 2, a1 guarded.
pub fn tlc_bounds() -> Bounds {
    Bounds { accounts: 3, ids: 3, max_amt: 2, guarded: vec![1] }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CreatePosted { id: u64, debit: u32, credit: u32, amount: u64 },
    CreatePending { id: u64, debit: u32, credit: u32, amount: u64 },
    Post(u64),
    Void(u64),
}

pub fn actions(b: &Bounds) -> Vec<Action> {
    let mut v = Vec::new();
    for id in 1..=b.ids {
        for debit in 1..=b.accounts {
            for credit in 1..=b.accounts {
                for amount in 1..=b.max_amt {
                    v.push(Action::CreatePosted { id, debit, credit, amount });
                    v.push(Action::CreatePending { id, debit, credit, amount });
                }
            }
        }
        v.push(Action::Post(id));
        v.push(Action::Void(id));
    }
    v
}

pub fn apply<L: LedgerPort>(l: &mut L, a: Action) -> Result<(), LedgerError> {
    let new = |id, debit, credit, amount, kind| NewTransfer {
        id: TransferId(id),
        debit: AccountId(debit),
        credit: AccountId(credit),
        amount,
        kind,
    };
    match a {
        Action::CreatePosted { id, debit, credit, amount } => l.create_transfer(new(id, debit, credit, amount, TransferKind::Single)),
        Action::CreatePending { id, debit, credit, amount } => l.create_transfer(new(id, debit, credit, amount, TransferKind::Pending)),
        Action::Post(id) => l.post_pending(TransferId(id)),
        Action::Void(id) => l.void_pending(TransferId(id)),
    }
}

/// Build the initial ledger of the model from an empty engine.
pub fn open_accounts<L: LedgerPort>(mut l: L, b: &Bounds) -> L {
    for a in 1..=b.accounts {
        l.open_account(AccountId(a), b.guarded.contains(&a)).expect("fresh ledger");
    }
    l
}

type Key = (Vec<AccountView>, Vec<TransferRecord>);

fn key<L: LedgerPort>(l: &L) -> Key {
    (l.accounts(), l.transfers())
}

#[derive(Debug)]
pub struct Found {
    pub violation: InvariantViolation,
    pub trace: Vec<Action>,
}

#[derive(Debug)]
pub struct Exploration {
    /// Initial state plus every enabled action application (TLC's "states generated").
    pub generated: usize,
    pub distinct: usize,
    /// Enabled `Post`/`Void` applications. TLC's count of the spec as written weights each of these
    /// by `accounts^2 * max_amt` (see the test), because of the way `Next` is laid out in Ledger.tla.
    pub post_void_applications: usize,
    /// Longest shortest-path, in steps. TLC's reported "depth" counts the initial state too (steps + 1).
    pub max_steps: usize,
    pub violation: Option<Found>,
    /// Reachability probes so a clean result is not vacuous.
    pub saw_pending: bool,
    pub saw_posted_from_pending: bool,
    pub saw_voided: bool,
    pub saw_guard_rejection: bool,
}

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
            match apply(&mut next, a) {
                Err(LedgerError::GuardViolation(_)) => {
                    out.saw_guard_rejection = true;
                    continue;
                }
                Err(_) => continue,
                Ok(()) => {}
            }
            out.generated += 1;
            match a {
                Action::CreatePending { .. } => out.saw_pending = true,
                Action::Post(_) => out.saw_posted_from_pending = true,
                Action::Void(_) => out.saw_voided = true,
                _ => {}
            }
            if matches!(a, Action::Post(_) | Action::Void(_)) {
                out.post_void_applications += 1;
            }
            let nk = key(&next);
            let bad = check_legal_move(&cur_key.1, &nk.1).and_then(|_| check_invariants(&next));
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
pub fn expected(d: Defect) -> &'static [Invariant] {
    match d {
        Defect::VoidReleasesOnlyDebit | Defect::PostForgetsCredit => &[Invariant::I1Conservation],
        Defect::NoGuardCheck => &[Invariant::I2Guard],
        Defect::VoidLeaksReservation | Defect::PostLeaksPending => &[Invariant::I3PendingExact],
        Defect::PostAfterVoid => &[Invariant::LegalMove],
    }
}
