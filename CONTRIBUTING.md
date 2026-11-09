# Contributing

## The gate

Nothing merges unless `cargo xtask ci` passes locally and in CI. It runs, in order: toolchain consistency, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo doc` with warnings denied. The formal job (`cargo xtask tlc`, `cargo xtask lean`) must pass too.

## Rules

- **Specification first.** Behaviour in the Truth Plane (ledger, period close, policy) changes in the TLA+/Lean spec and the arc42 appendix in the same PR as the code. A spec change that moves recorded TLC state counts updates `SPECS` in `xtask/src/main.rs` deliberately.
- **No `unsafe`** (forbidden workspace-wide). No `unwrap`, `expect`, `panic!`, `todo!` or `print!` in library code; test and `xtask` code opt out explicitly with a comment saying why.
- **Deterministic kernel.** Kernel crates take a `Clock` and `Rng` from `sov-sim`; they never read the wall clock or OS entropy (ADR 046).
- **Every public item is documented**, with `# Errors` and `# Panics` sections where relevant (`missing_docs` is a lint).
- **No third-party dependency without an ADR** ([ENG-0003](docs/adr/eng-0003-dependency-policy.md)).
- **Decisions are written down** in `docs/adr/` using the MADR template.
- **Tests must be able to fail.** New invariants come with a seeded defect (the `fault-injection` feature, test-only) that the checker catches.
- Small, reviewable commits; imperative subject line; one concern per PR.

## Setting up a fresh machine

`bash scripts/bootstrap.sh` installs everything (system packages, the pinned Rust toolchain, Java for the TLA+ checker, the checker itself) and finishes with `cargo xtask doctor`, which lists anything still missing with the exact fix. Add `--with-lean` to also install Lean.
