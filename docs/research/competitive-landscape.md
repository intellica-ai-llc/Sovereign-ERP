# Competitive landscape and where to build distance

Status: 2026-11-01. Research round 1, from public pages found by web search. **Evidence labels:** *vendor* means a vendor's own or partner's marketing; *independent* means a neutral source; *inferred* means my reasoning, not a sourced fact. No competitor's product was tested. Market share, pricing and customer counts were not established and are not claimed.

## Who is moving into this space

| Player | What they publicly do | Evidence |
|---|---|---|
| **SAP, Palantir, Accenture, Deloitte** | AI-assisted ERP data migration toward SAP targets: automated discovery of source systems, mapping, rule generation and continuous validation, with pilots in 2026 and wider rollout announced for early 2027. Vendor-reported figures for speed, cost and "verification accuracy" circulate; I could not verify them. | *vendor* (SAP partner page, Deloitte webcasts, press summaries) |
| **Palantir Foundry/AIP generally** | An ontology layer over legacy systems, positioned around data sovereignty and in-place processing. | *vendor* |
| **Datafold and similar data-diff tools** | Cross-database diffing to prove source and target data match: value-level comparison using in-database checksums, aimed mostly at warehouse and analytics migrations. | *vendor* docs |
| **IBM, and Deloitte's Maximo upgrade framework** | IBM's own upgrade path to the Maximo Application Suite (duplicate or back up the 7.6 database, then migrate); Deloitte's framework builds on IBM's process and tools and says it captures and validates Maximo configurations. | *vendor* |
| **Real-time sync products (e.g. Interloc EdgeSync)** | Synchronise 7.6 and the new suite at object level to cut upgrade downtime. | *vendor* (trade article) |
| **Third-party Maximo support firms** | "Stay on 7.6 and pay 50 to 65 percent less" positioning against IBM's support deadlines. | *vendor* marketing; figures unverified |
| **Alternatives (SAP PM/CS, Hexagon, Infor, IFS, challenger CMMS)** | Replacement EAM suites; not researched further in this round. | *inferred* from the Maximo ecosystem pages |

Dates that frame the market (IBM community announcement, *vendor*): Maximo 7.6.x extended support ended on 30 September 2026, and dual-support arrangements end on 30 April 2027.

## What the research says is missing

