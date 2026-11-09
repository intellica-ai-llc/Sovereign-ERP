# Status: what is built, what is verified, what is not

Version 0.1.0, pre-production evaluation build. This page is the single honest statement of maturity. If another document sounds more confident, this one wins.

**Legend.** *Model-checked*: a formal model plus exhaustive or bounded checking. *Tested*: automated tests, no formal model. *Measured*: scored against ground truth. *Designed*: written down, no code. *Not built*: neither.

| Component | State | Evidence and limits |
|---|---|---|
| `sov` tool: `profile`, `audit`, `demo`, `digest` | Tested | 8 integration tests; deterministic output; never modifies input. Reads tab-separated exports from disk; everything in memory; default cap 200,000 rows per table. **Never run on a real Maximo export.** |
| Relationship inference | Measured | OFBiz held-out F1 0.84; four-database leave-one-out mean 0.86 (0.77 to 0.98); opaque names about 0.70. Small corpora; one ERP |
| Maximo dictionary parser | Tested | Generated and adversarial tests; only documented clause forms. **No real dictionary has been seen** |
| Reconciler (direction, cardinality, orphans) | Tested | Synthetic Maximo-shaped database, 4 seeds. Inference misses role-named references and links into tiny tables |
| Balance audit | Tested | Exact fixed-point; synthetic data |
| Period Controller, Ledger Port | Model-checked | TLC counts match recorded runs; Lean proofs compile; small bounds; in-memory reference only |
| Offline sync core (`sov-sync`) | Model-checked | Lattice laws on every reachable state; TLA+ model; simulation. **Never run on a phone**; whole-document exchange; one aggregate |
| SHA-256 (`sov-evidence`) | Tested | FIPS vectors, million-`a`, Python `hashlib` vectors at every block boundary, every streaming split. Not constant-time; for public data only |
| Release packaging (`cargo xtask dist`) | Tested | Reproducible tarball, SHA-256 checksums, SBOM, smoke test of the packaged binary |
| Signing and build provenance (release workflow) | Designed | Written, **not yet run** (needs a GitHub tag on a private fork) |
| Live database connectors (Db2, Oracle, SQL Server) | Not built | Pilot input is exported files |
| Event store, service and API, web or mobile UI | Not built | Mobile delivery proposed in eng-0012; WebAssembly never built |
| Licence verification, signed evidence reports | Not built | Designed in eng-0005 and `competitive-landscape.md`; needs an audited crypto dependency |
| Agent (LLM) gateway: capabilities, taint tracking, approval, audit | Not built | Research and design in `docs/research/agents-and-erp.md` |
| Authentication, roles, multi-tenancy, observability | Not built | |

## Do not use this build for

Production decisions or data; regulated or personal data without your own review; as a security boundary; or as proof that a migration was correct. It is an evaluation instrument whose output you should check against your own knowledge of the system.

## What moves it to a pilot release

See [`roadmap/pilot-readiness.md`](roadmap/pilot-readiness.md): a real Maximo schema through the tool (the largest unknown), a signed release, licence verification, a security review, and a pilot agreement reviewed by counsel.
