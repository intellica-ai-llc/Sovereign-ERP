//! Scenario tests every `LedgerPort` engine adapter must pass.
//!
//! Usage from an adapter crate: `sov_ledger_port::suite::run(|| MyEngineAdapter::connect_fresh());`
//! The factory must return a fresh, empty ledger each call. Panics with a scenario name on failure.
//!
//! This module is test support shipped as library code on purpose, so a scenario failing means
//! panicking: unwrap, expect and panic are the assertion mechanism here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::invariants::{check_invariants, check_legal_move};
use crate::{
    AccountId, LedgerError, LedgerPort, NewTransfer, TransferId, TransferKind, TransferState,
};

const A: AccountId = AccountId(1); // guarded in these scenarios
const B: AccountId = AccountId(2);
const C: AccountId = AccountId(3);

fn fresh<L: LedgerPort>(new: &impl Fn() -> L) -> L {
    let mut l = new();
    l.open_account(A, true).unwrap();
    l.open_account(B, false).unwrap();
    l.open_account(C, false).unwrap();
    l
}

fn t(id: u64, d: AccountId, c: AccountId, amount: u64, kind: TransferKind) -> NewTransfer {
    NewTransfer {
        id: TransferId(id),
        debit: d,
        credit: c,
        amount,
        kind,
    }
}

fn bal(l: &impl LedgerPort, id: AccountId) -> (u64, u64, u64, u64) {
    let a = l
        .account(id)
        .unwrap_or_else(|| panic!("account {id:?} missing"));
    (
        a.debits_posted,
        a.credits_posted,
        a.debits_pending,
        a.credits_pending,
    )
}

fn invariants_hold(l: &impl LedgerPort, scenario: &str) {
    if let Err(v) = check_invariants(l) {
        panic!("[{scenario}] invariant broken: {v:?}");
    }
}

/// Run every scenario against engines built by `new`.
///
/// # Panics
///
/// When the engine violates the contract; the message names the scenario.
pub fn run<L: LedgerPort>(new: impl Fn() -> L) {
    single_phase_moves_posted_balances(&new);
    pending_reserves_then_post(&new);
    pending_then_void_restores_everything(&new);
    guard_blocks_overdraw_including_pending(&new);
    terminal_states_are_terminal(&new);
    rejects_malformed_requests(&new);
    failed_calls_change_nothing(&new);
    accounts_are_unique(&new);
}

fn single_phase_moves_posted_balances<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    l.create_transfer(t(1, B, A, 10, TransferKind::Single))
        .unwrap();
    assert_eq!(bal(&l, B), (10, 0, 0, 0), "single_phase: debit side");
    assert_eq!(bal(&l, A), (0, 10, 0, 0), "single_phase: credit side");
    assert_eq!(
        l.transfer(TransferId(1)).unwrap().state,
        TransferState::Posted
    );
    invariants_hold(&l, "single_phase");
}

fn pending_reserves_then_post<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    l.create_transfer(t(1, B, C, 7, TransferKind::Pending))
        .unwrap();
    assert_eq!(bal(&l, B), (0, 0, 7, 0), "pending: reserved on debit side");
    assert_eq!(bal(&l, C), (0, 0, 0, 7), "pending: reserved on credit side");
    invariants_hold(&l, "pending");
    l.post_pending(TransferId(1)).unwrap();
    assert_eq!(bal(&l, B), (7, 0, 0, 0), "post: debit side");
    assert_eq!(bal(&l, C), (0, 7, 0, 0), "post: credit side");
    assert_eq!(
        l.transfer(TransferId(1)).unwrap().state,
        TransferState::Posted
    );
    invariants_hold(&l, "post");
}

fn pending_then_void_restores_everything<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    l.create_transfer(t(1, B, C, 5, TransferKind::Pending))
        .unwrap();
    l.void_pending(TransferId(1)).unwrap();
    assert_eq!(bal(&l, B), (0, 0, 0, 0), "void: debit side released");
    assert_eq!(bal(&l, C), (0, 0, 0, 0), "void: credit side released");
    assert_eq!(
        l.transfer(TransferId(1)).unwrap().state,
        TransferState::Voided
    );
    invariants_hold(&l, "void");
}

fn guard_blocks_overdraw_including_pending<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    // A holds nothing, so any debit from A is refused.
    assert_eq!(
        l.create_transfer(t(1, A, B, 1, TransferKind::Single)),
        Err(LedgerError::GuardViolation(A)),
        "guard: empty account"
    );
    l.create_transfer(t(2, B, A, 10, TransferKind::Single))
        .unwrap(); // A now holds 10
    l.create_transfer(t(3, A, C, 6, TransferKind::Pending))
        .unwrap(); // reserves 6
                   // 6 pending + 5 > 10: the reservation counts against the guard.
    assert_eq!(
        l.create_transfer(t(4, A, C, 5, TransferKind::Single)),
        Err(LedgerError::GuardViolation(A)),
        "guard: pending counts"
    );
    l.create_transfer(t(5, A, C, 4, TransferKind::Single))
        .unwrap(); // exactly at the limit
    assert_eq!(
        l.create_transfer(t(6, A, C, 1, TransferKind::Pending)),
        Err(LedgerError::GuardViolation(A)),
        "guard: at limit"
    );
    // Voiding frees the reservation, so room reappears.
    l.void_pending(TransferId(3)).unwrap();
    l.create_transfer(t(7, A, C, 6, TransferKind::Single))
        .unwrap();
    invariants_hold(&l, "guard");
}

