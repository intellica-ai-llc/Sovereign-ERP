//! Conformance harness for the Period Controller (arc42 Appendix C spike 8, ADR 046).
//!
//! Three independent checks:
//! 1. [`spec`]: a line-by-line Rust transcription of `formal/PeriodClose.tla`, explored exhaustively
//!    with the same bounds as the recorded TLC run (3 periods, MaxAmt 2, MaxTotal 3).
//! 2. [`explore_controller`]: the same exhaustive exploration driven through the real controller,
//!    checking P1 to P3 and A1 to A3 on every state and step.
//! 3. Lockstep: for every reachable spec state and every action, the controller and the spec agree
//!    on whether the action is enabled and on the resulting state (see `tests/`).

use period_controller::{
    check_state, check_transition, Command, Defect, PeriodController, PeriodId, PeriodState, Rule,
    Status, Violation,
};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub n: usize,
    pub max_amt: u64,
    pub max_total: u64,
}

/// The bounds of the TLC run recorded in Appendix M.
pub const TLC_BOUNDS: Bounds = Bounds { n: 3, max_amt: 2, max_total: 3 };

/// One disjunct of `Next` in PeriodClose.tla. `p` is 1-based, as in the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Post(usize, u64),
    PostClosing(usize, u64),
    BeginClose(usize),
    Reconcile(usize),
    Close(usize),
    ApproveReopen(usize),
    Reopen(usize),
    /// Not in the TLA+ `Next`: a posting with credit 0. Must always be disabled on a correct implementation.
    PostUnbalanced(usize, u64),
}

/// Every action of `Next`, each exactly once: postings per (period, amount), the rest per period.
/// With this convention the generated-state count equals TLC's reported 18,833 (see tests).
pub fn tla_actions(b: Bounds) -> Vec<Action> {
    let mut v = Vec::new();
    for p in 1..=b.n {
        for amt in 1..=b.max_amt {
            v.push(Action::Post(p, amt));
            v.push(Action::PostClosing(p, amt));
        }
        v.extend([
            Action::BeginClose(p),
            Action::Reconcile(p),
            Action::Close(p),
            Action::ApproveReopen(p),
            Action::Reopen(p),
        ]);
    }
    v
}

pub fn probe_actions(b: Bounds) -> Vec<Action> {
    let mut v = tla_actions(b);
    for p in 1..=b.n {
        for amt in 1..=b.max_amt {
            v.push(Action::PostUnbalanced(p, amt));
        }
    }
    v
}

