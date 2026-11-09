# eng-0013: Pinned toolchain raised to Rust 1.99.1

- Status: accepted (supersedes the toolchain clause of eng-0002)
- Date: 2026-11-04

## Context
The toolchain was pinned at 1.75.0 because that was the only version the early build environment could verify. That is nearly three years old, blocks audited dependencies whose minimum Rust version is newer (eng-0005), and was never a deliberate choice for the product.

## Decision
Pin **1.99.1** (`rust-toolchain.toml`, with `rust-version = "1.99"`). Before the change, the whole workspace was built, linted with clippy under `-D warnings`, formatted, documented and tested on 1.99.1: all tests passed (106 at the 0.1.0 checkpoint), TLC and Lean runs unchanged. Seven modern lints were fixed at the source; one style lint (`assert_is_empty`) is allowed workspace-wide with a stated reason. `cargo xtask ci` still enforces that the pin and `rust-version` agree. Bumps stay deliberate: a pull request that changes both, fixes any new lints, and passes the gate.

## Consequences
Anyone with rustup (including a fresh Codespace after `scripts/bootstrap.sh`) installs exactly this version. The 1.75 compatibility that existed before is no longer promised.
