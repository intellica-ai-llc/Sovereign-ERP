# eng-0004: Quality gates and verification in CI

- Status: accepted
- Date: 2026-10-29

## Context
Claims about correctness are only worth what is checked. Formal artefacts that are never run drift: this repository's first independent TLC and Lean run found two defects in specs that had been recorded as checked.

## Decision
CI runs on every push and PR: `cargo xtask ci` (fmt, clippy with warnings denied, tests, docs) and a formal job that runs TLC on every spec in `formal/tla` against its recorded state counts and compiles every Lean file in `formal/lean`. TLC (`tla2tools.jar` 2.19) is downloaded from an immutable release and verified by SHA-256. Lean is pinned in `formal/lean/lean-toolchain`. `docs/verification.md` states what each claim rests on.

## Consequences
CI takes longer (about a minute for TLC). Spec edits that change state counts must update `xtask` and the arc42 appendix together.
