//! Period Controller (arc42 section 5.2 P2, Appendix M, ADR 039).
//!
//! The only component allowed to change a period's status. It is a pure decide/evolve state
//! machine: [`PeriodController::decide`] validates a [`Command`] against current state and returns
//! an [`Event`] or an [`Error`]; [`PeriodController::apply`] folds an event into state. There is no
//! I/O, clock or entropy, so it runs unchanged under deterministic simulation (ADR 046).
//!
//! Rules (Appendix M): P1 balanced, P2 sequential close, P3 closed implies reconciled,
//! A1 closed totals immutable, A2 reopen only when approved, A3 reopen only when no later period
//! is closed. Adjustments to a closed period are posted to an open period with a back-reference.
//! [`check_state`] and [`check_transition`] state these rules as executable checks.

use std::fmt;

/// A period number, 1-based (`P1` is the first period).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeriodId(pub u32);

impl fmt::Display for PeriodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}", self.0)
    }
}

/// Where a period is in its close cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    /// Accepting postings.
    Open,
    /// Close has begun: only closing entries and reconciliation are allowed.
    Closing,
    /// Final. Changes only through an approved reopen.
    Closed,
}

/// Per-period state. Amounts are in minor units; debit and credit are tracked separately so that
/// invariant P1 is a real check rather than a tautology.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeriodState {
    /// Lifecycle status.
    pub status: Status,
    /// Total debits posted.
    pub debit: u64,
    /// Total credits posted.
    pub credit: u64,
    /// Whether the period is currently reconciled (a closing entry clears this).
    pub reconciled: bool,
    /// Whether a reopen has been approved for this period.
    pub reopen_approved: bool,
}

impl Default for PeriodState {
    fn default() -> Self {
        Self {
            status: Status::Open,
            debit: 0,
            credit: 0,
            reconciled: false,
            reopen_approved: false,
        }
    }
}

/// A request to change state. Validated by [`PeriodController::decide`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Balanced posting into an open period.
    Post {
        /// Target period.
        period: PeriodId,
        /// Debit amount.
        debit: u64,
        /// Credit amount; must equal `debit` (P1).
        credit: u64,
    },
    /// Closing entry (accrual, revaluation); only while the period is closing. Clears reconciliation.
    PostClosing {
        /// Target period.
        period: PeriodId,
        /// Debit amount.
        debit: u64,
        /// Credit amount; must equal `debit` (P1).
        credit: u64,
    },
    /// Move an open period to closing; all earlier periods must already be closed (P2).
    BeginClose(PeriodId),
    /// Mark a closing period reconciled.
    Reconcile(PeriodId),
    /// Close a reconciled period whose predecessors are all closed (P2, P3).
    Close(PeriodId),
    /// Approve reopening a closed period (A2).
    ApproveReopen(PeriodId),
    /// Reopen an approved closed period when no later period is closed (A2, A3).
    Reopen(PeriodId),
    /// Correct a closed period by posting into an open period with a back-reference.
    PostAdjustment {
        /// The closed period being corrected.
        closed_period: PeriodId,
        /// The open period that receives the entry.
        into_period: PeriodId,
        /// Amount posted as a balanced debit and credit.
        amount: u64,
        /// Back-reference to the corrected item; must not be blank.
        reference: String,
    },
}

/// A fact that happened. [`PeriodController::apply`] folds events into state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A balanced posting landed in an open period.
    Posted {
        /// Target period.
        period: PeriodId,
        /// Debit amount.
        debit: u64,
        /// Credit amount.
        credit: u64,
    },
    /// A closing entry landed in a closing period; reconciliation was cleared.
    ClosingEntryPosted {
        /// Target period.
        period: PeriodId,
        /// Debit amount.
        debit: u64,
        /// Credit amount.
        credit: u64,
    },
    /// The period moved from open to closing.
    CloseBegun(PeriodId),
    /// The period was reconciled.
    Reconciled(PeriodId),
    /// The period closed.
    Closed(PeriodId),
    /// A reopen was approved.
    ReopenApproved(PeriodId),
    /// The period reopened; reconciliation and approval were cleared.
    Reopened(PeriodId),
    /// A correction to a closed period was posted into an open one.
    AdjustmentPosted {
        /// The closed period being corrected.
        closed_period: PeriodId,
        /// The open period that received the entry.
        into_period: PeriodId,
        /// Amount posted as a balanced debit and credit.
        amount: u64,
        /// Back-reference to the corrected item.
        reference: String,
    },
}

