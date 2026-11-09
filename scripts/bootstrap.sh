#!/usr/bin/env bash
# Sets up a fresh Debian or Ubuntu machine (for example a brand-new Codespace) so that every command in
# this repository works. Safe to run again: each step is skipped when already done.
#
#   bash scripts/bootstrap.sh              # system packages, Rust toolchain, TLC, then a self-check
#   bash scripts/bootstrap.sh --with-lean  # also install Lean (only needed for `cargo xtask lean`)
#   bash scripts/bootstrap.sh --check      # only report what is installed (no changes)
#
# This is the one place shell is used instead of Rust, because it must run before Rust exists.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

WITH_LEAN=0
CHECK_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --with-lean) WITH_LEAN=1 ;;
    --check) CHECK_ONLY=1 ;;
    -h | --help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg (try --help)" >&2; exit 2 ;;
  esac
done

log() { printf '==> %s\n' "$*"; }
export PATH="$HOME/.cargo/bin:$HOME/.elan/bin:$PATH"

if [ "$CHECK_ONLY" = 1 ]; then
  command -v cargo >/dev/null || { echo "cargo is not installed; run: bash scripts/bootstrap.sh" >&2; exit 1; }
  exec cargo xtask doctor
fi

SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

# 1. System packages (curl, git, python3, Java for the TLA+ checker, tar, a C linker for Rust).
missing=()
command -v curl >/dev/null || missing+=(curl ca-certificates)
command -v git >/dev/null || missing+=(git)
command -v python3 >/dev/null || missing+=(python3)
command -v java >/dev/null || missing+=(default-jre-headless)
command -v tar >/dev/null || missing+=(tar)
command -v cc >/dev/null || missing+=(build-essential)
if [ "${#missing[@]}" -gt 0 ]; then
  if command -v apt-get >/dev/null; then
    log "installing system packages: ${missing[*]}"
    $SUDO apt-get update -y
    $SUDO apt-get install -y --no-install-recommends "${missing[@]}"
  else
    echo "Please install these with your package manager, then run this script again: ${missing[*]}" >&2
    exit 1
  fi
else
  log "system packages already present"
fi

# 2. Rust. rustup reads rust-toolchain.toml, so the exact pinned version is installed.
if command -v rustup >/dev/null; then
  channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
  log "ensuring Rust $channel with rustfmt and clippy"
  rustup toolchain install "$channel" --profile minimal --component rustfmt --component clippy
elif command -v cargo >/dev/null; then
  log "cargo found without rustup; assuming a managed toolchain (cargo xtask doctor will check the version)"
else
  log "installing rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
  export PATH="$HOME/.cargo/bin:$PATH"
  channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
  rustup toolchain install "$channel" --profile minimal --component rustfmt --component clippy
fi

# 3. Lean, only when asked.
if [ "$WITH_LEAN" = 1 ] && ! command -v lean >/dev/null; then
  log "installing Lean (elan) at the version pinned in formal/lean/lean-toolchain"
  curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh |
    sh -s -- -y --default-toolchain "$(cat formal/lean/lean-toolchain)"
fi

# 4. The TLA+ model checker, downloaded and verified against a pinned SHA-256.
log "fetching the pinned TLA+ model checker"
cargo xtask tools

# 5. Self-check.
cargo xtask doctor
log "ready. Next: cargo xtask ci   (then: cargo xtask tlc, cargo xtask dist)"
