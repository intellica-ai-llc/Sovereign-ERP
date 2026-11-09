//! Appendix D invariants as functions over any [`LedgerPort`].

use crate::{AccountId, AccountView, LedgerPort, TransferId, TransferRecord, TransferState};
use std::collections::HashMap;

/// The properties Appendix D requires of every ledger state and step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Invariant {
    /// Posted debits equal posted credits; pending debits equal pending credits.
    I1Conservation,
    /// Guarded accounts never owe more than they hold.
    I2Guard,
    /// Pending totals equal the sum of pending transfers.
    I3PendingExact,
    /// Posted totals equal the sum of posted transfers.
    I4PostedExact,
    /// Transfer fields never change; posted and voided are terminal; pending may become either.
    LegalMove,
}

/// A broken invariant with a human-readable explanation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantViolation {
    /// Which invariant broke.
    pub invariant: Invariant,
    /// What exactly was wrong.
    pub detail: String,
}

fn violation(invariant: Invariant, detail: impl Into<String>) -> InvariantViolation {
    InvariantViolation {
        invariant,
        detail: detail.into(),
    }
}

/// State invariants I1 to I4, checked in that order.
///
/// # Errors
///
/// The first [`InvariantViolation`] found.
pub fn check_invariants<L: LedgerPort + ?Sized>(l: &L) -> Result<(), InvariantViolation> {
    let accounts = l.accounts();
    let transfers = l.transfers();

    let sum = |f: fn(&AccountView) -> u64| accounts.iter().map(|a| u128::from(f(a))).sum::<u128>();
    if sum(|a| a.debits_posted) != sum(|a| a.credits_posted) {
        return Err(violation(
            Invariant::I1Conservation,
            "posted debits != posted credits",
        ));
    }
    if sum(|a| a.debits_pending) != sum(|a| a.credits_pending) {
        return Err(violation(
            Invariant::I1Conservation,
            "pending debits != pending credits",
        ));
    }

    for a in accounts.iter().filter(|a| a.guarded) {
        if u128::from(a.debits_posted) + u128::from(a.debits_pending) > u128::from(a.credits_posted)
        {
            return Err(violation(
                Invariant::I2Guard,
                format!("{:?} owes more than it holds", a.id),
            ));
        }
    }

    let mut pend_d: HashMap<AccountId, u128> = HashMap::new();
    let mut pend_c: HashMap<AccountId, u128> = HashMap::new();
    let mut post_d: HashMap<AccountId, u128> = HashMap::new();
    let mut post_c: HashMap<AccountId, u128> = HashMap::new();
    for t in &transfers {
        let (d, c) = match t.state {
            TransferState::Pending => (&mut pend_d, &mut pend_c),
            TransferState::Posted => (&mut post_d, &mut post_c),
            TransferState::Voided => continue,
        };
        *d.entry(t.debit).or_default() += u128::from(t.amount);
        *c.entry(t.credit).or_default() += u128::from(t.amount);
    }
    let get = |m: &HashMap<AccountId, u128>, id: AccountId| m.get(&id).copied().unwrap_or(0);
    for a in &accounts {
        if u128::from(a.debits_pending) != get(&pend_d, a.id)
            || u128::from(a.credits_pending) != get(&pend_c, a.id)
        {
            return Err(violation(
                Invariant::I3PendingExact,
                format!("{:?} pending totals != sum of pending transfers", a.id),
            ));
        }
    }
    for a in &accounts {
        if u128::from(a.debits_posted) != get(&post_d, a.id)
            || u128::from(a.credits_posted) != get(&post_c, a.id)
        {
            return Err(violation(
                Invariant::I4PostedExact,
                format!("{:?} posted totals != sum of posted transfers", a.id),
            ));
        }
    }
    Ok(())
}

/// The `LegalMove` action property over one step (transfer lists as returned by `transfers()`).
///
/// # Errors
///
/// The first [`InvariantViolation`] found.
pub fn check_legal_move(
    before: &[TransferRecord],
    after: &[TransferRecord],
) -> Result<(), InvariantViolation> {
    let after_by_id: HashMap<TransferId, &TransferRecord> =
        after.iter().map(|t| (t.id, t)).collect();
    for b in before {
        let Some(a) = after_by_id.get(&b.id) else {
            return Err(violation(
                Invariant::LegalMove,
                format!("{:?} disappeared", b.id),
            ));
        };
        if (a.debit, a.credit, a.amount) != (b.debit, b.credit, b.amount) {
            return Err(violation(
                Invariant::LegalMove,
                format!("{:?} changed its debit, credit or amount", b.id),
            ));
        }
        let legal = match b.state {
            TransferState::Posted => a.state == TransferState::Posted,
            TransferState::Voided => a.state == TransferState::Voided,
            TransferState::Pending => true,
        };
        if !legal {
            return Err(violation(
                Invariant::LegalMove,
                format!("{:?} left a terminal state", b.id),
            ));
        }
    }
    Ok(())
}
