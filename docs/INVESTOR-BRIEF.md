# Sovereign-ERP: investor brief (evaluation checkpoint, 0.1.0)

*Evidence labels: **built** = exists in this repository and is tested; **measured** = scored against ground truth; **vendor** = a vendor's own statement; **inferred** = our reasoning. Items in [brackets] are for the founder to complete; they are left blank on purpose rather than guessed.*

## The problem
Organisations running IBM Maximo and similar systems face costly, risky migrations. Maximo's database declares no foreign keys; its relationships live in an application dictionary, so understanding a system well enough to move it is slow, manual work. Existing validation tools prove that rows match; they do not show that business state survived. *(inferred from competitor review: `docs/research/competitive-landscape.md`)*

## Why now
- IBM's Maximo 7.6.x extended support was due to end on 30 September 2026 and dual-support arrangements end on 30 April 2027, so migrations are being decided now. *(vendor, IBM community announcement)*
- SAP, Palantir and Accenture announced AI-assisted ERP migration, with pilots in 2026 and wider rollout in early 2027, aimed at SAP targets. *(vendor)* A neutral instrument for any destination is room we believe they do not fill. *(inferred)*

## What we have, with evidence
| Capability | State |
|---|---|
| Recover undeclared relationships from data | **measured**: held-out precision 0.86 / recall 0.82 on a real ERP; mean F1 0.86 across four databases; about 0.70 with meaningless names; the old heuristic scores 0.34 on the same data |
| Read Maximo's relationship dictionary and say what cannot be read | **built**, tested on generated and adversarial inputs |
| Resolve key direction, cardinality and orphan rows | **built**, synthetic database |
| Re-derive stored balances from transactions | **built**, exact on a synthetic database |
| Verified period-close and ledger rules | **built** and model-checked, independently re-run |
| Offline field work that provably converges | **built** and model-checked, never on a device |
| A command-line evaluation tool and reproducible reports | **built**, tested, deterministic |
| Dependency-free, auditable core; one-command setup | **built** |

## What does not exist yet
A real Maximo database has never been run through the tool (the largest unknown). No live-database connectors, service, web or mobile apps, licence enforcement, signed reports, authentication, or agent (LLM) gateway. See `docs/STATUS.md`; it is the authoritative statement.

## Strategy
Sell the instrument before the destination: **migration assurance** that works for any target, including IBM's own, then replacement as the later upsell. Differentiators we are building toward: checkable evidence (signed, content-addressed reports an auditor can re-run), business-state equivalence (not data parity), sovereign operation (one static binary, no telemetry, offline licence verification), and formally checked offline field work. Research on how enterprise AI agents should safely act on an ERP (capabilities, information-flow control, kernel-enforced invariants) is written up in `docs/research/agents-and-erp.md`; the gateway is designed, not built.

## Risks (read these)
Larger players could add proof-grade validation. Formal methods mean little to buyers until they become an artefact an auditor accepts. All results so far use synthetic or open-source data. Execution depends on a very small team. Legal review of the pilot agreement, vendor-schema handling and data protection has not happened.

## Roadmap
Real Maximo schema through the tool; signed releases and licence verification; security review; pilot agreement; a first design-partner pilot. Detail and exit criteria: `docs/roadmap/pilot-readiness.md`.

## For the founder to complete
Team and advisors: [ ]. Traction, conversations and letters of intent: [ ]. Market size with sources: [ ]. Business model and pricing (parked by decision): [ ]. Funding sought and use of funds: [ ]. Legal entity and IP assignment status: [ ].