/// Why a command was rejected. Rejection never changes state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The period id is zero or beyond the configured periods.
    UnknownPeriod(PeriodId),
    /// The period is not in the status the command requires.
    WrongStatus {
        /// The period addressed.
        period: PeriodId,
        /// The status the command needs.
        expected: Status,
        /// The status it has.
        actual: Status,
    },
    /// A posting of zero.
    ZeroAmount,
    /// Debit and credit differ (P1).
    Unbalanced {
        /// Debit amount.
        debit: u64,
        /// Credit amount.
        credit: u64,
    },
    /// A total would exceed `u64::MAX`.
    Overflow,
    /// An earlier period is not closed yet (P2).
    EarlierPeriodNotClosed {
        /// The period addressed.
        period: PeriodId,
        /// The first earlier period that is not closed.
        blocking: PeriodId,
    },
    /// The period is not reconciled (P3).
    NotReconciled(PeriodId),
    /// The period is already reconciled.
    AlreadyReconciled(PeriodId),
    /// A reopen is already approved for this period.
    AlreadyApproved(PeriodId),
    /// No reopen approval exists (A2).
    ReopenNotApproved(PeriodId),
    /// A later period is still closed (A3).
    LaterPeriodClosed {
        /// The period addressed.
        period: PeriodId,
        /// The first later period that is closed.
        blocking: PeriodId,
    },
    /// An adjustment needs a non-blank back-reference.
    EmptyReference,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// Which invariant a state or step broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    /// Every period is balanced.
    P1,
    /// A closed period has only closed predecessors.
    P2,
    /// A closed period is reconciled.
    P3,
    /// A closed period's totals never change while it stays closed.
    A1,
    /// A closed period leaves `Closed` only by an approved reopen.
    A2,
    /// A reopen never leaves a later period closed.
    A3,
}

/// A broken rule and the period it broke on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The rule.
    pub rule: Rule,
    /// The offending period.
    pub period: PeriodId,
}

/// Seeded defects for mutation testing. Only reachable through `PeriodController::inject`, which
/// exists only with the `fault-injection` feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Defect {
    /// Postings accepted into closed periods (breaks A1).
    PostIntoClosed,
    /// Close without reconciliation (breaks P3).
    CloseWithoutReconciliation,
    /// Close without checking predecessors (breaks P2).
    CloseOutOfSequence,
    /// Reopen without approval (breaks A2).
    ReopenWithoutApproval,
    /// Unbalanced postings accepted (breaks P1).
    UnbalancedPost,
    /// Reopen while a later period is closed (breaks A3).
    ReopenOutOfOrder,
}

/// A recorded correction to a closed period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjustmentRecord {
    /// The closed period corrected.
    pub closed_period: PeriodId,
    /// The open period that received the entry.
    pub posted_into: PeriodId,
    /// Amount posted.
    pub amount: u64,
    /// Back-reference supplied by the caller.
    pub reference: String,
}

/// The period-close state machine over a fixed number of periods.
#[derive(Clone, Debug)]
pub struct PeriodController {
    periods: Vec<PeriodState>,
    adjustments: Vec<AdjustmentRecord>,
    defect: Option<Defect>,
}

/// The period at zero-based `index`, saturating at `u32::MAX` (unreachable for any real ledger).
fn period_id(index: usize) -> PeriodId {
    PeriodId(u32::try_from(index).map_or(u32::MAX, |i| i.saturating_add(1)))
}

