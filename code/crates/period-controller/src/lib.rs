//! Period Controller (arc42 5.2 P2, Appendix M, ADR 039).
//!
//! The only component allowed to change a period's status. It is a pure decide/evolve state
//! machine: [`PeriodController::decide`] validates a [`Command`] against current state and returns
//! an [`Event`] or an [`Error`]; [`PeriodController::apply`] folds an event into state. No I/O,
//! no clock, no entropy, so it runs unchanged under deterministic simulation (ADR 046).
//!
//! Rules (Appendix M): P1 balanced, P2 sequential close, P3 closed implies reconciled,
//! A1 closed totals immutable, A2 reopen only when approved, A3 reopen only when no later period
//! is closed. Adjustments to a closed period are posted to an open period with a back-reference.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeriodId(pub u32);

impl fmt::Display for PeriodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Open,
    Closing,
    Closed,
}

/// Per-period state. Amounts are in minor units; debit and credit are tracked separately so that
/// invariant P1 is a real check rather than a tautology.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeriodState {
    pub status: Status,
    pub debit: u64,
    pub credit: u64,
    pub reconciled: bool,
    pub reopen_approved: bool,
}

impl Default for PeriodState {
    fn default() -> Self {
        Self { status: Status::Open, debit: 0, credit: 0, reconciled: false, reopen_approved: false }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Balanced posting into an open period.
    Post { period: PeriodId, debit: u64, credit: u64 },
    /// Closing entry (accrual, revaluation); only while the period is closing. Invalidates reconciliation.
    PostClosing { period: PeriodId, debit: u64, credit: u64 },
    BeginClose(PeriodId),
    Reconcile(PeriodId),
    Close(PeriodId),
    ApproveReopen(PeriodId),
    Reopen(PeriodId),
    /// Correct a closed period by posting into an open period with a back-reference.
    PostAdjustment { closed_period: PeriodId, into_period: PeriodId, amount: u64, reference: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Posted { period: PeriodId, debit: u64, credit: u64 },
    ClosingEntryPosted { period: PeriodId, debit: u64, credit: u64 },
    CloseBegun(PeriodId),
    Reconciled(PeriodId),
    Closed(PeriodId),
    ReopenApproved(PeriodId),
    Reopened(PeriodId),
    AdjustmentPosted { closed_period: PeriodId, into_period: PeriodId, amount: u64, reference: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnknownPeriod(PeriodId),
    WrongStatus { period: PeriodId, expected: Status, actual: Status },
    ZeroAmount,
    Unbalanced { debit: u64, credit: u64 },
    Overflow,
    EarlierPeriodNotClosed { period: PeriodId, blocking: PeriodId },
    NotReconciled(PeriodId),
    AlreadyReconciled(PeriodId),
    AlreadyApproved(PeriodId),
    ReopenNotApproved(PeriodId),
    LaterPeriodClosed { period: PeriodId, blocking: PeriodId },
    EmptyReference,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

/// Which invariant a transition or state broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    P1,
    P2,
    P3,
    A1,
    A2,
    A3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    pub rule: Rule,
    pub period: PeriodId,
}

/// Seeded defects for mutation testing. Only reachable through [`PeriodController::inject`],
/// which exists only with the `fault-injection` feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Defect {
    PostIntoClosed,
    CloseWithoutReconciliation,
    CloseOutOfSequence,
    ReopenWithoutApproval,
    UnbalancedPost,
    ReopenOutOfOrder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjustmentRecord {
    pub closed_period: PeriodId,
    pub posted_into: PeriodId,
    pub amount: u64,
    pub reference: String,
}

#[derive(Clone, Debug)]
pub struct PeriodController {
    periods: Vec<PeriodState>,
    adjustments: Vec<AdjustmentRecord>,
    defect: Option<Defect>,
}

impl PeriodController {
    /// `n` periods numbered `P1..=Pn`, all open and empty.
    pub fn new(n: u32) -> Self {
        Self { periods: vec![PeriodState::default(); n as usize], adjustments: Vec::new(), defect: None }
    }

    /// Rebuild from a snapshot (replay, test harnesses). Does not validate; use [`check_state`].
    pub fn from_snapshot(periods: Vec<PeriodState>) -> Self {
        Self { periods, adjustments: Vec::new(), defect: None }
    }

    #[cfg(feature = "fault-injection")]
    pub fn inject(mut self, defect: Defect) -> Self {
        self.defect = Some(defect);
        self
    }

    pub fn snapshot(&self) -> Vec<PeriodState> {
        self.periods.clone()
    }

    pub fn periods(&self) -> &[PeriodState] {
        &self.periods
    }

    pub fn period(&self, id: PeriodId) -> Option<&PeriodState> {
        self.index(id).map(|i| &self.periods[i])
    }

    pub fn adjustments(&self) -> &[AdjustmentRecord] {
        &self.adjustments
    }

    fn has(&self, d: Defect) -> bool {
        self.defect == Some(d)
    }

    fn index(&self, id: PeriodId) -> Option<usize> {
        let i = id.0.checked_sub(1)? as usize;
        (i < self.periods.len()).then_some(i)
    }

    fn get(&self, id: PeriodId) -> Result<&PeriodState, Error> {
        self.index(id).map(|i| &self.periods[i]).ok_or(Error::UnknownPeriod(id))
    }

    fn first_not_closed_before(&self, id: PeriodId) -> Option<PeriodId> {
        let i = self.index(id)?;
        (0..i).find(|&q| self.periods[q].status != Status::Closed).map(|q| PeriodId(q as u32 + 1))
    }

    fn first_closed_after(&self, id: PeriodId) -> Option<PeriodId> {
        let i = self.index(id)?;
        (i + 1..self.periods.len()).find(|&q| self.periods[q].status == Status::Closed).map(|q| PeriodId(q as u32 + 1))
    }

    fn expect_status(&self, id: PeriodId, expected: Status) -> Result<&PeriodState, Error> {
        let st = self.get(id)?;
        if st.status == expected {
            Ok(st)
        } else {
            Err(Error::WrongStatus { period: id, expected, actual: st.status })
        }
    }

    fn check_posting(&self, st: &PeriodState, debit: u64, credit: u64) -> Result<(), Error> {
        if debit == 0 && credit == 0 {
            return Err(Error::ZeroAmount);
        }
        if debit != credit && !self.has(Defect::UnbalancedPost) {
            return Err(Error::Unbalanced { debit, credit });
        }
        st.debit.checked_add(debit).ok_or(Error::Overflow)?;
        st.credit.checked_add(credit).ok_or(Error::Overflow)?;
        Ok(())
    }

    /// Validate a command against current state. Pure: never mutates.
    pub fn decide(&self, cmd: &Command) -> Result<Event, Error> {
        match cmd {
            Command::Post { period, debit, credit } => {
                let st = if self.has(Defect::PostIntoClosed) && self.get(*period)?.status == Status::Closed {
                    self.get(*period)?
                } else {
                    self.expect_status(*period, Status::Open)?
                };
                self.check_posting(st, *debit, *credit)?;
                Ok(Event::Posted { period: *period, debit: *debit, credit: *credit })
            }
            Command::PostClosing { period, debit, credit } => {
                let st = self.expect_status(*period, Status::Closing)?;
                self.check_posting(st, *debit, *credit)?;
                Ok(Event::ClosingEntryPosted { period: *period, debit: *debit, credit: *credit })
            }
            Command::BeginClose(p) => {
                self.expect_status(*p, Status::Open)?;
                if let Some(blocking) = self.first_not_closed_before(*p) {
                    return Err(Error::EarlierPeriodNotClosed { period: *p, blocking });
                }
                Ok(Event::CloseBegun(*p))
            }
            Command::Reconcile(p) => {
                let st = self.expect_status(*p, Status::Closing)?;
                if st.reconciled {
                    return Err(Error::AlreadyReconciled(*p));
                }
                Ok(Event::Reconciled(*p))
            }
            Command::Close(p) => {
                let st = self.expect_status(*p, Status::Closing)?;
                if !st.reconciled && !self.has(Defect::CloseWithoutReconciliation) {
                    return Err(Error::NotReconciled(*p));
                }
                if !self.has(Defect::CloseOutOfSequence) {
                    if let Some(blocking) = self.first_not_closed_before(*p) {
                        return Err(Error::EarlierPeriodNotClosed { period: *p, blocking });
                    }
                }
                Ok(Event::Closed(*p))
            }
            Command::ApproveReopen(p) => {
                let st = self.expect_status(*p, Status::Closed)?;
                if st.reopen_approved {
                    return Err(Error::AlreadyApproved(*p));
                }
                Ok(Event::ReopenApproved(*p))
            }
            Command::Reopen(p) => {
                let st = self.expect_status(*p, Status::Closed)?;
                if !st.reopen_approved && !self.has(Defect::ReopenWithoutApproval) {
                    return Err(Error::ReopenNotApproved(*p));
                }
                if !self.has(Defect::ReopenOutOfOrder) {
                    if let Some(blocking) = self.first_closed_after(*p) {
                        return Err(Error::LaterPeriodClosed { period: *p, blocking });
                    }
                }
                Ok(Event::Reopened(*p))
            }
            Command::PostAdjustment { closed_period, into_period, amount, reference } => {
                self.expect_status(*closed_period, Status::Closed)?;
                let target = self.expect_status(*into_period, Status::Open)?;
                if reference.trim().is_empty() {
                    return Err(Error::EmptyReference);
                }
                self.check_posting(target, *amount, *amount)?;
                Ok(Event::AdjustmentPosted {
                    closed_period: *closed_period,
                    into_period: *into_period,
                    amount: *amount,
                    reference: reference.clone(),
                })
            }
        }
    }

    /// Fold an event into state. Events come from [`decide`](Self::decide) or from replay;
    /// unknown periods are ignored rather than panicking so a corrupt log cannot crash the kernel.
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Posted { period, debit, credit } => self.add(*period, *debit, *credit, false),
            Event::ClosingEntryPosted { period, debit, credit } => self.add(*period, *debit, *credit, true),
            Event::CloseBegun(p) => self.set(*p, |s| s.status = Status::Closing),
            Event::Reconciled(p) => self.set(*p, |s| s.reconciled = true),
            Event::Closed(p) => self.set(*p, |s| s.status = Status::Closed),
            Event::ReopenApproved(p) => self.set(*p, |s| s.reopen_approved = true),
            Event::Reopened(p) => self.set(*p, |s| {
                s.status = Status::Open;
                s.reconciled = false;
                s.reopen_approved = false;
            }),
            Event::AdjustmentPosted { closed_period, into_period, amount, reference } => {
                self.add(*into_period, *amount, *amount, false);
                self.adjustments.push(AdjustmentRecord {
                    closed_period: *closed_period,
                    posted_into: *into_period,
                    amount: *amount,
                    reference: reference.clone(),
                });
            }
        }
    }

    fn set(&mut self, p: PeriodId, f: impl FnOnce(&mut PeriodState)) {
        if let Some(i) = self.index(p) {
            f(&mut self.periods[i]);
        }
    }

    fn add(&mut self, p: PeriodId, debit: u64, credit: u64, invalidates_reconciliation: bool) {
        self.set(p, |s| {
            s.debit = s.debit.saturating_add(debit);
            s.credit = s.credit.saturating_add(credit);
            if invalidates_reconciliation {
                s.reconciled = false;
            }
        });
    }

    /// Validate then apply.
    pub fn execute(&mut self, cmd: &Command) -> Result<Event, Error> {
        let event = self.decide(cmd)?;
        self.apply(&event);
        Ok(event)
    }
}

/// State invariants P1 to P3.
pub fn check_state(periods: &[PeriodState]) -> Result<(), Violation> {
    for (i, st) in periods.iter().enumerate() {
        let id = PeriodId(i as u32 + 1);
        if st.debit != st.credit {
            return Err(Violation { rule: Rule::P1, period: id });
        }
        if st.status == Status::Closed {
            if periods[..i].iter().any(|q| q.status != Status::Closed) {
                return Err(Violation { rule: Rule::P2, period: id });
            }
            if !st.reconciled {
                return Err(Violation { rule: Rule::P3, period: id });
            }
        }
    }
    Ok(())
}

/// Action properties A1 to A3 over one step.
pub fn check_transition(before: &[PeriodState], after: &[PeriodState]) -> Result<(), Violation> {
    for (i, (b, a)) in before.iter().zip(after).enumerate() {
        let id = PeriodId(i as u32 + 1);
        if b.status != Status::Closed {
            continue;
        }
        if a.status == Status::Closed && (a.debit != b.debit || a.credit != b.credit) {
            return Err(Violation { rule: Rule::A1, period: id });
        }
        if a.status != Status::Closed && !(b.reopen_approved && a.status == Status::Open) {
            return Err(Violation { rule: Rule::A2, period: id });
        }
        if a.status == Status::Open && after[i + 1..].iter().any(|q| q.status == Status::Closed) {
            return Err(Violation { rule: Rule::A3, period: id });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const P1: PeriodId = PeriodId(1);
    const P2: PeriodId = PeriodId(2);

    fn post(p: PeriodId, a: u64) -> Command {
        Command::Post { period: p, debit: a, credit: a }
    }

    fn close_cycle(c: &mut PeriodController, p: PeriodId) {
        c.execute(&Command::BeginClose(p)).unwrap();
        c.execute(&Command::Reconcile(p)).unwrap();
        c.execute(&Command::Close(p)).unwrap();
    }

    #[test]
    fn happy_path_close_in_sequence() {
        let mut c = PeriodController::new(3);
        c.execute(&post(P1, 100)).unwrap();
        close_cycle(&mut c, P1);
        assert_eq!(c.period(P1).unwrap().status, Status::Closed);
        assert!(check_state(c.periods()).is_ok());
    }

    #[test]
    fn cannot_close_out_of_sequence() {
        let mut c = PeriodController::new(3);
        assert_eq!(
            c.execute(&Command::BeginClose(P2)),
            Err(Error::EarlierPeriodNotClosed { period: P2, blocking: P1 })
        );
    }

    #[test]
    fn cannot_close_without_reconciliation() {
        let mut c = PeriodController::new(1);
        c.execute(&Command::BeginClose(P1)).unwrap();
        assert_eq!(c.execute(&Command::Close(P1)), Err(Error::NotReconciled(P1)));
    }

    #[test]
    fn closing_entry_invalidates_reconciliation() {
        let mut c = PeriodController::new(1);
        c.execute(&Command::BeginClose(P1)).unwrap();
        c.execute(&Command::Reconcile(P1)).unwrap();
        c.execute(&Command::PostClosing { period: P1, debit: 5, credit: 5 }).unwrap();
        assert_eq!(c.execute(&Command::Close(P1)), Err(Error::NotReconciled(P1)));
    }

    #[test]
    fn closed_period_rejects_posting() {
        let mut c = PeriodController::new(1);
        close_cycle(&mut c, P1);
        assert!(matches!(c.execute(&post(P1, 1)), Err(Error::WrongStatus { .. })));
    }

    #[test]
    fn unbalanced_and_zero_postings_rejected() {
        let mut c = PeriodController::new(1);
        assert_eq!(c.execute(&Command::Post { period: P1, debit: 5, credit: 4 }), Err(Error::Unbalanced { debit: 5, credit: 4 }));
        assert_eq!(c.execute(&post(P1, 0)), Err(Error::ZeroAmount));
        assert_eq!(c.execute(&Command::Post { period: P1, debit: u64::MAX, credit: u64::MAX }), Ok(Event::Posted { period: P1, debit: u64::MAX, credit: u64::MAX }));
        assert_eq!(c.execute(&post(P1, 1)), Err(Error::Overflow));
    }

    #[test]
    fn reopen_needs_approval_and_order() {
        let mut c = PeriodController::new(2);
        close_cycle(&mut c, P1);
        close_cycle(&mut c, P2);
        assert_eq!(c.execute(&Command::Reopen(P1)), Err(Error::ReopenNotApproved(P1)));
        c.execute(&Command::ApproveReopen(P1)).unwrap();
        assert_eq!(c.execute(&Command::ApproveReopen(P1)), Err(Error::AlreadyApproved(P1)));
        assert_eq!(c.execute(&Command::Reopen(P1)), Err(Error::LaterPeriodClosed { period: P1, blocking: P2 }));
        c.execute(&Command::ApproveReopen(P2)).unwrap();
        c.execute(&Command::Reopen(P2)).unwrap();
        c.execute(&Command::Reopen(P1)).unwrap();
        let s = c.period(P1).unwrap();
        assert_eq!((s.status, s.reconciled, s.reopen_approved), (Status::Open, false, false));
    }

    #[test]
    fn adjustment_goes_to_open_period_with_back_reference() {
        let mut c = PeriodController::new(2);
        c.execute(&post(P1, 100)).unwrap();
        close_cycle(&mut c, P1);
        let before = c.snapshot();
        c.execute(&Command::PostAdjustment { closed_period: P1, into_period: P2, amount: 7, reference: "INV-42 correction".into() }).unwrap();
        let after = c.snapshot();
        assert_eq!(after[0], before[0], "closed period untouched");
        assert_eq!((after[1].debit, after[1].credit), (7, 7));
        assert_eq!(c.adjustments().len(), 1);
        assert_eq!(c.adjustments()[0].closed_period, P1);
        assert!(check_transition(&before, &after).is_ok());
    }

    #[test]
    fn adjustment_rejects_open_source_closed_target_and_blank_reference() {
        let mut c = PeriodController::new(2);
        let adj = |from, to, r: &str| Command::PostAdjustment { closed_period: from, into_period: to, amount: 1, reference: r.into() };
        assert!(matches!(c.decide(&adj(P1, P2, "x")), Err(Error::WrongStatus { .. })));
        close_cycle(&mut c, P1);
        assert!(matches!(c.decide(&adj(P1, P1, "x")), Err(Error::WrongStatus { .. })));
        assert_eq!(c.decide(&adj(P1, P2, "  ")), Err(Error::EmptyReference));
    }

    #[test]
    fn unknown_period_is_an_error_not_a_panic() {
        let c = PeriodController::new(2);
        assert_eq!(c.decide(&Command::Close(PeriodId(0))), Err(Error::UnknownPeriod(PeriodId(0))));
        assert_eq!(c.decide(&Command::Close(PeriodId(9))), Err(Error::UnknownPeriod(PeriodId(9))));
    }

    #[test]
    fn decide_is_pure() {
        let c = PeriodController::new(2);
        let before = c.snapshot();
        let _ = c.decide(&post(P1, 5));
        let _ = c.decide(&Command::Close(P2));
        assert_eq!(c.snapshot(), before);
    }

    #[test]
    fn transition_checker_flags_each_action_property() {
        let closed = PeriodState { status: Status::Closed, debit: 1, credit: 1, reconciled: true, reopen_approved: false };
        let mut changed = closed.clone();
        changed.debit = 2;
        changed.credit = 2;
        assert_eq!(check_transition(&[closed.clone()], &[changed]).unwrap_err().rule, Rule::A1);
        let reopened = PeriodState { status: Status::Open, ..closed.clone() };
        assert_eq!(check_transition(&[closed.clone()], &[reopened]).unwrap_err().rule, Rule::A2);
        let approved = PeriodState { reopen_approved: true, ..closed.clone() };
        let reopened = PeriodState { status: Status::Open, ..approved.clone() };
        assert_eq!(check_transition(&[approved.clone(), approved.clone()], &[reopened, approved]).unwrap_err().rule, Rule::A3);
    }
}
