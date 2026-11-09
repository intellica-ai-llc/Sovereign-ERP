# Sovereign-ERP

A next-generation ERP and enterprise asset management (EAM) platform that can replace Oracle, Microsoft and IBM Maximo estates, and that learns an organisation's existing databases on day one instead of requiring a multi-year data migration project.

> **Status: pre-production evaluation build (0.1.0).** What exists today is a verified foundation and a command-line profiler for evaluating on exported data. It is **not** a production ERP. See [`docs/STATUS.md`](docs/STATUS.md) for exactly what is built, verified and not built.

## Try it in five minutes (new, empty Codespace or any Debian/Ubuntu machine)

```sh
bash scripts/bootstrap.sh                       # installs everything needed (Rust, Java, Python, TLA+ checker)
cargo xtask ci                                  # format, lint, tests, docs: all must pass
cargo run --release -p sov-cli -- demo          # guided tour on a synthetic database
cargo run --release -p sov-cli -- demo --export ./sample
cargo run --release -p sov-cli -- profile ./sample
```

Pilot partners receive the compiled `sov` binary instead: see [`docs/EVALUATION-GUIDE.md`](docs/EVALUATION-GUIDE.md).

## What is evidenced

| Claim | Evidence |
|---|---|
| Relationships in a database that declares no keys can be recovered from data | Held-out precision 0.86, recall 0.82 on Apache OFBiz; leave-one-corpus-out mean F1 0.86 over four databases; about 0.70 with opaque names ([research](docs/research/fk-discovery.md)) |
| Maximo's own relationship dictionary can be read, and what cannot be read is reported | Parser with generated and adversarial tests ([`sov-profiler`](crates/sov-profiler)) |
| Business state, not just rows, can be checked | Stored balances re-derived from transactions; exact on a synthetic database |
| Period close and ledger rules are enforced by a small verified core | TLA+ models re-run independently with TLC; Lean proofs compile; exhaustive state exploration |
| Offline field work converges and loses nothing | Lattice laws checked on every reachable state; TLA+ model; 500 simulated unreliable-network histories |
| Reports are reproducible | Same input gives byte-identical output and the same SHA-256 digest |

Every number above has a reproduction command in [`docs/verification.md`](docs/verification.md). Read the limits there too: results on real Maximo databases do not exist yet.

## Layout

```
Cargo.toml            virtual workspace: shared version, lints and dependencies
rust-toolchain.toml   one pinned toolchain for everyone (1.99.1)
crates/
  sov-cli/               the `sov` command-line tool (profile, audit, demo, digest)
  sov-profiler/          dictionary parsing, relationship discovery, reconciliation, balance audit
  sov-period-controller/ period-close state machine (arc42 Appendix M)
  sov-ledger-port/       engine-neutral ledger contract, reference ledger, conformance suite (Appendix D)
  sov-sync/              offline field replication core: verified state-based CRDT (eng-0011)
  sov-evidence/          dependency-free SHA-256 for reproducible reports (eng-0014)
  sov-sim/               deterministic clock and entropy for simulation testing
xtask/                cargo xtask <doctor|ci|tools|tlc|lean|dist|sbom|bench-fk|bench-transfer>
bench/                benchmark converters: OFBiz, SQLite sample databases, name obfuscator
formal/               TLA+ specs and Lean proofs, checked in CI
scripts/bootstrap.sh  one-command setup of a fresh machine
docs/                 architecture, ADRs, research, status, evaluation guide, verification ledger
site/                 static pilot portal front end
```

## Everyday commands

`cargo xtask doctor` checks prerequisites and prints the exact fix for anything missing. `cargo xtask ci` is the gate every change must pass. `cargo xtask dist` builds the release tarball with checksums and a software bill of materials. Contribution rules: [`CONTRIBUTING.md`](CONTRIBUTING.md). Security: [`SECURITY.md`](SECURITY.md) and [`docs/SECURITY-AND-PRIVACY.md`](docs/SECURITY-AND-PRIVACY.md).

All rights reserved; licensing is undecided (arc42 ADR 032). See [`EVALUATION-NOTICE.md`](EVALUATION-NOTICE.md).