impl PeriodController {
    /// `n` periods numbered `P1..=Pn`, all open and empty.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            periods: vec![PeriodState::default(); n],
            adjustments: Vec::new(),
            defect: None,
        }
    }

    /// Rebuild from a snapshot (replay, test harnesses). Does not validate: call [`check_state`].
    #[must_use]
    pub fn from_snapshot(periods: Vec<PeriodState>) -> Self {
        Self {
            periods,
            adjustments: Vec::new(),
            defect: None,
        }
    }

    /// Seed a known defect. Test-only: exists only with the `fault-injection` feature.
    #[cfg(feature = "fault-injection")]
    #[must_use]
    pub fn inject(mut self, defect: Defect) -> Self {
        self.defect = Some(defect);
        self
    }

    /// A copy of every period's state, in order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<PeriodState> {
        self.periods.clone()
    }

    /// Every period's state, in order.
    #[must_use]
    pub fn periods(&self) -> &[PeriodState] {
        &self.periods
    }

    /// One period's state, or `None` for an unknown id.
    #[must_use]
    pub fn period(&self, id: PeriodId) -> Option<&PeriodState> {
        self.index(id).map(|i| &self.periods[i])
    }

    /// All corrections posted so far, oldest first.
    #[must_use]
    pub fn adjustments(&self) -> &[AdjustmentRecord] {
        &self.adjustments
    }

    fn has(&self, d: Defect) -> bool {
        self.defect == Some(d)
    }

    fn index(&self, id: PeriodId) -> Option<usize> {
        let i = usize::try_from(id.0.checked_sub(1)?).ok()?;
        (i < self.periods.len()).then_some(i)
    }

    fn get(&self, id: PeriodId) -> Result<&PeriodState, Error> {
        self.period(id).ok_or(Error::UnknownPeriod(id))
    }

    fn first_not_closed_before(&self, id: PeriodId) -> Option<PeriodId> {
        let i = self.index(id)?;
        (0..i)
            .find(|&q| self.periods[q].status != Status::Closed)
            .map(period_id)
    }

    fn first_closed_after(&self, id: PeriodId) -> Option<PeriodId> {
        let i = self.index(id)?;
        (i + 1..self.periods.len())
            .find(|&q| self.periods[q].status == Status::Closed)
            .map(period_id)
    }

    fn expect_status(&self, id: PeriodId, expected: Status) -> Result<&PeriodState, Error> {
        let st = self.get(id)?;
        if st.status == expected {
            Ok(st)
        } else {
            Err(Error::WrongStatus {
                period: id,
                expected,
                actual: st.status,
            })
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
    ///
    /// # Errors
    ///
    /// Returns the [`Error`] naming the first rule the command would break. State is untouched.
    pub fn decide(&self, cmd: &Command) -> Result<Event, Error> {
        match cmd {
            Command::Post {
                period,
                debit,
                credit,
            } => self.decide_post(*period, *debit, *credit),
            Command::PostClosing {
                period,
                debit,
                credit,
            } => self.decide_closing_post(*period, *debit, *credit),
            Command::BeginClose(p) => self.decide_begin_close(*p),
            Command::Reconcile(p) => self.decide_reconcile(*p),
            Command::Close(p) => self.decide_close(*p),
            Command::ApproveReopen(p) => self.decide_approve_reopen(*p),
            Command::Reopen(p) => self.decide_reopen(*p),
            Command::PostAdjustment {
                closed_period,
                into_period,
                amount,
                reference,
            } => self.decide_adjustment(*closed_period, *into_period, *amount, reference),
        }
    }

    fn decide_post(&self, period: PeriodId, debit: u64, credit: u64) -> Result<Event, Error> {
        let seeded_hole =
            self.has(Defect::PostIntoClosed) && self.get(period)?.status == Status::Closed;
        let st = if seeded_hole {
            self.get(period)?
        } else {
            self.expect_status(period, Status::Open)?
        };
        self.check_posting(st, debit, credit)?;
        Ok(Event::Posted {
            period,
            debit,
            credit,
        })
    }

    fn decide_closing_post(
        &self,
        period: PeriodId,
        debit: u64,
        credit: u64,
    ) -> Result<Event, Error> {
        let st = self.expect_status(period, Status::Closing)?;
        self.check_posting(st, debit, credit)?;
        Ok(Event::ClosingEntryPosted {
            period,
            debit,
            credit,
        })
    }

    fn decide_begin_close(&self, p: PeriodId) -> Result<Event, Error> {
        self.expect_status(p, Status::Open)?;
        match self.first_not_closed_before(p) {
            Some(blocking) => Err(Error::EarlierPeriodNotClosed {
                period: p,
                blocking,
            }),
            None => Ok(Event::CloseBegun(p)),
        }
    }

    fn decide_reconcile(&self, p: PeriodId) -> Result<Event, Error> {
        if self.expect_status(p, Status::Closing)?.reconciled {
            return Err(Error::AlreadyReconciled(p));
        }
        Ok(Event::Reconciled(p))
    }

    fn decide_close(&self, p: PeriodId) -> Result<Event, Error> {
        let st = self.expect_status(p, Status::Closing)?;
        if !st.reconciled && !self.has(Defect::CloseWithoutReconciliation) {
            return Err(Error::NotReconciled(p));
        }
        if !self.has(Defect::CloseOutOfSequence) {
            if let Some(blocking) = self.first_not_closed_before(p) {
                return Err(Error::EarlierPeriodNotClosed {
                    period: p,
                    blocking,
                });
            }
        }
        Ok(Event::Closed(p))
    }

    fn decide_approve_reopen(&self, p: PeriodId) -> Result<Event, Error> {
        if self.expect_status(p, Status::Closed)?.reopen_approved {
            return Err(Error::AlreadyApproved(p));
        }
        Ok(Event::ReopenApproved(p))
    }

    fn decide_reopen(&self, p: PeriodId) -> Result<Event, Error> {
        let st = self.expect_status(p, Status::Closed)?;
        if !st.reopen_approved && !self.has(Defect::ReopenWithoutApproval) {
            return Err(Error::ReopenNotApproved(p));
        }
        if !self.has(Defect::ReopenOutOfOrder) {
            if let Some(blocking) = self.first_closed_after(p) {
                return Err(Error::LaterPeriodClosed {
                    period: p,
                    blocking,
                });
            }
        }
        Ok(Event::Reopened(p))
    }

    fn decide_adjustment(
        &self,
        closed_period: PeriodId,
        into_period: PeriodId,
        amount: u64,
        reference: &str,
    ) -> Result<Event, Error> {
        self.expect_status(closed_period, Status::Closed)?;
        let target = self.expect_status(into_period, Status::Open)?;
        if reference.trim().is_empty() {
            return Err(Error::EmptyReference);
        }
        self.check_posting(target, amount, amount)?;
        Ok(Event::AdjustmentPosted {
            closed_period,
            into_period,
            amount,
            reference: reference.to_owned(),
        })
    }

    /// Fold an event into state. Events come from [`decide`](Self::decide) or from replay;
    /// unknown periods are ignored rather than panicking so a corrupt log cannot crash the kernel.
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Posted {
                period,
                debit,
                credit,
            } => self.add(*period, *debit, *credit, false),
            Event::ClosingEntryPosted {
                period,
                debit,
                credit,
            } => self.add(*period, *debit, *credit, true),
            Event::CloseBegun(p) => self.set(*p, |s| s.status = Status::Closing),
            Event::Reconciled(p) => self.set(*p, |s| s.reconciled = true),
            Event::Closed(p) => self.set(*p, |s| s.status = Status::Closed),
            Event::ReopenApproved(p) => self.set(*p, |s| s.reopen_approved = true),
            Event::Reopened(p) => self.set(*p, |s| {
                s.status = Status::Open;
                s.reconciled = false;
                s.reopen_approved = false;
            }),
            Event::AdjustmentPosted {
                closed_period,
                into_period,
                amount,
                reference,
            } => {
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
    ///
    /// # Errors
    ///
    /// Same as [`decide`](Self::decide); on error nothing is applied.
    pub fn execute(&mut self, cmd: &Command) -> Result<Event, Error> {
        let event = self.decide(cmd)?;
        self.apply(&event);
        Ok(event)
    }
}

/// State invariants P1 to P3.
///
/// # Errors
///
/// Returns the first [`Violation`] found, scanning periods in order.
pub fn check_state(periods: &[PeriodState]) -> Result<(), Violation> {
    for (i, st) in periods.iter().enumerate() {
        let period = period_id(i);
        if st.debit != st.credit {
            return Err(Violation {
                rule: Rule::P1,
                period,
            });
        }
        if st.status == Status::Closed {
            if periods[..i].iter().any(|q| q.status != Status::Closed) {
                return Err(Violation {
                    rule: Rule::P2,
                    period,
                });
            }
            if !st.reconciled {
                return Err(Violation {
                    rule: Rule::P3,
                    period,
                });
            }
        }
    }
    Ok(())
}

/// Action properties A1 to A3 over one step from `before` to `after`.
///
/// # Errors
///
/// Returns the first [`Violation`] found, scanning periods in order.
pub fn check_transition(before: &[PeriodState], after: &[PeriodState]) -> Result<(), Violation> {
    for (i, (b, a)) in before.iter().zip(after).enumerate() {
        let period = period_id(i);
        if b.status != Status::Closed {
            continue;
        }
        if a.status == Status::Closed && (a.debit != b.debit || a.credit != b.credit) {
            return Err(Violation {
                rule: Rule::A1,
                period,
            });
        }
        if a.status != Status::Closed && !(b.reopen_approved && a.status == Status::Open) {
            return Err(Violation {
                rule: Rule::A2,
                period,
            });
        }
        if a.status == Status::Open && after[i + 1..].iter().any(|q| q.status == Status::Closed) {
            return Err(Violation {
                rule: Rule::A3,
                period,
            });
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
        Command::Post {
            period: p,
            debit: a,
            credit: a,
        }
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
            Err(Error::EarlierPeriodNotClosed {
                period: P2,
                blocking: P1
            })
        );
    }

    #[test]
    fn cannot_close_without_reconciliation() {
        let mut c = PeriodController::new(1);
        c.execute(&Command::BeginClose(P1)).unwrap();
        assert_eq!(
            c.execute(&Command::Close(P1)),
            Err(Error::NotReconciled(P1))
        );
    }

    #[test]
    fn closing_entry_invalidates_reconciliation() {
        let mut c = PeriodController::new(1);
        c.execute(&Command::BeginClose(P1)).unwrap();
        c.execute(&Command::Reconcile(P1)).unwrap();
        c.execute(&Command::PostClosing {
            period: P1,
            debit: 5,
            credit: 5,
        })
        .unwrap();
        assert_eq!(
            c.execute(&Command::Close(P1)),
            Err(Error::NotReconciled(P1))
        );
    }

    #[test]
    fn closed_period_rejects_posting() {
        let mut c = PeriodController::new(1);
        close_cycle(&mut c, P1);
        assert!(matches!(
            c.execute(&post(P1, 1)),
            Err(Error::WrongStatus { .. })
        ));
    }

    #[test]
    fn unbalanced_and_zero_postings_rejected() {
        let mut c = PeriodController::new(1);
        assert_eq!(
            c.execute(&Command::Post {
                period: P1,
                debit: 5,
                credit: 4
            }),
            Err(Error::Unbalanced {
                debit: 5,
                credit: 4
            })
        );
        assert_eq!(c.execute(&post(P1, 0)), Err(Error::ZeroAmount));
        assert_eq!(
            c.execute(&Command::Post {
                period: P1,
                debit: u64::MAX,
                credit: u64::MAX
            }),
            Ok(Event::Posted {
                period: P1,
                debit: u64::MAX,
                credit: u64::MAX
            })
        );
        assert_eq!(c.execute(&post(P1, 1)), Err(Error::Overflow));
    }

    #[test]
    fn reopen_needs_approval_and_order() {
        let mut c = PeriodController::new(2);
        close_cycle(&mut c, P1);
        close_cycle(&mut c, P2);
        assert_eq!(
            c.execute(&Command::Reopen(P1)),
            Err(Error::ReopenNotApproved(P1))
        );
        c.execute(&Command::ApproveReopen(P1)).unwrap();
        assert_eq!(
            c.execute(&Command::ApproveReopen(P1)),
            Err(Error::AlreadyApproved(P1))
        );
        assert_eq!(
            c.execute(&Command::Reopen(P1)),
            Err(Error::LaterPeriodClosed {
                period: P1,
                blocking: P2
            })
        );
        c.execute(&Command::ApproveReopen(P2)).unwrap();
        c.execute(&Command::Reopen(P2)).unwrap();
        c.execute(&Command::Reopen(P1)).unwrap();
        let s = c.period(P1).unwrap();
        assert_eq!(
            (s.status, s.reconciled, s.reopen_approved),
            (Status::Open, false, false)
        );
    }

    #[test]
    fn adjustment_goes_to_open_period_with_back_reference() {
        let mut c = PeriodController::new(2);
        c.execute(&post(P1, 100)).unwrap();
        close_cycle(&mut c, P1);
        let before = c.snapshot();
        c.execute(&Command::PostAdjustment {
            closed_period: P1,
            into_period: P2,
            amount: 7,
            reference: "INV-42 correction".into(),
        })
        .unwrap();
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
        let adj = |from, to, r: &str| Command::PostAdjustment {
            closed_period: from,
            into_period: to,
            amount: 1,
            reference: r.into(),
        };
        assert!(matches!(
            c.decide(&adj(P1, P2, "x")),
            Err(Error::WrongStatus { .. })
        ));
        close_cycle(&mut c, P1);
        assert!(matches!(
            c.decide(&adj(P1, P1, "x")),
            Err(Error::WrongStatus { .. })
        ));
        assert_eq!(c.decide(&adj(P1, P2, "  ")), Err(Error::EmptyReference));
    }

    #[test]
    fn unknown_period_is_an_error_not_a_panic() {
        let c = PeriodController::new(2);
        assert_eq!(
            c.decide(&Command::Close(PeriodId(0))),
            Err(Error::UnknownPeriod(PeriodId(0)))
        );
        assert_eq!(
            c.decide(&Command::Close(PeriodId(9))),
            Err(Error::UnknownPeriod(PeriodId(9)))
        );
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
        let closed = PeriodState {
            status: Status::Closed,
            debit: 1,
            credit: 1,
            reconciled: true,
            reopen_approved: false,
        };
        let mut changed = closed.clone();
        changed.debit = 2;
        changed.credit = 2;
        assert_eq!(
            check_transition(std::slice::from_ref(&closed), &[changed])
                .unwrap_err()
                .rule,
            Rule::A1
        );
        let reopened = PeriodState {
            status: Status::Open,
            ..closed.clone()
        };
        assert_eq!(
            check_transition(std::slice::from_ref(&closed), &[reopened])
                .unwrap_err()
                .rule,
            Rule::A2
        );
        let approved = PeriodState {
            reopen_approved: true,
            ..closed.clone()
        };
        let reopened = PeriodState {
            status: Status::Open,
            ..approved.clone()
        };
        assert_eq!(
            check_transition(&[approved.clone(), approved.clone()], &[reopened, approved])
                .unwrap_err()
                .rule,
            Rule::A3
        );
    }
}
