# Sovereign ERP: Architecture Documentation (arc42)

| | |
|---|---|
| **Version** | 0.8 (Round 7 amendments) |
| **Date** | 2026-10-26 |
| **Owner** | Damain Peter Ramsajan, Intellica AI |
| **Status** | Draft for review. No code until the Definition of Ready (Appendix C) is met |
| **Evidence grades** | **[S]** strong (standard, peer-reviewed or multiple independent sources) · **[M]** moderate (single reputable source or preprint) · **[V]** vendor, press or self-reported · **[H]** our hypothesis, unvalidated |

Previous versions: v0.1 (Claude Doc), v0.2 to v0.7 (Rounds 1 to 6). **This file is self-contained**: sources, ontology summary, kernel, governance and period-close specifications (with checked Lean and TLA+ artefacts), legal-gate matrix, threat model, discovery kit, ingestion and scheduling spikes, counsel brief and Trinidad and Tobago market facts are in Appendices B to N. Requirement IDs use `R`; risk IDs use `RK`.

**Reading note (v0.8).** Under decision D8 the commercial material (Sections 0.4, 0.5 and 0.10, and Appendices I, K and L) is **parked** and kept only as reference. Rounds 7 onward concentrate on the product: depth, innovation and connectivity.

---

## 0. Executive Summary and Strategy

### 0.1 Thesis

A next-generation ERP and enterprise asset management (EAM) platform that can replace Oracle, Microsoft and IBM Maximo estates, built around seven ideas:

1. **Ingestion is the product.** The legacy database is the most complete specification of how a business actually runs. A Semantic Onboarding Engine (SOE) turns a connection string into a reconciled, queryable shadow system.
2. **Schema is not enough: capture behaviour.** Business logic lives in stored procedures, triggers, scripts and customisations. v0.2 adds a Behavioural Specification Extractor and an Equivalence Oracle that replays legacy history through the new rules and proves the results match.
3. **Immutable bitemporal REA event core** with a **formally verified double-entry ledger** derived from it. Money and asset state are appended and corrected, never mutated.
4. **Governed agents operate the business** through typed capabilities checked by a small verified kernel. The LLM proposes, the kernel disposes. Outcomes are provable from the ledger, which makes outcome-based pricing auditable.
5. **Strangler-fig delivery** with a sellable first step (Wave 0 "Legacy X-ray") that earns revenue and builds the mapping corpus before any cutover.
6. **Connect to anything, safely.** A Connectivity Fabric (connector SDK with capability descriptors, contract tests and sandboxing, built on industrial, utility, financial and enterprise standards) means that whatever can be connected to can be built into the system without touching the verified kernel.
7. **A Business Twin.** Because history is immutable and replayable, the system can fork its own event store, perturb it and simulate what would have happened, so maintenance, finance and agent policies are tested on the organisation's own history before they are switched on.

### 0.2 Market Map and White Space

