# Verification status

What is claimed, what it rests on, and what is not yet covered. Update in the same PR as any change.

| Claim | Evidence | Status |
|---|---|---|
| Period-close model satisfies P1 to P3, A1 to A3 (3 periods, amounts 1..2, total 3) | TLC, `formal/tla/PeriodClose.tla` | checked in CI |
| Rust `PeriodController` implements that model | Exhaustive exploration through the real API reaches the same 4,160 states with no violation; lockstep comparison with a Rust transcription of the spec on every state and action (~150k comparisons) | checked in `cargo test` |
| The checks can fail | Six seeded defects, each caught by the intended rule | checked in `cargo test` |
| Ledger model satisfies I1 to I4 and LegalMove (3 accounts, 3 ids, amounts 1..2) | TLC, `formal/tla/Ledger.tla` | checked in CI |
| `MemLedger` implements that model | Exhaustive exploration: 21,433 distinct states, depth 7, same as TLC; TLC's 713,185 generated reproduced under its counting convention | checked in `cargo test` |
| Any engine adapter honours the contract | `sov_ledger_port::suite::run` plus invariant checks | suite exists; no adapter yet |
| Maximo relationship where-clauses are parsed into joins, and everything unsupported is named | 12 tests incl. 3,000 generated clauses that must round-trip, injected unsupported constructs that must be rejected, and 20,000 random strings that must never panic. Not yet run against a real `MAXRELATIONSHIP` export | checked in `cargo test`; real-dictionary coverage pending |
| Undeclared relationships are recoverable from data on a real ERP | `cargo xtask bench-fk` (CI gate) on Apache `OFBiz` at a pinned commit: held-out precision 0.86, recall 0.82; inclusion only 0.12 and 0.94; arc42 App. J heuristic reimplemented 0.53 and 0.25. See `docs/research/fk-discovery.md` for limits | checked in CI |
| Discovery transfers to databases it was not tuned on | `cargo xtask bench-transfer` (manual, about 6 minutes): leave-one-corpus-out over `OFBiz`, Chinook, Northwind, Sakila with neutral-start tuning: mean held-out F1 0.86 (range 0.77 to 0.98); the old heuristic wins on Northwind alone | checked manually, floor 0.80 |
| Discovery works when names are meaningless | `OFBiz` rewritten with opaque names (and shuffled columns), weights re-fitted: F1 0.70 (0.67); unadapted readable-name weights score recall 0.08, hence `Weights::auto` | measured; opaque profile fitted on one corpus |
| Dictionary and inferred links reconcile into one correct answer | `sov-profiler::reconcile` on the synthetic Maximo-shaped database, 4 seeds: all 22 declared relationships oriented correctly (a 1:1 settled by the hub tie-break), orphan rows counted exactly (3, 4, 2 injected), no relationship reported that is not real. Synthetic data, not a real Maximo | checked in `cargo test`; real-data pending |
| Stored balances are re-derived and drift is caught | `sov-profiler::audit`: exact fixed-point; 3 of 3 injected drifts found, 0 false alarms among 400 balances; error cases tested | checked in `cargo test`; synthetic data |
| Offline work-order replication converges and loses nothing | `sov-sync`: lattice laws and inflation on every reachable state of two replicas to a bounded depth (6,005 states, 12,282 transitions); 500 seeded multi-replica histories with lost, duplicated, reordered messages; TLC `SyncMerge` (9,018 distinct states, three deliberate defects caught) | checked in CI; small bounds, never run on a device |
| The evaluation tool behaves as documented and never alters input | `sov-cli`: 8 integration tests (usage errors exit 2, runtime errors exit 1, deterministic output, input files byte-identical before and after, row-limit disclosed, report digest equals SHA-256 of the report body, real-binary exit codes) | checked in `cargo test` |
| SHA-256 is correct | `sov-evidence`: FIPS 180-4 examples, million-`a`, Python `hashlib` vectors at every block boundary, every streaming split | checked in `cargo test` |
| The workspace builds and passes on the pinned modern toolchain | Rust 1.99.1: fmt, clippy `-D warnings`, docs and all tests | checked in CI (eng-0013) |
| A release tarball can be reproduced and smoke-tested | `cargo xtask dist`: reproducible archive, checksums, SBOM, packaged binary runs | checked in CI; signing not yet run |
| Closed-period and conservation lemmas | Lean 4, `formal/lean` | checked in CI |
| Rust code is lint-clean and `unsafe`-free | clippy pedantic with warnings denied, `unsafe_code = "forbid"` | checked in CI |

## Not covered yet

- Known recall gaps on the synthetic Maximo-shaped database: role-named references, different names on each side of a link, and links into tiny tables; inference finds 8 of 22 declared links there (the dictionary finds all 22). See `docs/research/competitive-landscape.md`.
- Bounds are small. Multi-currency, linked transfers, consolidation, time and concurrency are outside both models.
- Discovery quality is measured on one open-source ERP with small demo data (about 13,000 rows) plus three small, clean sample databases, never on Maximo or on production-sized tables. The opaque-name profile rests on one corpus. The sketch-based path for large tables is designed, not built.
- No real Maximo database has touched this code. The where-clause parser has only seen clauses written from IBM's documented form, not a customer dictionary. Arc42 Appendix C's Definition of Ready items on real schemas are open.
- No TigerBeetle adapter; the Ledger Port contract is exercised only by `MemLedger`.
- No property-based fuzzing, mutation testing of the Rust tests themselves (`cargo-mutants`), Miri or coverage gate. Candidates for the next quality ADR.
- Seeded random walks are fixed-seed; they do not explore beyond what the exhaustive checks and seeds reach.
