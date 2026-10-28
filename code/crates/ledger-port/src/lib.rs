//! Ledger Port (arc42 5.2 P2, Appendix D, ADR 002).
//!
//! The engine-neutral contract the Truth Plane programs against, so TigerBeetle (provisional
//! engine) can be replaced without touching callers. This crate holds:
//! - the port trait and its value types,
//! - [`MemLedger`], a pure in-memory reference implementation (the executable form of Appendix D),
//! - [`invariants`]: I1 to I4 and LegalMove as functions over any `LedgerPort`,
//! - [`suite`]: scenario tests any engine adapter must pass,
//! - [`model`]: a bounded exhaustive model checker over any `LedgerPort + Clone`.
//!
//! Scope is Appendix D v0: single ledger and currency, two-phase transfers, one guard rule.
//! Out of scope until later rounds: multi-currency, linked transfers, account closure, time.

pub mod invariants;
pub mod mem;
pub mod model;
pub mod suite;

pub use mem::{Defect, MemLedger};

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferId(pub u64);

/// Lifecycle of a transfer once it exists. "None" (never created) is represented by absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferState {
    Pending,
    Posted,
    Voided,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferKind {
    /// Immediate: moves posted balances at once.
    Single,
    /// Two-phase: reserves in the pending totals until posted or voided.
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewTransfer {
    pub id: TransferId,
    pub debit: AccountId,
    pub credit: AccountId,
    pub amount: u64,
    pub kind: TransferKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransferRecord {
    pub id: TransferId,
    pub state: TransferState,
    pub debit: AccountId,
    pub credit: AccountId,
    pub amount: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountView {
    pub id: AccountId,
    /// Guard rule: debits (posted + pending) may not exceed credits posted.
    pub guarded: bool,
    pub debits_posted: u64,
    pub credits_posted: u64,
    pub debits_pending: u64,
    pub credits_pending: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    DuplicateAccount(AccountId),
    UnknownAccount(AccountId),
    TransferExists(TransferId),
    UnknownTransfer(TransferId),
    SameAccount,
    ZeroAmount,
    GuardViolation(AccountId),
    NotPending { id: TransferId, state: TransferState },
    Overflow,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LedgerError {}

/// The contract. Implementations must make every method atomic: on `Err`, state is unchanged.
pub trait LedgerPort {
    fn open_account(&mut self, id: AccountId, guarded: bool) -> Result<(), LedgerError>;
    fn create_transfer(&mut self, t: NewTransfer) -> Result<(), LedgerError>;
    fn post_pending(&mut self, id: TransferId) -> Result<(), LedgerError>;
    fn void_pending(&mut self, id: TransferId) -> Result<(), LedgerError>;
    /// All accounts, sorted by id.
    fn accounts(&self) -> Vec<AccountView>;
    /// All transfers ever created, sorted by id.
    fn transfers(&self) -> Vec<TransferRecord>;

    fn account(&self, id: AccountId) -> Option<AccountView> {
        self.accounts().into_iter().find(|a| a.id == id)
    }
    fn transfer(&self, id: TransferId) -> Option<TransferRecord> {
        self.transfers().into_iter().find(|t| t.id == id)
    }
}
