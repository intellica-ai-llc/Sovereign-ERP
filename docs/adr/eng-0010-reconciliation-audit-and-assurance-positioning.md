# eng-0010: Reconciliation, audit and migration-assurance positioning

- Status: accepted
- Date: 2026-11-01

## Context
Two discovery tiers (eng-0007) and a measured inference method (eng-0008, eng-0009) produce separate answers. A dictionary says which tables navigate to which but not which side holds the key; inference finds undeclared links. Competitor research (`docs/research/competitive-landscape.md`) found data-diff tools that prove rows match and large vendor programmes that validate toward their own targets, but no target-agnostic, independently checkable proof that business state survived a migration.

## Decision
- **One reconciled answer.** `sov-profiler::reconcile` resolves every dictionary link against the data (which side holds the key, one-to-one or many-to-one, orphan rows), matches it with inferred links, and reports provenance (`Dictionary`, `Inferred`, `Both`). Direction comes from which side is a unique key; when the data cannot tell (a true 1:1), tables that other relationships point at break the tie, and if that fails the link says so (`direction_certain = false`). Orphan rows are a finding, never a reason to hide a link.
- **Audit before replay.** `sov-profiler::audit` re-derives stored balances from transactions in exact fixed-point arithmetic and reports drift. It is the first slice of equivalence checking; full replay through the verified ledger and period-close kernel follows.
- **Positioning.** The pilot instrument (eng-0006) is sold as **migration assurance**, target-agnostic: valid for an upgrade to IBM's suite, a move to SAP, or a move to our platform. Replacement is a later upsell, not the entry point.
- **Evidence format is deferred** until the hashing and signature dependency is approved (eng-0003, eng-0005); meanwhile `Findings` and `AuditReport` are deterministic and sorted so a report is reproducible.
- **Synthetic data is labelled.** `sov-profiler::synth` generates a Maximo-shaped database from documented conventions with ground truth by construction. Results on it test consistency of the machinery, never real-world accuracy, and every document that quotes them says so.

## Consequences
The product's differentiator becomes checkable evidence, which needs the signed report format next. Known gaps found while building this (role-named references, composite weights, three-column keys, scale) are recorded in the research note and tracked as work, not hidden.