pub mod spec {
    //! Transcription of PeriodClose.tla. Keep it boring and literal: this is the oracle.
    use super::{Action, Bounds};
    use period_controller::{PeriodState, Status};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum S {
        Open,
        Closing,
        Closed,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct State {
        pub status: Vec<S>,
        pub d: Vec<u64>,
        pub c: Vec<u64>,
        pub recon: Vec<bool>,
        pub approved: Vec<bool>,
    }

    pub fn init(b: Bounds) -> State {
        State {
            status: vec![S::Open; b.n],
            d: vec![0; b.n],
            c: vec![0; b.n],
            recon: vec![false; b.n],
            approved: vec![false; b.n],
        }
    }

    fn all_earlier_closed(s: &State, p: usize) -> bool {
        (1..p).all(|q| s.status[q - 1] == S::Closed)
    }
    fn no_later_closed(s: &State, p: usize) -> bool {
        (p + 1..=s.status.len()).all(|q| s.status[q - 1] != S::Closed)
    }

    /// Successor state if the action is enabled, else `None`.
    pub fn next(s: &State, b: Bounds, a: Action) -> Option<State> {
        let mut t = s.clone();
        match a {
            Action::Post(p, amt) => {
                let i = p - 1;
                if !(s.status[i] == S::Open && (1..=b.max_amt).contains(&amt) && s.d[i] + amt <= b.max_total) {
                    return None;
                }
                t.d[i] += amt;
                t.c[i] += amt;
            }
            Action::PostClosing(p, amt) => {
                let i = p - 1;
                if !(s.status[i] == S::Closing && (1..=b.max_amt).contains(&amt) && s.d[i] + amt <= b.max_total) {
                    return None;
                }
                t.d[i] += amt;
                t.c[i] += amt;
                t.recon[i] = false;
            }
            Action::BeginClose(p) => {
                if !(s.status[p - 1] == S::Open && all_earlier_closed(s, p)) {
                    return None;
                }
                t.status[p - 1] = S::Closing;
            }
            Action::Reconcile(p) => {
                if !(s.status[p - 1] == S::Closing && !s.recon[p - 1]) {
                    return None;
                }
                t.recon[p - 1] = true;
            }
            Action::Close(p) => {
                if !(s.status[p - 1] == S::Closing && s.recon[p - 1] && all_earlier_closed(s, p)) {
                    return None;
                }
                t.status[p - 1] = S::Closed;
            }
            Action::ApproveReopen(p) => {
                if !(s.status[p - 1] == S::Closed && !s.approved[p - 1]) {
                    return None;
                }
                t.approved[p - 1] = true;
            }
            Action::Reopen(p) => {
                if !(s.status[p - 1] == S::Closed && s.approved[p - 1] && no_later_closed(s, p)) {
                    return None;
                }
                t.status[p - 1] = S::Open;
                t.recon[p - 1] = false;
                t.approved[p - 1] = false;
            }
            Action::PostUnbalanced(..) => return None, // not an action of the spec
        }
        Some(t)
    }

    pub fn to_snapshot(s: &State) -> Vec<PeriodState> {
        (0..s.status.len())
            .map(|i| PeriodState {
                status: match s.status[i] {
                    S::Open => Status::Open,
                    S::Closing => Status::Closing,
                    S::Closed => Status::Closed,
                },
                debit: s.d[i],
                credit: s.c[i],
                reconciled: s.recon[i],
                reopen_approved: s.approved[i],
            })
            .collect()
    }
}

/// Map a model action onto the controller's command language.
pub fn to_command(a: Action) -> Command {
    let id = |p: usize| PeriodId(p as u32);
    match a {
        Action::Post(p, amt) => Command::Post { period: id(p), debit: amt, credit: amt },
        Action::PostClosing(p, amt) => Command::PostClosing { period: id(p), debit: amt, credit: amt },
        Action::BeginClose(p) => Command::BeginClose(id(p)),
        Action::Reconcile(p) => Command::Reconcile(id(p)),
        Action::Close(p) => Command::Close(id(p)),
        Action::ApproveReopen(p) => Command::ApproveReopen(id(p)),
        Action::Reopen(p) => Command::Reopen(id(p)),
        Action::PostUnbalanced(p, amt) => Command::Post { period: id(p), debit: amt, credit: 0 },
    }
}

/// Successor of `state` under `a` according to the real controller, with the model's `max_total`
/// bound applied to postings (the bound belongs to the checker, not to the controller).
pub fn controller_next(state: &[PeriodState], b: Bounds, a: Action, defect: Option<Defect>) -> Option<Vec<PeriodState>> {
    let (Action::Post(p, amt) | Action::PostClosing(p, amt) | Action::PostUnbalanced(p, amt)) = a else {
        return step(state, a, defect);
    };
    if state[p - 1].debit + amt > b.max_total {
        return None;
    }
    step(state, a, defect)
}

fn step(state: &[PeriodState], a: Action, defect: Option<Defect>) -> Option<Vec<PeriodState>> {
    let mut c = PeriodController::from_snapshot(state.to_vec());
    if let Some(d) = defect {
        c = c.inject(d);
    }
    c.execute(&to_command(a)).ok()?;
    Some(c.snapshot())
}

#[derive(Debug)]
pub struct Exploration {
    /// Initial state plus every enabled successor, duplicates included (TLC's "states generated").
    pub generated: usize,
    pub distinct: usize,
    pub violation: Option<Found>,
    pub max_depth: usize,
    /// Reachability probes: the model is not vacuous.
    pub reached_closed: bool,
    pub posted_while_closing: bool,
}

#[derive(Debug)]
pub struct Found {
    pub violation: Violation,
    pub trace: Vec<Action>,
}

/// Exhaustive breadth-first exploration through the real controller. Stops at the first violation
/// and returns a shortest counterexample trace.
pub fn explore_controller(b: Bounds, defect: Option<Defect>) -> Exploration {
    let init: Vec<PeriodState> = (0..b.n).map(|_| PeriodState::default()).collect();
    let actions = probe_actions(b);
    let mut seen: HashSet<Vec<PeriodState>> = HashSet::new();
    let mut parent: HashMap<Vec<PeriodState>, (Vec<PeriodState>, Action)> = HashMap::new();
    let mut depth: HashMap<Vec<PeriodState>, usize> = HashMap::new();
    let mut queue = VecDeque::new();
    seen.insert(init.clone());
    depth.insert(init.clone(), 0);
    queue.push_back(init);
    let mut out = Exploration {
        generated: 1,
        distinct: 1,
        violation: None,
        max_depth: 0,
        reached_closed: false,
        posted_while_closing: false,
    };

    while let Some(cur) = queue.pop_front() {
        let d = depth[&cur];
        for &a in &actions {
            let Some(next) = controller_next(&cur, b, a, defect) else { continue };
            out.generated += 1;
            if matches!(a, Action::PostClosing(..)) {
                out.posted_while_closing = true;
            }
            let bad = check_transition(&cur, &next).and_then(|_| check_state(&next));
            if let Err(v) = bad {
                let mut trace = vec![a];
                let mut at = cur.clone();
                while let Some((prev, act)) = parent.get(&at) {
                    trace.push(*act);
                    at = prev.clone();
                }
                trace.reverse();
                out.violation = Some(Found { violation: v, trace });
                return out;
            }
            if next.iter().any(|s| s.status == Status::Closed) {
                out.reached_closed = true;
            }
            if seen.insert(next.clone()) {
                out.distinct += 1;
                out.max_depth = out.max_depth.max(d + 1);
                depth.insert(next.clone(), d + 1);
                parent.insert(next.clone(), (cur.clone(), a));
                queue.push_back(next);
            }
        }
    }
    out
}

/// Exhaustive exploration of the spec transcription itself. Returns (generated, distinct, states).
pub fn explore_spec(b: Bounds) -> (usize, usize, Vec<spec::State>) {
    let init = spec::init(b);
    let actions = tla_actions(b);
    let mut seen: HashSet<spec::State> = HashSet::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::new();
    seen.insert(init.clone());
    queue.push_back(init.clone());
    order.push(init);
    let mut generated = 1;
    while let Some(cur) = queue.pop_front() {
        for &a in &actions {
            if let Some(next) = spec::next(&cur, b, a) {
                generated += 1;
                if seen.insert(next.clone()) {
                    order.push(next.clone());
                    queue.push_back(next);
                }
            }
        }
    }
    (generated, seen.len(), order)
}

/// Which rules a defect is expected to trip. The recorded TLC run reports "reopening out of
/// order" under P2 because TLC checks state invariants first; this harness checks action
/// properties first, so the same defect surfaces as A3. Both are accepted.
pub fn expected_rules(d: Defect) -> &'static [Rule] {
    match d {
        Defect::PostIntoClosed => &[Rule::A1],
        Defect::CloseWithoutReconciliation => &[Rule::P3],
        Defect::CloseOutOfSequence => &[Rule::P2],
        Defect::ReopenWithoutApproval => &[Rule::A2],
        Defect::UnbalancedPost => &[Rule::P1],
        Defect::ReopenOutOfOrder => &[Rule::A3, Rule::P2],
    }
}

pub const ALL_DEFECTS: [Defect; 6] = [
    Defect::PostIntoClosed,
    Defect::CloseWithoutReconciliation,
    Defect::CloseOutOfSequence,
    Defect::ReopenWithoutApproval,
    Defect::UnbalancedPost,
    Defect::ReopenOutOfOrder,
];
