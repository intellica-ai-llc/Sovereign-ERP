//! Test code: failures surface as panics and progress is printed on purpose.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout
)]

use sov_period_controller::{
    check_state, check_transition, Command, PeriodController, PeriodId, PeriodState, Status,
};
use sov_sim::Rng;

/// A period id in `0..bound`, so ids `0` and anything past the last period get exercised too.
fn period_below(rng: &mut Rng, bound: u32) -> PeriodId {
    PeriodId(u32::try_from(rng.below(u64::from(bound))).unwrap_or(0))
}

fn random_command(rng: &mut Rng, n: u32) -> Command {
    // Includes deliberately invalid ids and amounts so rejections are exercised too.
    let p = period_below(rng, n + 2);
    let amt = rng.below(1000);
    match rng.below(9) {
        0 => Command::Post {
            period: p,
            debit: amt,
            credit: amt,
        },
        1 => Command::PostClosing {
            period: p,
            debit: amt,
            credit: amt,
        },
        2 => Command::BeginClose(p),
        3 => Command::Reconcile(p),
        4 => Command::Close(p),
        5 => Command::ApproveReopen(p),
        6 => Command::Reopen(p),
        7 => Command::PostAdjustment {
            closed_period: p,
            into_period: period_below(rng, n + 2),
            amount: amt,
            reference: if rng.chance(1, 8) {
                String::new()
            } else {
                format!("REF-{}", rng.below(100))
            },
        },
        _ => Command::Post {
            period: p,
            debit: amt,
            credit: amt.wrapping_add(rng.below(3)),
        },
    }
}

fn run(seed: u64, steps: usize) -> (Vec<PeriodState>, usize, usize) {
    let n: u32 = 5;
    let mut rng = Rng::new(seed);
    let mut c = PeriodController::new(usize::try_from(n).unwrap());
    let (mut accepted, mut rejected) = (0, 0);
    // Bias towards progress so deep states (several closed periods, reopenings) are reached.
    for _ in 0..steps {
        let cmd = if rng.chance(1, 3) {
            let p = PeriodId(period_below(&mut rng, n).0 + 1);
            match rng.below(4) {
                0 => Command::BeginClose(p),
                1 => Command::Reconcile(p),
                2 => Command::Close(p),
                _ => Command::Post {
                    period: p,
                    debit: 10,
                    credit: 10,
                },
            }
        } else {
            random_command(&mut rng, n)
        };
        let before = c.snapshot();
        if c.execute(&cmd).is_ok() {
            accepted += 1;
            let after = c.snapshot();
            check_transition(&before, &after)
                .unwrap_or_else(|v| panic!("seed {seed}: {v:?} after {cmd:?}"));
            check_state(&after).unwrap_or_else(|v| panic!("seed {seed}: {v:?} after {cmd:?}"));
        } else {
            rejected += 1;
            assert_eq!(
                c.snapshot(),
                before,
                "seed {seed}: rejected command changed state ({cmd:?})"
            );
        }
    }
    (c.snapshot(), accepted, rejected)
}

#[test]
fn invariants_hold_over_many_random_walks() {
    let (mut acc, mut rej, mut deepest_closed) = (0, 0, 0);
    for seed in 0..300 {
        let (snap, a, r) = run(seed, 3000);
        acc += a;
        rej += r;
        deepest_closed =
            deepest_closed.max(snap.iter().filter(|s| s.status == Status::Closed).count());
    }
    println!(
        "random walks: {acc} accepted, {rej} rejected, max closed periods at end {deepest_closed}"
    );
    assert!(acc > 50_000, "walks did not make progress: {acc}");
    assert!(rej > 50_000, "walks did not exercise rejections: {rej}");
}

#[test]
fn replay_is_deterministic() {
    assert_eq!(run(7, 5000), run(7, 5000));
    assert_ne!(run(7, 5000).0, run(8, 5000).0);
}

#[test]
fn event_log_replay_reproduces_state() {
    let mut rng = Rng::new(99);
    let mut live = PeriodController::new(4);
    let mut log = Vec::new();
    for _ in 0..4000 {
        if let Ok(e) = live.execute(&random_command(&mut rng, 4)) {
            log.push(e);
        }
    }
    let mut replayed = PeriodController::new(4);
    for e in &log {
        replayed.apply(e);
    }
    assert_eq!(replayed.snapshot(), live.snapshot());
    assert_eq!(replayed.adjustments(), live.adjustments());
}