1. **Nobody found offers target-agnostic, independently checkable proof that business state survived a migration.** Data-diff tools prove rows match. The SAP/Palantir wave validates continuously inside its own platform, toward its own target. Neither re-derives a stored balance or lifecycle state from transaction history and shows the result in a form an outside auditor can re-run. *(Inferred from the absence of such claims in what I found; absence in search results is not proof of absence.)*
2. **The big programmes are shaped by their targets.** SAP's and IBM's tooling migrates *into* SAP and IBM; a customer deciding whether to leave, stay or compare has no neutral instrument. *(Inferred.)*
3. **They are cloud-centred and engagement-heavy** (platform plus large integrators). Air-gapped, sovereign, small-budget and small-state buyers are structurally harder for them to serve. *(Inferred; Palantir's sovereignty messaging is *vendor* positioning and I did not test its on-premises story.)*
4. **Legacy schemas with no declared keys and opaque names** are where generic mapping struggles. Maximo declares no foreign keys; the vendors' public material does not say how they recover relationships. *(Inferred; their methods are not public.)*

## What this repository already has, measured

| Capability | Status | Evidence |
|---|---|---|
| Parse Maximo's own relationship dictionary; report what it cannot read | built | `sov-profiler::maximo`, 12 tests; never run on a real Maximo dictionary |
| Recover undeclared relationships from data | built, measured | held-out F1 0.84 on `OFBiz`; leave-one-corpus-out mean 0.86 (0.77 to 0.98); opaque names about 0.67 to 0.70 |
| Merge both tiers; decide key direction and cardinality from data; count orphan rows | built | `sov-profiler::reconcile`; on the synthetic Maximo-shaped database: 22 of 22 declared relationships oriented correctly, orphan counts exact, no false relationships, across 4 seeds |
| Re-derive stored balances from transactions and report drift | built (first slice) | `sov-profiler::audit`; exact on the synthetic database (3 of 3 injected drifts, 0 false alarms among 400) |
| Formally specified, model-checked period close and ledger kernel | built, small bounds | TLC and Lean re-run independently; 22 plus ledger tests |
| Deterministic, offline, dependency-free Rust core | built | workspace gates |
| Signed, content-addressed evidence report an auditor can re-run | **designed, not built** | needs a hash and signature dependency (eng-0003, eng-0005) |
| Replay of full transaction history through verified rules (beyond a stored balance) | **designed, not built** | arc42 equivalence replay |
| Offline-first field replication with proven convergence and deterministic conflict surfacing | built (core), not on a device | `crates/sov-sync`; TLC and exhaustive state exploration; see `mobile-and-connectivity.md` |
| Customer-runnable binary | **not built** | pilot-readiness roadmap |

Honest limits on the table above: the reconciler and audit were tested only against a synthetic database I generated, so they show the machinery is consistent, not that it works on a customer's data.

## Where to build distance (ranked)

1. **Proof-carrying migration assurance.** Sell the instrument, not the destination: a deterministic evidence report (relationships, orphans, re-derived balances, kernel invariants) that is content-addressed and signed, so an auditor or regulator can re-run it offline. This is credible for *any* target, including IBM's own, which removes the need to displace anyone on day one. Build order: deterministic report structure (done in spirit by `Findings` and `AuditReport`), then hashing and signing behind an ADR.
2. **Business-state equivalence, not data parity.** Extend the audit from stored-balance checks to replaying transactions through the verified ledger and period-close rules. This is where the formal-methods foundation is a real moat rather than a talking point: a competitor cannot add it with a prompt.
3. **Dictionary-first plus inference for schemas that declare no keys.** Already ahead of generic approaches on measured results; deepen with the fixes under "known gaps".
4. **Sovereign by construction.** One static Rust binary, no outbound calls, offline licence verification, deterministic output. Cheap for us, awkward for platform vendors.
5. **Cheaper to adopt.** A profiler plus report is a weeks-long engagement, not a programme. *(Inferred advantage; unmeasured.)*

## Known gaps in our own work, found this round

- **Role-named references** (`REPORTEDBY` to `PERSON.PERSONID`, `CHANGEBY`, `OWNER`): no shared words with the target, so name-based scoring rejects them. Planned fix: a value-pattern feature (child values share the parent's value shape, such as `P0001`), then refit.
- **Different names on each side** (`REFWO` to `WONUM`): same family; same fix.
- **Composite keys are penalised** by weights fitted on corpora where composites are rare relative to Maximo; on the synthetic Maximo-shaped database inference found 8 of 22 declared links against the dictionary's 22 of 22. The dictionary carries Maximo; inference matters for custom links.
- **Three-column keys** are outside inference (dictionary only).
- **Tiny parents** (a 2-row `ORGANIZATION`) are weak evidence by design; those links are found only when the dictionary declares them.
- **Scale:** exact set operations; sketch-based inclusion for million-row tables is designed, not built.

## Mobile and field execution

See `mobile-and-connectivity.md`. In short: competitors race on feature breadth (offline work orders, scanning, photo and voice, AI vision, phone assistants; IBM's 9.2 release ships a mobile assistant and local-inference visual inspection). None found competes on provable offline correctness, sovereign operation or checkable field evidence.

## How we could lose (read this part twice)

- **Bigger players add the same proof.** Palantir with an integrator could ship auditable validation; IBM could bundle verification into its upgrade tooling. Our lead lasts only if we reach a design partner and a signed evidence format first.
- **The formal-methods story is invisible to buyers** unless it becomes a concrete artefact they value (the evidence report an auditor accepts). Without that it is engineering pride.
- **Everything is synthetic or open-source-corpus so far.** One real Maximo dictionary and database would change many numbers, in either direction.
- **Solo execution risk** is the largest risk of all; the plan should minimise what has to be built before a first paid assurance engagement.

## Next research (not done)

- A real Maximo-style dictionary: look for any legitimately available export or documented relationship list beyond the Javadoc.
- Regulators and auditors: what evidence public-sector auditors accept for system migration (this decides the evidence-report format).
- Competitor hands-on: trial or documentation review of Datafold, any Palantir migration material under NDA-free access, and IBM's upgrade tools.
- Sketch-based inclusion; value-pattern feature; three-column composites.
- LLM-assisted adjudication of ambiguous links, constrained so a model can only choose among candidates the data already supports, with the choice recorded as provenance.