| Segment | Who (examples from Round 1 research) | Their focus | Gap we can exploit |
|---|---|---|---|
| Incumbent suites adding agents | SAP (Autonomous Enterprise, Joule), Oracle (Fusion agentic applications, governance envelope), Microsoft | Embed agents in their own stack; governance as battleground **[V]** | Vendor-locked; SAP's April 2026 API policy reportedly restricts third-party AI agents and bulk extraction outside endorsed routes **[M]**, which pushes customers toward vendor-neutral platforms |
| AI-native finance ERP | Rillet (reported $100M Series C at $1B valuation, Aug 2026; 600+ customers, company-reported), Campfire, DualEntry, Light; Doss (ops-first) | Office of the CFO, high-growth and mid-market, 4 to 6 week implementations **[V]** | Finance-led, mid-market; no evidence found of EAM depth, sovereign or air-gapped deployment, or verified ledgers |
| CMMS / EAM | MaintainX, Limble, UpKeep, Fiix (SMB and mid-market, mobile-first); IBM Maximo (enterprise) | Work management; AI often gated behind premium tiers **[V]** | Enterprise-grade EAM with finance and procurement in one verified, sovereign stack |
| Integrated asset-intensive ERP + EAM | **IFS Cloud** (ERP, EAM, APM and field service on one data model; named a leader in the 2026 IDC MarketScape for AI-enabled asset-intensive EAM; reported 25% ARR growth in H1 2026; agentic platform IFS Loops with a claimed 60% of agentic transactions fully automated; Migration Accelerator Service for SAP customers; partnerships with Siemens and AVEVA; pricing is not published, and third-party estimates put subscriptions at roughly USD 100 to 300 per user per month by user type, mid-market implementation services at roughly USD 100,000 to 400,000 over 6 to 12 months, and complex industrial estates at 1 to 2.5 times first-year software fees in services **[V, estimates, not IFS price lists]**). **Hexagon HxGN EAM** (Infor EAM acquired 2021; APM via Itus Digital 2024) | Asset lifecycle platform with services-led co-creation (Nexus Black) **[V, company-reported]** | Services-led migration. Managed cloud is the default; a customer-hosted Remote option (virtual machine with MicroK8s, documented air-gapped installation) is positioned for defence and data-residency cases and needs customer operations skills; deployment directly on a customer's own Kubernetes cluster was described by an IFS employee in March 2025 as neither announced nor supported **[M, vendor community forum]**. No verified-ledger story found. **IFS is the incumbent to beat in our niche** |
| AI-native ERP modernisation and migration automation | **Tessera Labs** ($60M led by Andreessen Horowitz, announced May 2026; co-founders a young AI researcher and a former SAP executive; reported Fortune 500 customers in manufacturing, technology, retail, consumer goods and utilities; vendor-agnostic multi-agent platform for migration, custom-code analysis, data harmonisation and documentation; claims timelines cut from years to weeks and costs by more than half). **Trope** (YC Summer 2026; two founders; agent-driven, fixed-fee delivery for ERP implementation partners and mid-market ERPs; a data aggregator lists a $125,000 seed). Also Mechanical Orchard, Isoform and Epicor Ascend | Attack the systems-integrator model **[V; claims are company-reported]** | Neither Tessera nor Trope sells a replacement ERP; they automate work around SAP, Oracle, NetSuite and others, and say they are not replacing those systems. They validate buyer demand and raise the bar on governance evidence (traceability, testing evidence, approval gates). They are more likely partners or channels than rivals for now; watch for expansion into EAM |
| ERP-agnostic AI overlays | ChatFin and similar vendors layer agentic finance automation over SAP, Oracle, NetSuite and Workday without replacing them | Give existing ERPs AI features without a migration **[V]** | This is the **overlay objection** to our whole thesis: why replace when you can overlay? See 0.2a |
| Open-source ERP | Odoo (open-core: LGPL community edition plus proprietary Enterprise; General Atlantic's January 2026 follow-on investment reportedly valued it at about EUR 7 billion; thousands of partners), ERPNext/Frappe (fully AGPL; revenue from hosting, support and services); Mordor Intelligence estimates the open-source ERP market at about USD 5.3 billion in 2026 | Breadth, low licence cost, community distribution **[V]** | No EAM or asset-intensive depth, no verified kernel, AI mostly third-party add-ons. Their models show what trust and distribution an open approach can buy (ADR 032) |
| Overlay vendors | Billing and revenue-recognition tools that delay ERP replacement | Fix pain without migrating | Our Wave 0 competes here first, then expands |

**White space (hypothesis [H], revised in v0.3):** v0.2 claimed that no one combines finance, procurement and EAM. That was wrong: IFS Cloud already does, with agentic AI and a migration offer. The defensible white space is narrower: (1) **automated, evidence-backed migration** (ingestion-first, behavioural capture, equivalence replay) versus services-led accelerators and AI-native migration startups that automate migration into someone else's ERP; (2) **sovereign deployment with a small operations footprint**, customer-held keys and local models (IFS also offers customer-hosted deployment, so on-premises capability alone is not a differentiator); (3) a **verified ledger and policy kernel** with proof certificates; (4) **vendor neutrality**, including open access for the customer's own agents; (5) **price and implementation weight** suited to mid-sized utilities and public bodies that IFS, Oracle and SAP price for large enterprises. All five are hypotheses until tested in customer discovery. Rounds 2 and 3 examined IFS and Hexagon through press releases and third-party estimates only; a hands-on teardown (data model, migration tooling, agent governance, deployment options) is still required.

**0.2a The overlay objection (new in v0.7).** If agents can sit on top of any ERP, a buyer may reasonably ask why they should replace one. Our answers, in order of strength: (1) vendors are restricting third-party agent and bulk-extraction access (SAP's reported 2026 API policy is the precedent), which makes overlays fragile on the largest platforms; (2) finance, procurement and asset maintenance on one event model avoids reconciling three overlays; (3) an overlay inherits the host's weaknesses, including customisation debt and licence cost, while we target cost displacement of support and licence renewals (Appendix L shows one Trinidad and Tobago agency paying about TT$1.28 million a year for Oracle support and licences); (4) a verified policy kernel and equivalence evidence give auditors something an overlay cannot; (5) sovereignty and a small operations footprint. The honest limit: customers who are content with their ERP will often choose an overlay, and our Wave 0 deliberately works as an overlay-compatible entry product (read-only, no cutover) so that we are not forced into an all-or-nothing replacement pitch.

### 0.3 Moat Hypotheses (ranked)

| Rank | Moat | Why it compounds | Evidence / caveat |
|---|---|---|---|
| M1 | **Verified Invariant Library**: Lean 4 specifications of accounting, inventory, procurement and asset invariants, plus certified posting rules | Proof generation is getting cheap while specifications stay hard, so curated specifications become the scarce asset | Function-level AI proof synthesis is advancing quickly; repository-scale verified synthesis still falls short on current benchmarks **[M]**. Specs must be audited for vacuity |
| M2 | **Ingestion flywheel**: every onboarding adds verified schema-to-ontology mappings, rule extractions and a private benchmark | Competitors cannot recreate a corpus of real enterprise schemas | Requires contracts that let us retain derived, non-confidential mapping patterns (never customer data) **[H]** |
| M3 | **Sovereign and vendor-neutral**: air-gapped capable, open to customers' own agents through published capability contracts | Turns incumbents' access restrictions into our selling point | SAP precedent **[M]**; Oracle, Microsoft, IBM terms unverified |
| M4 | **Federated reliability learning** across tenants with differential privacy | Cross-fleet failure-mode models without sharing raw data | Studies report near-centralised accuracy (one reports 93.7% of centralised) but are small and often simulated **[M]** |
| M5 | **Behavioural capture**: legacy customisations become verified specifications | Removes the biggest switching-cost excuse ("our customisations") | Techniques are emerging; accuracy on ERP customisations unproven **[M]** |
| M6 | **Replay-derived agent evaluation corpus**: every onboarding yields a private, ground-truth task suite built from the customer's own history (what people actually posted, approved and scheduled) | Lets us measure and raise agent reliability (pass^k, policy adherence) on real ERP work, which public benchmarks do not cover; improves with each customer | Public benchmarks show weak reliability: frontier agents pass under half of a finance-and-accounting workflow benchmark, and reliability decays over repeated runs **[M]**. The corpus advantage itself is **[H]** |
| M7 | **Customisation-pattern catalogue**: a growing library of how Maximo (later other) customisations map to canonical rules, with tests | Turns the most dreaded part of every migration into reusable assets | IBM community migration guidance makes customisation inventory and refactoring central to MAS migration **[M]**. Catalogue value **[H]** |
| M8 | **Labelled failure-mode corpus and curated failure libraries**: confirmed failure-mode labels, asset-class libraries and FMEA templates built through the Data Doctor with expert confirmation | Makes prognostics useful from day one and improves with every onboarding; competitors with unlabelled history cannot match it | Fine-tuned LLM failure-mode classification beat a conventional classifier (F1 0.80 versus 0.60) in one study **[M]**; corpus value **[H]**; only patterns, never customer data, may be retained |
| Not a moat | LLM agents per se, chat UI, generic dashboards | Commoditising quickly | |

### 0.4 Beachheads (low-hanging fruit), ranked

| # | Beachhead | Why now | Fit |
|---|---|---|---|
| B1 | **"Legacy X-ray" (Wave 0, standalone product)**: ingest, semantic layer, natural-language query over verified mappings, process discovery, data-quality and migration-readiness report | Sellable without cutover risk; earns revenue and builds M2; overlay vendors show customers pay to fix pain without migrating **[V]** | Any source system; fastest to demonstrate |
| B1a | **MAS Migration Risk Report** (lead-in product for Maximo estates) | IBM community guidance for moving 7.6.1.x to MAS starts with an inventory of Java customisations (field, action and event classes), automation scripts with launch points, integrations and escalations. MAS runs on OpenShift or Kubernetes, uses AppPoints licensing, moves legacy JMS and SOAP interfaces toward Kafka events and REST, and MAS 9.1 needs Java 17 compatibility; Java customisations are typically converted to automation scripts or reworked **[M]** | Our Behavioural Specification Extractor and X-ray Reporter can produce this inventory and a risk score from a read-only copy. Useful even to customers who pick MAS, and it warms the lead for Maximo Escape **[H]** |
| B2 | **Maximo 7.6.x estates ("Maximo Escape")** | IBM's published dates for the 7.6.1 stream: base support ended 30 Sep 2025; extended support to 30 Sep 2026; sustained support to 31 Dec 2026; dual-support under the MAS licence ends 30 Apr 2027 **[S]**. Another IBM lifecycle page lists extension availability to 30 Sep 2030, so entitlement must be verified per customer. A third-party support vendor claims about a 30% extended-support surcharge **[V, biased]**. Technically favourable: Maximo keeps its own data dictionary in the database (MAXOBJECT, MAXTABLE, MAXVIEW, MAXATTRIBUTE, MAXVIEWCOLUMN, MAXRELATIONSHIP) **[S]**; automation scripts (JavaScript and Jython) are stored with their configuration in the Maximo database and library scripts in the AUTOSCRIPT table, and security lives in tables such as MAXGROUP, SITEAUTH, APPLICATIONAUTH, MAXAPPS and SIGOPTION, with row and attribute restrictions defined per security group **[S]**; read-only SQL for reporting and analytics is common practice and IBM documents read-only database access for Maximo SaaS over VPN **[V]**; direct SQL writes are widely warned against and can affect IBM support **[M]** | Our EAM module must be real before we can replace; Wave 0 and a read-only Maximo companion can start earlier. Customisation surface to capture (S11): custom Java classes, automation scripts (Jython or JavaScript), actions, escalations, cron tasks, database-configuration formulas, integration framework. Java classes live outside the database |
| B3 | **Public sector in Trinidad and Tobago and the wider Caribbean** | IDB-supported digital transformation and procurement programmes are under way; IDB documents cite public procurement at about 12% of GDP and more than 100 ministries, departments and agencies **[M]**. Founder track record with government systems | Procurement, public financial management, asset registers; sovereign story fits. Procurement via multilateral rules. Data-protection context: the Data Protection Act 2011 has been only partially in force since January 2012 and secondary sources report no general data-localisation requirement **[M]**, so sovereignty is a procurement and trust preference today rather than a legal mandate; design to the Act's principles in case full proclamation arrives. **[H]** until validated with ministry contacts |
| B3a | **Trinidad and Tobago state-controlled enterprises, statutory bodies and utilities (refines B3)** | The Ministry of Finance's published contracts list shows the central-government IFMIS implementation already awarded to FreeBalance (US$2,000,399.63, 25 August 2025), with further FreeBalance sustainability services listed earlier **[S]**, so core central-government finance is **not** an open target. The same list shows individual agencies paying large annual vendor renewals, for example Oracle software support and licences for the Customs and Excise Division at $1,275,519.23 (contract dated 1 September 2025; listed without a currency marker, while US dollar amounts are marked, so presumably TT dollars) **[S]**. The procurement Act has been fully in force since 26 April 2023 and defines state-controlled enterprises as public bodies **[S]**; an amendment bill was tabled in September 2026 **[S]** | Targets: asset and maintenance management, procurement support and operational systems at state enterprises, statutory bodies and utilities; cost-displacement offers against support renewals; public contract award lists are a free lead source (Appendix I.2). Local presence is also needed for grant programmes. **[H]** until validated in conversations |
| B4 | Asset-intensive utilities and process industries (regional) | Maximo, Oracle and SAP estates plus ageing assets | EAM plus prognostics; longer sales cycle |
| B5 | **Dynamics GP stragglers (finance and distribution)** | Microsoft stopped selling new GP licences from 1 Apr 2026; product support and updates end 31 Dec 2029; security patches to 30 Apr 2031 **[V, several partner sources agree]**. GP runs on SQL Server, so the customer's own database is directly readable | Finance-centred mid-market overlaps AI-native finance vendors; lower priority for an EAM-led strategy; useful for validating the finance core **[H]** |
| B6 (not a beachhead) | **Oracle EBS and JD Edwards** | Oracle states Premier Support for EBS 12.2 through at least 2036 and JD Edwards EnterpriseOne 9.2 through at least 2034 **[V, Oracle announcements relayed by several sources]**, so there is **no near-term forcing event**. EBS 12.1.3 customers have been outside Premier Support since Dec 2021 | Treat as opportunistic and as a source profile for Wave 0; do not lead with it **[H]** |

### 0.5 Business Model and Profitability Levers

| Lever | Design | Notes |
|---|---|---|
| Productised onboarding | Fixed-price readiness assessment and onboarding; margin comes from automation (deterministic baseline first, LLM second) | Implementation services are the largest cost line in ERP replacement; automating them is our margin story **[H]** |
| Platform subscription | Per legal entity and per managed asset band plus module flags; not per seat | Buyers reject per-seat for core software but want predictable pricing for AI (Futurum 2H 2026 survey, reported) **[V]** |
| Agent consumption | Predictable credit bundles; optional outcome-linked pricing (per matched invoice, per auto-closed work order) with caps and audit | Our verified event log makes outcomes provable, which addresses principal-agent disputes that critics raise about outcome pricing **[H]** |
| Sovereign premium | Air-gapped builds, local model packs, signed update bundles, in-country support | Higher price, lower inference cost (local models) |
| Assurance reports | Auditor-ready evidence packs and proof certificates | New revenue line tied to M1 **[H]** |
| Partner marketplace | Jurisdiction and industry rule packs with revenue share | Controls localisation cost |

**Unit-economics instruments to build before pricing is fixed:** onboarding hours per 1,000 tables; mapping auto-accept rate; first-pass reconciliation tie-out rate; LLM cost per onboarding; gross margin per SKU; sales-cycle length by beachhead. No target numbers are asserted in this version.

**Price anchor (new in v0.4).** The incumbent in our niche does not publish prices, and third-party estimates for IFS Cloud are per-user subscriptions plus implementation services that commonly equal a material multiple of first-year software fees **[V, estimates]**. Our subscription is by entity and asset band, not per user, and our claimed advantage is a lower services share through automated onboarding. Both statements are hypotheses until tested in pilots and quotes; no price is proposed in this version.

**Services-as-software evidence (new in v0.5).** McKinsey describes one-off implementation costs as usually a multiple of the subscription cost **[M]**, and well-funded startups (Tessera Labs, Trope) are betting that agent-driven, fixed-fee delivery can replace large integrator teams **[V]**. Buyers will therefore challenge proposals that rely on big teams without automation evidence. Our margin story is fixed-fee onboarding where the work is done by the Semantic Onboarding Engine and the Equivalence Oracle, with human review limited to flagged items. As a reference point for a services-heavy business, Rimini Street (third-party enterprise software support) reported a 60.4% gross margin for fiscal 2025 **[V, company-reported]**; no target margin is asserted for us until the Section 0.5 instruments have data.

**Observed price points (new in v0.7; none is an IFS ERP price list).**
- IFS Applications 9 licences and services for a small African utility were announced at about US$1.2 million (IFS press release, undated, older product) **[V]**.
- Portsmouth Water's renewal of IFS asset investment planning software was awarded for £657,600 including VAT (official UK notice, 11 December 2025) **[S]**; this is a planning product, not an ERP.
- In Trinidad and Tobago, Oracle software support and licences for one customs division cost $1,275,519.23 a year (1 September 2025), and the central IFMIS implementation was awarded at US$2,000,399.63 **[S]**.
Takeaway: public and utility buyers in small markets pay in the high hundreds of thousands to low millions of US dollars per programme; a credible challenger offer sits well below that. This is an order-of-magnitude anchor, not a price.

**Bootstrapped cash plan (new in v0.7).** The company has no outside funding, so the first revenue has to pay for the learning. Phases are relative, not dated forecasts:
- *Phase A (discovery and paid pilots):* run the Appendix I conversations; sell fixed-fee Legacy X-ray pilots with a written success criterion and a price floor that covers review hours and compute. Pilots fund founder time and cloud costs; they also produce anonymised mapping patterns for the corpus (with permission).
- *Phase B (productise Wave 0):* turn repeated pilot work into the product; keep a short list of source profiles (Maximo first).
- *Phase C (Maximo Escape and public-sector expansion):* only after pilots show willingness to pay for the report and access to a real schema.
- *Cash arithmetic to fill in after discovery:* monthly cash need = living costs + cloud and inference + legal and accounting + tools; pilot contribution = fee − (review hours × loaded hourly cost) − compute; pilots needed per month = cash need ÷ contribution. No values are asserted here.
- *Non-dilutive routes to check (eligibility unverified):* Trinidad and Tobago's Innovation Challenge Facility (EU, IDB Lab and the Government; earlier calls offered grants from EUR 75,000 to registered local SMEs, and a second call funded 19 of 67 applicants from a US$7.5 million pool) **[M]**; the IDB TechLab launched in Trinidad and Tobago in January 2025 **[M]**; an IDB Lab loan call for early-growth Caribbean firms at US$500,000 to US$2 million reported in May 2026, with Trinidad and Tobago eligibility unconfirmed **[M]**; startup cloud-credit programmes (a Trinidad and Tobago firm received up to US$120,000 of Microsoft support and credits in 2021; current terms unknown) **[M]**.

**Pricing experiments (new in v0.7).** (1) *Pilot ladder:* offer three fixed-fee X-ray sizes by number of tables and systems, and record which the buyer picks. (2) *Price sensitivity:* ask each discovery respondent the four Van Westendorp questions (too cheap, cheap, expensive, too expensive) for the report and for an annual subscription. (3) *Gabor-Granger:* present two or three price points for the subscription and record acceptance. (4) *Design-partner terms:* discount in exchange for a reference, a case study and anonymised pattern rights; compare acceptance with and without the discount. (5) *Packaging:* test a per-entity plus asset-band subscription against a per-site one. Record every response in the discovery table; publish no price until at least ten responses exist.

### 0.6 Scope Honesty

Oracle, Microsoft and IBM Maximo are different categories. "Completely replace" is a multi-wave programme (Section 4). Round 1 added a commercial view and found that timing matters: the Maximo 7.6.1 window is already narrowing, so B1 and a read-only Maximo companion are the realistic first revenue.

### 0.7 Founder Decisions (recorded 2026-10-24)

| # | Decision | Consequence |
|---|---|---|
| D1 | The earlier Verity ledger is **not reused**; everything the ERP needs is built fresh in this programme | ADR 002's alternative is a new Rust ledger kernel built from the Appendix D specification; no hidden dependency on earlier platforms |
| D2 | ASL is the only existing asset that may be reused, **only if necessary** | ADR 016 resolved as "optional, adopt only if it wins a spike"; default stack is mainstream |
| D3 | The recommended wedge is accepted: Wave 0 first, **Maximo (B2) as the first replacement target**, Caribbean public sector (B3) as a parallel discovery track | Interpretation of "yes to all recommendations"; revise if it is wrong |
| D4 | ADR 002, 003, 019 and 021 accepted, gated by spikes | Spikes are in Appendix C |
| D5 | Until GitHub push works, the deliverable is this single arc42 file | No zips of repository files |
| D6 | Round 5 scope: unblock stalled items, add mobile and frontier research, defer the Tessera Labs and Trope teardown and the economics and pilot work to Round 6 (recorded 2026-10-25) | Plan in Appendix C updated |
| D7 | Round 6 (economics and go-to-market, competitor teardown, Trinidad and Tobago market facts) proceeds as a single-file deliverable while the GitHub connector is being set up (recorded 2026-10-26) | Connector guidance was given in chat; repository upload stays manual until it works |
| D8 | **Product-first**: forget pricing and go-to-market for now; put research and design effort into making the best possible product (recorded 2026-10-26) | Commercial sections are parked; Definition of Ready item on pilots is suspended |
| D9 | **"Whatever we can connect to we can build"**: connectivity is a first-class architectural capability | Connectivity Fabric (Section 5.2, 8.16, ADR 038); standards alignment (AAS, OPC UA, MQTT Sparkplug B, IEC CIM, ISO 20022 expected) |

### 0.8 Unblocking Log (Round 5)

| Blocker | What was tried | Status |
|---|---|---|
| Lean 4 toolchain unreachable (official download host blocked in the research sandbox) | Downloaded a Lean 4.15.0 release archive directly from GitHub releases, unpacked it and ran the compiler | **Resolved.** Proofs in Appendix D compile (core library, no Mathlib); a deliberately false theorem is rejected |
| No real Maximo or ERP schema to test on | (1) Built a synthetic mini ERP and EAM database with cryptic names and no declared foreign keys and ran a first ingestion spike (Appendix J). (2) Identified open-source ERP and maintenance systems with declared schemas (openMAINT, Odoo, ERPNext, Apache OFBiz, Dolibarr, Tryton, LedgerSMB) as a free benchmark corpus via foreign-key stripping | **Partly resolved.** Synthetic evidence only; the open-source benchmark must be built next |
| No counsel available | Wrote a brief ready to hand to a lawyer or a university legal clinic (Appendix K); default connector mode (customer-run Extraction Pack) avoids needing an answer before the first pilot | **Partly resolved** |
| No customer conversations yet | Added outreach templates and a target list to the discovery kit (Appendix I) | **Ready to execute by the founder** |
| IFS price and data model not available | Searched public procurement notices and press releases (Round 6). Found a few price points but no ERP list price (Section 0.5, Appendix L) | **Partly resolved**; a real quote still needs customers or a prospective-buyer request |
| ASL spike | Requires the ASL repository, which this session cannot see. Spike defined (ADR 016, Appendix C) | Open, needs the founder or a Claude Code session |
| GitHub push | Cannot be done from this chat (no GitHub tool, no credentials). Round 6: gave the founder step-by-step instructions for adding GitHub's MCP server as a custom connector on the free plan (one custom connector allowed; fine-grained token in a request header; no-sign-in mode) | **Open, in the founder's hands**; fallback is GitHub's web upload |
| Tessera Labs and Trope teardown | Public-source review (Round 6): neither replaces ERPs; both automate migration or implementation work around other vendors' systems | **Resolved at public-source depth** (Section 0.2) |
| Trinidad and Tobago public-sector fit unknown | Official Ministry of Finance contract lists, procurement law and tax facts reviewed (Appendix L) | **Resolved enough to refine B3**: central-government core finance is already awarded |

### 0.9 Where We Are Ahead and Where We Are Behind

**Ahead (design choices that legacy vendors cannot adopt quickly because of their dependencies):** an immutable event core with a derived ledger rather than mutable tables; bitemporal reporting by construction; extension through verified rule packs instead of modifying the core (the root cause of customisation debt); agents inside the permission and policy kernel from day one rather than bolted on through APIs that vendors now restrict; ingestion, behavioural capture and equivalence replay as the product's front door; a small, single-binary-style footprint with local models; one event model shared by server, mobile and edge, so offline field work is not a separate product; hybrid post-quantum cryptography from the start.

**Behind (and not fixable by design alone):** functional breadth (statutory, payroll and tax localisation, industry packs), references and auditor acceptance, partner and channel ecosystem, sales capacity, and capital. IFS, Oracle, SAP, Microsoft and well-funded startups have years of coverage and thousands of customers. Our plan treats these gaps as the reason for a narrow wedge (Wave 0, Maximo estates, a few jurisdictions), not as something to hide.

### 0.10 Go-to-Market Plan v0

| Element | Plan |
|---|---|
| Ideal customer profile | Tier 1: organisations on Maximo 7.6.x, especially on-premises, with a customisation burden. Tier 2: Trinidad and Tobago and wider Caribbean state enterprises, statutory bodies and utilities with ageing ERP or asset systems. Tier 3: mid-sized asset-intensive operators in the region |
| Entry offer | Fixed-fee Legacy X-ray pilot: inventory of tables, customisations and integrations; mapping coverage and reconciliation report; process map; MAS migration risk report for Maximo shops; failure-mode coding sample from the Data Doctor; no write access, customer-run extraction by default (Mode M2) |
| Pilot rules | Written success criteria before work starts; NDA and data-handling addendum; price floor covering review hours and compute; scope cap; customer keeps the report whether or not they continue; no free work without a stated purpose (reference, case study, pattern rights) |
| Expansion | Recurring read-only shadow analytics and natural-language query; then Maximo Escape per site; then procurement and finance modules |
| Channels | (1) Maximo consultants and IBM business partners (white-label or referral reports); (2) local systems integrators, who today hold vendor support contracts (an Oracle support renewal in Trinidad and Tobago went to a local reseller) and are both potential channels and competitors; (3) multilateral programmes and grant ecosystems; (4) founder network in government; (5) community presence in Maximo forums |
| Public-sector route | Register as a supplier in the Procurement Depository (Section 26 of the Act, per a public notice); target state-controlled enterprises and statutory bodies; mine the Ministry of Finance's published contracts-awarded lists for support-renewal leads; track the OPR's electronic government procurement initiative; respond to tenders only where the fit is real |
| Reference strategy | Three design partners in the first year; each gets a discount in return for a reference and anonymised pattern rights |
| Competitive framing | Against overlays: Section 0.2a. Against IFS: price, weight and automated evidence. Against Tessera, Trope and similar: partner where they migrate into other systems; compete only if they build an EAM target |
| First-jurisdiction localisation | Trinidad and Tobago rule pack (Appendix L): VAT, payroll deductions, filing outputs |
| What would change the plan | If Maximo shops will not share any schema or extract (H4), shift weight to the public-sector track; if public bodies will only buy through large tenders, shift to partner-led delivery |

### 0.11 Product Innovation Agenda (Round 7)

| # | Innovation | What is new about it | Evidence and status |
|---|---|---|---|
| I1 | **Verified period close** | Close, reopen and adjustment rules specified as a state machine and model-checked; closed-period immutability proved in Lean | Appendix M: TLA+ base passes, six seeded defects caught; Lean lemmas compile **[checked]** |
| I2 | **Reporting classification as a replayable projection** | Presentation categories (for example IFRS 18's operating, investing, financing, income taxes, discontinued operations, and management-defined performance measures) are versioned rules over the event store, so comparatives restate by replay, not by data migration | IFRS 18 applies from 1 January 2027, retrospectively, and advisers warn ERPs need reconfiguring **[S/M]**; design **[H]** |
| I3 | **Rules as code for tax and payroll** | Jurisdiction rules written in a legal domain language with formal semantics and generated tests, instead of ad hoc code | Catala is a default-logic language for law, used by the French tax administration since 2023, with concolic test generation and research on LLM-assisted translation **[M]**; spike needed |
| I4 | **Business Twin** | Fork the event store, perturb it (remove, duplicate, reweight, recombine), simulate and compare with comparative object-centric mining | OCEL 2.0 simulation tools presented at BPM 2026 and van der Aalst's generative event-driven process intelligence **[M]**; faithful object-centric simulation is acknowledged as hard |
| I5 | **Maintenance Decision Engine** | One chain from calibrated RUL to risk, policy, schedule and spare-parts stocking, with joint optimisation rather than separate planners | Joint maintenance and spare-parts optimisation under RUL uncertainty (distributionally robust, EJOR 2026), RL for fleet decisions, review of post-prognostics decision making **[M]**; Appendix N spike **[synthetic]** |
| I6 | **Governed semantic layer** | Natural-language queries compile only from governed metric contracts over the canonical model, never from raw schemas, and carry lineage | Text-to-SQL accuracy falls from about 91% on Spider to 17-21% on Spider 2.0 and near zero on a private-warehouse benchmark; semantic-layer grounding raised accuracy in one vendor-reported benchmark (90.0% to 98.2%, and 84.1% to 100%) **[M/V]** |
| I7 | **Connectivity Fabric** | Standards-first connectors with contract tests, security profiles and sandboxing; one registry for sensors, utilities, finance, EDI, files and messaging | AAS (IDTA), OPC UA, ISO 23247, IEC 61968/61970 CIM **[M]**; MQTT Sparkplug B and ISO 20022 **[H, standards background, to verify]** |
| I8 | **Source-grounded technician copilot** | Extractive-first: safety-critical values are shown from the cited source span, not paraphrased; safe refusal outside approved sources | Industry practice favours RAG with citations and human verification for safety-critical steps **[V]**; design **[H]** |
| I9 | **Deterministic simulation testing by design** | Kernel, sync and agents run on a simulatable runtime from day one | madsim (Apache-2.0, used by RisingWave), turmoil, and published Rust practice; **[M]** |

---

## 1. Introduction and Goals

### 1.1 Requirements Overview

| ID | Requirement | Priority |
|---|---|---|
| R1 | Ingest data directly from source databases (Oracle DB, SQL Server, Db2, PostgreSQL; export APIs for SaaS sources) and reach a reconciled shadow state within days | Must |
| R2 | Financial core: GL, AP, AR, fixed assets, multi-currency, multi-ledger, IFRS-aligned reporting | Must |
| R3 | Procurement, inventory, order management, basic manufacturing and MRP | Must |
| R4 | EAM/CMMS parity with Maximo Manage: asset hierarchy, work management, preventive maintenance, inventory, labour | Must |
| R5 | Condition monitoring and prognostics with calibrated uncertainty, usable on day one without long history | Should |
| R6 | HCM and payroll with pluggable jurisdictional rule packs | Should |
| R7 | Agentic automation with enforceable governance, audit and rollback | Must |
| R8 | Parallel-run and phased cutover tooling (shadow mode, reconciliation, fail-back) | Must |
| R9 | Sovereign deployment: on-premises, government cloud or air-gapped | Must |
| R10 | Open extension model (APIs, MCP, A2A, event streams) including customer-owned agents | Should |
| **R11** | **Behavioural capture**: extract business rules from stored procedures, triggers, scripts and customisations, with traceability and confidence | Must |
| **R12** | **Continuous close**: books reflect transactions as they occur; month-end is a verification step, not a batch | Should |
| **R13** | **Metering and entitlements** supporting subscription, credits and outcome-linked billing with auditable evidence | Must |
| **R14** | Opt-in privacy-preserving cross-tenant learning (federated, differential privacy) | Could |
| **R15** | Standalone Wave 0 "Legacy X-ray" deliverable: semantic layer, query, process discovery, readiness report, no write path | Must |
| **R16** | **Authorisation capture**: ingest the source system's security model (users, groups, site and organisation filters, record-level rules) and map it to the policy engine, because application-level access control is invisible to direct database reads | Must |
| **R17** | **Source profiles**: reusable, versioned profiles per source product (Maximo, Oracle EBS and JD Edwards, Dynamics GP, NAV, AX, BC) that exploit application metadata and known table semantics | Must |
| **R18** | **Connector modes and licence-position check**: each source connects through one of four modes (direct read-only; customer-run extraction pack; vendor-sanctioned export or API; restore into the customer's own licensed environment) after a recorded licence-position check | Must |
| **R19** | **Agent evaluation harness**: replay-derived task suites, pass^k reliability, policy-adherence and do-nothing baselines, deterministic grading against ledger and work-order state | Must |
| **R20** | **Agent identity and delegation**: every agent has a verifiable identity, a named human principal and a recorded delegation chain; delegated scope can only narrow; revocation cascades | Must |
| **R21** | **Untrusted-content handling**: all text from ingested databases, documents and messages is treated as untrusted by agents, with provenance carried on every value | Must |
| **R22** | **Mobile field app**: offline-first execution of work (view, create, complete work orders; record labour and materials; inspections, meter readings, photos, signatures) with barcode, QR and NFC scanning and voice input | Must |
| **R23** | **Mobile security**: passkey sign-in, device attestation, device-bound short-lived tokens, encrypted local store, remote wipe, MDM compatibility | Must |
| **R24** | **Low-bandwidth and intermittent connectivity**: delta sync, resumable uploads, bounded first-sync size, graceful degradation | Must |
| **R25** | **Tiered on-device intelligence** with graceful degradation; mobile AI outputs are proposals under the same governance | Should |
| **R26** | **Data Doctor**: failure-mode coding, hierarchy and duplicate cleaning, and FMEA drafting for legacy maintenance data, with confidence, review and provenance | Must |
| **R27** | **Localisation rule packs as data**: tax, payroll and filing rules per jurisdiction, versioned and verified against kernel invariants; first pack is Trinidad and Tobago (VAT, PAYE, National Insurance, Health Surcharge, filing exports) | Should |
| **R28** | **Pilot instrumentation**: with customer consent, record onboarding hours, mapping auto-accept rate, first-pass reconciliation tie-out and LLM cost per pilot, to feed the unit-economics instruments and pricing experiments | Must |
| **R29** | **Connectivity Fabric**: connector SDK with capability descriptors, typed schemas, contract tests, security profile and sandboxed execution; protocol catalogue covering databases, files, REST and messaging, industrial protocols, utility standards, financial messaging and EDI | Must |
| **R30** | **Industrial and utility standards alignment**: ingest and export Asset Administration Shell submodels; read OPC UA and MQTT (Sparkplug B) data; import and export IEC CIM profiles for utility asset and work data | Should |
| **R31** | **Period close**: close sequence, reconciliation gate, immutability, approved reopen, adjustments posted to an open period with a back-reference (Appendix M); continuous close support | Must |
| **R32** | **Reporting classification projections**: versioned classification rules (IFRS 18 categories and management-defined performance measures) separate from postings; comparatives restated by replay | Must |
| **R33** | **Rules as code**: jurisdiction rules (tax, payroll) in a verifiable domain language with generated tests and source citations | Should |
| **R34** | **Business Twin**: scenario forks of the event store with perturbation operators; results labelled as simulation; no write path back | Should |
| **R35** | **Maintenance Decision Engine**: schedule and spare-parts optimisation from calibrated RUL and constraints (crews, skills, permits, shutdown windows, parts, precedence), with explainable plans | Must |
| **R36** | **Governed semantic layer**: natural-language queries compile only from governed metric contracts; refuse outside the contract; lineage on every answer | Must |
| **R37** | **Source-grounded technician copilot**: answers cite approved sources; safety-critical values shown from source spans; safe refusal | Should |
| **R38** | **Deterministic simulation testing**: kernel, sync and policy code run on a simulatable runtime from day one | Must |

### 1.2 Quality Goals (ranked)

| Rank | Goal | Meaning |
|---|---|---|
| 1 | Correctness and auditability | Money and asset records provably consistent; every change explainable, replayable, attributable |
| 2 | Time-to-value | From connection string to reconciled shadow system in days |
| 3 | Sovereignty and security | Customer controls data, keys, models, code; post-quantum ready |
| 4 | **Onboarding efficiency** | Low human effort and low LLM cost per onboarded schema (directly drives margin) |
| 5 | Evolvability | New modules and jurisdictions without touching the verified kernel |
| 6 | Operability | Low-connectivity sites; small ops team |

### 1.3 Stakeholders

| Role | Expectation |
|---|---|
| CFO / Controller | Parallel close ties out to the cent; auditors get full lineage |
| CIO / Enterprise architect | No lock-in; predictable cost; integration with what exists |
| Asset and maintenance manager | Familiar work-order flow, offline field use, trustworthy predictions |
| Auditors and regulators | Immutable history, point-in-time reconstruction, proof certificates |
| Public-sector procurement and PFM officers | Transparency, local sovereignty, multilateral-funding compliance |
| Incumbent vendors | (Adverse) contractual and technical friction on extraction |
| Integrators and partners | Stable extension points, documented ontology |
| Customer-owned AI agents | Safe, documented capability contracts |

---

## 2. Architecture Constraints

| Type | Constraint |
|---|---|
| Organisational | Small core team; verified kernel must stay small enough to prove and maintain |
| Technical | Rust primary systems language; TLA+ and Lean 4 for formal artefacts; no mandatory hyperscaler dependency |
| Legal and contractual | Extraction rights differ by source and licence. SaaS sources vary: Fusion and Dynamics online typically expose export APIs rather than database access, while IBM documents read-only database access for Maximo SaaS over VPN **[V]**. Incumbent terms may restrict bulk extraction or AI-agent access (SAP precedent). **Counsel review per source before any connector ships.** Round 3 finding: licensing-advisory sources warn that connecting third-party tools directly to an Oracle E-Business Suite database, which ships under restricted-use terms, can void the restricted grant and invite indirect-access claims **[V, advisers with a commercial interest; Oracle's own contract wording not reviewed]**. Hence connector modes (S19) and the Licence Position Check (Appendix E) |
| Regulatory | Data-protection law (in Trinidad and Tobago the Data Protection Act 2011 is only partially in force; design to its principles), financial reporting standards, e-invoicing and payroll tax regimes, data residency, public-procurement rules (including multilateral lender rules) |
| Interoperability | Open standards: OCEL 2.0, XBRL, MCP, A2A, OpenTelemetry, SQL:2011 temporal |
| Ethical and safety | No irreversible financial action by agents without a verified policy permitting it |
| Commercial | Every onboarding must feed the mapping corpus only with derived, non-confidential patterns; never customer data |

---

## 3. Context and Scope

### 3.1 Business Context

| Neighbour | Interaction |
|---|---|
| Legacy ERP and EAM (Oracle EBS, JD Edwards, Fusion; Dynamics NAV, AX, BC, 365; Maximo 7.6.x and MAS) | Source of schema, logic, history and live changes |
| Banks, tax authorities, e-invoicing networks | Payments, statutory filing |
| Trinidad and Tobago portals and regulators (Inland Revenue e-Tax and GENTAX, National Insurance Board, Procurement Depository, electronic government procurement) | Statutory filings, supplier registration, tender notices |
| Suppliers and customers | EDI, portals, REA-based exchange |
| IoT, SCADA, historians (OSIsoft PI, Honeywell, Siemens) | Condition data into the asset graph |
| Identity providers | SSO, SCIM |
| Auditors | Read-only evidence workspace with proof certificates |
| Internal and customer-owned AI agents | Governed capability calls via MCP and A2A |
| Other tenants (opt-in) | Federated model updates only; no raw data |
| Multilateral lenders and procurement authorities | Reporting and compliance for funded programmes |

### 3.2 Technical Context

- **Inbound:** read-only JDBC/ODBC introspection; log-based change data capture; source-code and script export for behavioural capture (PL/SQL, T-SQL, Db2 routines, Maximo automation scripts and custom classes); file and API adapters for SaaS; OPC-UA and MQTT for sensors; EDI and APIs for partners.
- **Outbound:** REST, gRPC, GraphQL; event streams; OCEL 2.0 export; XBRL and regulatory filings; MCP tool server and A2A endpoints; billing and usage events.
- **Trust boundary:** everything inside is under customer-held keys; models run inside by default; external model calls are opt-in per policy.

---

## 4. Solution Strategy

| # | Decision | Rationale | Basis / grade |
|---|---|---|---|
| S1 | Ingestion-first product (SOE) | Legacy database is the fullest spec; fastest value | RIGOR, LLM4VKG, schema-matching literature **[M]** |
| S2 | REA event core with derived double-entry ledger | Rich semantics plus statutory double-entry | McCarthy REA, ISO/IEC 15944-4 **[S]** |
| S3 | Bitemporal immutable storage | Corrections and "as known" reporting | SQL:2011, XTDB v2 **[S]** |
| S4 | Small verified kernel, large conventional shell | Prove what must never be wrong | TLA+, Lean 4, Verus/Kani, TigerBeetle simulation approach **[S/M]** |
| S5 | Governed, neurosymbolic agents | LLMs propose; deterministic kernel decides | Agentic ERP architecture preprint; incumbents' governance push **[M/V]** |
| S6 | Object-centric process intelligence | Infer as-is processes from ingested data | OCEL 2.0, SAP extraction work **[S/M]** |
| S7 | EAM first-class with prognostics | Maximo-class capability | ISO 55000, ISO 14224, PHM literature **[S/M]** |
| S8 | Consistency by domain: strong for money, CRDT for field documents | Offline use without risking balances | Distributed-systems literature **[S]** |
| S9 | Strangler-fig delivery with reconciliation as a product | Reversible slices | Strangler-fig and parallel-close practice **[M]** |
| S10 | Post-quantum, zero-trust, sovereign by default | Long-lived records | NIST FIPS 203/204/205 **[S]** |
| **S11** | **Behavioural capture and Equivalence Oracle** | Schema alone misses logic; replaying history through new rules gives a checkable equivalence | AgentModernize, Reversa, ACL 2026 industry paper on deterministic parsing plus schema-constrained generation with traceability **[M]** |
| **S12** | **Spec-first, AI-proved verification** | Proof cost falling; invest human effort in specifications and audit specs for vacuity | Lean-based agentic provers and benchmarks; Vero shows repository-scale gaps **[M]** |
| **S13** | **Cold-start prognostics with time-series foundation models**, then physics-informed and survival models as history accrues | Predictive value on day one without years of failure data | Chronos-2 embeddings for RUL on industrial data; zero-shot forecasting caveats **[M]** |
| **S14** | **Wave 0 sold standalone** | Revenue, corpus, trust before cutover | Overlay-vendor pattern **[V]** |
| **S15** | **Metering by verifiable evidence** | Outcome-linked pricing needs provable outcomes | Pricing-shift research; our event log **[H]** |
| **S16** | **Dictionary-driven mapping and source profiles** | Many ERPs describe themselves. Maximo stores its object, attribute and relationship dictionary in the database; use it to seed mappings before any model call | IBM data dictionary documentation **[S]** |
| **S17** | **Reuse open ontologies** | REA for the economic side (ISO/IEC 15944-4), IOF Core and Maintenance Reference Ontology (BFO-aligned, OWL 2 DL, MIT-licensed) for assets and maintenance, QUDT for units. We own the bridging axioms and SHACL shapes | IOF releases, Iof-maint paper **[M]** |
| **S18** | **CDC strategy per database, no mandatory GoldenGate** | Oracle: LogMiner-based capture with ARCHIVELOG and supplemental logging, from the primary instance only (not from a physical or logical standby); XStream needs GoldenGate licensing, so it is avoided by default. SQL Server and Db2 use native change capture. Fallback: snapshot plus incremental on timestamp or version columns | Debezium and connector documentation **[M]** |
| **S19** | **Four connector modes with a licence-position check** | Legal posture differs by mode; where licences are restrictive the customer performs the read under its own licence | Licensing-advisory analyses of Oracle restricted-use terms **[V]**; IBM read-only access documentation **[V]** |
| **S20** | **Adopt a purpose-built ledger engine behind a Ledger Port; prove our layer** | Reuse a simulation-tested engine for balances; our proofs target posting rules, the port contract and invariants; keep a fresh Rust ledger kernel, built in this programme from the Appendix D specification, as a drop-in alternative | TigerBeetle documentation, Jepsen analysis (Round 0) **[M]**; model-check results in Appendix D |
| **S21** | **Bounded model checking and mutation tests in CI for the kernel** | A specification that cannot fail proves nothing; every kernel spec ships with mutants that must be caught | Appendix D **[S for the method, M for coverage]** |
| **S22** | **Planner and Reader separation with capability-tagged values** | A planner that sees only trusted instructions writes the plan; a quarantined reader processes untrusted content with no tool access; values carry provenance and permitted-use tags enforced by the policy kernel | CaMeL (Google DeepMind): 77% of AgentDojo tasks solved with provable security versus 84% undefended; limitation: tasks whose next action depends on untrusted content need a human **[M]** |
| **S23** | **Conformal calibration for prognostics** | Distribution-free, finite-sample coverage wrappers around any RUL model; adaptive variants for drift; abstain when calibration data is thin | Conformal RUL intervals (IJPHM 2023); conformal time-series survey (2025) **[M]** |
| **S24** | **Replay-derived evaluation harness with pass^k** | Public benchmarks have known validity problems (a do-nothing agent passed a large share of one benchmark's tasks); our ground truth comes from real history | Benchmark-validity critiques; tau-bench pass^k; Finch finance-and-accounting benchmark **[M]** |
| **S25** | **Command-queue mobile architecture with read-model sync** | The phone produces intents and caches read models; balances and approvals stay server-authoritative; conflicts become explicit tasks | Local-first sync engines (Electric, PowerSync, Zero) in 2026 **[M]** |
| **S26** | **Tiered on-device intelligence** | Deterministic features first; platform on-device models where hardware supports them; a site edge node for no-connectivity sites; no correctness dependency on device models | Apple Foundation Models and Core AI (WWDC 2026), Gemini Nano through AICore and ML Kit GenAI **[M]** |
| **S27** | **Data Doctor in Wave 0** | Legacy maintenance history is dirty; labelling it makes prognostics and reliability reports useful on day one | LLM failure-mode classification, FMEA drafting and verified-versus-generated separation **[M]** |
| **S28** | **Hybrid post-quantum cryptography profile with crypto agility** | Long-lived financial and asset records outlive classical public-key algorithms | NIST FIPS 203, 204, 205; IR 8547 draft timeline; CNSA 2.0 parameters **[S/M]** |
| **S29** | **Open engine, closed corpus (proposed; founder decision pending, ADR 032)** | Open ingestion and source-profile engine for trust and distribution; keep invariant library, mapping and customisation corpora, evaluation corpus and labelled failure data proprietary | Odoo open-core and Frappe fully open models **[V]** |
| **S30** | **Connectivity Fabric with a connector SDK** | Make connection a capability, not a project; each connector is a tested, sandboxed, described component | AAS, OPC UA, CIM standards; supply-chain lessons from agent tooling **[M]** |
| **S31** | **Period-close kernel specified and model-checked** | Close, reopen and adjustment are where audit trails break in real ERPs | Appendix M **[checked, bounded]** |
| **S32** | **Classification projections for reporting standards** | Presentation rules change (IFRS 18 in 2027); events do not | IFRS 18 effective-date and ERP-impact guidance **[S/M]** |
| **S33** | **Rules as code via a legal DSL (evaluate Catala)** | Tax and payroll rules need traceability to law and generated tests | Catala papers and adoption **[M]** |
| **S34** | **Business Twin on event-store forks** | Test policies on history before activation | OCEL 2.0 simulation research **[M]** |
| **S35** | **Maintenance Decision Engine on constraint programming** | Real maintenance planning is a constrained scheduling problem | Appendix N spike; joint optimisation literature **[M, synthetic]** |
| **S36** | **Governed semantic layer for natural-language query** | Raw-schema text-to-SQL is unreliable on enterprise schemas | Spider 2.0, BEAVER, semantic-layer benchmarks **[M/V]** |
| **S37** | **Extractive-first technician copilot** | A confident wrong value in a procedure is a safety incident | Industry guidance on grounded RAG **[V]** |
| **S38** | **Deterministic simulation testing from day one** | Reproducible failures for kernel, sync and agents | madsim, turmoil, S2's experience **[M]** |

**Delivery waves (revised).**
- **Wave 0 "Legacy X-ray":** connect, ingest, map, reconcile, query, process discovery, readiness report, and the Data Doctor (failure-mode coding and cleaning of maintenance history with human review). Read-only. Sellable.
- **Wave 1a "Maximo Escape":** EAM and work management with Maximo-schema connector, behavioural capture of automation scripts and the offline-first mobile field app; shadow then cutover per site.
- **Wave 1b:** procurement, inventory, AP (clean seams, fewer upstream dependencies).
- **Wave 2:** finance core in shadow from Wave 0 ledger replay; parallel close; cutover at a period boundary.
- **Wave 3:** manufacturing, HCM and payroll, projects, industry packs, federated learning.

---

## 5. Building Block View

### 5.1 Level 1: Planes

| Plane | Responsibility | Verification level |
|---|---|---|
| P1 Onboarding | Connectors, introspection, mapping, **behavioural capture**, reconciliation, **equivalence replay**, shadow sync | Reconciler and equivalence checker verified |
| P2 Truth (trusted kernel) | Event store, ledger kernel, posting rules, authorisation, audit log, **metering ledger** | Formally specified and proved or model-checked |
| P3 Domain | Finance, P2P, O2C, inventory, EAM, manufacturing, HCM, projects | Conventional engineering, property tests, deterministic simulation |
| P4 Intelligence | Agent runtime, process mining, prognostics, optimisation, NL query, **federated learning coordinator** | Proposals only; commits go through P2 |
| P5 Experience and Integration | UI, offline mobile, APIs, MCP/A2A gateway, partner portals | Contract-tested |
| P6 Platform | Identity, KMS, observability, deployment, tenancy, **entitlements** | Hardened, minimal |

### 5.2 Level 2: Key Building Blocks

**P1 Semantic Onboarding Engine**
- *Connector Farm:* read-only introspection, log-based CDC (Oracle, SQL Server, Db2, PostgreSQL, MySQL), file and API adapters.
- *Profiler:* statistics, key discovery (including undeclared keys), value-domain clustering, code-table detection.
- *Direct Mapper:* deterministic table-to-class, column-to-property, key-to-relation mapping guaranteeing full coverage before any LLM call.
- ***Dictionary Reader and Source Profiles (new in v0.3):*** reads application metadata where it exists (for Maximo: MAXOBJECT, MAXTABLE, MAXVIEW, MAXATTRIBUTE, MAXVIEWCOLUMN, MAXRELATIONSHIP) and applies a versioned source profile (known tables, status domains, key conventions, custom-attribute conventions) to seed the Direct Mapper before any model call.
- ***Security Model Importer (new):*** imports users, groups, site and organisation filters and record rules from the source, maps them to policies, and flags access that direct SQL would bypass.
- ***Licence Position Check and Extraction Pack (new in v0.4):*** before any direct read, produces a report of the customer's licence terms for the source and the chosen connector mode; the default for restricted-use sources is a signed, read-only Extraction Pack run by the customer's own DBA, which outputs files for ingestion.
- *Semantic Enricher + Judge:* retrieval-augmented LLM proposes mappings to the canonical ontology; independent Judge model re-checks; mappings carry provenance and confidence.
- ***Behavioural Specification Extractor (new):*** deterministic parsers for SQL dialects, routines, triggers and scripts build an enriched intermediate representation (control flow, data flow, call graph); schema-constrained LLM generation turns it into a Business Rule Inventory (rule text, source location, inputs, outputs, confidence, explicit vs implicit); bidirectional traceability to code; gaps preserved for human validation.
- *Behavioural capture scope by source:* for Maximo, IBM documentation confirms that automation scripts (JavaScript and Jython) and their configuration are stored in the database, with library scripts in the AUTOSCRIPT table, so they are extractable by query **[S]**; actions, escalations, cron tasks and database-configuration formulas are expected to be configuration as well and are to be confirmed on a live schema; custom Java classes are deployed outside the database and need build-artifact or file-system access. Sources without code access fall back to black-box equivalence replay.
- *Constraint Oracle:* executes candidate mappings and rules on real data; accepts only if invariants hold (trial balance, sub-ledger to control account, inventory valuation, open items, referential integrity).
- ***Equivalence Oracle (new):*** replays historical source transactions through the new posting rules and workflows and compares outcomes with what the legacy system recorded; divergences are classified (mapping error, rule gap, legacy defect, timing) and drive the fix loop. Acceptance is a statistical and invariant-based criterion per domain, not a proof of total equivalence.
- *Process Discoverer:* OCEL 2.0 extraction to reveal as-is variants.
- *Review Workbench:* human resolution of low-confidence items; decisions feed retrieval context.
- *Shadow Sync:* CDC into P2 once reconciliation is green.
- ***X-ray Reporter (new, Wave 0):*** migration-readiness report: coverage, quality issues, dead tables, custom logic inventory, process maps, estimated effort.
- ***Data Doctor (new in v0.6):*** (1) a failure-mode coder assigns ISO 14224-style failure modes to historical work-order free text from a restricted code list, with confidence and human review; (2) hierarchy and duplicate cleaning; (3) FMEA and maintenance-strategy drafts for expert review, never auto-accepted; (4) outputs stay labelled as model-generated structure until a person confirms them, and only confirmed labels feed prognostics and reliability statistics. Research: a fine-tuned LLM reached F1 0.80 against 0.60 for a conventional text classifier on the same annotated failure-mode data; LLMs generated over half of an FMEA's key content under expert supervision; recent work stresses keeping model-generated structure separate from verified reliability evidence **[M]**.

**P2 Truth Plane**
- *Event Store:* append-only REA events, bitemporal, content-hashed and Merkle-chained.
- *Ledger Kernel:* double-entry accounts and transfers with pending and posted phases, multi-ledger, multi-currency; conservation invariants.
- ***Ledger Port (new in v0.4):*** the only interface to balances (create, pend, post, void, query), implemented first by an adopted ledger engine and conformance-tested against the Appendix D specification. Chart-of-accounts metadata and analytic dimensions (cost centre, project, asset) live in the event store and projections, not in the engine.
- ***Period Controller (new in v0.8):*** enforces the Appendix M state machine: open, closing, closed; sequential close; reconciliation gate; closing entries only while closing; immutability of closed periods; approved reopen only when no later period is closed; adjustments to a closed period posted into an open period with a back-reference.
- *Posting Rules Engine:* declarative event-to-transfer rules, versioned, verified against kernel invariants. Certified rule packs form part of the **Verified Invariant Library**.
- *Policy and Authorisation Engine:* policies as code for humans and agents; same engine, deterministic.
- *Audit and Lineage:* every value traces to source row, mapping, rule, actor.
- ***Metering Ledger (new):*** usage and outcome events (agent actions, matched invoices, closed work orders) recorded as events with evidence pointers; basis for credits and outcome-linked billing.

**P3 Domain Plane** (bounded contexts)
- *Finance:* GL, AP, AR, fixed assets, treasury, consolidation, **continuous close orchestration** on the Period Controller, **reporting classification projections** (R32), and **jurisdiction rule packs written as code** (R33).
- *Supply:* purchasing, inventory, warehouse, supplier portal.
- *Revenue:* quote-to-cash, contracts, billing.
- *EAM:* asset registry and hierarchy (ISO 14224-style taxonomy), locations, meters, work requests and orders, PM, job plans, labour, permits, inspections, spares, failure codes, RCM strategies, **Maximo-schema import profile**.
- *Manufacturing, HCM/Payroll, Projects.*

**P4 Intelligence Plane**
- *Agent Runtime:* role-aligned agents, graph orchestration, typed capabilities, risk tiers.
- ***Plan-Execute Separation (new in v0.5):*** Planner (trusted instructions only) emits a typed plan; Reader (quarantined, no tools) extracts structured values from untrusted content; every value carries a capability tag (origin, readers, permitted sinks); the policy kernel checks each tool call's arguments against those tags before execution.
- ***Agent Identity and Delegation Service (new):*** workload identity for each agent instance, delegated authorisation tokens bound to a human principal, recorded delegation chains with scope narrowing, cascaded revocation, audit linking each action to agent, delegator and human authority.
- ***Evaluation Harness (new):*** builds replay-derived task suites from ingested history, runs agents repeatedly (pass^k), grades against ledger and work-order state, tracks policy adherence, cost and latency, includes do-nothing and random baselines, and gates every agent or model release.
- *Prognostics Service:* layered by data availability. (1) No history: asset-class priors and survival baselines. (2) Short history: time-series foundation-model forecasting of degradation signals and embedding-based RUL heads. (3) Rich history: physics-informed and hybrid models. Uncertainty is calibrated with conformal wrappers (8.13). Foundation models forecast signals, not failure labels, so a failure-mode and threshold layer sits on top.
- *Federated Learning Coordinator (opt-in):* secure aggregation and differential privacy; per-tenant consent; no raw data leaves a tenant.
- ***Maintenance Decision Engine (replaces the generic optimiser):*** calibrated RUL feeds a risk estimate; a policy layer (condition thresholds from cost and criticality) proposes work; a constraint-programming scheduler places work against crews, skills, permits, shutdown windows, parts availability and precedence; a stocking layer sets spare-parts levels from the same uncertainty (joint optimisation, with distributionally robust formulations as the research direction). Plans are explainable (binding constraints and trade-offs) and are proposals under the governance kernel. Spike results: Appendix N. Other planners (MRP, cash forecasting) follow the same pattern.
- *Process Intelligence:* conformance checking on live OCEL streams.
- ***Business Twin (new in v0.8):*** forks the event store at a chosen time, applies perturbation operators (remove, duplicate, reweight, recombine events) or a candidate policy, replays or simulates forward, and compares outcomes with comparative object-centric mining. Results are labelled as simulation and carry the fork's provenance.
- ***Technician Copilot Service (new):*** hybrid retrieval over approved manuals, procedures, drawings and closed work orders; extractive-first answers (the cited passage or crop is shown, and numeric values are copied from verified spans); safe refusal when no approved source supports an answer; offline index on the device or site node.
- ***Governed Semantic Layer and Query Gateway:*** metrics, dimensions and certified join paths are defined once over the canonical model; natural-language questions compile only into queries over those contracts, with lineage and uncertainty on every answer; questions outside the contract are refused or routed to a person.

**P5 Experience and Integration:** web UI, **offline-first mobile field app (Section 8.15)**, APIs, MCP tool server, A2A endpoints, event streams, partner portals, **agent capability catalogue for customer-owned agents**. Components: Mobile Core (shared Rust library for local queue, sync client, validation and crypto), Sync Gateway (partial replication of role- and site-scoped read models), Command API (idempotent intents), Device Trust Service (attestation, passkeys, token binding, remote wipe), Site Edge Node (optional local server with a local model for sites without connectivity).

**Connectivity Fabric (new in v0.8, part of P5).** A connector is a packaged component with: a capability descriptor (what it reads, writes and emits), typed schemas mapped to the canonical ontology, a security profile (credentials, network egress, data classes), contract tests (recorded fixtures plus simulation), a licence-position note (Appendix E style) and a quality report. Connectors run sandboxed with no ambient credentials and declared egress. Protocol catalogue (target): SQL and file sources; REST, GraphQL, gRPC and webhooks; messaging and email; EDI and e-invoicing; payment and bank files (ISO 20022 expected); industrial protocols (OPC UA, MQTT with Sparkplug B, Modbus through gateways); utility data (IEC 61968 and 61970 CIM profiles); digital-twin exchange (Asset Administration Shell submodels); geospatial services; building models; messaging channels such as WhatsApp or SMS subject to platform policy. A connector registry tracks versions, signatures and test results, and the same gate applies to third-party and customer-written connectors.

**P6 Platform:** identity, SCIM, KMS/HSM, secrets, OpenTelemetry, tenancy, deployment automation, **entitlements and metering enforcement**.

---

## 6. Runtime View

### 6.1 Scenario A: Day-Zero Ingestion (Wave 0)
1. Operator supplies read-only credentials; catalogue snapshot stays inside the boundary.
2. Profiler infers keys, domains, code tables; Direct Mapper produces a complete baseline mapping.
3. Enricher and Judge propose semantic mappings; Constraint Oracle scores each against invariants.
4. Behavioural Specification Extractor inventories routines, triggers and scripts into the Business Rule Inventory.
5. Unresolved items go to the Review Workbench; approved mappings are versioned.
6. Historical load into the bitemporal store with original timestamps as valid time; ledger replay.
7. Reconciliation report per period. Only when green does Shadow Sync go live.
8. X-ray Reporter produces the readiness report and process maps.

### 6.2 Scenario B: Agent-Handled Supplier Invoice
As v0.1, plus: the policy engine decision, model version and evidence pack are written to the Metering Ledger so the outcome (matched and posted invoice) can be billed and audited.

### 6.3 Scenario C: Prognostics to Work Order
1. Sensor stream updates condition state; Prognostics Service selects the layer appropriate to available history and updates RUL with uncertainty.
2. If failure probability inside the planning horizon crosses a criticality-scaled threshold, a maintenance proposal is created.
3. Optimiser sequences against crews, permits, spares and production windows.
4. Planner approves or policy auto-approves; work order and reservation events commit; procurement triggers if stock is short.
5. Completion feedback updates asset reliability parameters and, if opted in, contributes a federated update.

### 6.4 Scenario D: Parallel Close and Module Cutover
As v0.1: both systems close the same period; variances classified to zero; controller sign-off; write authority flips at a period boundary; one writer per entity per period; fail-back triggers agreed in advance.

### 6.5 Scenario E: Behavioural Equivalence Replay (new)
1. Select a historical window (for example, 12 closed months).
2. Replay source transactions as REA events through the new posting rules and workflows.
3. Compare generated ledger entries, stock movements and work-order states with the legacy records.
4. Classify every divergence; fix mappings or rules; re-run until unexplained divergence is below the agreed threshold and all invariants hold.
5. Publish an equivalence certificate (scope, window, coverage, residual divergences) for the auditor.

### 6.6 Scenario F: Outcome-Metered Agent Action (new)
1. Agent invokes a typed capability; policy engine classifies risk.
2. Action executes through the kernel; resulting events are linked to the metering event.
3. Billing computes credits or outcome fees only from events with valid evidence; disputes replay the evidence.

---

## 7. Deployment View

| Aspect | Design |
|---|---|
| Topology | Kubernetes or bare metal; 3-node minimum for the Truth Plane; stateless services scale horizontally |
| Sovereign modes | On-premises, government cloud, air-gapped (signed model and update bundles) |
| Wave 0 footprint | Single-node or small-cluster read-only deployment next to the customer's database replica; no write path to source |
| Edge | Site nodes with local-first sync for field operations |
| Model hosting | Local open-weight models by default; optional external models behind a policy-gated egress proxy |
| Tenancy | Cryptographic isolation; optional confidential computing for the Truth Plane |
| Data stores | Event log and ledger kernel (strong consistency); bitemporal relational projection; search and vector index; object store; time-series store |
| Upgrades | Rolling, versioned rules and mappings, deterministic replay to verify an upgrade |

---

## 8. Cross-cutting Concepts

**8.1 Domain model.** Layered, reuse-first ontology (summary and verified external identifiers in Appendix F): REA primitives (Resource, Event, Agent, Commitment) for the economic layer; IOF Core and the IOF Maintenance Reference Ontology (BFO-aligned, OWL 2 DL, MIT-licensed) for assets, maintenance activities and failure concepts; QUDT for units and quantities; our own bridging axioms and SHACL shapes tie them together. Double-entry is a derived view.

**8.2 Time.** Bitemporal everywhere; corrections add versions; reports reproducible "as known at" any date.

**8.3 Consistency.** Ledger and inventory-of-record: linearisable. Field documents: CRDT merge. Workflows: sagas with compensation. Never CRDTs on balances.

**8.4 Verification strategy (spec-first).**
- *Specification:* TLA+ for protocols and state machines; Lean 4 for accounting, posting-rule and policy invariants.
- *Proof:* AI-assisted proof synthesis with kernel-checked results; humans own the specifications.
- *Specification audit:* prove specifications are satisfiable and non-vacuous; review for missing properties. A proved but weak spec is the main residual risk.
- *Implementation:* Rust with Verus or Kani on kernel code; refinement links where tooling allows.
- *System:* deterministic simulation testing with injected faults and controlled clock, disk, network.
- *Scope rule:* the proved surface stays small; everything else relies on tests, reconciliation and runtime monitoring.

**8.5 Agent governance.** Layers, outermost first.
1. *Identity and delegation:* every agent instance has a workload identity and a human principal; delegation can only narrow scope; revocation cascades (Appendix G, G5 and G6). Standards direction: NIST's February 2026 concept paper on software and AI agent identity and authorisation names OAuth 2.x, OpenID Connect, SPIFFE/SPIRE, SCIM, NGAC and MCP as building blocks; no single agent-identity standard was ratified as of mid-2026 and multi-hop delegation is an acknowledged open problem **[M]**.
2. *Capability contracts:* every tool is a typed contract with effect class (read, propose, commit), risk tier, reversibility, budget cost, required approvals, idempotency key and evidence outputs.
3. *Plan-execute separation (S22):* planner and reader split; capability-tagged values; the policy kernel, not the model, decides whether tagged data may flow to a sink.
4. *Policy kernel:* deterministic rules checked before every execution (Appendix G): capability held, budget available, human approval for high-risk or untrusted-derived actions, kill switch honoured.
5. *Human approval with evidence packs:* approvals are specific to one action and show the evidence, the diff and the policy decision; approval rates are monitored to detect fatigue.
6. *Supply chain:* signed tool manifests, allow-listed MCP servers, pinned versions, an inventory of agents, tools and models (AI bill of materials), pinned local model hashes, sandboxed execution with no default egress.
7. *Adaptive red-teaming:* NIST's red-teaming with AgentDojo-Inspect reported novel attack techniques reaching an 81% task-hijacking success rate against 11% for baseline attacks **[M]**; static test sets are not enough, so every release is attacked adaptively and the NIST AI Agent Test Suite (expected late 2026) is adopted when available.
8. *Observability and kill switch:* OpenTelemetry traces across planner, reader, tools and approvals; per-agent and global kill switches; anomaly detection on the metering ledger.
MCP and A2A remain the wire protocols; session-type-style checks on agent choreographies are retained.

**8.6 Security.** Zero trust, relationship-based authorisation, field-level encryption, tamper-evident logs, optional enclaves, secrets never visible to models. **Post-quantum profile (ADR 031):** hybrid key exchange (ML-KEM with X25519) in transport; ML-DSA signatures for events, Extraction Packs and tokens; SLH-DSA for long-lived signing roots and log anchors (conservative hash-based design); FN-DSA only after FIPS 206 is final (it was still an initial public draft in mid-2026, with finalisation expected late 2026 or early 2027, and early Falcon code is not interoperable with it); ML-KEM-1024 and ML-DSA-87 available for customers following CNSA 2.0; algorithm identifiers versioned in every signature and key record; a cryptographic inventory and FIPS 140-3 validated modules where customers require them (FIPS 140-2 validations move to historical status on 21 September 2026). NIST IR 8547 proposes deprecating RSA-2048 and ECC P-256 after 2030 and disallowing quantum-vulnerable public-key algorithms after 2035; it was still a draft in mid-2026 **[M]**.

**8.7 Data quality.** Legacy data assumed dirty; quality rules first-class; fixes are new events with provenance.

**8.8 Explainability.** Every figure drills to events, rules, mappings and actors; agent decisions carry rationale and evidence.

**8.9 Extensibility.** Rule packs, workflows, jurisdiction and industry packs are validated against kernel invariants; none can bypass the policy engine.

**8.10 Observability.** OpenTelemetry across human, agent and system actions; business SLOs (close duration, match rates, mapping coverage, auto-accept rate).

**8.11 Privacy-preserving learning.** Opt-in federated learning with secure aggregation and differential privacy; privacy budget tracked per tenant; no cross-tenant raw data or identifiers.

**8.12 Commercial instrumentation.** Metering events are first-class, signed and replayable; no billing without evidence.

**8.13 Prognostic calibration.** Every prognostic output is an interval with a stated coverage level, produced by a conformal wrapper around the layer-appropriate model (S13, S23). Split-conformal calibration provides finite-sample, distribution-free marginal coverage under exchangeability; time series violate exchangeability, so we use weighted or adaptive variants and calibrate per asset class (Mondrian-style) rather than globally **[M]**. The guarantee is marginal, not per asset, and degrades under drift; the system reports empirical coverage per asset class, abstains when the calibration set is too small, and never presents a bare point estimate. Decision thresholds use interval bounds and the asset's criticality.

**8.14 Agent evaluation.** Release gates use pass^k (all of k repeated runs succeed), policy-adherence score, cost-normalised accuracy and latency (the CLEAR dimensions: cost, latency, efficacy, assurance, reliability **[M]**), always with do-nothing and random baselines, because published benchmarks have shown validity failures. Tasks come from replayed customer history (what was actually posted, approved or scheduled); grading is deterministic against ledger and work-order state, not by a model judge. External benchmarks (tau-bench family, Finch for finance and accounting workflows, WorkArena) are used for orientation only. For orientation, frontier agents passed under half of Finch workflows in its 2026 version, so human approval stays the default for consequential actions.

**8.15 Mobile and field architecture.**
1. *The phone is an untrusted, intermittently connected producer of intents and a cache of read models.*
2. *Reads* come from a local SQLite replica of role- and site-scoped read models kept current by a sync engine over the event store's projections. Candidates in 2026: Electric (Apache 2.0, Postgres logical replication over HTTP; the team joined Neon in August 2026), PowerSync (source-available under FSL-1.1-ALv2, converting to Apache 2.0 after two years; Postgres GA, SQL Server and MySQL in beta; durable upload queue), Zero 1.0 (June 2026; Postgres only; views not synced) **[M]**. The sync layer sits behind an interface and is chosen by spike (ADR 028).
3. *Writes* never sync as rows. The app appends commands (create work request, record labour, count stock, attach photo, complete task) to a durable local queue with idempotency keys. On reconnect the server validates each through the policy kernel and emits events; rejected commands return as explicit conflict tasks.
4. *Conflict policy by entity:* notes, checklists and photos merge (CRDT or last-writer-wins with history); anything touching balances, stock or approvals is server-authoritative.
5. *Security:* passkey sign-in, device attestation, device-bound short-lived tokens, encrypted local store, partial sync with least data, remote wipe, offline credential expiry. Push notifications carry only a "sync now" hint with no sensitive payload, because iOS push goes through Apple's service and Android's default goes through Google's.
6. *Hardware:* camera barcode and QR scanning, NFC tag reading, Bluetooth beacons, OCR and speech. Asset labelling (QR or NFC tags) is part of onboarding.
7. *Tiered intelligence (S26):* T0 deterministic on-device (scan, OCR, speech to text, form logic); T1 platform on-device model where hardware supports it (Apple Foundation Models, with image input and Core AI for third-party local models as of WWDC 2026; Gemini Nano through AICore and ML Kit GenAI; open small models), restricted to supported hardware and fixed feature sets **[M]**, used only to propose; T2 site edge node with a local model; T3 core cluster. No correctness depends on T1, because device fleets are heterogeneous.
8. *Shell:* a cross-platform native-capable shell with a shared Rust core for sync, validation and crypto (ADR 029, to be proven by spike). A progressive web app is retained for administration and light use; iOS web apps are generally limited for NFC and Bluetooth, which field work needs, so this must be re-checked at spike time **[H]**.
9. *Other channels:* web, rugged tablets, voice, and messaging (for example work-request intake over WhatsApp or SMS), subject to platform policy and data-protection review **[H]**.
10. *Budgets measured, not assumed:* first-sync size, delta size, battery drain and cold-start time on mid-range devices (Q28).

**8.16 Connectivity Fabric.** Principles: (1) *standards first, adapters second:* prefer a published information model (AAS submodels, OPC UA companion specifications, CIM profiles) over bespoke mapping; (2) *every connector is a contract:* typed inputs and outputs, declared side effects, idempotency, rate and failure behaviour, tested against recorded fixtures and simulated faults; (3) *least authority:* no ambient credentials, declared network egress, data-class tags, sandboxing; (4) *provenance:* every inbound value carries source, connector version and time, and untrusted text is tagged untrusted for agents (ADR 026); (5) *quality gates:* a connector cannot be promoted without contract tests, a security profile and a licence-position note; (6) *two-way only where safe:* write paths to external systems go through capability contracts with approvals. Research basis: AAS (IDTA) is the standardised digital-twin information model, with an OPC UA and AAS joint working group and ISO 23247 as a manufacturing digital-twin framework; IEC 61968 and 61970 define the CIM for utility asset, work and network data with RDF exchange profiles **[M]**.

**8.17 Period close and reporting classification.** The Period Controller (Appendix M) is the only component that changes period status. Adjustments never edit closed periods; they post to an open period with a back-reference, and reports can show "as originally closed" or "as adjusted". Reporting classification is a separate, versioned projection: each posting line is classified by rules (account, dimension, counterparty, event type) into presentation categories; IFRS 18's five categories, its new subtotals and management-defined performance measures with reconciliations are one classification version, and restating comparatives means replaying the event store through the new version **[S/M for IFRS 18; design H]**. Multi-ledger and multi-GAAP are further classification and measurement projections, not copies of data.

**8.18 Rules as code.** Tax, payroll and statutory rules live in versioned rule packs written in a language with formal semantics where possible (Catala is the leading candidate: default-logic rules mirroring legal text, exact arithmetic for money, concolic test generation, adoption by the French tax administration) **[M]**. Every rule cites its source paragraph and effective dates; test suites are generated and reviewed by a person; rule-pack changes are checked against kernel invariants. A Catala spike must confirm integration with the Rust core (compilation targets, foreign-function boundary) before adoption (ADR 041).

**8.19 Business Twin.** Scenarios are forks of the event store plus perturbation operators or candidate policies. Outputs are labelled simulation and carry the fork point, operators and random seed. Uses: test a maintenance policy before switching it on, estimate the effect of an agent approval threshold, check a reclassification, rehearse a cutover. Limits: faithful object-centric simulation is an open research problem, so the twin is used for comparison and sensitivity, not for forecasts presented as facts.

**8.20 Maintenance Decision Engine.** Layers: (1) calibrated RUL and failure-probability inputs (8.13); (2) risk by asset criticality and consequence; (3) policy: when to act, from cost-benefit with interval bounds; (4) scheduling: constraint programming over crews with skills and shifts, permits, shutdown windows, parts arrival, precedence and travel; (5) stocking: spare-parts levels from the same uncertainty; (6) explanation: binding constraints, trade-offs and alternatives. Evaluation uses the Business Twin on history and held-out periods, with off-policy caveats. Research basis: joint maintenance and spare-parts optimisation under uncertainty, post-prognostics decision-making reviews, and RL for fleet-level maintenance **[M]**.

**8.21 Governed semantic layer.** Natural-language access is a compiler from questions to governed metric queries. It never generates SQL against raw source tables. Each answer includes the metric definitions used, filters, lineage to events and rules, freshness, and a confidence or refusal. Evaluation uses a private question set derived from real reports, graded against certified queries, and tracks wrong-but-confident answers as the primary failure.

**8.22 Technician copilot.** Retrieval over approved documents only; hybrid search and reranking; safety-critical content (torque, clearances, lock-out steps, chemical handling) is shown as the original passage or image crop with page and revision, and generated text is limited to navigation, summarisation and translation; numeric values in answers must match a verified span; below-threshold retrieval produces a refusal and a request to consult the source or a qualified person; offline operation uses an on-device or site-node index. A hallucinated procedure is treated as a safety incident, not a quality defect **[V for industry practice; H for design]**.

**8.23 Deterministic simulation testing.** All kernel, sync, policy and scheduling code runs on a simulatable runtime: no direct wall-clock, entropy, filesystem or network calls; deterministic hash maps; all I/O behind traits with simulated implementations. Candidate tooling: madsim (a Tokio-like runtime with simulation, Apache-2.0, used by RisingWave, with a mode that reruns a test with the same seed to detect nondeterminism) and turmoil, plus lessons from S2's blend of both and the need to control hash-map randomisation **[M]**. Failing seeds are checked in as regression tests. Ledger-engine conformance tests (ADR 002) and Appendix D, G and M properties are the oracles.

---

## 9. Architecture Decisions

| ADR | Decision | Status | Trade-off |
|---|---|---|---|
| 001 | Event-sourced REA core with derived ledger | Proposed | Conceptual shift for accountants vs richer semantics |
| 002 | Ledger kernel: adopt a purpose-built engine behind a Ledger Port (provisional choice: TigerBeetle, Apache 2.0) for Waves 0 to 2; keep a fresh Rust ledger kernel, built in this programme from the Appendix D specification, as the alternative | **Accepted by founder 2026-10-24 (spike-gated)** | Time-to-market and a simulation-tested engine vs a fixed account and transfer schema (unsigned 128-bit amounts, one debit and one credit per transfer, compound entries as linked transfers), single-vendor dependency, and a testing-based rather than proof-based assurance story for the engine itself. Spike: encode the Appendix D invariants as conformance tests against the engine and measure a month-end-close workload |
| 003 | Event store on PostgreSQL with explicit valid-time and recorded-time columns and append-only discipline; XTDB 2.x as an optional temporal projection store | **Accepted by founder 2026-10-24** | Immutable events already carry both times, so a bitemporal database is a query convenience, not a correctness requirement. XTDB 2.x (MPL-2.0, SQL:2011 bitemporal over the Postgres wire protocol, JVM, a 2.2 release candidate adds a Postgres logical-replication source) suits time-travel reporting, but its general-availability status must be verified. PostgreSQL 18 (released 25 September 2025) provides native temporal primary and unique keys (WITHOUT OVERLAPS) and temporal foreign keys (PERIOD) on range types, and the PostgreSQL 19 beta (June 2026) adds UPDATE and DELETE FOR PORTION OF **[M]**, which resolves the earlier uncertainty |
| 004 | Deterministic mapping first, LLM second, oracle always | Accepted | Slower first pass vs guaranteed coverage |
| 005 | Verified kernel limited to Truth Plane | Accepted | Cost control vs guarantee surface |
| 006 | Local models by default | Accepted | Sovereignty vs peak capability |
| 007 | CRDTs only for non-monetary data | Accepted | Offline vs simple invariants |
| 008 | Finance cutover last; ledger shadow from Wave 0 | Proposed | Delayed headline vs de-risked close |
| 009 | Source-by-source legal gate on connectors | Accepted | Slower rollout vs contractual safety |
| **010** | Wave 0 "Legacy X-ray" sold standalone, read-only | Proposed | Early revenue vs distraction from the kernel |
| **011** | Behavioural capture via deterministic parsing plus schema-constrained LLM, with traceability | Proposed | Cost and coverage vs accuracy on opaque customisations |
| **012** | Spec-first, AI-proved verification with specification audit | Proposed | Lower proof cost vs spec-quality risk |
| **013** | Cold-start prognostics via time-series foundation models, layered by history | Proposed | Day-one value vs calibration risk |
| **014** | Subscription by entity and asset band plus credits; outcome-linked fees optional and capped | Proposed | Predictability vs upside |
| **015** | Federated learning opt-in, Wave 3 | Proposed | Moat M4 vs privacy and complexity |
| **016** | ASL (in-house agent language) is **optional**: the default agent specification and runtime stack is mainstream (Rust services, a deterministic policy kernel, MCP and A2A); ASL is adopted for a layer only if it wins a time-boxed spike | **Decided (founder D2); spike pending** | Spike: implement the Appendix G policy kernel and one capability contract in ASL and in plain Rust; compare against the criteria: graded effects for risk tiers, Uncertain<T> soundness for model outputs, MCP and A2A bindings, typed CRDT merge algebra for offline documents, footprint in sovereign deployments, linkability of formal semantics to Lean 4 and TLA+, conformance-suite maturity, hiring risk |
| **017** | Reuse open ontologies (REA, IOF Core, IOF Maintenance, QUDT) with our own bridging axioms | Proposed | Interoperability and credibility vs alignment overhead |
| **018** | Dictionary-driven mapping with versioned source profiles | Proposed | Faster, safer ingestion vs per-product profile maintenance |
| **019** | Oracle CDC via LogMiner from the primary; no mandatory GoldenGate | Accepted by founder 2026-10-24 | Licensing and simplicity vs primary-instance prerequisites (a vendor blog estimates 1 to 5% redo overhead **[V]**) |
| **020** | Import the source authorisation model and re-enforce it in our policy engine | Proposed | Safety vs mapping effort |
| **021** | Connector modes M1 to M4 with a recorded licence-position check before any direct read | Accepted by founder 2026-10-24 | Legal safety and customer trust vs slower first connection |
| **022** | Planner and Reader separation with capability-tagged values for all agent work touching untrusted content | Proposed | Provable flow control vs some tasks needing human approval and added latency |
| **023** | Agent identity: workload identity (SPIFFE-style) plus delegated authorisation tokens bound to a human principal, with recorded delegation chains | Proposed | Alignment with emerging standards vs unsettled multi-hop delegation semantics |
| **024** | Conformal prediction wrappers for all prognostic intervals | Proposed | Coverage guarantees vs conservative intervals and exchangeability assumptions |
| **025** | Replay-derived evaluation harness with pass^k as the release gate | Proposed | Realistic ground truth vs effort to build and maintain task suites |
| **026** | All ingested text is untrusted by default for agents | Accepted | Safety vs convenience of letting agents read free-text fields directly |
| **027** | Mobile architecture: command queue for writes, read-model sync for reads, server-authoritative balances | Proposed | Integrity and auditability vs more engineering than naive row sync |
| **028** | Sync layer behind an interface; select among Electric, PowerSync and Zero by spike. Criteria: self-hostable, permissive or convertible licence, partial sync with per-user authorisation, durable upload queue, Postgres source, mobile SDK maturity, vendor stability | **Open** | Time-to-market vs licence and vendor risk (Electric's team moved to Neon; PowerSync is source-available) |
| **029** | Native-capable cross-platform shell with a shared Rust core; PWA for administration and light use | **Open (spike)** | Device capabilities (NFC, Bluetooth, background work) vs one-codebase simplicity |
| **030** | Tiered on-device AI with no correctness dependency on device models | Proposed | Offline value vs inconsistent hardware support |
| **031** | Post-quantum profile: hybrid ML-KEM with X25519; ML-DSA for signatures; SLH-DSA for long-lived roots; FN-DSA only after FIPS 206 is final; versioned algorithm identifiers | Proposed | Larger signatures and keys, standard churn vs long-term record safety |
| **032** | Licensing strategy: open engine, closed corpus (candidates: permissive, copyleft such as AGPL with commercial dual licence, or source-available) | **Open (founder decision; counsel needed)** | Trust and free distribution vs competitors reusing the engine |
| **033** | Data Doctor outputs are provisional until a person confirms them; only confirmed labels feed statistics and prognostics | Proposed | Slower labelling vs defensible evidence |
| **034** | Pricing structure: fixed-fee pilots with a price floor; subscription by legal entity and asset band; predictable AI credits; no per-seat core pricing; publish prices only after at least ten pricing-probe responses | Proposed | Predictability and fit vs revenue upside from usage |
| **035** | Design-partner agreement: discount in exchange for reference, case study and anonymised pattern rights; no exclusivity; data-handling addendum; written success criteria | Proposed | Early references and corpus vs concessions on price |
| **036** | Partner-first channel: Maximo consultants and local integrators before direct enterprise sales | Proposed | Reach with little capital vs margin share and dependence |
| **037** | First localisation pack is Trinidad and Tobago; scope in Appendix L; verify every rate and rule with the Inland Revenue Division and the National Insurance Board before use | Proposed | Local credibility vs narrow initial market |
| **038** | Connectivity Fabric: connector SDK, registry and contract tests; standards-first protocol catalogue | Proposed | Upfront engineering vs scalable, safe connectivity |
| **039** | Period-close state machine per Appendix M, enforced by the Period Controller | Proposed | Strict rules vs flexibility some customers expect |
| **040** | Reporting classification as versioned projections; restate comparatives by replay | Proposed | Extra projection layer vs avoiding data migrations when standards change |
| **041** | Rules as code: evaluate Catala for tax and payroll rule packs, with a spike on Rust integration | **Open (spike)** | Formal semantics and generated tests vs tooling maturity and integration effort |
| **042** | Business Twin on event-store forks with perturbation operators; simulation labelled as such | Proposed | Decision support vs risk of over-trusting simulations |
| **043** | Maintenance Decision Engine: constraint-programming scheduler (CP-SAT as first candidate) with uncertainty-aware inputs and explainable plans; joint spare-parts optimisation as the research direction | Proposed | Plan quality vs model-maintenance effort |
| **044** | Governed semantic layer for natural-language query; no SQL over raw source tables | Proposed | Reliability vs slower coverage of new questions |
| **045** | Technician copilot is extractive-first with cited source spans and safe refusal | Proposed | Safety vs fluency |
| **046** | Deterministic simulation testing: runtime abstraction compatible with madsim and turmoil from day one | Proposed | Discipline and tooling cost vs reproducible failures |
| **047** | Standards alignment: AAS submodels and IEC CIM profiles as import and export formats; OPC UA and MQTT Sparkplug B ingestion | Proposed | Breadth of standards vs depth per standard |

---

## 10. Quality Requirements

### 10.1 Quality Tree (summary)
Correctness (ledger conservation, reproducible reports, equivalence) / Time-to-value (mapping coverage, review effort) / Security (sovereignty, PQC, agent containment) / Onboarding efficiency (hours and LLM cost per 1,000 tables) / Evolvability / Operability / Performance (posting throughput, close time).

### 10.2 Quality Scenarios
*All targets are proposed and must be validated by prototype and pilots.*

| ID | Scenario | Measure |
|---|---|---|
| Q1 | Ledger transfer under node failure and partition | Commits exactly once or is rejected; trial balance zero-sum in all simulated fault runs |
| Q2 | Auditor asks for balance sheet "as known" on a past date | Reproduced exactly with lineage |
| Q3 | New customer connects a 5,000-table schema | Complete baseline mapping without LLM; human review limited to flagged items; reconciled shadow within the pilot window |
| Q4 | Plausible but wrong mapping | Constraint Oracle rejects before live posting |
| Q5 | Agent attempts out-of-policy payment | Blocked deterministically, logged, no partial effects |
| Q6 | Technician offline for a shift | Merge without loss on reconnect |
| Q7 | Month-end close | Variance explainable to zero before cutover sign-off |
| Q8 | Algorithm deprecated | Rotation without rewriting historical records |
| **Q9** | Replay of 12 closed months through new rules | Unexplained divergence below agreed threshold; all invariants hold; equivalence certificate issued |
| **Q10** | Customised routine in legacy system | Rule extracted with traceability to code lines and a confidence grade; ambiguous cases routed to human review, none silently dropped |
| **Q11** | Asset with no failure history | Prognostics returns a calibrated interval, labelled by data layer, never a bare point estimate |
| **Q12** | Disputed outcome-based invoice | Evidence replay reproduces or refutes the billed outcome |
| **Q13** | Specification review | Every proved invariant has a satisfiability and non-vacuity check |
| **Q14** | Wave 0 onboarding effort | Hours and LLM cost per 1,000 tables tracked, trending down each release |
| **Q15** | Source system filters rows by site or group above the database | Imported policy reproduces visibility for sampled users; broader access is flagged and blocked by default |
| **Q16** | Oracle CDC enabled on a live primary | Capture lag and source overhead measured and reported; no application change on the source; documented fallback when ARCHIVELOG or supplemental logging cannot be enabled |
| **Q17** | A change to posting rules or the Ledger Port | Bounded model check re-runs in CI; every seeded mutant (for example a void that releases only one side, a removed guard) is detected; Lean proofs (Appendix D) re-checked |
| **Q18** | A customer asks us to read an Oracle EBS database directly | Licence Position Check report produced and signed off by the customer before connection; default mode is a customer-run Extraction Pack; no connection otherwise |
| **Q19** | An invoice whose vendor-notes field contains injected instructions | Planner never sees the text; reader output is tagged untrusted; any action derived from it needs human approval; zero unauthorised executions across an injection test corpus |
| **Q20** | Agent release candidate | pass^4 on the replay-derived suite meets the agreed threshold, beats do-nothing and random baselines, and policy adherence is perfect on forbidden actions |
| **Q21** | Revoking a parent agent's delegation | All children lose authority; no action executes after revocation; audit shows agent, delegator and human principal for every past action |
| **Q22** | Held-out assets, 90% prognostic intervals | Empirical coverage within a stated tolerance per asset class; system abstains where calibration data is insufficient |
| **Q23** | A change to governance rules | Appendix G model check re-runs; all six seeded mutants are caught; reachability probes confirm each action can execute |
| **Q24** | Kill switch pressed during a multi-step plan | No new execution after the switch; in-flight steps are compensated or flagged; incident record complete |
| **Q25** | A technician works a ten-hour shift offline and the phone battery dies once | Every action is in the local queue after restart; on reconnect each command is applied or returned as an explicit conflict; none lost, none duplicated |
| **Q26** | The same command is delivered twice after a network retry | Applied exactly once (idempotency key) |
| **Q27** | A device is lost or stolen | Token revocation and remote wipe within the agreed time; local store encrypted; no secrets recoverable |
| **Q28** | First sync for a typical crew scope on a poor connection | Completes within the agreed data and time budget with resumable transfer; later delta syncs are small; battery and cold-start budgets measured on mid-range devices |
| **Q29** | A work order is created by voice | Appears as a proposal with transcript and confidence; below the threshold it needs confirmation; fully auditable |
| **Q30** | Failure-mode coder on held-out, expert-labelled work orders | Accuracy and calibration reported per class; low-confidence items routed to review; no unconfirmed label reaches reliability statistics |
| **Q31** | Foreign-key and mapping discovery on foreign-key-stripped open-source schemas | Precision and recall reported per release, with and without the Dictionary Reader; regression gates on both |
| **Q32** | Fixed-fee X-ray pilot | Delivered within the agreed scope and hours; actual review hours, auto-accept rate and tie-out recorded; margin per pilot computed |
| **Q33** | Trinidad and Tobago VAT and payroll test cases | Computed VAT return, PAYE, National Insurance (by earnings class) and Health Surcharge match independently calculated expected values for a prepared case set; rule-pack versions are traceable |
| **Q34** | Pricing probe | At least ten responses per probe recorded before any price is published; results stored with the respondent's role and context |
| **Q35** | Procurement readiness | Supplier registration complete; tender-tracking process in place; a one-page capability statement and an evidence pack (equivalence certificate sample, security overview) ready |
| **Q36** | Posting into a closed period | Rejected; the adjustment is posted to an open period with a back-reference; closed-period totals are unchanged (Appendix M, Lean lemma) |
| **Q37** | Reopening a closed period | Needs a recorded approval and no later closed period; reconciliation is invalidated; audit trail complete |
| **Q38** | Comparatives restated for a new presentation standard | Produced by replaying events through the new classification version; totals tie to the ledger; differences are classification-only |
| **Q39** | Tax or payroll rule change | Rule pack updated with cited source and effective date; generated tests pass; prior periods unaffected unless explicitly replayed |
| **Q40** | Business Twin run on a candidate maintenance policy | Reproducible from fork point, operators and seed; labelled simulation; difference from actual history explained |
| **Q41** | Maintenance schedule with crews, skills, permits, windows, parts and precedence | All hard constraints satisfied; weighted tardiness reported against a baseline rule; plan explanation lists binding constraints; solve time within the agreed budget |
| **Q42** | Natural-language question outside the governed contract | Refused or escalated; no query generated against raw tables; wrong-but-confident rate tracked |
| **Q43** | Copilot asked for a torque value | Answer shows the cited passage or crop and revision; value matches the source span; if no approved source supports it, the copilot refuses |
| **Q44** | New connector submitted | Fails promotion without contract tests, security profile and licence-position note; passes with recorded fixtures and simulated faults |
| **Q45** | Simulation seed from CI | The same seed reproduces the same failure on a developer machine; determinism check passes (runs twice, identical traces) |

---

## 11. Risks and Technical Debt

| ID | Risk | L / I | Mitigation |
|---|---|---|---|
| RK1 | Scope: three incumbent categories | H / H | Wedge strategy, wave plan, parity matrix |
| RK2 | Vendor contract and licensing friction (SAP precedent; Oracle, Microsoft, IBM unreviewed) | M-H / H | Legal gate (ADR 009), customer-owned database access, documented export routes |
| RK3 | Ingestion accuracy on opaque enterprise schemas; published results are on public or domain-specific benchmarks | H / H | Deterministic baseline, judge, oracles, human review, private benchmark from pilots |
| RK4 | Verification cost and skill scarcity | M / H | Small kernel, AI-assisted proofs, simulation backstop |
| RK5 | LLM non-determinism in finance | H / H | Neurosymbolic split, replayable logs, model pinning |
| RK6 | Long-tail localisation | H / M | Rule-pack framework, partners |
| RK7 | Trust and references for a new finance system | H / H | Shadow-first, auditor-accepted evidence |
| RK8 | Legacy data quality | H / M | Quality gates, visible reconciliation |
| RK9 | Predictive claims overstated | M / M | Calibrated uncertainty, baseline-controlled pilots |
| RK10 | Team capacity | H / H | Build fresh in this programme, reuse ASL only if it passes the ADR 016 spike, sequence the roadmap |
| **RK11** | **Well-funded AI-native ERP vendors expand scope** (finance-led, now raising large rounds) | M / H | Focus on asset-intensive, sovereign and public-sector niche; EAM depth; ingestion corpus |
| **RK12** | **Timing: Maximo 7.6.1 window narrowing** before EAM module is ready | H / M | Lead with Wave 0 and read-only Maximo companion; treat the 7.6 tail as rolling, not a single date |
| **RK13** | **Behavioural extraction misses implicit rules** | M / H | Equivalence replay, human sign-off on rule inventory, confidence grading |
| **RK14** | **Outcome-based pricing disputes and principal-agent incentives** | M / M | Evidence-linked metering, caps, hybrid base plus usage |
| **RK15** | **Specification gaps behind proved claims** (proved code, weak spec) | M / H | Spec audit, independent review, property-based and simulation tests |
| **RK16** | **Wave 0 distracts from the kernel** | M / M | Time-box; reuse ingestion outputs as kernel test data |
| **RK17** | **Public-sector sales cycles and procurement rules** | H / M | Local partners, multilateral-programme alignment, pilot with a single agency |
| **RK18** | **Federated learning leakage or low-value** | L-M / M | Opt-in, differential privacy, defer to Wave 3 |
| **RK19** | **IFS Cloud and Hexagon already integrate ERP, EAM and APM with agentic features** | H / H | Differentiate on automated evidence-backed migration, sovereignty, verified kernel, vendor neutrality and price; hands-on teardown of IFS and Hexagon in a later round |
| **RK20** | Source support terms: vendors may limit support for customers with third-party database access; direct writes are warned against | M / M | Read-only access only; writes through documented integration interfaces; customer-side legal review |
| **RK21** | Oracle CDC prerequisites unavailable (no ARCHIVELOG, supplemental logging refused, standby-only access) | M / M | Snapshot plus incremental fallback; Wave 0 works without CDC |
| **RK22** | Application-layer security bypassed by direct SQL (exposure of personal data during onboarding) | M / H | Security Model Importer, least-privilege read accounts, data stays inside the boundary, masking in the Review Workbench |
| **RK23** | Trinidad and Tobago data-protection regime changes (fuller proclamation) | L-M / M | Design to the Act's principles; cross-border controls already part of the trust boundary |
| **RK24** | Maximo Java customisations unreachable without code access | M / M | Contractual access to build artefacts; black-box behavioural tests via replay and golden masters |
| **RK25** | Direct reads of restricted-use vendor databases breach or convert the customer's licence (Oracle EBS) and expose the customer to indirect-access claims | M / H | Licence Position Check, customer-run Extraction Pack, vendor-sanctioned exports, counsel review (Appendix E) |
| **RK26** | Ledger-engine dependency: single vendor, fixed schema, assurance by testing rather than proof | M / M | Ledger Port, Appendix D conformance tests, fresh Rust ledger kernel built from the Appendix D specification as alternative |
| **RK27** | IFS and peers shift price or migration offers | M / M | Compete on automated evidence-backed onboarding and sovereignty; re-check competitor offers every round |
| **RK28** | **AI-native migration startups (Tessera Labs, Trope and others) erode our ingestion differentiator** or move into EAM | M-H / H | Differentiate on the integrated target, equivalence evidence and asset-intensive depth; teardown done at public-source depth in Round 6; consider partnership |
| **RK29** | **Prompt injection through ingested data** (free-text fields in legacy databases, documents, emails) | H / H | S22, ADR 026, Appendix G, adaptive red-teaming, no tool access for the reader |
| **RK30** | **Overclaiming agent reliability** from flawed benchmarks or single-run scores | M / H | pass^k, baselines, deterministic grading, replay-derived tasks, publish only measured results |
| **RK31** | **Agent identity and delegation standards remain unsettled**, forcing rework | M / M | Build on mature protocols (OAuth, SPIFFE), keep delegation logic behind an interface, track NIST outputs |
| **RK32** | Mobile platform limits (iOS web capabilities, store policies, background execution, mixed device fleets) | M / M | Native-capable shell, capability spikes, graceful degradation, device-fleet survey during discovery |
| **RK33** | Sync-engine dependency: licence terms (source-available) or vendor changes (team acquisition) | M / M | Interface boundary, spike on at least two engines, fallback to a thin in-house sync over Postgres logical replication |
| **RK34** | Offline conflicts corrupting operational data | M / H | Command queue, server-authoritative balances, explicit conflict tasks, Q25 and Q26 tests |
| **RK35** | On-device model variability causing inconsistent proposals | M / L | No correctness dependency; proposals only; server-side validation |
| **RK36** | Open-sourcing the engine helps competitors more than it helps us | M / M | Keep corpora, invariant library and evaluation data proprietary; choose licence with counsel; decide after discovery |
| **RK37** | Synthetic or small-sample evidence misleads (for example the Appendix J spike) | H / M | Label such evidence clearly; build the open-source benchmark and test on real schemas before relying on any number |
| **RK38** | Post-quantum standard churn (FIPS 206 not final; IR 8547 still draft) | M / L | Versioned algorithm identifiers, crypto inventory, avoid unfinalised algorithms |
| **RK39** | **Overlay objection**: customers choose AI overlays instead of replacement | H / M | Wave 0 as overlay-compatible entry; cost-displacement offers; vendor-restriction argument; verified evidence |
| **RK40** | **Central-government finance in Trinidad and Tobago already awarded** (FreeBalance IFMIS) narrows the public-sector target | H / M | Focus on state enterprises, statutory bodies, utilities, asset and procurement modules; partner with the incumbent where sensible |
| **RK41** | **Grant and programme timing and eligibility** (calls may be closed, require local registration, or take months) | M / M | Treat grants as upside, not the plan; fund through paid pilots first |
| **RK42** | **Pilot scope creep and unpaid work** erodes cash | H / M | Pilot rules in 0.10, price floor, scope cap, success criteria |
| **RK43** | **Small-market prices are lower than assumed**, making unit economics hard | M / H | Price probes, regional expansion, partner-led delivery, productised automation to cut hours |
| **RK44** | **Connector sprawl, quality and supply-chain risk** | H / M | Connector SDK, registry, signed packages, contract tests, sandboxing, same gate for third parties |
| **RK45** | **Standards complexity** (AAS, OPC UA, CIM) outpaces our capacity | M / M | Start with narrow profiles, add per customer need, test with public sample models |
| **RK46** | **Rules-as-code tooling immaturity or poor Rust integration** | M / M | Spike first; keep an escape route to plain tested Rust with citations |
| **RK47** | **Optimisation models mis-specified** (synthetic spike evidence only) | M / M | Replay evaluation on real history, human review of plans, explainable constraints |
| **RK48** | **Confident wrong answers from natural-language query** | M / H | Governed contracts only, refusal, lineage, wrong-but-confident metric, private question set |
| **RK49** | **Copilot hallucination in a safety-critical step** | L-M / H | Extractive-first, cited spans, refusal, human verification, no procedure generation |
| **RK50** | **Business Twin misleads** (over-trusted simulation) | M / M | Label as simulation, sensitivity analysis, compare with history, no forecasts presented as facts |
| **RK51** | **Determinism discipline slows development** | M / L | Runtime abstraction from day one, lint rules, CI determinism check |

**Technical debt watch-list:** ontology drift between versions; mapping library curation; proof maintenance; model upgrade regressions; connector sprawl.

---

## 12. Glossary

| Term | Meaning |
|---|---|
| arc42 | Template for architecture documentation in 12 sections |
| Behavioural Specification Extractor | Component that recovers business rules from legacy code and scripts with traceability |
| Bitemporal | Valid time plus transaction time |
| CDC | Change data capture |
| CRDT | Conflict-free replicated data type |
| EAM / CMMS | Enterprise asset management / maintenance management |
| Equivalence Oracle | Replays history through new rules and compares with legacy outcomes |
| Federated learning | Training across tenants without sharing raw data |
| OCEL 2.0 | Object-centric event log standard |
| Posting rule | Declarative mapping from economic event to ledger transfers |
| REA | Resource-Event-Agent ontology |
| RUL | Remaining useful life |
| SOE | Semantic Onboarding Engine |
| Strangler fig | Incremental replacement behind a routing facade |
| TSFM | Time-series foundation model |
| Truth Plane | Small verified kernel: events, ledger, policy, audit, metering |
| Verified Invariant Library | Curated, proved specifications and certified rule packs |
| X-ray | Wave 0 read-only readiness product |
| Extraction Pack | Signed, read-only extraction scripts run by the customer's own DBA so the customer performs the read under its own licence |
| Licence Position Check | Pre-connection review of the customer's licence terms for the source and the chosen connector mode |
| Ledger Port | Interface between the Truth Plane and the ledger engine |
| Planner / Reader | Agent roles: the planner sees only trusted instructions and writes the plan; the reader processes untrusted content without tool access |
| Capability-tagged value | A value carrying provenance and permitted-use metadata that the policy kernel enforces |
| Conformal prediction | Method that wraps any predictor to give intervals with finite-sample coverage guarantees under stated assumptions |
| pass^k | Reliability metric: the probability that all k independent runs of a task succeed |
| Command queue | Durable on-device list of intents (with idempotency keys) that the server validates and turns into events |
| Read-model sync | Replication of role- and site-scoped, read-only views to the device |
| Data Doctor | Wave 0 component that codes failure modes and cleans legacy maintenance data with human review |
| FK-stripping benchmark | Test in which declared foreign keys of an open-source schema are hidden and then rediscovered |
| Connectivity Fabric | Connector SDK, registry and gates that make connecting to external systems a tested, sandboxed capability |
| Business Twin | Forkable, replayable model of the organisation built on the event store for what-if analysis |
| Governed semantic layer | Single definition of metrics, dimensions and certified joins from which queries are compiled |
| Classification projection | Versioned rules that map postings to presentation categories without changing the ledger |
| Catala | A domain-specific language for legislation with formal semantics (default logic) |
| CP-SAT | Constraint-programming and SAT solver from Google OR-Tools used here for scheduling |
| DST | Deterministic simulation testing: runs the system under a controlled simulated world |
| AAS | Asset Administration Shell: standardised digital-twin information model from IDTA |
| CIM | Common Information Model for electric utilities (IEC 61968 and 61970) |

---

## Appendix A. Academic and Technical Domain Map

| Domain | Why it matters | Where it lands |
|---|---|---|
| Database theory: schema matching, data integration, provenance, temporal databases | Ingestion, audit | P1, P2 |
| Knowledge representation: OWL, SHACL, R2RML, upper ontologies | Canonical ontology | P1, 8.1 |
| Ontology learning and LLM alignment | Mapping at scale | P1 |
| **Program comprehension and reverse engineering; specification recovery** | Behavioural capture | P1 BSE |
| **Software testing theory: differential and golden-master testing; equivalence checking** | Equivalence Oracle | P1, 6.5 |
| Accounting information systems: REA, double entry, IFRS, XBRL, continuous auditing | Core semantics, continuous close | P2, P3 |
| Process science: object-centric mining, BPMN/DMN, conformance | As-is discovery | P1, P4 |
| Formal methods: TLA+, Lean 4, SMT-based Rust verification, refinement, session types | Verified kernel | P2, 8.4 |
| **AI-assisted theorem proving and verified code generation** | Lower proof cost | 8.4, S12 |
| Distributed systems: consensus, simulation testing, CRDTs, sagas, local-first | Correctness, offline | P2, 8.3 |
| Multi-agent systems, MCP/A2A, neurosymbolic AI, uncertainty quantification | Agents | P4, 8.5 |
| Reliability engineering and prognostics: RCM, survival analysis, physics-informed ML, digital twins, ISO 55000/14224 | EAM | P3, P4 |
| **Time-series foundation models** | Cold-start prognostics | P4 |
| **Federated learning and differential privacy** | Cross-tenant learning | P4, 8.11 |
| Operations research: scheduling, inventory, MRP, stochastic optimisation | Planning | P4 |
| Security and cryptography: post-quantum, zero trust, ReBAC, tamper-evident logs, confidential computing | Sovereignty | P6, 8.6 |
| Software engineering for migration: strangler fig, spec-driven migration | Cutover | 6.4, S9 |
| **Economics and mechanism design: pricing, principal-agent problems in outcome contracts, platform economics** | Profitability | 0.5, S15 |
| **Technology adoption (TAM) and change management in ERP programmes** | Adoption, sales motion | 0.4, RK7 |
| **Public financial management, procurement law, multilateral-lender rules** | Public-sector beachhead | B3, RK17 |
| Information systems and law: licensing, indirect access, data protection | Legal gate | RK2 |
| HCI and explainable AI | Trust | P5, 8.8 |
| **Ontology engineering and top-level ontologies (BFO, IOF, REA, QUDT)** | Canonical model reuse | 8.1, S17 |
| **Metadata-driven integration; application data dictionaries** | Source profiles | S16 |
| **Log-based change capture and database internals (redo logs, supplemental logging)** | CDC | S18 |
| **Competitive strategy and platform economics** | Positioning against IFS, SAP, Oracle | 0.2, RK19 |
| **Access-control models and data-protection engineering** | Security model import | R16, RK22 |
| **Software licensing law and compliance (restricted-use, indirect access)** | Connector modes | S19, RK25 |
| **Model checking and mutation testing** | Kernel assurance | S21, Appendix D |
| **Capability-based security and information-flow control** | Plan-execute separation | S22, 8.5 |
| **Non-human identity and delegated authorisation** | Agent identity | R20, ADR 023 |
| **Conformal prediction and uncertainty calibration** | Prognostic intervals | S23, 8.13 |
| **Evaluation science and benchmark validity** | Agent release gates | S24, 8.14 |
| **Services economics and AI-native professional services** | Fixed-fee onboarding, margins | 0.5, RK28 |
| **Local-first computing and distributed data synchronisation** | Offline mobile | 8.15, S25 |
| **Mobile systems, device security and edge AI** | Field app, tiered intelligence | R22 to R25, S26 |
| **Technical language processing for maintenance records** | Data Doctor | S27, R26 |
| **Cryptographic engineering and post-quantum migration** | PQC profile | S28, 8.6 |
| **Open-source economics and software licensing** | Engine strategy | S29, ADR 032 |
| **Human factors for field work (voice, gloves, glare, poor light)** | Mobile design | R22 |
| **Pricing science and willingness-to-pay methods** | Pricing experiments | 0.5, ADR 034 |
| **Public procurement law and public financial management** | Public-sector route | 0.10, Appendix L |
| **Enterprise software sales and channel strategy** | Go-to-market | 0.10, ADR 036 |
| **Tax and payroll compliance engineering** | Localisation packs | R27, Appendix L |
| **Development finance and grant ecosystems** | Non-dilutive funding | 0.5, RK41 |
| **Industrial automation and interoperability standards (OPC UA, MQTT, AAS, ISO 23247)** | Connectivity Fabric | S30, R30 |
| **Utility information models (IEC CIM)** | Utility asset and work data | R30, 8.16 |
| **Legal informatics and rules as code (Catala, default logic)** | Jurisdiction rule packs | S33, 8.18 |
| **Financial reporting standards and digital reporting (IFRS 18, XBRL)** | Classification projections | S32, 8.17 |
| **Constraint programming and scheduling theory** | Maintenance Decision Engine | S35, 8.20 |
| **Stochastic and robust optimisation for maintenance and inventory** | Joint maintenance and spares | 8.20 |
| **Discrete-event simulation and generative process mining** | Business Twin | S34, 8.19 |
| **Semantic layers, metrics governance and text-to-SQL evaluation** | Governed queries | S36, 8.21 |
| **Safety-critical information retrieval and HCI** | Technician copilot | S37, 8.22 |
| **Deterministic simulation testing and systems verification practice** | Test strategy | S38, 8.23 |

---

## Appendix B. Sources and Evidence

Grades as in the header. Vendor, press and advisory-firm sources are treated as claims, not facts. Items marked "verify" must be re-checked before any external use.

**Ingestion and semantics:** RIGOR (arXiv 2506.01232); LLM4VKG (IJCAI 2025); constraint-guided enterprise data mapping (arXiv 2608.24218); LLM schema matching papers (arXiv 2507.14376, 2601.20482); specification-driven architecture reconstruction (arXiv 2609.13929). **[M]**

**Behavioural capture:** AgentModernize (arXiv 2605.17535); Reversa (arXiv 2605.18684); ACL 2026 industry paper on deterministic parsing plus schema-constrained generation. **[M]**

**Process intelligence:** OCEL 2.0 (arXiv 2403.01982); SAP event extraction (arXiv 2110.03467). **[S/M]**

**Accounting ontology:** McCarthy REA; ISO/IEC 15944-4; REA2 formalisation. **[S]**

**Ledger and temporal data:** TigerBeetle design and data-modelling documentation (fixed account and transfer schema, ledger partitions, linked transfers, user_data fields) **[M]**; Jepsen analysis of TigerBeetle (2025) **[M]**; SQL:2011 temporal; XTDB 2.x documentation and release notes (MPL-2.0; 2.2 release candidate) **[M]**.

**Formal methods:** TLA-Verus (arXiv 2610.14923); Rust-Prover (arXiv 2610.03994); Vero (arXiv 2608.13522); CoCo-Prover (arXiv 2610.09681); Lean-guarded agentic finance (arXiv 2604.01483, weak). **[M]**

**Agentic ERP and incumbents:** Agentic ERP architecture (arXiv 2607.17331) **[M]**; SAP Sapphire 2026 and Oracle Fusion agentic coverage **[V]**; SAP API Policy v.4.2026 reporting (The Register and others) **[M]**.

**Competition and pricing:** Rillet funding coverage (Aug 2026) **[V]**; IFS press releases (IDC MarketScape leader, H1 2026 growth, Loops) **[V]**; third-party IFS pricing estimates (erpresearch, rfp.wiki and others) **[V, estimates]**; Futurum 2H 2026 pricing survey **[V]**.

**Maximo:** IBM data dictionary tables documentation **[S]**; IBM automation-scripts documentation (scripts and configuration stored in the database; library scripts in AUTOSCRIPT) **[S]**; IBM security documentation and support posts (MAXGROUP, SITEAUTH, APPLICATIONAUTH, MAXAPPS, SIGOPTION; row and attribute restrictions) **[S/M]**; IBM support pages on 7.6.1 lifecycle (conflicting dates) **[S]**; IBM CDS wiki on read-only SaaS database access **[V]**; IBM community on direct SQL practice **[M]**.

**Lifecycles:** Oracle EBS 12.2 Premier Support through at least 2036 and JD Edwards 9.2 through at least 2034 (Oracle announcements relayed) **[V]**; Dynamics GP end of support 31 Dec 2029 (partner sources) **[V]**.

**Licensing:** Redress Compliance and Oracle Licensing Experts analyses of restricted-use database and indirect-access exposure **[V, commercial interest]**.

**CDC:** Debezium, Airbyte, Decodable and Databricks connector documentation on LogMiner prerequisites **[M]**.

**Ontology:** IOF release 202601 (Maintenance Ontology released, 20 classes, 3 properties; Core Ontology; Supply Chain ontology) **[S for existence of classes, M for fit]**; Iof-maint paper (arXiv 2404.05224) **[M]**.

**Regulation:** Trinidad and Tobago Data Protection Act 2011, partially proclaimed (Lex Mundi, UWI and other secondary sources; conflicting vendor brief) **[M]**.

**Public-sector context:** IDB programme and technical-cooperation documents for Trinidad and Tobago digital transformation and public procurement **[M]**.

**Round 4 additions.** OWASP Top 10 for Agentic Applications 2026, published 9 Dec 2025 (goal hijack, tool misuse, identity and privilege abuse, supply chain, plus memory poisoning, insecure inter-agent communication, cascading failures, trust exploitation and rogue agents among its categories) **[M]**; CaMeL, "Defeating Prompt Injections by Design" (arXiv 2503.18813) **[M]**; NIST CAISI AI Agent Standards Initiative (17 Feb 2026) and NCCoE concept paper on agent identity and authorisation (Feb 2026), via CSA and other summaries **[M]**; NIST AgentDojo-Inspect red-teaming results as relayed by secondary sources **[M]**; conformal prediction for RUL (arXiv 2212.14612; IJPHM 2023) and conformal time-series review (arXiv 2511.13608) **[M]**; tau-bench pass^k and its successors, CLEAR framework (arXiv 2511.14136), Finch finance-and-accounting benchmark (arXiv 2512.13168), benchmark-validity critiques **[M]**; IBM community and IBM Support migration guidance for Maximo 7.6.1.x to MAS and IBM Ideas notes on customisation builds **[M]**; IFS community and press on deployment options **[M/V]**; PostgreSQL 18 temporal constraints and PostgreSQL 19 beta coverage **[M]**; Oracle BICC documentation and Microsoft Learn on Dynamics 365 export options **[S/M]**; Tessera Labs, Trope, McKinsey and Epicor coverage on AI-native modernisation **[V/M]**; Rimini Street FY2025 results coverage **[V]**.

**Round 5 additions.** Lean 4 release archives from the Lean GitHub repository (version 4.15.0 used) **[S]**; PowerSync documentation (alternatives comparison, October 2026), Electric and Zero coverage including InfoQ on Zero 1.0 (June 2026), Neon Realtime beta notes **[M]**; Apple developer and WWDC 2026 summaries (Foundation Models image input, Core AI), Android AICore and ML Kit GenAI documentation, Gemini Nano talks **[M]**; Stewart, Hodkiewicz and Li, "Large Language Models for Failure Mode Classification" (arXiv 2309.08181), FMEA Builder (arXiv 2411.05054), wind-turbine maintenance-log labelling framework (arXiv 2605.31281), SAE 2026 paper on LLM-based FMEA **[M]**; NIST post-quantum summaries (FIPS 203, 204, 205 final; FIPS 206 draft; IR 8547 draft; CNSA 2.0 parameters) via standards-advisory sources **[M]**; open-source ERP model coverage (erp.today, Frappe and Odoo comparisons, Mordor estimate, General Atlantic announcement as relayed) **[V]**; openMAINT project page (Property and Facility Management on CMDBuild; PostgreSQL; Affero GPL as listed, one reviewer disputes source availability, so verify) **[M]**.

**Round 6 additions.** ERP Today on Tessera Labs' $60M round (published 26 June 2026), with references to the company announcement and press coverage **[V/M]**; Y Combinator company and launch pages for Trope, Dealroom and aggregator listings **[V]**; ChatFin and other ERP-agnostic overlay marketing **[V]**; IFS press releases (Rwanda utility), UK Find a Tender notices (Portsmouth Water, Hull City Council) **[S for notices, V for press]**; Government of Trinidad and Tobago Ministry of Finance procurement unit pages and published contract-award lists (2024 Q1 and 2025) **[S]**; Public Procurement and Disposal of Public Property Act 2015 text, the 2026 amendment bill (Parliament), Trinidad and Tobago Securities and Exchange Commission notice on the Procurement Depository, OPR site **[S/M]**; KPMG and VAT Update coverage of Budget 2027 tax measures (October 2026), Thomson Reuters regulatory updates, payroll-provider guides for rates (Mercans, Remote, Pebl) **[M/V]**; IDB and EU releases on the Innovation Challenge Facility, IDB TechLab and IDB Lab loan call; Microsoft for Startups coverage **[M]**; Anthropic documentation on custom connectors (plan availability, authentication choices, request headers beta) and GitHub's MCP server documentation (hosted endpoint, bearer-token header, repository tools) **[S/M]**.

**Round 7 additions.** Catala: "Catala: A Programming Language for the Law" (arXiv 2103.03198), CUTECat concolic test generation (arXiv 2410.18212), research on LLM translation of French tax law into Catala (arXiv 2606.23913 as listed), Northwestern and Discover coverage of French tax-administration use **[M]**; IFRS 18 (IASB standard April 2024; EY, KPMG, BDO and Protiviti guidance on effective date, five categories, MPMs and ERP reconfiguration) **[S/M]**; IDTA material on the Asset Administration Shell and Hannover Messe 2026, OPC Foundation AAS working group, Fraunhofer and IFAC 2026 items on ISO 23247 and digital twins **[M]**; IEC 61968 and 61970 CIM documentation (CIGRE methodology, EPRI and BSI listings) **[M]**; joint maintenance and spare-parts optimisation (arXiv 2507.08174, EJOR 2026), sequential multi-agent RL for predictive maintenance (arXiv 2502.02071), hierarchical RL for fleet PHM (arXiv 2604.07171), IJPHM review of post-prognostics decision-making **[M]**; text-to-SQL: Spider 2.0 (arXiv 2411.07763, ICLR 2025), semantic-layer benchmark (arXiv 2604.25149), dbt Labs and Forrester 2026 reports as relayed by vendors **[M/V]**; OCEL 2.0 resources (arXiv 2403.01982), BPM 2026 demo papers (DORY, OC-SimPN), van der Aalst's papers on digital twins of organisations and generative event-driven process intelligence **[M]**; madsim and turmoil documentation, RisingWave and S2 engineering posts **[M]**; industrial copilot practice from vendor and consortium material **[V]**; OR-Tools CP-SAT (used in Appendix N) **[S]**.

---

## Appendix C. Open Questions, Round Plan and Definition of Ready

**Rounds done:** 1 (strategy, moat, beachheads), 2 (source systems, CDC, ontology reuse), 3 (legal gate, ledger kernel specification), 4 (agent governance, threat model, prognostic calibration, evaluation), 5 (unblocking, mobile and field architecture, Data Doctor, post-quantum profile, ingestion spike), 6 (economics and go-to-market; now parked), 7 (product depth and innovation: period close, classification projections, rules as code, Business Twin, Maintenance Decision Engine, governed semantic layer, Connectivity Fabric, technician copilot, deterministic simulation testing).

**Proposed next rounds (product-first, per D8)**
- **8. Truth Plane completion:** multi-currency and FX revaluation, linked (atomic multi-entry) transfers, intercompany and consolidation, fixed assets and leases, extended TLA+ and Lean artefacts.
- **9. Procurement, inventory and EAM domain designs:** state machines, data models, valuation methods, work-management lifecycle, inspections and permits, with model-checked cores where it pays.
- **10. Intelligence Plane detail:** information-flow policy for planner and reader, prognostics model portfolio and calibration protocol, optimisation formulations, copilot evaluation, semantic-layer contracts.
- **11. Connectivity Fabric specification:** connector SDK, descriptor schema, contract-test format, registry and signing, first adapters (database, file, OPC UA, MQTT, AAS, CIM, ISO 20022).
- **12. Consolidation:** walking-skeleton scope, repository layout, first-sprint backlog, risk burn-down.

**Spikes (no production code)**
1. *Ledger Port conformance (ADR 002):* encode Appendix D invariants as tests against the chosen engine; measure a month-end-close workload.
2. *ASL (ADR 016):* implement the Appendix G policy kernel and one capability contract in ASL and in plain Rust; compare using the listed criteria. Needs the ASL repository.
3. *FK-stripping benchmark* on open-source schemas (Odoo, ERPNext, openMAINT, Apache OFBiz, Dolibarr, Tryton, LedgerSMB); check licences first.
4. *Injection spike:* corpus of injected payloads against the planner and reader split.
5. *Mobile spike (ADR 028, 029):* offline work requests, two sync engines, QR and NFC scanning, first-sync size and battery on two mid-range phones.
6. *Failure-mode coder spike* on a public maintenance-record dataset or anonymised data.
7. *Catala spike (ADR 041):* encode a small tax or payroll rule set (for example National Insurance classes and PAYE bands) in Catala, generate tests, and confirm compilation targets and Rust integration.
8. *Period Controller conformance:* implement the Appendix M state machine in Rust on a simulatable runtime and replay the TLA+ traces as tests.
9. *Scheduler spike on real structure:* use public work-order data or an anonymised sample to compare the CP-SAT scheduler with the customer's historical schedule (Business Twin replay).
10. *Semantic-layer spike (ADR 044):* define 20 governed metrics over the canonical model and measure wrong-but-confident rate on a question set.
11. *Connector SDK spike (ADR 038):* build two connectors (a database and an OPC UA or MQTT source) with contract tests and a sandbox.
12. *DST spike (ADR 046):* run the ledger conformance tests under madsim with fault injection and a determinism check.
13. *Lead-mining spike* (parked under D8).

**Definition of Ready for coding**
1. First customer profile and first three source systems chosen (access to real schemas still needed).
2. ADR 002, 003, 019 and 021 accepted (done); ADR 016, 028, 029 and 041 spikes recorded; ADR 032 decided.
3. Truth Plane kernel specification extended (multi-currency, linked transfers, consolidation) with Lean and TLA+ artefacts; the Appendix M period-close model reviewed.
4. Governance kernel (Appendix G) reviewed and conformance tests designed.
5. Canonical ontology v0 hand-tested against a real or realistic Maximo schema; FK-stripping benchmark run.
6. Legal gate outcome known for the first three connectors (Appendix K).
7. Private ingestion benchmark plan, agent evaluation harness design (R19), mobile acceptance tests (Q25 to Q29) and simulation-testing harness (Q45) written.
8. Walking-skeleton scope defined (connect, ingest one module, reconcile, query through the governed semantic layer, with minimal Constraint and Equivalence Oracles).
9. Unit-economics instruments (Section 0.5) defined; parked under D8 until product scope stabilises.
10. (Parked under D8) pilot commitment; not required while the project is product-first.
11. Top risks (RK1, RK2, RK3, RK4, RK12, RK19, RK25, RK28, RK29, RK36, RK37, RK44, RK47, RK48, RK49) each have an owner and a mitigation experiment.

**Decisions waiting on the founder**
1. ADR 032: licensing route for the ingestion engine (and advice on licences).
2. Which source systems can you reach, even anonymised?
3. Which innovation (I1 to I9) should lead the walking skeleton? Suggested: ingestion plus governed semantic layer plus Period Controller, because they make the system useful on day one.
4. GitHub connector: report what the dialog shows.

**Open questions after Round 7**
1. Live Maximo schema: storage for actions, escalations and cron tasks; custom attribute conventions; security-group export.
2. Catala: supported compilation targets and Rust integration; whether Trinidad and Tobago rules are practical to encode from the statutes.
3. Standards: which AAS submodels, OPC UA companion specifications and CIM profiles are worth supporting first; ISO 20022 message sets for payments.
4. Real data for the scheduler and the semantic-layer question set.
5. Device-fleet reality and connectivity in target sites.
6. Counsel answers (Appendix K) and data-protection status.
7. NIST AI Agent Test Suite; ratified agent-identity standard; FIPS 206 final text.
8. XTDB 2.x general-availability status (only if used as a projection store).
9. GitHub push access.

---

## Appendix D. Truth Plane Ledger Kernel: Specification v0 and Model-Check Results

**Purpose.** A small, checkable specification of the Ledger Port semantics, used (1) to define conformance tests for any ledger engine, (2) as the seed of the Verified Invariant Library, and (3) to show the verification approach working rather than only describing it.

**Scope of v0.** Accounts with posted and pending debit and credit totals; transfers with four lifecycle states (none, pending, posted, voided); single-phase and two-phase transfers; one guard rule (a guarded account's debits, posted plus pending, may not exceed its credits). Out of scope in v0: multiple ledgers and currencies, linked (atomic multi-entry) transfers, period close, account closure, timestamps, authorisation, replication.

**Invariants checked**
- **I1 Conservation:** total posted debits equal total posted credits; total pending debits equal total pending credits.
- **I2 Guard:** guarded accounts never owe more than they hold.
- **I3 Pending exactness:** each account's pending totals equal the sum of its pending transfers (no leaked reservations).
- **I4 Posted exactness:** each account's posted totals equal the sum of its posted transfers (balances derivable from the journal).
- **LegalMove (action property):** a transfer's debit, credit and amount never change once created; posted and voided are terminal; pending may become posted or voided.

**Results (TLC, bounded model check, 2026-10-24).** Constants: 3 accounts, 3 transfer ids, maximum amount 2, one guarded account. The correct specification explored 713,185 states (21,433 distinct, search depth 7) with no violation of TypeOK, I1 to I4 or LegalMove (deadlock checking disabled because the bounded id set is eventually exhausted).

**Non-vacuity (mutation) check.** Two deliberately broken variants were run with the same bounds: a void that releases only the debit side was caught as a violation of I1 after 115 states generated; removal of the guard check was caught as a violation of I2 after 81 states generated. A third variant, in which posting a pending transfer forgets to credit the destination account, is caught as a violation of I1. The checker therefore can fail on these defect classes.

**What this does and does not establish.** It is a bounded model check, not a proof: it covers small constants only, and says nothing about the engine's implementation. It does give a precise, executable contract for the Ledger Port, and it found no flaw in the design of the four transitions. Extension to multi-currency, linked transfers and period close is Round 4+ work.

**Lean 4 conservation lemmas (CHECKED, 2026-10-25).** Compiled with Lean 4.15.0 using only the core library (no Mathlib), obtained as a release archive from GitHub because the official download host is unreachable from the research sandbox; the compiler reported no errors. A negative control (a false claim that a single credit entry is balanced) is rejected by the same compiler. The lemmas cover journal-level conservation: a transfer is balanced, balanced journals compose, reversal negates the total, voiding nets to zero, and any sequence of transfers is balanced (journal-level form of I1). They do not yet cover pending versus posted state, guards, per-account balances or multi-currency; those are the next formalisation steps.

```lean
-- Truth Plane ledger: journal-level conservation lemmas (Round 5; checked with Lean 4.15.0, core library only).

structure Entry where
  account : Nat
  amount  : Int          -- positive = credit, negative = debit
deriving Repr

/-- Net amount of a journal. -/
def total : List Entry → Int
  | []      => 0
  | e :: es => e.amount + total es

def Balanced (es : List Entry) : Prop := total es = 0

/-- A double-entry transfer of `amt` from debit account `d` to credit account `c`. -/
def transfer (d c amt : Nat) : List Entry :=
  [⟨d, -(amt : Int)⟩, ⟨c, (amt : Int)⟩]

/-- Reversal (used by void and by corrections): every entry negated. -/
def reverse : List Entry → List Entry
  | []      => []
  | e :: es => ⟨e.account, -e.amount⟩ :: reverse es

theorem total_append (a b : List Entry) : total (a ++ b) = total a + total b := by
  induction a with
  | nil => simp [total]
  | cons e t ih => simp [total, ih]; omega

theorem transfer_balanced (d c amt : Nat) : Balanced (transfer d c amt) := by
  simp [Balanced, transfer, total]
  omega

theorem balanced_append {a b : List Entry} (ha : Balanced a) (hb : Balanced b) :
    Balanced (a ++ b) := by
  unfold Balanced at *
  rw [total_append, ha, hb]; rfl

theorem total_reverse (es : List Entry) : total (reverse es) = - total es := by
  induction es with
  | nil => rfl
  | cons e t ih => simp [reverse, total, ih]; omega

/-- Voiding a pending journal nets to zero, whether or not the journal was balanced. -/
theorem void_nets_to_zero (es : List Entry) : total (es ++ reverse es) = 0 := by
  rw [total_append, total_reverse]; omega

/-- A journal made only of transfers. -/
def journalOf : List (Nat × Nat × Nat) → List Entry
  | []                => []
  | (d, c, a) :: rest => transfer d c a ++ journalOf rest

/-- Ledger-kernel invariant I1 at journal level: any sequence of transfers conserves value. -/
theorem journalOf_balanced (ts : List (Nat × Nat × Nat)) : Balanced (journalOf ts) := by
  induction ts with
  | nil => rfl
  | cons t rest ih =>
    obtain ⟨d, c, a⟩ := t
    exact balanced_append (transfer_balanced d c a) ih
```

**TLA+ specification (Ledger.tla)**

```tla
---------------------------- MODULE Ledger ----------------------------
\* Truth Plane ledger kernel, abstract model (Round 3 sketch).
\* Two-phase transfers in the style of purpose-built double-entry engines:
\* single-phase (immediate), pending, post, void. Amounts are positive naturals.
EXTENDS Naturals, FiniteSets

CONSTANTS Accounts,      \* set of account ids
          Ids,           \* set of transfer ids (bounds the model)
          MaxAmt,        \* maximum amount per transfer
          Guarded        \* accounts whose debits (posted+pending) may not exceed credits posted

VARIABLES dPosted, cPosted, dPending, cPending,   \* per-account totals
          tState, tDebit, tCredit, tAmt            \* per-transfer record

vars == <<dPosted, cPosted, dPending, cPending, tState, tDebit, tCredit, tAmt>>

States == {"none", "pending", "posted", "voided"}

RECURSIVE SumOver(_, _)
SumOver(S, f) == IF S = {} THEN 0
                 ELSE LET x == CHOOSE y \in S : TRUE IN f[x] + SumOver(S \ {x}, f)

TypeOK == /\ dPosted \in [Accounts -> Nat] /\ cPosted \in [Accounts -> Nat]
          /\ dPending \in [Accounts -> Nat] /\ cPending \in [Accounts -> Nat]
          /\ tState \in [Ids -> States]
          /\ tDebit \in [Ids -> Accounts] /\ tCredit \in [Ids -> Accounts]
          /\ tAmt \in [Ids -> Nat]

Init == /\ dPosted = [a \in Accounts |-> 0] /\ cPosted = [a \in Accounts |-> 0]
        /\ dPending = [a \in Accounts |-> 0] /\ cPending = [a \in Accounts |-> 0]
        /\ tState = [i \in Ids |-> "none"]
        /\ tDebit = [i \in Ids |-> CHOOSE a \in Accounts : TRUE]
        /\ tCredit = [i \in Ids |-> CHOOSE a \in Accounts : TRUE]
        /\ tAmt = [i \in Ids |-> 0]

\* Guard rule: a guarded debit account may not owe more than it holds.
GuardOK(d, amt) == d \in Guarded => dPosted[d] + dPending[d] + amt <= cPosted[d]

Bump(f, a, n) == [f EXCEPT ![a] = @ + n]
Drop(f, a, n) == [f EXCEPT ![a] = @ - n]

\* Immediate transfer.
CreatePosted(i, d, c, amt) ==
  /\ tState[i] = "none" /\ d # c /\ amt \in 1..MaxAmt /\ GuardOK(d, amt)
  /\ tState' = [tState EXCEPT ![i] = "posted"]
  /\ tDebit' = [tDebit EXCEPT ![i] = d] /\ tCredit' = [tCredit EXCEPT ![i] = c]
  /\ tAmt' = [tAmt EXCEPT ![i] = amt]
  /\ dPosted' = Bump(dPosted, d, amt) /\ cPosted' = Bump(cPosted, c, amt)
  /\ UNCHANGED <<dPending, cPending>>

\* Reserve amounts without final posting.
CreatePending(i, d, c, amt) ==
  /\ tState[i] = "none" /\ d # c /\ amt \in 1..MaxAmt /\ GuardOK(d, amt)
  /\ tState' = [tState EXCEPT ![i] = "pending"]
  /\ tDebit' = [tDebit EXCEPT ![i] = d] /\ tCredit' = [tCredit EXCEPT ![i] = c]
  /\ tAmt' = [tAmt EXCEPT ![i] = amt]
  /\ dPending' = Bump(dPending, d, amt) /\ cPending' = Bump(cPending, c, amt)
  /\ UNCHANGED <<dPosted, cPosted>>

Post(i) ==
  /\ tState[i] = "pending"
  /\ LET d == tDebit[i] c == tCredit[i] amt == tAmt[i] IN
       /\ dPending' = Drop(dPending, d, amt) /\ cPending' = Drop(cPending, c, amt)
       /\ dPosted' = Bump(dPosted, d, amt) /\ cPosted' = Bump(cPosted, c, amt)
  /\ tState' = [tState EXCEPT ![i] = "posted"]
  /\ UNCHANGED <<tDebit, tCredit, tAmt>>

Void(i) ==
  /\ tState[i] = "pending"
  /\ LET d == tDebit[i] c == tCredit[i] amt == tAmt[i] IN
       /\ dPending' = Drop(dPending, d, amt) /\ cPending' = Drop(cPending, c, amt)
  /\ tState' = [tState EXCEPT ![i] = "voided"]
  /\ UNCHANGED <<dPosted, cPosted, tDebit, tCredit, tAmt>>

Next == \E i \in Ids : \/ \E d, c \in Accounts, amt \in 1..MaxAmt :
                            CreatePosted(i, d, c, amt) \/ CreatePending(i, d, c, amt)
                        \/ Post(i) \/ Void(i)

Spec == Init /\ [][Next]_vars

\* ---------------- Invariants ----------------
\* I1 Conservation: posted debits equal posted credits; pending debits equal pending credits.
Conservation == /\ SumOver(Accounts, dPosted) = SumOver(Accounts, cPosted)
                /\ SumOver(Accounts, dPending) = SumOver(Accounts, cPending)

\* I2 Guard: guarded accounts never owe more than they hold.
GuardInv == \A a \in Guarded : dPosted[a] + dPending[a] <= cPosted[a]

\* I3 Pending totals are exactly the sum of pending transfers (no leaked reservations).
PendingExact ==
  \A a \in Accounts :
    /\ dPending[a] = SumOver({i \in Ids : tState[i] = "pending" /\ tDebit[i] = a}, tAmt)
    /\ cPending[a] = SumOver({i \in Ids : tState[i] = "pending" /\ tCredit[i] = a}, tAmt)

\* I4 Posted totals are exactly the sum of posted transfers (balances derivable from the journal).
PostedExact ==
  \A a \in Accounts :
    /\ dPosted[a] = SumOver({i \in Ids : tState[i] = "posted" /\ tDebit[i] = a}, tAmt)
    /\ cPosted[a] = SumOver({i \in Ids : tState[i] = "posted" /\ tCredit[i] = a}, tAmt)

\* ---------------- Action property: immutability and legal state moves ----------------
LegalMove == [][ \A i \in Ids :
    /\ (tState[i] # "none" => tDebit'[i] = tDebit[i] /\ tCredit'[i] = tCredit[i] /\ tAmt'[i] = tAmt[i])
    /\ (tState[i] = "posted" => tState'[i] = "posted")
    /\ (tState[i] = "voided" => tState'[i] = "voided")
    /\ (tState[i] = "pending" => tState'[i] \in {"pending", "posted", "voided"})
  ]_vars
=============================================================================
```

**TLC configuration (Ledger.cfg)**

```
SPECIFICATION Spec
CONSTANTS
  Accounts = {a1, a2, a3}
  Ids = {t1, t2, t3}
  MaxAmt = 2
  Guarded = {a1}
INVARIANTS TypeOK Conservation GuardInv PendingExact PostedExact
PROPERTY LegalMove
```

Run with: `java -cp tla2tools.jar tlc2.TLC -deadlock -config Ledger.cfg Ledger.tla`.

---

## Appendix E. Legal Gate Matrix (working document, not legal advice)

The advisory sources behind some rows are licensing consultancies with a commercial interest, and none of the vendor contracts themselves were reviewed. Counsel must confirm every row before a connector ships.

**Connector modes**
- **M1 Direct read-only:** our connector reads the customer's database under read-only credentials.
- **M2 Customer-run Extraction Pack:** the customer's DBA runs our signed read-only scripts and hands us the files.
- **M3 Vendor-sanctioned export or API:** documented export services or APIs of the vendor.
- **M4 Restore into the customer's own licensed environment:** a backup is restored by the customer under its own licence and read there.

| Source | Likely default mode | What is known | Ask counsel or the customer |
|---|---|---|---|
| Oracle EBS, JD Edwards (on-premises) | M2 or M3; M1 only after check | EBS ships restricted-use database and middleware licences. Advisers state that connecting third-party tools or ETL directly to the database can void the restricted grant and invite indirect-access claims **[V]** | Does the customer hold full-use database licences? Does the contract allow our read path? Are LogMiner and export utilities covered? Can the DBA export instead? |
| Oracle Fusion Cloud (SaaS) | M3 | Oracle documents the Business Intelligence Cloud Connector (BICC) for bulk extraction from public view objects into external storage areas, described as for outbound integration with downstream applications **[S]**; no direct database access **[V]** | Which extracts are licensed for our use; limits on bulk extraction or on AI-agent access |
| Dynamics GP, NAV, AX, BC (on-premises) | M1 or M2 | Runs on SQL Server; SQL Server licensing for third-party reads not researched | Client access licence or processor coverage for our read path; Microsoft terms |
| Dynamics 365 (online) | M3 | Export to Data Lake for Finance and Operations was deprecated (usable until 1 Nov 2024); replacements are Azure Synapse Link (Delta or Parquet into the customer's own storage) and Fabric Link, with BYOD as an older route **[M]** | Storage and compute costs borne by the customer; Dataverse storage quota for Fabric Link; whether our reading of the exported files raises any licence question |
| IBM Maximo (on-premises) | M1 (read-only) | Customer-owned database; read-only SQL for reporting is common practice; direct writes warned against; IBM Support may decline cases involving custom SQL updates **[M]**. Whether the customer's database is a bundled restricted-use edition is not established | Database licence type; IBM support terms for third-party read access |
| Maximo SaaS and MAS SaaS | M1 over VPN, if the contract allows | IBM documents read-only database access over VPN, with direct production SQL updates not allowed **[V]** | Contract terms; connection procedure |
| SAP (reference only) | M3 | API Policy v.4.2026 reported to restrict third-party AI-agent use and bulk extraction outside endorsed routes; disputed **[M]** | Not a target now; use as a precedent for what other vendors may attempt |

**Cross-cutting questions for counsel:** indirect-access exposure created for the customer by our connector; whether Extraction Pack output is "use" by us or by the customer; data-protection law in each customer's jurisdiction (Trinidad and Tobago Data Protection Act 2011 is partially in force); contractual language on use of derived mapping patterns (never customer data) for our corpus.

---

## Appendix F. Canonical Ontology v0: Summary and Verified External Identifiers

**Layers.** L0 BFO (via IOF Core); L1 IOF Core; L2 IOF Maintenance Reference Ontology; L3 REA (economic layer); L4 QUDT (units); L5 our bridge (work orders to economic events, assets to resources and fixed-asset records); L6 ledger projection (accounts, transfers, posting rules, periods); L7 governance (policy, capability, risk tier, metering, provenance).

**Verified in this round (IOF release 202601, spec portal):**
- IOF Maintenance Ontology, maturity Released, 20 classes and 3 object properties, IRI `https://spec.industrialontologies.org/ontology/maintenance/Maintenance/`, MIT licence.
- Maintenance classes include: degraded state, failed state, operating state, maintenance state, disposition to fail, failure event, failure process, failure effect, failure mode code, functioning process, maintenance activity, maintenance process, maintenance strategy, maintenance work order record, qualified maintenance person, qualification specification, required function, supporting maintenance activity. Properties: has maintenance state, maintenance state of, initiates.
- IOF Core classes relevant to us include: agent, business organization, buyer, supplier, customer, person, planned process, plan specification, action specification, maintainable material item, piece of equipment, material resource, measurement information content entity, measured value expression, identifier, event, agreement.
- IOF Supply Chain classes relevant to us include: purchase order, receiving process, industrial inventory, inventory management process, warehousing process, facility, geospatial site.
- Note: "maintenance work order record" is an information entity (a record), not the work order process itself, so a source work order maps to a record plus a planned process plus our Commitment.

**First mapping candidates for Maximo (all to be hand-tested against a real schema).**

| Maximo object | Canonical candidates | Note |
|---|---|---|
| ASSET | IOF Core maintainable material item or piece of equipment; REA Resource when capitalised | Hierarchy and parent relationships |
| LOCATIONS | IOF Supply Chain facility or geospatial site; own functional-location class | Site and organisation scoping must also be mapped (R16) |
| WORKORDER | IOF maintenance work order record, maintenance activity or planned process, our Commitment | Status domain via source profile |
| PM | IOF maintenance strategy; IOF Core plan specification | Generates work orders by schedule |
| JOBPLAN | IOF Core plan specification and action specification | |
| Failure codes | IOF failure mode code, failure event, failure process | Table names to be confirmed |
| ITEM, INVENTORY | IOF Core material resource; Supply Chain industrial inventory; REA Resource | Inventory table names to be confirmed |
| PO, POLINE | IOF Supply Chain purchase order; REA Commitment | To be confirmed |
| LABTRANS, MATUSETRANS | REA economic events (labour, material issue) | To be confirmed |
| MEASUREMENT, meters | IOF Core measurement information content entity, measured value expression | |
| MAXOBJECT, MAXTABLE, MAXVIEW, MAXATTRIBUTE, MAXVIEWCOLUMN, MAXRELATIONSHIP | Not mapped to business classes | Read by the Dictionary Reader |
| MAXGROUP, SITEAUTH, APPLICATIONAUTH, MAXAPPS, SIGOPTION | Policy layer (L7) | Read by the Security Model Importer |

**Open ontology questions:** REA duality for non-monetary asset events; FIBO fragments for treasury; multi-ledger and multi-GAAP as projections; versioning policy for ontology and mappings; controlled namespace for our own vocabulary.

---

## Appendix G. Agent Governance Kernel: Specification v0 and Model-Check Results

**Purpose.** The deterministic rules that sit between an agent's proposal and execution. The model is deliberately abstract: three actions (a routine low-risk action, a high-risk action such as a payment, and a low-risk action whose parameters came from untrusted content), three agents (one root, two potential delegates), two capabilities and a small budget.

**Rules checked**
- **G1** no high-risk action executes without human approval.
- **G2** no action derived from untrusted content executes without human approval.
- **G3** an agent never exceeds its budget.
- **G4** an agent only executes actions whose capability it holds.
- **G5** delegation never widens authority: a delegated agent's scope is a subset of its delegator's.
- **G6** (action property) once killed, an agent executes nothing further.
- **G7** (action property) an agent's scope does not grow after delegation.

**Results (TLC, 2026-10-24).** Base specification, 3 agents, 3 actions, 2 capabilities, budget 2: 99,489 states generated, 26,176 distinct, no violation of the type invariant, G1 to G5, or G6 and G7. **Mutation check:** six seeded defects were each caught by the intended rule: no taint check (G2, 46 states), no budget check (G3, 2,538), delegation allowed beyond the delegator's scope (G5, 188), kill switch ignored (G6, 686), no capability check (G4, 36), no risk-tier approval (G1, 46).

**A lesson recorded honestly.** The first version of this model passed its checks vacuously: capabilities were compared as text against model values, so no action could ever execute. The mutation tests exposed it (several seeded defects went undetected), and the fix was verified with reachability probes confirming that both the high-risk action and the untrusted-derived action can execute. Every future kernel specification therefore ships with mutants and reachability probes (S21, Q23).

**Limits.** Bounded model check, not a proof. It does not model the planner and reader separation itself, taint propagation through intermediate values, time, partial execution, approval fatigue or malicious approvers. Those need the information-flow policy design in a later round and implementation-level tests.

**TLA+ specification (AgentGov.tla)**

```tla
---------------------------- MODULE AgentGov ----------------------------
\* Agent governance kernel, abstract model (Round 4 sketch).
\* Checks the authorisation rules that sit between an agent's proposal and execution:
\* capability scope, delegation narrowing, budgets, risk-tier approval, taint, kill switch.
EXTENDS Naturals, FiniteSets

CONSTANTS Agents, Caps, c1, c2, x1, x2, x3, Budget

Acts   == {x1, x2, x3}
NoOne  == "none"

\* Fixed attributes of the three modelled actions.
\* x1: routine low-risk action, trusted origin
\* x2: high-risk action (for example a payment), trusted origin
\* x3: low-risk action whose parameters derive from untrusted content (for example an email)
Tier    == [a \in Acts |-> IF a = x2 THEN 3 ELSE 1]
Amount  == [a \in Acts |-> IF a = x2 THEN 2 ELSE 1]
Tainted == [a \in Acts |-> a = x3]
Cap     == [a \in Acts |-> IF a = x2 THEN c2 ELSE c1]

VARIABLES scope,      \* scope[ag] = set of capabilities the agent holds
          delegated,  \* set of agents that have received a delegation
          parentOf,   \* parentOf[c] = the agent that delegated to c
          approved,   \* set of actions a human has approved
          doneBy,     \* doneBy[x] = agent that executed x, or NoOne
          spent,      \* spent[ag] = budget consumed
          killed      \* set of agents under the kill switch

vars == <<scope, delegated, parentOf, approved, doneBy, spent, killed>>

Root == CHOOSE a \in Agents : TRUE

TypeOK == /\ scope \in [Agents -> SUBSET Caps]
          /\ delegated \subseteq Agents /\ parentOf \in [Agents -> Agents] /\ approved \subseteq Acts /\ killed \subseteq Agents
          /\ doneBy \in [Acts -> Agents \cup {NoOne}]
          /\ spent \in [Agents -> Nat]

Init == /\ scope = [a \in Agents |-> IF a = Root THEN Caps ELSE {}]
        /\ delegated = {} /\ parentOf = [a \in Agents |-> Root] /\ approved = {} /\ killed = {}
        /\ doneBy = [x \in Acts |-> NoOne]
        /\ spent = [a \in Agents |-> 0]

\* A parent may give a child a subset of what the parent itself holds, once.
Delegate(p, c, S) ==
  /\ p # c /\ c # Root /\ c \notin delegated /\ p \notin killed
  /\ S \subseteq scope[p]
  /\ scope' = [scope EXCEPT ![c] = S]
  /\ delegated' = delegated \cup {c}
  /\ parentOf' = [parentOf EXCEPT ![c] = p]
  /\ UNCHANGED <<approved, doneBy, spent, killed>>

\* A human approves a specific action.
Approve(x) ==
  /\ x \notin approved
  /\ approved' = approved \cup {x}
  /\ UNCHANGED <<scope, delegated, parentOf, doneBy, spent, killed>>

NeedsApproval(x) == Tier[x] >= 2 \/ Tainted[x]

Execute(a, x) ==
  /\ doneBy[x] = NoOne
  /\ a \notin killed
  /\ Cap[x] \in scope[a]
  /\ spent[a] + Amount[x] <= Budget
  /\ NeedsApproval(x) => x \in approved
  /\ doneBy' = [doneBy EXCEPT ![x] = a]
  /\ spent' = [spent EXCEPT ![a] = @ + Amount[x]]
  /\ UNCHANGED <<scope, delegated, parentOf, approved, killed>>

Kill(a) ==
  /\ a \notin killed
  /\ killed' = killed \cup {a}
  /\ UNCHANGED <<scope, delegated, parentOf, approved, doneBy, spent>>

Next == \/ \E p, c \in Agents, S \in SUBSET Caps : Delegate(p, c, S)
        \/ \E x \in Acts : Approve(x)
        \/ \E a \in Agents, x \in Acts : Execute(a, x)
        \/ \E a \in Agents : Kill(a)

Spec == Init /\ [][Next]_vars

\* ---------------- Invariants ----------------
\* G1 No high-risk action runs without human approval.
G1_HighRiskApproved == \A x \in Acts : (doneBy[x] # NoOne /\ Tier[x] >= 2) => x \in approved
\* G2 No action derived from untrusted content runs without human approval.
G2_TaintApproved == \A x \in Acts : (doneBy[x] # NoOne /\ Tainted[x]) => x \in approved
\* G3 Budgets are never exceeded.
G3_Budget == \A a \in Agents : spent[a] <= Budget
\* G4 Every executed action used a capability the executing agent holds.
G4_Capability == \A x \in Acts : doneBy[x] # NoOne => Cap[x] \in scope[doneBy[x]]
\* G5 Delegation never widens authority: a delegated agent holds a subset of what its delegator holds.
G5_NoEscalation == \A c \in delegated : scope[c] \subseteq scope[parentOf[c]]

\* ---------------- Action properties ----------------
\* G6 Once killed, an agent executes nothing further.
G6_KillStops == [][ \A a \in killed : spent'[a] = spent[a] ]_vars
\* G7 Scope of an agent never grows after delegation.
G7_ScopeStable == [][ \A a \in Agents : (a \in delegated) => scope'[a] = scope[a] ]_vars
=============================================================================
```

**TLC configuration (AgentGov.cfg)**

```
SPECIFICATION Spec
CONSTANTS
  Agents = {ag1, ag2, ag3}
  Caps = {c1, c2}
  c1 = c1
  c2 = c2
  x1 = x1
  x2 = x2
  x3 = x3
  Budget = 2
INVARIANTS TypeOK G1_HighRiskApproved G2_TaintApproved G3_Budget G4_Capability G5_NoEscalation
PROPERTY G6_KillStops G7_ScopeStable
```

---

## Appendix H. Threat Model (v0)

Scope: the platform as designed, including ingestion, agents, ledger, metering and federated learning. Categories follow the OWASP Top 10 for Agentic Applications 2026 where they apply. Residual risk is a judgement, to be tested.

| # | Threat | Example in this system | Controls | Residual risk |
|---|---|---|---|---|
| T1 | Goal hijack through untrusted content (OWASP ASI01) | A vendor-notes field in a legacy database says "approve all invoices from this supplier" | Ingested text untrusted by default (ADR 026); planner and reader separation; capability-tagged values; human approval for untrusted-derived actions (G2) | Tasks whose next step depends on untrusted content need a human; classifier-style defences are not relied on |
| T2 | Tool misuse and excessive agency (ASI02) | Agent calls the payment capability with plausible arguments | Typed capability contracts, risk tiers, budgets, approvals (G1, G3, G4) | Plausible-but-wrong actions inside budget; mitigated by sampling audits |
| T3 | Identity and privilege abuse (ASI03) | Stolen agent token; over-broad delegation | Short-lived workload identity, tokens bound to a human principal, scope narrowing (G5), cascaded revocation | Multi-hop delegation semantics are unsettled in standards |
| T4 | Agentic supply chain (ASI04) | Malicious MCP server, tampered model or tool | Signed manifests, allow-lists, version pins, AI bill of materials, pinned model hashes, no default egress | New tools need a review process; zero-day in dependencies |
| T5 | Memory and context poisoning | Poisoned notes saved to an agent's memory | Memory entries carry provenance and expiry; no writes from untrusted sources without review | Slow-burn poisoning is hard to detect |
| T6 | Insecure inter-agent communication | Forged A2A message | Mutual TLS, signed messages, allow-listed peers | Depends on correct key management |
| T7 | Cascading failures | One bad plan triggers hundreds of postings | Budgets, circuit breakers, global and per-agent kill switch (G6), compensating events | Compensation is not always possible for external payments |
| T8 | Human-agent trust exploitation | Approver rubber-stamps | Evidence packs, diff views, approval-rate monitoring, random audits | Human factors remain |
| T9 | Rogue or drifting agent | Agent behaves outside its intended pattern | Behavioural monitoring on the metering ledger, kill switch | Detection latency |
| T10 | Poisoned or wrong mappings at ingestion | LLM proposes a plausible but wrong mapping | Constraint Oracle, Equivalence Oracle, human review of low confidence | Subtle wrong mappings that keep totals tied |
| T11 | Tampered Extraction Pack or connector | Altered script exfiltrates data | Signed packs, hash verification, customer review of scripts, read-only credentials | Customer review quality |
| T12 | Connector credential theft | Read account leaked | Least privilege, vaulting, rotation, network allow-lists, no credentials in logs | Customer-side hygiene |
| T13 | Ledger tampering or replay | Rewrite of history; duplicate posting | Immutable events, Merkle chaining, idempotency keys, Ledger Port conformance tests | Insider with storage access; mitigated by chaining and external anchoring |
| T14 | Metering fraud or disputes | Inflated outcome claims | Signed metering events with evidence pointers, replay | Disputes over interpretation, not evidence |
| T15 | Cross-tenant leakage or federated poisoning | Malicious tenant skews shared model | Cryptographic tenant isolation, secure aggregation, differential privacy, update clipping and robust aggregation, opt-in only | Open research area |
| T16 | Model exfiltration or data leakage via external models | Prompt contains confidential data | Local models by default, egress proxy, redaction, no secrets in prompts | Policy errors when external models are enabled |

NIST's red-teaming reported that novel attacks reach much higher hijack success than baseline attacks, so a static injection test set is insufficient (Section 8.5, item 7).

---

## Appendix I. Customer Discovery Kit (low-cost, founder-led)

**Why now.** Every major open question (source access, willingness to pay, legal posture, which beachhead) is answered fastest by conversations, not more desk research.

**Targets (first ten conversations)**
- Five organisations still running Maximo 7.6.x (look for administrators and reliability or maintenance managers in public IBM community discussions and professional networks).
- Two Caribbean public-sector bodies with asset registers or procurement pain (use the founder's existing government network).
- One utility or industrial operator with ageing assets.
- Two Maximo consultants or IBM business partners who run migrations (they know the real customisation burden and may be channel partners).

**Hypotheses to test**
- H1 Maximo 7.6 shops have not decided between MAS migration and alternatives.
- H2 Customisation refactoring is their largest fear and cost driver.
- H3 They would pay for an inventory and risk report before committing to anything.
- H4 They will allow read-only access to a database copy, or a customer-run extraction, under an NDA.
- H5 Public-sector buyers care more about local support, sovereignty and procurement compliance than about features.
- H6 Per-entity-and-asset-band pricing is acceptable; per-user is not welcome.
- H7 Agent-driven automation is welcomed only with approvals and audit trails.
- H8 Reference customers are the main purchase barrier for a new finance core.

**Interview guide (about 30 minutes, listen more than pitch)**
1. Tell me about your current EAM or ERP landscape and who looks after it.
2. What version and database? Hosted where?
3. What is your plan for support dates and licensing in the next 18 months?
4. What have you customised (code, scripts, workflows, integrations)? Who knows how it works?
5. What went wrong in the last upgrade or migration?
6. What would a failed migration cost you in a month-end or a shutdown?
7. How much do you spend yearly on licences, support and services?
8. Who decides, who influences, who can veto?
9. What would you need to see to trust a new system with your books and assets?
10. Would you share an anonymised schema or let your DBA run a read-only extract if we returned an inventory and risk report?
11. What would that report need to contain to be worth paying for?
12. How do you feel about AI agents acting in the system? What approvals would you require?
13. What would you not want us to touch?
14. Who else should I speak to?
15. If we did this well, what would you pay for it, and how would you want to be charged?

**Rules.** No confidential data in notes; ask permission before recording; do not promise features or dates; record every conversation in one table (role, system, version, pain, budget signal, access signal, next step).

**Success after ten conversations.** At least three organisations willing to share anonymised schema or a read-only extract; at least two stating a willingness to pay for the inventory and risk report; a clear ranking of H1 to H8; a first list of customisation patterns for the catalogue (M7). If H3 and H4 fail, rethink the Maximo beachhead before building.

**Using what you learn.** Feed the answers into the next round (pricing and pilot design), the private ingestion benchmark (anonymised schemas only), and the legal gate (which connector modes customers will actually accept).

### I.1 Outreach templates (edit to your voice; no confidential details)

**To a Maximo administrator or reliability manager (professional network message):** "Hello [name], I am researching how organisations running Maximo 7.6 are planning for support dates and the move to MAS. I am not selling anything yet. Would you be open to a 20-minute conversation about what is hardest, especially customisations and integrations? I will share what I learn from others in return."

**To a public-sector contact (email):** "Dear [name], I am designing an asset and procurement system that can read an existing system's data and run beside it without a disruptive cutover. I would value 30 minutes to hear how your team manages assets and procurement today and what has been hardest. No commitment is requested."

**To a Maximo consultant or IBM partner:** "Hello [name], I am building a tool that inventories Maximo customisations and migration risk from a read-only copy. As someone who runs migrations, would you tell me what you wish you had before starting, and would a report like that save you time?"

**Where to look:** public IBM Maximo community discussion threads, professional networks, local professional bodies, the founder's existing government contacts, and utility and industrial associations. Record each conversation in the table described above.

### I.2 Lead generation from public contract award lists

Governments publish awarded contracts, and these list the buyer, the vendor, the date and the amount. In Trinidad and Tobago the Ministry of Finance publishes contracts-awarded lists by quarter or year. Read them for lines such as "software support and licences renewal", "Oracle", "SAP", "Maximo", "ERP", "maintenance management" and note the agency, vendor, amount and date. Each line identifies an agency with a recurring vendor cost and a named incumbent or reseller. Treat it as a lead list to approach with the cost-displacement offer, never as a statement about the agency's satisfaction. Use only public documents, and keep the table in the discovery log.

### I.3 Pricing probe card (for each discovery conversation)

Ask for the report and for an annual subscription: at what price would this be so cheap you would doubt its quality; a bargain; getting expensive but still considered; too expensive to consider. Then record the respondent's role, organisation type and context. Do not state your own price.

---

## Appendix J. Ingestion Spike v0: Method and Results (synthetic data)

**Purpose.** Move forward without a customer schema by testing the central difficulty of direct-database ingestion: finding relationships when none are declared, and rejecting wrong mappings with an accounting oracle. **This is synthetic evidence built by us; it demonstrates a mechanism and shows where it is weak. It does not predict performance on real schemas (RK37).**

**Method.** A generator built a mini ERP and EAM database with 11 tables (locations, assets, items, vendors, accounts, work orders, purchase orders and lines, journal lines, labour, inventory), cryptic table and column names (T01 to T11, C1 onward) and no declared foreign keys. Eleven true foreign keys were recorded as ground truth. Discovery used inclusion dependencies (a child column's values contained in a unique parent column of the same type), with and without extra heuristics (the child must use a meaningful share of the parent's key space; ambiguous parents go to review). Two key styles were generated: prefixed text keys and plain numeric surrogate keys. A Constraint Oracle checked, per period, that signed journal amounts sum to zero under three candidate mappings of the debit and credit columns.

**Results**
- *Text keys (one run):* inclusion alone found all 11 foreign keys with no false positives, because key spaces did not overlap.
- *Numeric surrogate keys (30 random datasets):* inclusion alone had recall 1.00 but precision about **0.09** (roughly ten false relationships for each true one). With the extra heuristics, precision averaged **0.81** (range 0.71 to 0.85) and recall **0.96** (range 0.82 to 1.00).
- *Constraint Oracle:* both correct sign conventions left 0 of 12 periods out of balance; the plausible but wrong mapping (debit column only) left **12 of 12** periods out of balance and was rejected.

**Lessons**
1. Inclusion dependencies alone are unusable when keys are numeric surrogates, which is common in real ERPs. Declared metadata is therefore essential: this is direct evidence for the Dictionary Reader and source profiles (S16, ADR 018).
2. Heuristics help but still accept about one wrong relationship in five here; low-confidence and ambiguous items must go to review, and model-based enrichment must be judged by the oracles.
3. Accounting invariants make a strong oracle: a wrong amount mapping was caught in every period. Similar invariants exist for inventory, open items and sub-ledgers.
4. Real schemas add composite keys, soft deletes, polymorphic references, multi-currency, localised charts and years of customisation, so expect worse numbers until measured.

**Next step.** The FK-stripping benchmark on open-source schemas (Appendix C, spike 3) replaces synthetic data with real designs whose foreign keys are known.

---

## Appendix K. Counsel Brief (to hand to a lawyer or university legal clinic)

**Who we are and what we want to do.** Intellica AI Inc. (a Delaware C-corporation, based in Trinidad and Tobago) is designing software that helps organisations migrate from or run alongside existing enterprise systems (Oracle E-Business Suite and JD Edwards, Microsoft Dynamics, IBM Maximo). The customer would supply a copy of, or read-only access to, its own database, or run our read-only extraction scripts itself. We do not intend to copy vendor software, documentation or code.

**Connector modes.** M1 direct read-only access to the customer's database; M2 customer-run extraction pack (signed read-only scripts, customer's DBA runs them, files returned); M3 vendor-sanctioned exports or APIs; M4 restore into the customer's own licensed environment.

**Questions**
1. Oracle: where an EBS database is licensed as restricted-use, does reading it through our connector (M1), or the customer exporting through its own tools (M2), risk breaching the customer's licence or creating indirect-access exposure? Which export routes does Oracle's contract language treat as safe?
2. Oracle Fusion Cloud: are BICC extracts for third-party analysis or migration permitted under standard terms, and are there limits on bulk extraction or AI-agent use?
3. Microsoft: does reading a customer's SQL Server database behind Dynamics GP, NAV or AX with a third-party tool raise licensing issues (client access licences, processor licences)? Are Synapse Link or Fabric Link exports into customer storage free of restrictions for third-party processing?
4. IBM: does third-party read-only access to a Maximo database affect IBM support entitlements or licence terms, including where the database is a bundled restricted-use edition? Are there terms on Maximo SaaS read-only access?
5. Do our processing of customer database contents and our retention of derived, non-confidential mapping patterns (never customer data) require specific contract terms? What wording would we need?
6. Data protection: what applies in Trinidad and Tobago (Data Protection Act 2011, partly in force) and in customers' jurisdictions for cross-border processing during onboarding?
7. Open-source licensing: which licence structure (permissive, copyleft with commercial dual licence, or source-available) best protects an ingestion engine while keeping a proprietary corpus? Any contributor-licence terms we should require?
8. Contracts: what limitation-of-liability and warranty language should apply to migration and equivalence reports, given they inform accounting decisions?
9. Employment and IP: ownership of work produced by the founder and contractors; protecting the verified invariant library as trade secret or copyright.

**Documents to gather from a prospective customer:** its Oracle, Microsoft and IBM licence agreements and order forms; the database licence type for the source system; any audit correspondence; its data-protection policies.

**Status of our research.** Our sources on Oracle restricted-use terms are licensing-advisory firms with a commercial interest; we have not read Oracle's contract wording. Nothing in the arc42 is legal advice.

---

## Appendix L. Trinidad and Tobago Market Facts (v0, verify before use)

Grades as in the header. These facts come from official pages where noted and otherwise from advisers and payroll providers; rates change, so every item must be confirmed with the responsible authority before it enters a rule pack.

**Public procurement**
- The Public Procurement and Disposal of Public Property Act 2015 (as amended) was partially proclaimed on 31 July 2015 and fully proclaimed on 26 April 2023; the Office of Procurement Regulation (OPR) oversees it **[S, Ministry of Finance]**.
- "Public body" includes ministries, the Tobago House of Assembly, service commissions and state-controlled enterprises **[S, Act text]**.
- Suppliers must register in the Procurement Depository (Section 26) **[M, public notice]**.
- An amendment bill (Public Procurement and Disposal of Public Property (Amendment) Bill, 2026) was tabled in September 2026 and would also revoke the Simplified Procurement Regulations 2024 **[S, Parliament]**.
- The OPR is developing an electronic government procurement (e-GP) ecosystem; its ICT head spoke on it during Procurement Month 2026 **[M]**.

**Central government finance system.** The Ministry of Finance's 2025 contracts-awarded list records an IFMIS implementation contract with FreeBalance Inc. dated 25 August 2025 at US$2,000,399.63 (about TT$13,602,720), and the Q1 2024 list records FreeBalance sustainability services at US$928,125 **[S]**. Core central-government finance is therefore not an open target (RK40).

**Vendor renewals visible in public lists.** Annual Oracle software support and licences renewal for the Customs and Excise Division, awarded to PBS Technologies (Trinidad) Limited on 1 September 2025 at $1,275,519.23, shown without a currency marker (US dollar items on the same list are marked, so presumably TT dollars; about US$188,000 at roughly 6.8 TT dollars per US dollar) **[S, with the currency caveat]**.

**Tax and payroll (first localisation pack; **[M/V]**, confirm with the Inland Revenue Division and the National Insurance Board)**
- VAT standard rate 12.5%; registration above TT$600,000 of sales in 12 months; returns filed bi-monthly through e-Tax.
- Budget 2027 proposals (October 2026): mandatory electronic VAT filing through the GENTAX system and a flat TT$100 penalty for every two-month period a VAT return is outstanding, effective 1 January 2027; 12.5% VAT on digital and remote services from non-resident providers from 1 March 2027. These were proposals at the time of the sources.
- VAT on essential food items removed from 17 October 2025.
- National Insurance combined contribution 16.2% of insurable earnings from January 2026 (employee 5.4%, employer 10.8%), assigned by earnings class with a ceiling.
- Health Surcharge: fixed weekly amount, maximum TT$8.25 per week, deducted by employers.
- PAYE: two bands (25% and 30% above TT$1 million chargeable) with a personal allowance of TT$90,000 a year, per a payroll provider's summary.
- Monthly PAYE and Health Surcharge returns and remittances due by the 15th of the following month; TD4 certificates due by the end of February; employers can file annual PAYE returns and TD4 supplementary forms online through e-Tax.
- E-invoicing: described as a post-audit model with no mandatory format or infrastructure as of November 2025 **[V]**.

**Funding and programmes (eligibility unverified)**
- Innovation Challenge Facility (EU, IDB Lab, Government; executed by CARIRI): grants started at EUR 75,000 in an early call for SMEs registered and operating in Trinidad and Tobago; the second call (2023) funded 19 firms out of 67 eligible applicants from a US$7.5 million pool **[M]**.
- IDB TechLab launched in Trinidad and Tobago, January 2025 **[M]**.
- IDB Lab loan call for early-growth Caribbean businesses (US$500,000 to US$2 million), reported May 2026; Trinidad and Tobago eligibility to be confirmed **[M]**.
- IDB approved a US$90 million conditional credit line for Trinidad and Tobago's digital transformation, with a first loan of US$42 million **[M]**.
- Startup cloud-credit and support programmes: a Trinidad and Tobago firm received up to US$120,000 from Microsoft for Startups in 2021; current terms unknown **[M]**.

**How the rule pack uses this.** Rates, bands and thresholds are data in versioned rule packs, each tagged with a source, an effective date and a verification date; payroll classes and VAT cases form the Q33 test set; filing outputs target the e-Tax and GENTAX formats once the authorities publish them.

---

## Appendix M. Period-Close Kernel: Specification v0 and Checked Results

**Purpose.** The state machine that controls when accounting periods close, reopen and accept adjustments. It is where real ERPs most often lose audit trails, so it is specified and checked before any code exists.

**Rules**
- **P1** every period's debit and credit totals are equal.
- **P2** periods close strictly in sequence (a period can be closed only if all earlier periods are closed).
- **P3** a closed period was reconciled.
- **A1** (action property) while a period stays closed, its totals never change.
- **A2** a closed period leaves the closed state only through an approved reopen.
- **A3** a period can be reopened only if no later period is closed.
- Closing entries (accruals, revaluations) are allowed only while a period is closing, and they invalidate any earlier reconciliation. Adjustments to a closed period are posted to an open period with a back-reference (not modelled here; covered by the Lean lemma below).

**TLC results (2026-10-26).** Bounds: 3 periods, maximum amount 2, maximum per-period total 3. Base specification: 18,833 states generated, 4,160 distinct, no violation of the type invariant, P1 to P3, or A1 to A3. **Mutation check:** six seeded defects were each caught by the intended rule: posting into a closed period (A1, 675 states generated), closing without reconciliation (P3, 116), closing out of sequence (P2, 2,478), reopening without approval (A2, 610), an unbalanced posting (P1, 2) and reopening out of order (P2, 2,520). **Reachability probes** confirmed that a period can reach the closed state and that entries can be posted while a period is closing, so the base result is not vacuous (the Appendix G lesson applied).

**Limits.** Bounded model check; small constants; no multi-currency, consolidation, sub-ledger detail, time or concurrency between processes.

**Lean 4 closed-period lemmas (CHECKED, 2026-10-26).** Compiled with Lean 4.15.0, core library only, no errors; a false variant (claiming that posting into the same period leaves its total unchanged) is rejected. The lemmas show that appending entries from other periods never changes a closed period's total, and that an adjustment posted to a later period leaves the closed period's total unchanged.

```lean
-- Closed-period lemmas for the Truth Plane (Round 7; checked with Lean 4.15.0, core library only).

structure PEntry where
  account : Nat
  period  : Nat
  amount  : Int
deriving Repr

/-- Net amount of the entries that fall in period `p`. -/
def periodTotal (p : Nat) : List PEntry → Int
  | []      => 0
  | e :: es => (if e.period = p then e.amount else 0) + periodTotal p es

theorem periodTotal_append (p : Nat) (a b : List PEntry) :
    periodTotal p (a ++ b) = periodTotal p a + periodTotal p b := by
  induction a with
  | nil => simp [periodTotal]
  | cons e t ih => simp [periodTotal, ih]; omega

theorem periodTotal_other (p : Nat) (new : List PEntry) (h : ∀ e ∈ new, e.period ≠ p) :
    periodTotal p new = 0 := by
  induction new with
  | nil => rfl
  | cons e t ih =>
    have he : e.period ≠ p := h e (List.mem_cons_self e t)
    have ht : ∀ x ∈ t, x.period ≠ p := fun x hx => h x (List.mem_cons_of_mem e hx)
    simp [periodTotal, he, ih ht]

/-- Appending entries that belong to other periods never changes a closed period's total. -/
theorem closed_period_unchanged (p : Nat) (es new : List PEntry)
    (h : ∀ e ∈ new, e.period ≠ p) :
    periodTotal p (es ++ new) = periodTotal p es := by
  rw [periodTotal_append, periodTotal_other p new h]; omega

/-- An adjustment to a closed period is posted into a later period with a back-reference; it
    leaves the closed period's total unchanged. -/
theorem adjustment_leaves_closed_total (p q : Nat) (hq : q ≠ p) (es : List PEntry)
    (d c : Nat) (amt : Int) :
    periodTotal p (es ++ [⟨d, q, -amt⟩, ⟨c, q, amt⟩]) = periodTotal p es := by
  apply closed_period_unchanged
  intro e he
  simp at he
  rcases he with rfl | rfl <;> simpa using hq
```

**TLA+ specification (PeriodClose.tla)**

```tla
------------------------- MODULE PeriodClose -------------------------
\* Period-close state machine for the Truth Plane (Round 7 sketch).
\* Periods are numbered 1..N. Each period keeps balanced debit and credit totals.
\* Rules: close in sequence, only after reconciliation, closed periods are immutable,
\*        reopening needs a recorded approval and no later period may be closed.
EXTENDS Naturals

CONSTANTS N, MaxAmt, MaxTotal
Periods == 1..N

VARIABLES status,    \* status[p] in {"open","closing","closed"}
          d, c,      \* posted debit and credit totals per period
          recon,     \* recon[p] = TRUE once sub-ledgers were reconciled for p
          approved   \* set of periods for which a reopen was approved

vars == <<status, d, c, recon, approved>>

Init == /\ status = [p \in Periods |-> "open"]
        /\ d = [p \in Periods |-> 0] /\ c = [p \in Periods |-> 0]
        /\ recon = [p \in Periods |-> FALSE] /\ approved = {}

AllEarlierClosed(p) == \A q \in Periods : q < p => status[q] = "closed"
NoLaterClosed(p)    == \A q \in Periods : q > p => status[q] # "closed"

\* A balanced posting into an open period.
Post(p, amt) ==
  /\ status[p] = "open" /\ amt \in 1..MaxAmt /\ d[p] + amt <= MaxTotal
  /\ d' = [d EXCEPT ![p] = @ + amt] /\ c' = [c EXCEPT ![p] = @ + amt]
  /\ UNCHANGED <<status, recon, approved>>

\* A balanced closing entry (accrual, revaluation) allowed only while closing.
PostClosing(p, amt) ==
  /\ status[p] = "closing" /\ amt \in 1..MaxAmt /\ d[p] + amt <= MaxTotal
  /\ d' = [d EXCEPT ![p] = @ + amt] /\ c' = [c EXCEPT ![p] = @ + amt]
  /\ recon' = [recon EXCEPT ![p] = FALSE]        \* new entries invalidate reconciliation
  /\ UNCHANGED <<status, approved>>

BeginClose(p) ==
  /\ status[p] = "open" /\ AllEarlierClosed(p)
  /\ status' = [status EXCEPT ![p] = "closing"]
  /\ UNCHANGED <<d, c, recon, approved>>

Reconcile(p) ==
  /\ status[p] = "closing" /\ ~recon[p]
  /\ recon' = [recon EXCEPT ![p] = TRUE]
  /\ UNCHANGED <<status, d, c, approved>>

Close(p) ==
  /\ status[p] = "closing" /\ recon[p] /\ AllEarlierClosed(p)
  /\ status' = [status EXCEPT ![p] = "closed"]
  /\ UNCHANGED <<d, c, recon, approved>>

ApproveReopen(p) ==
  /\ status[p] = "closed" /\ p \notin approved
  /\ approved' = approved \cup {p}
  /\ UNCHANGED <<status, d, c, recon>>

Reopen(p) ==
  /\ status[p] = "closed" /\ p \in approved /\ NoLaterClosed(p)
  /\ status' = [status EXCEPT ![p] = "open"]
  /\ recon' = [recon EXCEPT ![p] = FALSE]
  /\ approved' = approved \ {p}
  /\ UNCHANGED <<d, c>>

Next == \E p \in Periods : \/ \E amt \in 1..MaxAmt : Post(p, amt) \/ PostClosing(p, amt)
                           \/ BeginClose(p) \/ Reconcile(p) \/ Close(p)
                           \/ ApproveReopen(p) \/ Reopen(p)

Spec == Init /\ [][Next]_vars

TypeOK == /\ status \in [Periods -> {"open","closing","closed"}]
          /\ d \in [Periods -> Nat] /\ c \in [Periods -> Nat]
          /\ recon \in [Periods -> BOOLEAN] /\ approved \subseteq Periods

\* P1 Conservation per period (debits equal credits).
P1_Balanced == \A p \in Periods : d[p] = c[p]
\* P2 Periods close strictly in sequence.
P2_Sequential == \A p \in Periods : status[p] = "closed" => AllEarlierClosed(p)
\* P3 A closed period was reconciled.
P3_ClosedReconciled == \A p \in Periods : status[p] = "closed" => recon[p]

\* ---------------- Action properties ----------------
\* A1 Closed periods are immutable while they stay closed.
A1_ClosedImmutable == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] = "closed") => (d'[p] = d[p] /\ c'[p] = c[p]) ]_vars
\* A2 A closed period only becomes open again with a recorded approval.
A2_ReopenApproved == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] # "closed") => (p \in approved /\ status'[p] = "open") ]_vars
\* A3 No later period is closed when an earlier one is reopened.
A3_ReopenOrder == [][ \A p \in Periods :
      (status[p] = "closed" /\ status'[p] = "open") => (\A q \in Periods : q > p => status'[q] # "closed") ]_vars
=============================================================================
```

**TLC configuration (PeriodClose.cfg)**

```
SPECIFICATION Spec
CONSTANTS
  N = 3
  MaxAmt = 2
  MaxTotal = 3
INVARIANTS TypeOK P1_Balanced P2_Sequential P3_ClosedReconciled
PROPERTY A1_ClosedImmutable A2_ReopenApproved A3_ReopenOrder
```

---

## Appendix N. Maintenance Scheduling Spike v0 (synthetic data)

**Purpose.** Check that a constraint-programming scheduler can sit inside the Maintenance Decision Engine and see how much a simple dispatch rule leaves on the table. **Synthetic evidence only (RK37, RK47).**

**Method.** A generator produced work-order sets with skills, durations, criticality weights, release times (parts and permits), due dates, precedence pairs and four shutdown-window jobs that must run inside a fixed window, over six crews with different skill sets. The baseline was a weighted earliest-due-date dispatch rule (window jobs first) assigning each job to the crew that finishes it earliest. The challenger was Google OR-Tools CP-SAT (optional intervals per eligible crew, no-overlap per crew, precedence and window constraints) minimising weighted tardiness, with a 15-second limit and 8 workers.

**Results**
- *Light instances (80 work orders, 20 runs):* CP-SAT proved zero weighted tardiness in about 0.1 seconds on every instance, while the dispatch rule left positive tardiness on all of them.
- *Heavy instances (170 work orders, 12 runs):* CP-SAT reduced weighted tardiness against the dispatch rule by **93% on average (range 68% to 100%)**; it proved optimality in 1 of 12 within 15 seconds, and its lower bound stayed at zero, so its optimality gap was uninformative.
- *A lesson recorded:* the first generator produced infeasible instances in about half of the runs (windows that could not be met); test generators must be checked for feasibility, like specifications must be checked for vacuity.

**Interpretation.** The result shows headroom over a simple rule and that constraint programming handles skills, windows, precedence and parts constraints with no special effort. It does not show real-world gains: the baseline is deliberately simple, there are no shift calendars or travel times, real planners use better heuristics, and the data is synthetic. Real evaluation uses the Business Twin on the customer's own history and the planner's actual schedules.

**Next steps.** Add shift calendars, travel, multi-skill crews, criticality from risk estimates, spare-parts availability and uncertainty in durations; compare against a stronger heuristic and against recorded human schedules; study joint optimisation with spare-parts stocking.
