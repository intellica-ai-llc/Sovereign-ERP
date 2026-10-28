------------------------- MODULE PeriodClose -------------------------
\* Period-close state machine for the Truth Plane (Round 7 sketch).
\* Periods are numbered 1..N. Each period keeps balanced debit and credit totals.
\* Rules: close in sequence, only after reconciliation, closed periods are immutable,
\*        reopening needs a recorded approval and no later period may be closed.
EXTENDS Naturals

CONSTANTS N, MaxAmt, MaxTotal
Periods == 1..N

VARIABLES status,    \* status[p] in {"open","closing","closed"}
          d, c,      \* posted debit and credit totals per period
          recon,     \* recon[p] = TRUE once sub-ledgers were reconciled for p
          approved   \* set of periods for which a reopen was approved

vars == <<status, d, c, recon, approved>>

Init == /\ status = [p \in Periods |-> "open"]
        /\ d = [p \in Periods |-> 0] /\ c = [p \in Periods |-> 0]
        /\ recon = [p \in Periods |-> FALSE] /\ approved = {}

AllEarlierClosed(p) == \A q \in Periods : q < p => status[q] = "closed"
NoLaterClosed(p)    == \A q \in Periods : q > p => status[q] # "closed"

\* A balanced posting into an open period.
Post(p, amt) ==
  /\ status[p] = "open" /\ amt \in 1..MaxAmt /\ d[p] + amt <= MaxTotal
  /\ d' = [d EXCEPT ![p] = @ + amt] /\ c' = [c EXCEPT ![p] = @ + amt]
  /\ UNCHANGED <<status, recon, approved>>

\* A balanced closing entry (accrual, revaluation) allowed only while closing.
PostClosing(p, amt) ==
  /\ status[p] = "closing" /\ amt \in 1..MaxAmt /\ d[p] + amt <= MaxTotal
  /\ d' = [d EXCEPT ![p] = @ + amt] /\ c' = [c EXCEPT ![p] = @ + amt]
  /\ recon' = [recon EXCEPT ![p] = FALSE]        \* new entries invalidate reconciliation
  /\ UNCHANGED <<status, approved>>

BeginClose(p) ==
  /\ status[p] = "open" /\ AllEarlierClosed(p)
  /\ status' = [status EXCEPT ![p] = "closing"]
  /\ UNCHANGED <<d, c, recon, approved>>

Reconcile(p) ==
  /\ status[p] = "closing" /\ ~recon[p]
  /\ recon' = [recon EXCEPT ![p] = TRUE]
  /\ UNCHANGED <<status, d, c, approved>>

Close(p) ==
  /\ status[p] = "closing" /\ recon[p] /\ AllEarlierClosed(p)
  /\ status' = [status EXCEPT ![p] = "closed"]
  /\ UNCHANGED <<d, c, recon, approved>>

ApproveReopen(p) ==
  /\ status[p] = "closed" /\ p \notin approved
  /\ approved' = approved \cup {p}
  /\ UNCHANGED <<status, d, c, recon>>

Reopen(p) ==
  /\ status[p] = "closed" /\ p \in approved /\ NoLaterClosed(p)
  /\ status' = [status EXCEPT ![p] = "open"]
  /\ recon' = [recon EXCEPT ![p] = FALSE]
  /\ approved' = approved \ {p}
  /\ UNCHANGED <<d, c>>

Next == \E p \in Periods : \/ \E amt \in 1..MaxAmt : Post(p, amt) \/ PostClosing(p, amt)
                           \/ BeginClose(p) \/ Reconcile(p) \/ Close(p)
                           \/ ApproveReopen(p) \/ Reopen(p)

Spec == Init /\ [][Next]_vars

TypeOK == /\ status \in [Periods -> {"open","closing","closed"}]
          /\ d \in [Periods -> Nat] /\ c \in [Periods -> Nat]
          /\ recon \in [Periods -> BOOLEAN] /\ approved \subseteq Periods

\* P1 Conservation per period (debits equal credits).
P1_Balanced == \A p \in Periods : d[p] = c[p]
\* P2 Periods close strictly in sequence.
P2_Sequential == \A p \in Periods : status[p] = "closed" => AllEarlierClosed(p)
\* P3 A closed period was reconciled.
P3_ClosedReconciled == \A p \in Periods : status[p] = "closed" => recon[p]

\* ---------------- Action properties ----------------
\* A1 Closed periods are immutable while they stay closed.
A1_ClosedImmutable == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] = "closed") => (d'[p] = d[p] /\ c'[p] = c[p]) ]_vars
\* A2 A closed period only becomes open again with a recorded approval.
A2_ReopenApproved == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] # "closed") => (p \in approved /\ status'[p] = "open") ]_vars
\* A3 No later period is closed when an earlier one is reopened.
A3_ReopenOrder == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] = "open") => (\A q \in Periods : q > p => status'[q] # "closed") ]_vars
=============================================================================
