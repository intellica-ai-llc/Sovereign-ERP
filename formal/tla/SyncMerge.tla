------------------------------ MODULE SyncMerge ------------------------------
(***************************************************************************)
(* Offline replication of a work order (crates/sov-sync).                  *)
(*                                                                         *)
(* Replicas make local operations and exchange state at any time. State is *)
(* joined by union and maximum, so it must depend only on WHICH operations *)
(* a replica has seen, never on the order or number of deliveries. That is *)
(* checked as an invariant (Abstraction): each replica's state equals a    *)
(* fixed function of its set of seen operations. Strong eventual           *)
(* consistency (SEC) follows, and every step must only move a replica      *)
(* upward (Inflation).                                                     *)
(*                                                                         *)
(* Modelled: lifecycle status (a maximum register; a reopening is an       *)
(* upward step), field writes with Lamport stamps, add-only entries, and   *)
(* voids of known entries. Bounded: MaxLocal local operations per replica. *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets

CONSTANTS Replicas, MaxLocal

VARIABLES st, wr, en, vd, seen, n
vars == <<st, wr, en, vd, seen, n>>

Max(S) == CHOOSE x \in S : \A y \in S : y <= x
MaxStatus == 3
Values == {"a", "b"}
MaxStamp == MaxLocal * Cardinality(Replicas)

OpsOf(kind, O) == {o \in O : o.kind = kind}
StatusOf(O)  == Max({0} \cup {o.to : o \in OpsOf("adv", O)})
WritesOf(O)  == {[c |-> o.c, r |-> o.r, v |-> o.v] : o \in OpsOf("set", O)}
EntriesOf(O) == {o.e : o \in OpsOf("add", O)}
VoidsOf(O)   == {o.e : o \in OpsOf("void", O)}

TypeOK ==
    /\ st \in [Replicas -> 0..MaxStatus]
    /\ n  \in [Replicas -> 0..MaxLocal]
    /\ \A r \in Replicas :
        /\ wr[r] \subseteq [c : 1..MaxStamp, r : Replicas, v : Values]
        /\ en[r] \subseteq Replicas \X (0..(MaxLocal - 1))
        /\ vd[r] \subseteq en[r]

Init ==
    /\ st   = [r \in Replicas |-> 0]
    /\ wr   = [r \in Replicas |-> {}]
    /\ en   = [r \in Replicas |-> {}]
    /\ vd   = [r \in Replicas |-> {}]
    /\ seen = [r \in Replicas |-> {}]
    /\ n    = [r \in Replicas |-> 0]

Record(r, op) ==
    /\ seen' = [seen EXCEPT ![r] = @ \cup {op}]
    /\ n'    = [n EXCEPT ![r] = @ + 1]

SetOp(r, v) ==
    LET c  == 1 + Max({0} \cup {w.c : w \in wr[r]})
        op == [kind |-> "set", id |-> <<r, n[r]>>, c |-> c, r |-> r, v |-> v]
    IN /\ n[r] < MaxLocal
       /\ wr' = [wr EXCEPT ![r] = @ \cup {[c |-> c, r |-> r, v |-> v]}]
       /\ UNCHANGED <<st, en, vd>>
       /\ Record(r, op)

AdvOp(r) ==
    LET op == [kind |-> "adv", id |-> <<r, n[r]>>, to |-> st[r] + 1]
    IN /\ n[r] < MaxLocal
       /\ st[r] < MaxStatus
       /\ st' = [st EXCEPT ![r] = @ + 1]
       /\ UNCHANGED <<wr, en, vd>>
       /\ Record(r, op)

AddOp(r) ==
    LET op == [kind |-> "add", id |-> <<r, n[r]>>, e |-> <<r, n[r]>>]
    IN /\ n[r] < MaxLocal
       /\ en' = [en EXCEPT ![r] = @ \cup {<<r, n[r]>>}]
       /\ UNCHANGED <<st, wr, vd>>
       /\ Record(r, op)

VoidOp(r, e) ==
    LET op == [kind |-> "void", id |-> <<r, n[r]>>, e |-> e]
    IN /\ n[r] < MaxLocal
       /\ e \in en[r] \ vd[r]
       /\ vd' = [vd EXCEPT ![r] = @ \cup {e}]
       /\ UNCHANGED <<st, wr, en>>
       /\ Record(r, op)

\* Replica r receives replica s's whole state (or a stale copy of it: the same union).
Merge(r, s) ==
    /\ r # s
    /\ seen[s] \ seen[r] # {}
    /\ st'   = [st EXCEPT ![r] = Max({st[r], st[s]})]
    /\ wr'   = [wr EXCEPT ![r] = @ \cup wr[s]]
    /\ en'   = [en EXCEPT ![r] = @ \cup en[s]]
    /\ vd'   = [vd EXCEPT ![r] = @ \cup vd[s]]
    /\ seen' = [seen EXCEPT ![r] = @ \cup seen[s]]
    /\ UNCHANGED n

Next ==
    \E r \in Replicas :
        \/ \E v \in Values : SetOp(r, v)
        \/ AdvOp(r)
        \/ AddOp(r)
        \/ \E e \in en[r] : VoidOp(r, e)
        \/ \E s \in Replicas : Merge(r, s)

Spec == Init /\ [][Next]_vars

\* State is a fixed function of the set of seen operations: no dependence on delivery order or count.
Abstraction ==
    \A r \in Replicas :
        /\ st[r] = StatusOf(seen[r])
        /\ wr[r] = WritesOf(seen[r])
        /\ en[r] = EntriesOf(seen[r])
        /\ vd[r] = VoidsOf(seen[r])

\* Strong eventual consistency: same operations seen, same state.
SEC ==
    \A r, s \in Replicas :
        seen[r] = seen[s] =>
            /\ st[r] = st[s] /\ wr[r] = wr[s] /\ en[r] = en[s] /\ vd[r] = vd[s]

\* A void never refers to an entry the replica has not seen; no entry or void is ever lost.
VoidsKnown == \A r \in Replicas : vd[r] \subseteq en[r]

\* Every step only moves a replica upward.
Inflation ==
    [][\A r \in Replicas :
        /\ st[r] <= st'[r]
        /\ wr[r] \subseteq wr'[r]
        /\ en[r] \subseteq en'[r]
        /\ vd[r] \subseteq vd'[r]
        /\ seen[r] \subseteq seen'[r]]_vars
=============================================================================
