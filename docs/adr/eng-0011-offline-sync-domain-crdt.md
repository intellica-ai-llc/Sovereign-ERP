# eng-0011: Offline sync as a domain-specific, verified state-based CRDT

- Status: accepted (resolves arc42 ADR 028 for work-order replication)
- Date: 2026-11-02

## Context
Field technicians work for hours without a connection while planners change the same work orders. Competitors ship offline modes, but the mature sync engines are generic: Automerge, Yjs, Ditto, PowerSync, ElectricSQL, Zero and Triplit replicate documents or tables. One vendor's own guidance names the failure a naive design has: with a single status field, concurrent updates mean one action wins and the other is silently lost. Several engines are hosted or commercial, which conflicts with a sovereign deployment, and a third-party engine would enter the trusted base against eng-0003. None found ties convergence to field-work rules.

## Decision
Replicate work orders with our own **state-based CRDT** (`crates/sov-sync`):
- Lifecycle is a maximum register over `(epoch, rank)`; a reopen starts a new epoch, so it is also an upward move.
- Labour and material entries are **immutable and add-only**; correction is a **void**, as in the ledger. Nothing is edited or deleted.
- Field values are kept as a grow-only set of writes, each recording the write it overwrote. The newest wins for display, **no write is discarded**, and concurrent siblings are surfaced.
- Conflicts needing a person (an entry made without knowledge that the order was complete; concurrent edits of one field) are **computed from state**, so every replica reports the same ones.
- Ordering uses Lamport counters with the replica id as tie-break: no wall clock, no entropy.
- A canonical text encoding makes documents hashable and signable.

Verification, all in CI: lattice laws (commutative, associative, idempotent), inflation of every operation, and convergence checked on every state two replicas can reach to a bounded depth; seeded simulation of five replicas with lost, duplicated and reordered messages (no entry lost, no void undone); and a TLA+ model (`SyncMerge`) checking that each replica's state equals a fixed function of the operations it has seen, with three deliberate defects shown to be caught.

## Consequences
We own a small, fully understood sync core instead of integrating a large generic engine; partial replication per work order falls out naturally. Costs: documents are sent whole (fine for work-order sizes; delta encoding is future work); text-heavy collaborative editing is out of scope; the verified bounds are small. Other aggregates (assets, inventory counts) will need their own lattices, and each will need its own verification.
