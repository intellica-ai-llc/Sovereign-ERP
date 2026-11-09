# Pilot readiness: what stands between today and a licensed binary in a customer's hands

Status as of 2026-10-29. Written after researching how comparable on-premises products license, sign and ship software, what Rust release engineering looks like in 2026, the Trinidad and Tobago data-protection position, and what IBM Maximo allows in database access. Sizes are relative effort (S, M, L, XL), not dates.

## The honest starting point

Status update (0.1.0 checkpoint): there is now a runnable **evaluation tool** (`sov profile|audit|demo|digest`) over a verified foundation, packaged reproducibly by `cargo xtask dist`. It reads exported files only. There is still no live connector, event store, service, web or mobile app, licence enforcement or installer, and it has not run on a real Maximo database. `docs/STATUS.md` is authoritative. Licensing, signing and the portal are comparatively small jobs. The long pole is the product itself (M2).

## Milestones and exit criteria

| # | Milestone | Size | Exit criteria |
|---|---|---|---|
| M0 | Verified foundation | done | Gates green; TLC and Lean re-run independently; repo in order |
| M1 | Pilot scope and decisions | S | Written pilot scope, success criteria and platform matrix; ADRs below accepted; one design partner named |
| M2 | Vertical slice | XL | Read-only connector, schema profiler (incl. FK recovery), semantic model, event store and equivalence replay run against a **real** schema from a design partner; arc42 Definition of Ready met |
| M3 | Shippable binary | L | `sov` CLI and service, config, logging, health, upgrades with migrations, backup and restore, `doctor`; licence verification built in; Linux x86-64 tarball and OCI image |
| M4 | Release pipeline | M | Tag builds in CI only; checksums, SBOM, signatures and provenance produced and verifiable by a customer |
| M5 | Security and legal gate | L | Threat model, independent penetration test, pilot agreement, EULA, data-processing terms, IP review of vendor-schema handling, insurance |
| M6 | Pilot operations | M | Portal live behind authentication, manuals, support process and severity matrix, status channel, kickoff kit; first release to the design partner |

Critical path: M1, M2, M3, then M4 to M6 in parallel. Nothing is shippable before M3; do not promise a pilot date before M2 shows a real schema going through.

## Gaps, by area

### 1. Product (M1, M2, M3)
- **Positioning (eng-0010):** sold first as target-agnostic migration assurance (relationships, orphans, re-derived balances, kernel invariants), valid for any destination; replacement is the later upsell. Competitors and ways to lose: `docs/research/competitive-landscape.md`.
- **Pilot SKU: decided** in [eng-0006](../adr/eng-0006-pilot-sku-and-first-source.md): a Source Profiler and Equivalence Report for IBM Maximo, read-only, fed first from a restored backup or export. Engine order follows the design partner (default Db2, Oracle, SQL Server).
- **FK-stripping benchmark: done on one open-source ERP** (`docs/research/fk-discovery.md`): held-out precision 0.84 and recall 0.86, 0.68 and 0.72 without name evidence, gated in CI. Still open: Maximo-shaped data. There is no public Maximo dictionary or database dump, so the remaining route is a synthetic Maximo-shaped corpus built from IBM's documented conventions (clearly labelled synthetic) and, later, the first design partner's real export.
- **Key finding:** Maximo's database has no foreign keys; relationships live in `MAXRELATIONSHIP`. Discovery is therefore dictionary-first ([eng-0007](../adr/eng-0007-dictionary-first-discovery.md)); the where-clause parser exists in `sov-profiler`, but has not seen a real dictionary yet. A design partner's `MAXRELATIONSHIP`/`MAXATTRIBUTE` export is metadata, not business data, and is the cheapest high-value thing to ask for.
- **Timing:** Maximo 7.6.x extended support was due to end on 30 September 2026 (IBM community post), so migration to MAS is happening now. "Profile your database before you migrate" is a timely pilot message.
- **Mobile:** the offline replication core exists and is verified (`sov-sync`); the pilot delivery path is an installable web app first (eng-0012, proposed, needs a device spike). Without it, a pilot is a desk-only product, which competitors will point out.
- **Missing runtime components:** connector, profiler, event store (PostgreSQL), service and CLI binaries, configuration, structured logging, metrics and health endpoints, schema migrations for the event store, backup and restore, upgrade and rollback.
- **Compatibility matrix:** supported Maximo versions, source databases (DB2, Oracle, SQL Server), OS, PostgreSQL versions, sizing guidance.
- **Synthetic sample dataset** shaped like Maximo, so evaluation and docs never need customer data.
- **IBM constraints found in research:** Maximo SaaS supports read-only database access only through VPN or remote-desktop arrangements the customer must request; IBM-managed Maximo Application Suite states that direct access to production databases is not permitted even read-only, and a reporting replica is required; server-side third-party tools are restricted inside IBM's cloud. Design the connector to work from a **reporting replica, backup restore or export**, not only live production, and say so in the compatibility matrix. Also expect schema changes across Maximo upgrades, which is exactly what the profiler must tolerate.

