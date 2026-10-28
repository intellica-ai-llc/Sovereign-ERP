---------------------------- MODULE Ledger ----------------------------
\* Truth Plane ledger kernel, abstract model (Round 3 sketch).
\* Two-phase transfers in the style of purpose-built double-entry engines:
\* single-phase (immediate), pending, post, void. Amounts are positive naturals.
EXTENDS Naturals, FiniteSets

CONSTANTS Accounts,      \* set of account ids
          Ids,           \* set of transfer ids (bounds the model)
          MaxAmt,        \* maximum amount per transfer
          Guarded        \* accounts whose debits (posted+pending) may not exceed credits posted

VARIABLES dPosted, cPosted, dPending, cPending,   \* per-account totals
          tState, tDebit, tCredit, tAmt            \* per-transfer record

vars == <<dPosted, cPosted, dPending, cPending, tState, tDebit, tCredit, tAmt>>

States == {"none", "pending", "posted", "voided"}

RECURSIVE SumOver(_, _)
SumOver(S, f) == IF S = {} THEN 0
                 ELSE LET x == CHOOSE y \in S : TRUE IN f[x] + SumOver(S \ {x}, f)

TypeOK == /\ dPosted \in [Accounts -> Nat] /\ cPosted \in [Accounts -> Nat]
          /\ dPending \in [Accounts -> Nat] /\ cPending \in [Accounts -> Nat]
          /\ tState \in [Ids -> States]
          /\ tDebit \in [Ids -> Accounts] /\ tCredit \in [Ids -> Accounts]
          /\ tAmt \in [Ids -> Nat]

Init == /\ dPosted = [a \in Accounts |-> 0] /\ cPosted = [a \in Accounts |-> 0]
        /\ dPending = [a \in Accounts |-> 0] /\ cPending = [a \in Accounts |-> 0]
        /\ tState = [i \in Ids |-> "none"]
        /\ tDebit = [i \in Ids |-> CHOOSE a \in Accounts : TRUE]
        /\ tCredit = [i \in Ids |-> CHOOSE a \in Accounts : TRUE]
        /\ tAmt = [i \in Ids |-> 0]

\* Guard rule: a guarded debit account may not owe more than it holds.
GuardOK(d, amt) == d \in Guarded => dPosted[d] + dPending[d] + amt <= cPosted[d]

Bump(f, a, n) == [f EXCEPT ![a] = @ + n]
Drop(f, a, n) == [f EXCEPT ![a] = @ - n]

\* Immediate transfer.
CreatePosted(i, d, c, amt) ==
  /\ tState[i] = "none" /\ d # c /\ amt \in 1..MaxAmt /\ GuardOK(d, amt)
  /\ tState' = [tState EXCEPT ![i] = "posted"]
  /\ tDebit' = [tDebit EXCEPT ![i] = d] /\ tCredit' = [tCredit EXCEPT ![i] = c]
  /\ tAmt' = [tAmt EXCEPT ![i] = amt]
  /\ dPosted' = Bump(dPosted, d, amt) /\ cPosted' = Bump(cPosted, c, amt)
  /\ UNCHANGED <<dPending, cPending>>

\* Reserve amounts without final posting.
CreatePending(i, d, c, amt) ==
  /\ tState[i] = "none" /\ d # c /\ amt \in 1..MaxAmt /\ GuardOK(d, amt)
  /\ tState' = [tState EXCEPT ![i] = "pending"]
  /\ tDebit' = [tDebit EXCEPT ![i] = d] /\ tCredit' = [tCredit EXCEPT ![i] = c]
  /\ tAmt' = [tAmt EXCEPT ![i] = amt]
  /\ dPending' = Bump(dPending, d, amt) /\ cPending' = Bump(cPending, c, amt)
  /\ UNCHANGED <<dPosted, cPosted>>

Post(i) ==
  /\ tState[i] = "pending"
  /\ LET d == tDebit[i] c == tCredit[i] amt == tAmt[i] IN
       /\ dPending' = Drop(dPending, d, amt) /\ cPending' = Drop(cPending, c, amt)
       /\ dPosted' = Bump(dPosted, d, amt) /\ cPosted' = Bump(cPosted, c, amt)
  /\ tState' = [tState EXCEPT ![i] = "posted"]
  /\ UNCHANGED <<tDebit, tCredit, tAmt>>

Void(i) ==
  /\ tState[i] = "pending"
  /\ LET d == tDebit[i] c == tCredit[i] amt == tAmt[i] IN
       /\ dPending' = Drop(dPending, d, amt) /\ cPending' = Drop(cPending, c, amt)
  /\ tState' = [tState EXCEPT ![i] = "voided"]
  /\ UNCHANGED <<dPosted, cPosted, tDebit, tCredit, tAmt>>

Next == \E i \in Ids : \/ \E d, c \in Accounts, amt \in 1..MaxAmt :
                            CreatePosted(i, d, c, amt) \/ CreatePending(i, d, c, amt)
                        \/ Post(i) \/ Void(i)

Spec == Init /\ [][Next]_vars

\* ---------------- Invariants ----------------
\* I1 Conservation: posted debits equal posted credits; pending debits equal pending credits.
Conservation == /\ SumOver(Accounts, dPosted) = SumOver(Accounts, cPosted)
                /\ SumOver(Accounts, dPending) = SumOver(Accounts, cPending)

\* I2 Guard: guarded accounts never owe more than they hold.
GuardInv == \A a \in Guarded : dPosted[a] + dPending[a] <= cPosted[a]

\* I3 Pending totals are exactly the sum of pending transfers (no leaked reservations).
PendingExact ==
  \A a \in Accounts :
    /\ dPending[a] = SumOver({i \in Ids : tState[i] = "pending" /\ tDebit[i] = a}, tAmt)
    /\ cPending[a] = SumOver({i \in Ids : tState[i] = "pending" /\ tCredit[i] = a}, tAmt)

\* I4 Posted totals are exactly the sum of posted transfers (balances derivable from the journal).
PostedExact ==
  \A a \in Accounts :
    /\ dPosted[a] = SumOver({i \in Ids : tState[i] = "posted" /\ tDebit[i] = a}, tAmt)
    /\ cPosted[a] = SumOver({i \in Ids : tState[i] = "posted" /\ tCredit[i] = a}, tAmt)

\* ---------------- Action property: immutability and legal state moves ----------------
LegalMove == [][ \A i \in Ids :
    /\ (tState[i] # "none" => tDebit'[i] = tDebit[i] /\ tCredit'[i] = tCredit[i] /\ tAmt'[i] = tAmt[i])
    /\ (tState[i] = "posted" => tState'[i] = "posted")
    /\ (tState[i] = "voided" => tState'[i] = "voided")
    /\ (tState[i] = "pending" => tState'[i] \in {"pending", "posted", "voided"})
  ]_vars
=============================================================================
