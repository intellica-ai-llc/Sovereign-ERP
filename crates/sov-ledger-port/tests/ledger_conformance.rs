//! Test code: failures surface as panics and progress is printed on purpose.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout
)]

use sov_ledger_port::invariants::{check_invariants, check_legal_move};
use sov_ledger_port::model::{self, explore, open_accounts, tlc_bounds};
use sov_ledger_port::{
    AccountId, Defect, LedgerPort, MemLedger, NewTransfer, TransferId, TransferKind,
};
use sov_sim::Rng;

fn mem() -> MemLedger {
    MemLedger::new()
}

#[test]
fn reference_ledger_passes_the_engine_conformance_suite() {
    sov_ledger_port::suite::run(mem);
}

#[test]
fn exhaustive_model_check_matches_recorded_tlc_run() {
    let b = tlc_bounds();
    let r = explore(open_accounts(mem(), &b).unwrap(), &b);
    println!(
        "generated {} distinct {} steps {}",
        r.generated, r.distinct, r.max_steps
    );
    assert!(r.violation.is_none(), "{:?}", r.violation);
    // Appendix D: 21,433 distinct states, search depth 7 (TLC counts the initial state: 6 steps).
    assert_eq!(r.distinct, 21_433, "distinct states");
    assert_eq!(r.max_steps + 1, 7, "search depth as TLC counts it");
}

#[test]
fn generated_state_count_reproduces_tlcs_number_under_its_counting_convention() {
    // Appendix D reports 713,185 states generated. In Ledger.tla as printed, `Post(i) \/ Void(i)` sit
    // inside the `\E d, c, amt` quantifier (their `\/` is one column off the outer list), so TLC
    // evaluates each once per (d, c, amt): accounts^2 * max_amt = 18 times. Weighting accordingly
    // reproduces the recorded figure exactly, which is further evidence the models agree.
    let b = tlc_bounds();
    let r = explore(open_accounts(mem(), &b).unwrap(), &b);
    let accounts = usize::try_from(b.accounts).unwrap();
    let weight = accounts.pow(2) * usize::try_from(b.max_amt).unwrap();
    let creates = r.generated - 1 - r.post_void_applications;
    assert_eq!(1 + creates + weight * r.post_void_applications, 713_185);
}

#[test]
fn model_is_not_vacuous() {
    let b = tlc_bounds();
    let r = explore(open_accounts(mem(), &b).unwrap(), &b);
    assert!(
        r.saw_pending && r.saw_posted_from_pending && r.saw_voided && r.saw_guard_rejection,
        "{r:?}"
    );
}

#[test]
fn every_seeded_defect_is_caught_by_the_intended_invariant() {
    let b = tlc_bounds();
    let defects = [
        Defect::VoidReleasesOnlyDebit,
        Defect::NoGuardCheck,
        Defect::PostForgetsCredit,
        Defect::VoidLeaksReservation,
        Defect::PostLeaksPending,
        Defect::PostAfterVoid,
    ];
    for d in defects {
        let r = explore(open_accounts(mem().with_defect(d), &b).unwrap(), &b);
        let found = r
            .violation
            .unwrap_or_else(|| panic!("defect {d:?} was NOT caught"));
        assert!(
            model::expected(d).contains(&found.violation.invariant),
            "defect {d:?} tripped {:?}, expected {:?}; trace {:?}",
            found.violation.invariant,
            model::expected(d),
            found.trace
        );
        println!(
            "{d:?}: caught by {:?}, shortest trace {} steps, {} states generated",
            found.violation.invariant,
            found.trace.len(),
            r.generated
        );
    }
}

#[test]
fn defects_that_the_conformance_suite_can_see_fail_the_suite() {
    for d in [
        Defect::VoidReleasesOnlyDebit,
        Defect::NoGuardCheck,
        Defect::PostForgetsCredit,
        Defect::VoidLeaksReservation,
        Defect::PostLeaksPending,
    ] {
        let outcome = std::panic::catch_unwind(|| {
            sov_ledger_port::suite::run(move || MemLedger::new().with_defect(d));
        });
        assert!(outcome.is_err(), "suite did not notice defect {d:?}");
    }
}

fn random_op(rng: &mut Rng, l: &mut MemLedger) {
    let acct = |rng: &mut Rng| AccountId(u32::try_from(rng.below(5)).unwrap() + 1); // ids 4,5 are unknown
    let id = TransferId(rng.below(40));
    match rng.below(4) {
        0 | 1 => {
            let kind = if rng.chance(1, 2) {
                TransferKind::Single
            } else {
                TransferKind::Pending
            };
            let amount = if rng.chance(1, 50) {
                u64::MAX - rng.below(3)
            } else {
                rng.below(60)
            };
            let _ = l.create_transfer(NewTransfer {
                id,
                debit: acct(rng),
                credit: acct(rng),
                amount,
                kind,
            });
        }
        2 => {
            let _ = l.post_pending(id);
        }
        _ => {
            let _ = l.void_pending(id);
        }
    }
}

#[test]
fn invariants_hold_over_many_random_walks() {
    let mut transfers_made = 0usize;
    for seed in 0..200u64 {
        let mut rng = Rng::new(seed);
        let mut l = MemLedger::new();
        for a in 1..=3u32 {
            l.open_account(AccountId(a), a == 1).unwrap();
        }
        // Give the guarded account something to spend.
        l.create_transfer(NewTransfer {
            id: TransferId(1000),
            debit: AccountId(2),
            credit: AccountId(1),
            amount: 500,
            kind: TransferKind::Single,
        })
        .unwrap();
        for _ in 0..2000 {
            let (accounts, transfers) = (l.accounts(), l.transfers());
            random_op(&mut rng, &mut l);
            check_legal_move(&transfers, &l.transfers())
                .unwrap_or_else(|v| panic!("seed {seed}: {v:?}"));
            check_invariants(&l).unwrap_or_else(|v| panic!("seed {seed}: {v:?}"));
            if accounts == l.accounts() {
                assert_eq!(
                    transfers,
                    l.transfers(),
                    "seed {seed}: transfers changed without balances changing"
                );
            }
        }
        transfers_made += l.transfers().len();
    }
    assert!(
        transfers_made > 5_000,
        "walks made too few transfers: {transfers_made}"
    );
}
