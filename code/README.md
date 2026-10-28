# Sovereign ERP: code

Walking-skeleton workspace for the architecture in `arc42-v*.md` (repo root).
Status: **spike 8 (Period Controller) and spike 1 (Ledger Port conformance), Appendix C**. No production code yet; the Definition of Ready is not fully met.

## Layout

| Path | What it is | arc42 |
|---|---|---|
| `crates/sim` | Deterministic `Rng` (SplitMix64) and `Clock`; no wall clock or OS entropy in kernel code | 8.23, ADR 046 |
| `crates/period-controller` | Pure decide/evolve state machine for period close, reopen, adjustments; invariants P1 to P3, A1 to A3 as checkable functions | 5.2 (P2), App. M, ADR 039 |
| `crates/conformance` | Exhaustive model check of the controller, lockstep check against a transcription of the TLA+ spec, mutation tests, random walks | S21, spike 8 |
| `crates/ledger-port` | Engine-neutral Ledger Port trait, in-memory reference ledger (`MemLedger`), invariants I1 to I4 + LegalMove, a conformance suite any engine adapter must pass, bounded model checker | 5.2 (P2), App. D, ADR 002 |
| `formal/` | `PeriodClose.*`, `ClosedPeriod.lean` (App. M) and `Ledger.*`, `LedgerConservation.lean` (App. D), copied verbatim from the arc42 | App. D, M |

## Run

```sh
cargo test --workspace                       # everything (about 30 s in debug)
cargo test -p conformance -- --nocapture     # prints mutation results and state counts
```

## What the tests establish

- The spec transcription, explored with the TLC bounds (3 periods, MaxAmt 2, MaxTotal 3), gives **4,160 distinct / 18,833 generated states**, the same counts recorded in Appendix M.
- The real controller, explored through its public `decide`/`apply` API, reaches the same state space with no violation of P1 to P3 or A1 to A3.
- Lockstep: on all 4,160 reachable states and every action, the controller and the spec agree on enabledness and next state (about 150k comparisons).
- Six seeded defects (`fault-injection` feature, test-only) are each caught, with shortest counterexample traces.
- Reachability probes show Closed is reachable and closing entries are posted while closing (non-vacuity).
- 300 seeded random walks of 3,000 steps each (with invalid ids, blank references, unbalanced and overflowing postings) hold all invariants; rejected commands never change state; event-log replay reproduces state exactly.

### Ledger Port (spike 1)

- `MemLedger`, explored exhaustively with the recorded TLC constants (3 accounts, 3 transfer ids, max amount 2, one guarded account), reaches **21,433 distinct states at search depth 7**, the same as Appendix D, with no violation of I1 to I4 or LegalMove.
- The recorded **713,185 generated states** is reproduced exactly once Post and Void are weighted the way TLC evaluates the spec as printed (see the test for the arithmetic).
- Six seeded defects are each caught by the intended invariant, with shortest traces of 1 to 3 steps.
- `ledger_port::suite::run(factory)` is the contract an engine adapter (TigerBeetle, Postgres-backed, anything) must pass; five of the six defects also fail this suite.
- 200 seeded random walks of 2,000 operations (unknown accounts, `u64::MAX` amounts, repeated posts and voids) hold all invariants.

## Not covered yet

Bounded and small-constant; no multi-currency, consolidation, sub-ledger detail, time or concurrency (same limits as Appendix M). The TLA+ itself was not re-run here (no Java/TLC in the sandbox): run `formal/PeriodClose.tla` and `formal/Ledger.tla` with TLC to confirm the recorded results independently. The Lean files were likewise not recompiled here.

## Next

1. TigerBeetle adapter for `LedgerPort`, run through `ledger_port::suite::run` (needs a TigerBeetle binary).
2. Extend the ledger model to linked transfers, multi-currency and period close, then wire `PeriodController` to the ledger.
3. Event store crate on PostgreSQL with explicit valid/recorded time (ADR 003).
4. Wire the Period Controller to the event store; first slice of the governed semantic layer.
