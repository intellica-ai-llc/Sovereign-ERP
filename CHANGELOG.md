# Changelog

Format: [Keep a Changelog](https://keepachangelog.com). Nothing is released yet.

## 0.1.0 (evaluation checkpoint)

### Added
- `sov` command-line tool (`profile`, `audit`, `demo`, `digest`), `sov-evidence` (SHA-256), `cargo xtask doctor|dist|sbom`, `scripts/bootstrap.sh`, release workflow (not yet exercised), status page, evaluation guide, security and privacy statement, investor brief, evaluation notice (draft), agents research note, ADRs eng-0013 to eng-0015.
- Toolchain pinned to Rust 1.99.1 (verified).

## Earlier work (same release)

### Added
- `sov-sync`: offline work-order replication core with exhaustive and simulated verification; TLA+ `SyncMerge`; mobile and connectivity research; ADRs eng-0011 and eng-0012.
- `sov-profiler::reconcile`, `::audit`, `::synth`; competitive landscape research note; ADR eng-0010.
- Cross-corpus evaluation (Chinook, Northwind, Sakila via `bench/sqlite`), opaque-name rewrite of `OFBiz` (`bench/ofbiz/obfuscate.py`), `cargo xtask bench-transfer`, and ADR eng-0009.
- `Weights::opaque`, `Weights::auto`, `name_informativeness`; features `child_unique` and `parent_density`.
- `docs/architecture/arc42-amendments.md`: ten evidence-backed changes to apply to the arc42.
- Workspace, pinned toolchain, strict lint policy and `cargo xtask` automation.
- `sov-sim`, `sov-period-controller` (Appendix M), `sov-ledger-port` (Appendix D).
- Exhaustive model checking of both against the recorded TLC state counts, seeded-defect mutation tests, seeded random walks.
- `sov-profiler`: Maximo `MAXRELATIONSHIP` where-clause parser (dictionary-first discovery).
- Statistical relationship discovery (`sov-profiler::discovery`, `eval`), an FK-stripping benchmark on Apache `OFBiz` with a CI quality gate (`cargo xtask bench-fk`), and `docs/research/fk-discovery.md`.
- ADR eng-0008 (inference method and evaluation protocol).
- ADRs eng-0005 (licence format), eng-0006 (pilot SKU and first source), eng-0007 (dictionary-first discovery).
- TLA+ and Lean artefacts under `formal/`, checked in CI.