fn terminal_states_are_terminal<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    l.create_transfer(t(1, B, C, 3, TransferKind::Pending))
        .unwrap();
    l.post_pending(TransferId(1)).unwrap();
    let posted = TransferState::Posted;
    assert_eq!(
        l.post_pending(TransferId(1)),
        Err(LedgerError::NotPending {
            id: TransferId(1),
            state: posted
        }),
        "terminal: double post"
    );
    assert_eq!(
        l.void_pending(TransferId(1)),
        Err(LedgerError::NotPending {
            id: TransferId(1),
            state: posted
        }),
        "terminal: void after post"
    );

    l.create_transfer(t(2, B, C, 3, TransferKind::Pending))
        .unwrap();
    l.void_pending(TransferId(2)).unwrap();
    let voided = TransferState::Voided;
    assert_eq!(
        l.post_pending(TransferId(2)),
        Err(LedgerError::NotPending {
            id: TransferId(2),
            state: voided
        }),
        "terminal: post after void"
    );
    assert_eq!(
        l.void_pending(TransferId(2)),
        Err(LedgerError::NotPending {
            id: TransferId(2),
            state: voided
        }),
        "terminal: double void"
    );

    l.create_transfer(t(3, B, C, 3, TransferKind::Single))
        .unwrap();
    assert!(
        matches!(
            l.post_pending(TransferId(3)),
            Err(LedgerError::NotPending { .. })
        ),
        "terminal: post a single-phase transfer"
    );
    invariants_hold(&l, "terminal");
}

fn rejects_malformed_requests<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    assert_eq!(
        l.create_transfer(t(1, B, C, 0, TransferKind::Single)),
        Err(LedgerError::ZeroAmount)
    );
    assert_eq!(
        l.create_transfer(t(1, B, B, 1, TransferKind::Single)),
        Err(LedgerError::SameAccount)
    );
    assert_eq!(
        l.create_transfer(t(1, B, AccountId(99), 1, TransferKind::Single)),
        Err(LedgerError::UnknownAccount(AccountId(99)))
    );
    assert_eq!(
        l.create_transfer(t(1, AccountId(99), B, 1, TransferKind::Single)),
        Err(LedgerError::UnknownAccount(AccountId(99)))
    );
    l.create_transfer(t(1, B, C, 1, TransferKind::Single))
        .unwrap();
    assert_eq!(
        l.create_transfer(t(1, B, C, 1, TransferKind::Single)),
        Err(LedgerError::TransferExists(TransferId(1))),
        "duplicate id"
    );
    assert_eq!(
        l.post_pending(TransferId(42)),
        Err(LedgerError::UnknownTransfer(TransferId(42)))
    );
    assert_eq!(
        l.void_pending(TransferId(42)),
        Err(LedgerError::UnknownTransfer(TransferId(42)))
    );
    invariants_hold(&l, "malformed");

    let mut l = fresh(new);
    l.create_transfer(t(1, B, C, u64::MAX, TransferKind::Single))
        .unwrap();
    assert_eq!(
        l.create_transfer(t(2, B, C, 1, TransferKind::Single)),
        Err(LedgerError::Overflow),
        "overflow is an error, never a wrap"
    );
    assert_eq!(bal(&l, B), (u64::MAX, 0, 0, 0), "overflow: state unchanged");
    invariants_hold(&l, "overflow");
}

fn failed_calls_change_nothing<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    l.create_transfer(t(1, B, A, 10, TransferKind::Single))
        .unwrap();
    l.create_transfer(t(2, A, C, 4, TransferKind::Pending))
        .unwrap();
    let (accounts, transfers) = (l.accounts(), l.transfers());
    let _ = l.create_transfer(t(3, A, C, 100, TransferKind::Single)); // guard
    let _ = l.create_transfer(t(1, B, C, 1, TransferKind::Single)); // duplicate
    let _ = l.post_pending(TransferId(1)); // not pending
    let _ = l.void_pending(TransferId(9)); // unknown
    assert_eq!(
        l.accounts(),
        accounts,
        "atomicity: accounts changed after failed calls"
    );
    assert_eq!(
        l.transfers(),
        transfers,
        "atomicity: transfers changed after failed calls"
    );
    check_legal_move(&transfers, &l.transfers()).unwrap();
}

fn accounts_are_unique<L: LedgerPort>(new: &impl Fn() -> L) {
    let mut l = fresh(new);
    assert_eq!(
        l.open_account(A, false),
        Err(LedgerError::DuplicateAccount(A))
    );
    assert!(
        l.account(A).unwrap().guarded,
        "reopening must not change the guard flag"
    );
    assert_eq!(l.accounts().len(), 3);
}
