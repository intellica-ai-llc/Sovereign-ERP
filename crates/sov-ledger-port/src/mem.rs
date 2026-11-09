//! In-memory reference ledger: the executable form of `Ledger.tla`.

use crate::{
    AccountId, AccountView, LedgerError, LedgerPort, NewTransfer, TransferId, TransferKind,
    TransferRecord, TransferState,
};
use std::collections::BTreeMap;

/// Seeded defects for mutation testing; only constructible with the `fault-injection` feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Defect {
    /// Void releases the debit side of the reservation only (breaks I1).
    VoidReleasesOnlyDebit,
    /// Create transfers skip the guard check (breaks I2).
    NoGuardCheck,
    /// Posting a pending transfer forgets to credit the destination (breaks I1).
    PostForgetsCredit,
    /// Void releases nothing: a leaked reservation (breaks I3).
    VoidLeaksReservation,
    /// Post moves amounts to posted but leaves them in pending (breaks I3).
    PostLeaksPending,
    /// A voided transfer can still be posted (breaks `LegalMove`).
    PostAfterVoid,
}

/// A pure in-memory ledger: the executable form of `Ledger.tla`, and the oracle other engines are
/// tested against.
#[derive(Clone, Debug, Default)]
pub struct MemLedger {
    accounts: BTreeMap<AccountId, AccountView>,
    transfers: BTreeMap<TransferId, TransferRecord>,
    defect: Option<Defect>,
}

impl MemLedger {
    /// An empty ledger with no accounts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a known defect. Test-only: exists only with the `fault-injection` feature.
    #[cfg(feature = "fault-injection")]
    #[must_use]
    pub fn with_defect(mut self, d: Defect) -> Self {
        self.defect = Some(d);
        self
    }

    fn has(&self, d: Defect) -> bool {
        self.defect == Some(d)
    }

    fn acct(&self, id: AccountId) -> Result<&AccountView, LedgerError> {
        self.accounts
            .get(&id)
            .ok_or(LedgerError::UnknownAccount(id))
    }
}

fn add(a: u64, b: u64) -> Result<u64, LedgerError> {
    a.checked_add(b).ok_or(LedgerError::Overflow)
}

impl LedgerPort for MemLedger {
    fn open_account(&mut self, id: AccountId, guarded: bool) -> Result<(), LedgerError> {
        if self.accounts.contains_key(&id) {
            return Err(LedgerError::DuplicateAccount(id));
        }
        self.accounts.insert(
            id,
            AccountView {
                id,
                guarded,
                debits_posted: 0,
                credits_posted: 0,
                debits_pending: 0,
                credits_pending: 0,
            },
        );
        Ok(())
    }

    fn create_transfer(&mut self, t: NewTransfer) -> Result<(), LedgerError> {
        if t.amount == 0 {
            return Err(LedgerError::ZeroAmount);
        }
        if t.debit == t.credit {
            return Err(LedgerError::SameAccount);
        }
        if self.transfers.contains_key(&t.id) {
            return Err(LedgerError::TransferExists(t.id));
        }
        let d = *self.acct(t.debit)?;
        let c = *self.acct(t.credit)?;
        if d.guarded && !self.has(Defect::NoGuardCheck) {
            let owed = add(add(d.debits_posted, d.debits_pending)?, t.amount)?;
            if owed > d.credits_posted {
                return Err(LedgerError::GuardViolation(t.debit));
            }
        }
        let (mut d2, mut c2) = (d, c);
        let state = match t.kind {
            TransferKind::Single => {
                d2.debits_posted = add(d.debits_posted, t.amount)?;
                c2.credits_posted = add(c.credits_posted, t.amount)?;
                TransferState::Posted
            }
            TransferKind::Pending => {
                d2.debits_pending = add(d.debits_pending, t.amount)?;
                c2.credits_pending = add(c.credits_pending, t.amount)?;
                TransferState::Pending
            }
        };
        self.accounts.insert(t.debit, d2);
        self.accounts.insert(t.credit, c2);
        self.transfers.insert(
            t.id,
            TransferRecord {
                id: t.id,
                state,
                debit: t.debit,
                credit: t.credit,
                amount: t.amount,
            },
        );
        Ok(())
    }

    fn post_pending(&mut self, id: TransferId) -> Result<(), LedgerError> {
        let rec = *self
            .transfers
            .get(&id)
            .ok_or(LedgerError::UnknownTransfer(id))?;
        let was_pending = rec.state == TransferState::Pending;
        if !was_pending && (!self.has(Defect::PostAfterVoid) || rec.state != TransferState::Voided)
        {
            return Err(LedgerError::NotPending {
                id,
                state: rec.state,
            });
        }
        let mut d = *self.acct(rec.debit)?;
        let mut c = *self.acct(rec.credit)?;
        d.debits_posted = add(d.debits_posted, rec.amount)?;
        if !self.has(Defect::PostForgetsCredit) {
            c.credits_posted = add(c.credits_posted, rec.amount)?;
        }
        if was_pending && !self.has(Defect::PostLeaksPending) {
            d.debits_pending -= rec.amount;
            c.credits_pending -= rec.amount;
        }
        self.accounts.insert(rec.debit, d);
        self.accounts.insert(rec.credit, c);
        self.transfers.insert(
            id,
            TransferRecord {
                state: TransferState::Posted,
                ..rec
            },
        );
        Ok(())
    }

    fn void_pending(&mut self, id: TransferId) -> Result<(), LedgerError> {
        let rec = *self
            .transfers
            .get(&id)
            .ok_or(LedgerError::UnknownTransfer(id))?;
        if rec.state != TransferState::Pending {
            return Err(LedgerError::NotPending {
                id,
                state: rec.state,
            });
        }
        let mut d = *self.acct(rec.debit)?;
        let mut c = *self.acct(rec.credit)?;
        if !self.has(Defect::VoidLeaksReservation) {
            d.debits_pending -= rec.amount;
            if !self.has(Defect::VoidReleasesOnlyDebit) {
                c.credits_pending -= rec.amount;
            }
        }
        self.accounts.insert(rec.debit, d);
        self.accounts.insert(rec.credit, c);
        self.transfers.insert(
            id,
            TransferRecord {
                state: TransferState::Voided,
                ..rec
            },
        );
        Ok(())
    }

    fn accounts(&self) -> Vec<AccountView> {
        self.accounts.values().copied().collect()
    }

    fn transfers(&self) -> Vec<TransferRecord> {
        self.transfers.values().copied().collect()
    }
}
