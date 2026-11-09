//! Ledger Port (arc42 section 5.2 P2, Appendix D, ADR 002).
//!
//! The engine-neutral contract the Truth Plane programs against, so `TigerBeetle` (the provisional
//! engine) can be replaced without touching callers. This crate holds:
//! - the port trait and its value types,
//! - [`MemLedger`], a pure in-memory reference implementation (the executable form of Appendix D),
//! - [`invariants`]: I1 to I4 and `LegalMove` as functions over any [`LedgerPort`],
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

/// Identifies an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u32);

/// Identifies a transfer. Ids are chosen by the caller and must be unique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransferId(pub u64);

/// Lifecycle of a transfer once it exists. "Never created" is represented by absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferState {
    /// Reserved in the pending totals; may still be posted or voided.
    Pending,
    /// Final: moved into the posted totals.
    Posted,
    /// Final: the reservation was released.
    Voided,
}

/// How a transfer is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransferKind {
    /// Immediate: moves posted balances at once.
    Single,
    /// Two-phase: reserves in the pending totals until posted or voided.
    Pending,
}

/// A request to create a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewTransfer {
    /// Caller-chosen unique id.
    pub id: TransferId,
    /// Account debited.
    pub debit: AccountId,
    /// Account credited; must differ from `debit`.
    pub credit: AccountId,
    /// Amount in minor units; must be non-zero.
    pub amount: u64,
    /// Immediate or two-phase.
    pub kind: TransferKind,
}

/// A transfer as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransferRecord {
    /// The transfer's id.
    pub id: TransferId,
    /// Where it is in its lifecycle.
    pub state: TransferState,
    /// Account debited.
    pub debit: AccountId,
    /// Account credited.
    pub credit: AccountId,
    /// Amount in minor units.
    pub amount: u64,
}

/// An account and its balances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountView {
    /// The account's id.
    pub id: AccountId,
    /// Guard rule: debits (posted + pending) may not exceed credits posted.
    pub guarded: bool,
    /// Sum of posted debits.
    pub debits_posted: u64,
    /// Sum of posted credits.
    pub credits_posted: u64,
    /// Sum of pending (reserved) debits.
    pub debits_pending: u64,
    /// Sum of pending (reserved) credits.
    pub credits_pending: u64,
}

/// Why a ledger call was rejected. A rejected call changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    /// The account id is already in use.
    DuplicateAccount(AccountId),
    /// The account does not exist.
    UnknownAccount(AccountId),
    /// The transfer id is already in use.
    TransferExists(TransferId),
    /// The transfer does not exist.
    UnknownTransfer(TransferId),
    /// Debit and credit accounts are the same.
    SameAccount,
    /// The amount is zero.
    ZeroAmount,
    /// The debit would exceed what the guarded account holds (I2).
    GuardViolation(AccountId),
    /// The transfer is not pending, so it cannot be posted or voided.
    NotPending {
        /// The transfer addressed.
        id: TransferId,
        /// The state it is actually in.
        state: TransferState,
    },
    /// A balance would exceed `u64::MAX`.
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
    /// Create an account, optionally guarded (I2).
    ///
    /// # Errors
    ///
    /// [`LedgerError::DuplicateAccount`] if the id is taken.
    fn open_account(&mut self, id: AccountId, guarded: bool) -> Result<(), LedgerError>;

    /// Create an immediate or two-phase transfer.
    ///
    /// # Errors
    ///
    /// Zero amount, same account, duplicate id, unknown account, guard violation or overflow.
    fn create_transfer(&mut self, t: NewTransfer) -> Result<(), LedgerError>;

    /// Post a pending transfer: reservation becomes posted balance.
    ///
    /// # Errors
    ///
    /// [`LedgerError::UnknownTransfer`] or [`LedgerError::NotPending`].
    fn post_pending(&mut self, id: TransferId) -> Result<(), LedgerError>;

    /// Void a pending transfer: the reservation is released.
    ///
    /// # Errors
    ///
    /// [`LedgerError::UnknownTransfer`] or [`LedgerError::NotPending`].
    fn void_pending(&mut self, id: TransferId) -> Result<(), LedgerError>;

    /// All accounts, sorted by id.
    fn accounts(&self) -> Vec<AccountView>;

    /// All transfers ever created, sorted by id.
    fn transfers(&self) -> Vec<TransferRecord>;

    /// One account, if it exists.
    fn account(&self, id: AccountId) -> Option<AccountView> {
        self.accounts().into_iter().find(|a| a.id == id)
    }

    /// One transfer, if it exists.
    fn transfer(&self, id: TransferId) -> Option<TransferRecord> {
        self.transfers().into_iter().find(|t| t.id == id)
    }
}
