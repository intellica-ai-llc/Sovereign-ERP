# Gaps between today and a production-grade FREE pilot (2026-11-03)

Closed in round 31: RFC 3161 anchoring (tested against OpenSSL), tamper-evident admin audit log, /readyz, paper replay driving the claim card from the console, card cannot be proven from paper data (reconciliation required). Closed in round 30: failed-auth throttling (429 after 20 rejected requests per client per minute), systemd unit, env template,
backup script (tested with a fake remote, not a real one), runbook.

OPEN, must do before inviting a pilot user
- Real deployment never exercised on a real VM with real TLS (everything is tested locally and in Codespaces).
- Backup restore drill not done; backup tested only against a fake rclone remote.
- No monitoring beyond a shallow /healthz; nothing alerts if the collector dies.
- Admin access is one shared token: no rotation (change the env var and restart), no per-user accounts, no MFA. Admin actions ARE now logged tamper-evidently.
- Operator signing key lives on the VM disk (mode 0600) and in backups; no HSM or sealed storage.
- Dependency audit: done 2026-11-03 and clean after upgrading p256 (RUSTSEC-2026-0387); repeat before each release. No load test; no external security review.
- Pilot terms, privacy notice, security contact do not exist. Legal/tax check on trading from Trinidad and Tobago still open.

OPEN, product claims we must not overstate
- Time anchoring: RFC 3161 is built and tested against OpenSSL but not yet run against the real FreeTSA from a deployed host; OpenTimestamps is not built. A witness you run yourself is not independent: recruit at least one outside witness before claiming it. Anchors are not yet included in the downloadable pack.
- Passkey approvals are verified server-side only; offline verification by a third party is not built.
- No per-user accounts and no WASM verify page.

OPEN, trading
- The paper replay runs, but a baseline policy on a few days of data proves nothing; the card needs 90+ days, a live venue reconciliation and a kill-switch test to ever read Verified.
- No venue connector, signer or live risk engine (deliberate: the pilot is read-only for markets).
- 90+ days of data and a pre-registered trial are needed before any card can read Verified.
