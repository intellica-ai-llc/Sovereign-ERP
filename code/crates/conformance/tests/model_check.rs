use conformance::*;
use period_controller::PeriodState;

#[test]
fn spec_transcription_matches_recorded_tlc_counts() {
    let (generated, distinct, _) = explore_spec(TLC_BOUNDS);
    // Appendix M: "18,833 states generated, 4,160 distinct".
    assert_eq!(distinct, 4160, "distinct states");
    assert_eq!(generated, 18_833, "states generated");
}

#[test]
fn real_controller_explores_the_same_state_space_without_violations() {
    let r = explore_controller(TLC_BOUNDS, None);
    assert!(r.violation.is_none(), "unexpected violation: {:?}", r.violation);
    assert_eq!(r.distinct, 4160);
    assert_eq!(r.generated, 18_833);
}

#[test]
fn model_is_not_vacuous() {
    let r = explore_controller(TLC_BOUNDS, None);
    assert!(r.reached_closed, "no period ever reached Closed");
    assert!(r.posted_while_closing, "closing entries were never posted");
    assert!(r.max_depth >= 8, "state space suspiciously shallow: {}", r.max_depth);
}

#[test]
fn lockstep_controller_and_spec_agree_on_every_reachable_state_and_action() {
    let (_, _, states) = explore_spec(TLC_BOUNDS);
    let mut compared = 0usize;
    for s in &states {
        let snap = spec::to_snapshot(s);
        for a in probe_actions(TLC_BOUNDS) {
            let want = spec::next(s, TLC_BOUNDS, a).map(|t| spec::to_snapshot(&t));
            let got = controller_next(&snap, TLC_BOUNDS, a, None);
            assert_eq!(got, want, "divergence at state {snap:?} on action {a:?}");
            compared += 1;
        }
    }
    assert!(compared > 100_000, "compared only {compared}");
}

#[test]
fn every_seeded_defect_is_caught_by_the_intended_rule() {
    for d in ALL_DEFECTS {
        let r = explore_controller(TLC_BOUNDS, Some(d));
        let found = r.violation.unwrap_or_else(|| panic!("defect {d:?} was NOT caught"));
        assert!(
            expected_rules(d).contains(&found.violation.rule),
            "defect {d:?} tripped {:?}, expected one of {:?} (trace {:?})",
            found.violation.rule,
            expected_rules(d),
            found.trace
        );
        println!("{d:?}: caught by {:?}, shortest trace {} steps, {} states generated", found.violation.rule, found.trace.len(), r.generated);
    }
}

#[test]
fn unbalanced_posting_is_never_enabled_on_a_correct_controller() {
    let (_, _, states) = explore_spec(TLC_BOUNDS);
    for s in &states {
        let snap: Vec<PeriodState> = spec::to_snapshot(s);
        for p in 1..=TLC_BOUNDS.n {
            assert!(controller_next(&snap, TLC_BOUNDS, Action::PostUnbalanced(p, 1), None).is_none());
        }
    }
}

#[test]
fn larger_bounds_still_hold() {
    // Beyond the recorded TLC bounds: 4 periods, still exhaustive.
    let b = Bounds { n: 4, max_amt: 2, max_total: 2 };
    let r = explore_controller(b, None);
    assert!(r.violation.is_none(), "{:?}", r.violation);
    println!("n=4: {} distinct, {} generated", r.distinct, r.generated);
}
