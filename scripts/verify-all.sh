#!/usr/bin/env bash
# One command for a reviewer: format, lints as errors, every test, release build. Exit status 0 only if ALL pass.
#   ./scripts/verify-all.sh            (needs Rust; the anchor tests also use `openssl` if present)
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true
cd "$(dirname "$0")/.."
echo "== format";  cargo fmt --all -- --check
echo "== clippy (warnings are errors)"; cargo clippy --workspace --all-targets -- -D warnings
echo "== tests";   cargo test --workspace 2>&1 | tee /tmp/vc-test.log | grep -E "^test result|FAILED|panicked" || true
if grep -qE "FAILED|panicked" /tmp/vc-test.log; then echo "TESTS FAILED"; exit 1; fi
echo "== release build"; cargo build --release -p vc-server -p vc-verify -p mas-data
echo "== dependency advisories"; if command -v cargo-audit >/dev/null; then cargo audit; else echo "(cargo-audit not installed: cargo install cargo-audit --locked)"; fi
echo "ALL CHECKS PASSED"
