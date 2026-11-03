# VeriChain pilot: what is proven, what is not (for reviewers and investors)

Read this first. Every claim below is either backed by a test you can run or is listed as NOT established.
One command re-checks all of it: `./scripts/verify-all.sh` (format, lints as errors, every test, release build, advisories).

## What the product is today
A multi-tenant evidence log for AI-agent actions, served by one binary (`vc-server`) with a console, plus an offline verifier
(`vcv`) and a research track for trading agents. Pilot scope: invited tenants, no SLA, free-tier hosting.

## Established (test-backed)
| Claim | Evidence |
|---|---|
| Append-only Merkle log, signed checkpoints, inclusion and consistency proofs | RFC 9162 / C2SP tlog formats; `vc-log`, `vc-store` tests |
| A customer can verify a pack offline without trusting the server | `vcv` with pinned trust and policy; `vc-pack` tests |
| Witnesses cosign checkpoints; rollback and equivocation are refused and kept as evidence | `vc-witness`, `vc-witnessd` tests; real-HTTP two-witness test |
| "Approved before it happened" is a verifiable rule (log order, a different approver) | `approval_order` check; `vc-pack` tests |
| Phone passkey approvals, origin- and counter-checked | `vc-server` approvals test; real-browser test with a virtual authenticator |
| Independent timestamp of a checkpoint (RFC 3161) | request bytes equal OpenSSL's; a stored reply verifies with `openssl ts -verify`; wrong digest fails (`tests/anchor.rs`) |
| Operator actions are logged tamper-evidently, no secrets in the log | `tests/audit.rs` |
| Token guessing is throttled per client | `tests/throttle.rs` |
| Tenant isolation, auth before parsing, body limits | `tests/api.rs` |
| Dependencies free of known advisories (197 crates, RustSec database of 1,403 advisories, 2026-11-03) | `cargo audit` clean. It found RUSTSEC-2026-0387 (timing issues in crypto-bigint, reached through p256 0.13); fixed by upgrading p256 to 0.14, passkey tests unchanged and passing. Re-run before every release |
| Trading claim card: every number carries its check; empty or paper ledgers prove nothing | `mas-research::card` tests (empty ledger, each safety failure, hidden-trial deflation, recompute and tamper) |
| Paper engine: intent before order, hard cap, pessimistic fills, independent audit from the event list | `mas-research::paper`, `mas-data::replay` tests |
| Data collector: bounded disk, interruptible, errors reported, hash-chained recordings | `mas-data::runner`, `vc-server/tests/trading.rs` |

## NOT established (do not claim)
- Any trading profit or safety record. No live or paper track record exists. The card is machinery; today it proves nothing.
  A card built from paper data can never read Verified (no venue reconciliation, no live kill-switch test).
- Behaviour of the Polymarket parsers against the live API: not yet run (no network route from the build environment).
- The hosted setup on a real VM with real TLS, load behaviour, restore from backup.
- Independence of witnesses: a witness we run ourselves is not independent. At least one outside witness is needed.
- Per-user accounts, MFA, admin-token rotation, HSM-protected keys, an external security review, SOC 2 or similar.
- Legal and tax position of trading from Trinidad and Tobago; pilot terms and privacy notice.

## Known limits (by design, for the pilot)
Single operator, single region, free-tier hosting that may reclaim idle machines, shared admin token, backups to one provider.

## How a reviewer can check the key claims in 10 minutes
1. `./scripts/verify-all.sh`
2. Run the server (docs/RUN_FROM_DASHBOARD.md); in the console create a tenant, ingest, download a pack, verify it offline with `vcv`.
3. `POST /v1/anchor`, then `openssl ts -verify` the stored reply against the checkpoint digest (docs/PILOT_RUNBOOK.md).
4. Tamper with `admin-audit.jsonl` or a recording and watch verification fail.
