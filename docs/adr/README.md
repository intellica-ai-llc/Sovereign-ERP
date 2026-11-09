# Decision records

Format: [MADR](https://adr.github.io/madr/). One file per decision, never rewritten after acceptance; supersede with a new record.

| Series | Scope | Where |
|---|---|---|
| `ADR nnn` | Product architecture | arc42 section 9 (to be split into files here) |
| `eng-nnnn` | Repository, tooling, process | this directory |

| Record | Title | Status |
|---|---|---|
| [eng-0001](eng-0001-record-engineering-decisions.md) | Record engineering decisions | accepted |
| [eng-0002](eng-0002-repository-layout.md) | Repository layout | accepted |
| [eng-0003](eng-0003-dependency-policy.md) | Dependency policy | accepted |
| [eng-0004](eng-0004-quality-gates-and-verification.md) | Quality gates and verification in CI | accepted |
| [eng-0005](eng-0005-licence-format-and-verification.md) | Licence format and verification | accepted |
| [eng-0006](eng-0006-pilot-sku-and-first-source.md) | Pilot SKU, first source and platform | accepted |
| [eng-0007](eng-0007-dictionary-first-discovery.md) | Dictionary-first relationship discovery | accepted |
| [eng-0008](eng-0008-inference-tier-method-and-evaluation.md) | Inference tier: method and evaluation protocol | accepted |
| [eng-0009](eng-0009-weight-profiles-and-cross-corpus-evaluation.md) | Per-source weight profiles and cross-corpus evaluation | accepted |
| [eng-0010](eng-0010-reconciliation-audit-and-assurance-positioning.md) | Reconciliation, audit and migration-assurance positioning | accepted |
| [eng-0011](eng-0011-offline-sync-domain-crdt.md) | Offline sync as a domain-specific, verified state-based CRDT | accepted |
| [eng-0012](eng-0012-mobile-delivery-pwa-first.md) | Mobile delivery: installable web app first, native shells later | proposed |
| [eng-0013](eng-0013-toolchain-bump-to-1-99.md) | Pinned toolchain raised to Rust 1.99.1 | accepted |
| [eng-0014](eng-0014-in-house-sha256.md) | SHA-256 implemented in-house, hashing only | accepted |
| [eng-0015](eng-0015-evaluation-build-and-distribution.md) | The evaluation build and how it is distributed | accepted |
