# eng-0002: Repository layout

- Status: accepted
- Date: 2026-10-29

## Context
The product will grow to dozens of crates. A bad layout is expensive to fix later. Guidance from rust-analyzer's maintainer ("Large Rust Workspaces") and common production practice converge on one shape.

## Decision
- A **virtual workspace** at the repository root; no code at the root.
- All crates **flat under `crates/`**, directory name equal to package name, prefixed `sov-` because Cargo's namespace is flat.
- Version, edition, rust-version, lints and internal dependencies are declared **once** in `[workspace.*]`; crates inherit them.
- **Automation is Rust**: the `xtask` crate (`cargo xtask <task>`), not shell scripts.
- **One pinned toolchain** (`rust-toolchain.toml`) whose minor version must equal `rust-version`; `cargo xtask ci` enforces it. Bumps are their own PR.
- Specifications live apart from code in `formal/`; documents in `docs/`.
- Nothing is published to crates.io (`publish = false`).
- The test-only `fault-injection` feature is enabled through a crate's self dev-dependency, which Cargo never applies to normal or release builds.

## Consequences
Navigation is predictable and tooling (rust-analyzer, CI caching) works without configuration. The toolchain is currently 1.75.0, the version this foundation was verified on; raising it is a deliberate task.
