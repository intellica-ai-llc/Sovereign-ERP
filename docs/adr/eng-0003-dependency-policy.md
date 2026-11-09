# eng-0003: Dependency policy

- Status: accepted
- Date: 2026-10-29

## Context
The Truth Plane is meant to be small and formally verified. Every dependency enlarges the trusted base and the supply-chain surface.

## Decision
Kernel crates have **no third-party runtime dependencies**. Any dependency anywhere needs an ADR naming the alternative of not having it. `Cargo.lock` is committed. When the first dependency lands, `cargo-deny` (advisories, licences, sources, bans) and `cargo-audit` become CI gates in the same PR.

## Consequences
Slower to the first feature that wants a crate; much easier to reason about, audit and later certify.
