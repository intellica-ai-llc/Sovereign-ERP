# Formal artefacts

| File | Source | Checked by |
|---|---|---|
| `tla/PeriodClose.tla`, `.cfg` | arc42 Appendix M | `cargo xtask tlc` (TLC 2.19): 18,833 generated, 4,160 distinct |
| `tla/Ledger.tla`, `.cfg` | arc42 Appendix D | `cargo xtask tlc`: 713,185 generated, 21,433 distinct, depth 7 |
| `tla/SyncMerge.tla`, `.cfg` | `crates/sov-sync` (offline work-order replication) | `cargo xtask tlc`: 33,895 generated, 9,018 distinct (3 replicas, 1 local operation each). Also checked once with 2 replicas and 3 operations each: 567,584 distinct states, no error. Three deliberate breaks of the merge are each caught |
| `lean/ClosedPeriod.lean` | arc42 Appendix M | `cargo xtask lean` (Lean `lean-toolchain`) |
| `lean/LedgerConservation.lean` | arc42 Appendix D | `cargo xtask lean` |

Deviations from the arc42 text, all found by actually running the tools:
- `Ledger.cfg` adds `CHECK_DEADLOCK FALSE`. Once every transfer id is finished no action is enabled, which TLC otherwise reports as a deadlock.
- `ClosedPeriod.lean` uses `by simp` where the arc42 used `List.mem_cons_self e t`, whose signature changed in current Lean.

## Running locally

```sh
cargo xtask tools      # downloads and SHA-256-verifies tla2tools.jar into .tools/ (needs curl, Java 11+)
cargo xtask tlc
# Lean: install elan (https://github.com/leanprover/elan), then
cargo xtask lean       # elan picks the version from formal/lean/lean-toolchain
```
