# eng-0008: Inference tier: method and evaluation protocol

- Status: accepted
- Date: 2026-10-30

## Context
eng-0007 puts statistical discovery second, behind the source's own dictionary. It needs a method that is deterministic, explainable to a reviewer, and measured honestly. Literature: HoPF (joint primary and foreign key detection, recall-first), Auto-BI (local join prediction then global schema-graph resolution), and a 2026 study showing primary-key detection collapsing on hard schemas such as TPC-DS. arc42 Appendix J measured only a synthetic database.

## Decision
- **Method:** exact inclusion dependencies into unique columns and column pairs (tolerating a few orphans), scored by an explicit linear model over named features, resolved per child column set, with close runners-up flagged ambiguous for human review. No model-based component in the decision path. Provenance on every link.
- **Weights are data**, fitted by coordinate descent on training modules and recorded in `Weights::default()` with the evidence in `docs/research/fk-discovery.md`.
- **Protocol:** train and test are split by ERP module; headline numbers are held-out and use the conservative ground truth; recall is reported against what the data supports and against everything with data; a names-off ablation is reported beside every result; a benchmark fidelity problem found after seeing results is fixed and disclosed, and results that depended on it are not reused.
- **Gate:** `cargo xtask bench-fk` runs in CI at a pinned `OFBiz` commit and fails below held-out precision 0.80 or recall 0.80 (arc42 Q31). The thresholds sit below the measured 0.84 and 0.86 to absorb noise, not to excuse regression.
- **Benchmark tooling is Python and an example binary** (`bench/`, `examples/`), kept out of the product crates and out of the dependency policy's scope because it ships nothing; the data is downloaded at run time and never committed.

## Consequences
Every number in arc42 about discovery quality now has a reproducible source. The method is not state of the art; it is a measured baseline with a clear list of what to try next. Transfer to Maximo remains unmeasured until a Maximo-shaped corpus exists.