### 2. Licensing (M1 decision, M3 build)
Comparable on-premises products converge on one pattern, and it suits an air-gapped customer base: **an offline-verifiable, Ed25519-signed licence**.
- Format: a signed token (JWT with EdDSA, or an equivalent compact signed payload) carrying `license_id`, `product`, customer id, tier (`pilot`), `iat`/`nbf`/`exp`, limits (sources, sites, users), enabled features, a payload `version` the binary rejects when unknown, and a `kid` naming the signing key.
- Verification at every start, entirely offline: signature against a **public keyring compiled into the binary**, `kid` lookup, `nbf`/`exp` window with a small clock-skew allowance (comparable products use about 24 hours), `product` claim check. No phone-home, no activation server.
- Behaviour decisions to make (ADR): trial without a key (comparable products allow about 14 days), what happens at expiry (read-only grace, then stop), and whether a licence binds to a deployment id.
- Operator experience: `sov license install|show`, `sov doctor` reporting licence status, warnings starting about 30 days before expiry, a catalogue of `LIC0xx` error codes in the manual. Replacing a licence needs no restart.
- Keys: root key offline on a hardware token; signing key only in the issuance tool, never in any release artifact; several `kid`s so keys can rotate; embedded keyring with a checksum.
- Issuance: a private tool and an audit log of every licence issued. Revocation for an offline product is **short expiry plus replacement** (pilots: 30 to 90 days).
- **Decided** in [eng-0005](../adr/eng-0005-licence-format-and-verification.md). Verifying Ed25519 needs an audited crypto implementation (`ed25519-dalek`, verify-only). That will be the project's first third-party dependency and turns on `cargo-deny` and `cargo-audit` gates per eng-0003; its minimum Rust version may require a toolchain bump first.
- Realism: a licence file is a contractual and honest-user control. A determined attacker can patch any binary, so the pilot agreement carries the real enforcement.

### 3. Release engineering and supply chain (M4)
Established 2026 practice for Rust releases, all achievable in GitHub Actions:
- Build **only in CI from a tag**, pinned toolchain (already done), `--locked`, deterministic-build guardrails (`SOURCE_DATE_EPOCH`).
- **Checksums** (SHA-256 or SHA-512) published beside every artifact.
- **CycloneDX SBOM** from `cargo cyclonedx`, plus `cargo-auditable` so the dependency graph is embedded in the binary.
- **Keyless signing with Sigstore cosign** via GitHub OIDC, and **SLSA build provenance** via `actions/attest-build-provenance`, both verifiable by the customer. Publish the exact verify commands with each release (the portal page does).
- Harden CI: `permissions: contents: read` by default, elevate only in the release job, a protected `release` environment with required reviewers, third-party actions **pinned to full commit SHAs** (today they are pinned to tags; Dependabot will track them).
- Platforms: Linux x86-64 first (static musl tarball and OCI image). Windows and macOS binaries need paid code-signing and notarisation; defer until a pilot customer requires them.

### 4. Security (M5)
- Written **threat model**. The product holds database credentials and reads an organisation's most sensitive records, so it is a high-value target.
- Guidance and tooling for a **dedicated read-only database role**; credentials never in logs; secrets from a file or secret store, not the command line; encryption in transit and at rest; audit log of every read the product performs.
- **Independent penetration test** and fuzzing of every parser and connector input before any customer data is touched.
- Coordinated vulnerability disclosure: `SECURITY.md` exists; add a monitored contact and timelines before the first pilot.
- Expect a **security questionnaire** from every enterprise pilot. Prepare standard answers now. SOC 2 or ISO 27001 is a later certification; pilots usually proceed on questionnaire plus contract.

