# Mobile, offline and connectivity: competitors, state of the art, and where we differ

Status: 2026-11-02 (round 21). Evidence labels as in `competitive-landscape.md`: *vendor* = vendor or vendor-written comparison; *independent* = neutral source; *inferred* = my reasoning. Nothing here was tested hands-on against a competitor app. Adoption percentages in vendor comparisons are marketing and are not repeated as facts.

## What field-mobile competitors ship

| Capability | Who advertises it | Evidence |
|---|---|---|
| Offline work orders, offline parts lookup, auto-sync on reconnect | MaintainX, UpKeep, Limble, Fiix and Maximo all list some form; depth differs, and a vendor-written comparison says Maximo's and others' offline is "partial" | *vendor* (comparison written by a competing vendor, which ranks itself first) |
| QR and barcode scanning, photo and voice attachments, push alerts, rugged-tablet support | the same group | *vendor* |
| AI camera or visual inspection | IBM Maximo Visual Inspection with local inference; several challengers | *vendor* (IBM 9.2 announcement) |
| Natural-language assistant on the phone | IBM "Maximo Assistant on Mobile" (find asset information, review history, complete work); Fiix has an AI assistant | *vendor* |
| AI document abstraction (manuals, procedures) | IBM 9.2 | *vendor* |
| PLC or sensor alerts to the phone | challenger CMMS vendors | *vendor* |
| Listed starting prices (per user per month): MaintainX 25, Fiix 45, UpKeep 20; Maximo listed from about 3,150 per month | GetApp listings | *vendor-supplied* listings; treat as indicative |

Reading of the field *(inferred)*: everyone competes on *feature breadth* and *ease of use*. Nobody I found competes on **provable correctness of offline behaviour**, on **sovereign operation** (no cloud, no telemetry), or on **evidence a third party can re-check**.

## The sync state of the art

- Local-first tooling is mature and fashionable in 2026 (Automerge, Yjs, Ditto, PowerSync, ElectricSQL, Zero, Triplit; a conference room devoted to it). *(independent, vendor blogs)*
- They are **generic**: documents, tables or text. A vendor blog states the classic hazard plainly: with one status field, concurrent updates let one action win and the other is lost. *(vendor)*
- One project mentions TLA+ for its own protocol; none found ties convergence to **field-work business rules**. *(inferred from absence)*
- Several are hosted or commercial, which sits badly with sovereign and air-gapped buyers. *(inferred)*

## What we built this round (`crates/sov-sync`, ADR eng-0011)

A domain-specific state-based CRDT for work orders: lifecycle as a monotone register with reopen epochs, immutable add-only labour and material entries (corrected by voiding, like the ledger), fields kept as a set of writes so none is lost and concurrent edits are surfaced, and conflicts that need a person computed from state so every replica agrees on them.

| Claim | Evidence |
|---|---|
| Merge is commutative, associative, idempotent | checked on 293 to 1,172 distinct reachable documents (exhaustive pairs; triples sampled at 400,000) |
| Every operation only moves a replica upward; replicas that exchange state converge | checked on every one of 6,005 reachable states of two replicas, 12,282 transitions |
| Nothing lost under loss, duplication and reordering | 300 seeded five-replica histories plus 200 with two and three replicas: every entry survives, every void stays, wire format round-trips |
| State depends only on which operations were seen | TLA+ `SyncMerge`, TLC: 9,018 distinct states for three replicas (and 567,584 for two replicas with three operations each); three deliberate breaks of the merge each caught |
| Conflicts are a function of state alone | exploration test, including both conflict kinds being reachable |

Honest limits: small bounds; one aggregate (the work order); whole-document exchange; no network, storage or authentication layer yet; never run on a phone.

## Features ranked by moat and feasibility

| Rank | Feature | Why it separates us | Status |
|---|---|---|---|
| 1 | **Provably convergent offline work** with deterministic conflict surfacing | Nobody found offers it; it is exactly what field crews and auditors fear | core built and verified |
| 2 | **Tamper-evident field evidence**: each entry signed by the device, canonical encoding hashed into a chain, verifiable offline by an auditor | Ties mobile capture to the evidence report; competitors' photos and signatures are attachments, not checkable records | encoding built; signing needs the crypto dependency (eng-0003, eng-0005) |
| 3 | **Sovereign relay**: the server is just another replica that stores and forwards encoded documents and never interprets them; no cloud account, no telemetry | Palantir, IBM cloud and hosted sync engines cannot match it cheaply | designed |
| 4 | **Interest-set replication**: each device holds only its site's and crew's work orders (one document per work order makes this natural), so offline caches are small and data exposure is limited | Smaller attack surface and faster first sync | designed |
| 5 | **AI that proposes and the kernel disposes**: on-device or local models suggest an entry, part or fault, and deterministic checks (asset matches the scanned tag, part exists, quantity within bounds, status legal) decide what is recorded | IBM ships assistants and local visual inspection; ours adds a verified acceptance layer so a model mistake cannot corrupt records | designed (arc42 P4); needs a model-serving decision |
| 6 | Offline QR and NFC scanning with a cached asset index | Table stakes; needed to be credible | not built |
| 7 | Voice capture into entries; sensor alerts to the phone | Table stakes to differentiators depending on connectors | not built |

## Delivery path (ADR eng-0012, proposed)

Installable web app first, with the Rust core compiled to WebAssembly, so a pilot needs no app-store approval; native shells later over the same core. A spike must prove it on real phones, including a full offline shift and forced storage eviction. The build environment here has no WebAssembly target, so nothing about the browser has been tested.

## Risks specific to this area

- **Browser storage can be evicted and iOS limits background work**; the design must treat the server replica as the safety copy and sync eagerly.
- **Every new aggregate needs its own lattice and its own verification** (asset records, inventory counts, purchase orders). The work order is one case; inventory counts in particular are not a naive max or union.
- **A hosted competitor can add TLA+-style claims in marketing.** Our advantage holds only while the claims stay checkable by a customer: the proofs, tests and evidence format must stay open to inspection.

## Next research (not done)

Delta encoding (send only what the other side lacks); a lattice for inventory counts and stock transfers; device-key attestation and remote wipe; local model options for on-device inference and what hardware pilots actually own; what auditors accept as field evidence.