### 5. Legal and commercial (M5)
- **Pilot agreement:** scope, term, success criteria, no production use, confidentiality, data handling, IP and feedback, liability cap, termination, return or deletion of data.
- **EULA** for the binary; **licence decision** (arc42 ADR 032 is open; currently all rights reserved).
- **Data-processing terms.** Trinidad and Tobago's Data Protection Act 2011 has been only partially proclaimed (sections in 2012 and 2021) and is under review, so do not rely on its enforcement gaps: put processing terms in the contract, and expect customers subject to GDPR or similar to demand them. Have local counsel confirm the current position before signing.
- **IP review** by counsel: ingesting and replicating schemas of Oracle, Microsoft and IBM products, and any "replace" marketing claims, against those vendors' licence terms.
- Insurance (professional liability, cyber) before handling customer data.
- Entity and trademark checks on "Sovereign ERP".
- Pricing and go-to-market stay parked, as you decided; pilots can be free or fixed-fee under the agreement.

### 6. Documentation (M6)
Structure the manual by the four Diátaxis types so each page has one job:
- **Tutorial:** first ingestion on the synthetic sample database.
- **How-to:** create the read-only role, connect a source, install or renew a licence, upgrade, back up and restore, collect a diagnostics bundle.
- **Reference:** CLI, configuration, API, error and `LIC` codes, event catalogue, compatibility matrix.
- **Explanation:** architecture, data flow, what is read and stored, security model, how equivalence is proven.
Plus release notes, known issues, runbooks. Today only the arc42 exists, and it is not a customer manual.

### 7. Operations and support (M6)
- Support channel (shared mailbox then ticketing), **severity matrix** with response targets stated in the pilot agreement (the portal shows placeholders until you set them), escalation path, business hours across customer time zones.
- Status and release-announcement channel; incident process and post-incident notes.
- **Telemetry-free diagnostics:** `sov doctor` and a redacted support bundle the customer can inspect and send. Default no outbound calls, which is what air-gapped customers need.
- Upgrade, rollback, backup and restore procedures **tested** before they are documented.

### 8. Customer portal (M6)
The page in `site/` is a static front end. It cannot gate access by itself. For pilots:
- Host it **behind authentication** (SSO or per-customer accounts) and serve binaries via short-lived signed URLs from private storage; never public links to pilot builds.
- Per-customer licence download, access logging, and the release feed (`releases.json`) written by the release pipeline.
- Start simple (reverse proxy auth plus private object storage); a custom portal service is a later decision with its own ADR.

### 9. Repository hygiene still open
Branch protection and required checks on `main`; CODEOWNERS (needs your GitHub usernames); `cargo-deny`, `cargo-audit`, coverage, `cargo-mutants`, fuzzing, Miri (candidates for ADR eng-0006); toolchain bump from 1.75.0 as its own change; split arc42 section 9 into per-file ADRs.

## ADRs to write next
eng-0008 deeper quality gates; eng-0009 release pipeline and artifact formats; eng-0010 portal hosting and access control; eng-0011 telemetry and diagnostics policy. (eng-0005 licence, eng-0006 pilot SKU and eng-0007 dictionary-first discovery are done.)

## Sources consulted
- Offline Ed25519 licence designs: FalkorDB Enterprise licensing, Devin air-gapped licence file, ChorusControl enterprise licensing (public documentation of each).
- Rust release practice 2026: public release workflows using cargo-cyclonedx, cargo-auditable, cosign keyless signing and `actions/attest-build-provenance`; a 2026 guide on reproducible builds, SBOM and signed artifacts for Rust.
- Trinidad and Tobago Data Protection Act 2011: UWI data-protection resources, DataGuidance, IAPP (2023) report on the Ministry's operationalisation work, press reports of the Act's review.
- IBM Maximo database access: IBM Maximo SaaS direct-database-access and limitations pages, IBM Community integration-approaches discussion, a Maximo Application Suite managed-services migration guide.
Not legal advice; counsel must confirm every legal point above.
