# VeriChain — Roadmap

Rule: design gates first; no code before the G1 spec is approved by Damain.

## Design and build gates (platform; full scope per ARC42 v9 Appendix F)
| Gate | Scope | Done when |
|---|---|---|
| G0 | Repo reset, standard files, SOURCES | done |
| G0.5 | Engineering factory ready: roles, harness, CI gates, traceability, acceptance bundles (ARC42 v9 App. G) | **DONE 2026-10-24**: independent vector author + adversarial verifier found 15 defects (2 high); all fixed; differential vectors agree; bundle `factory/bundles/g05-l0-crates.json` |
| G1 | Spec freeze: schema, vectors, requirements, threat model | **FROZEN 2026-10-24 (owner approved; tag `g1-freeze`)**: `spec/statement.schema.json`, `spec/vectors/`, `spec/requirements.md`, `docs/THREAT_MODEL.md` |
| G2 | Tile log + `vcd` sequencer + checkpoints | crash/limits tests; <1 GB RAM |
| G3 | Witnesses + split-view detection | ≥2 independent witnesses |
| G4 | Anchors + COSE receipts + verifier CLI/WASM + evidence packs | offline verify; interop vector |
| G5 | L1: identity, delegation chains, KMS, PQ hybrid, rotation | Q7, Q12 |
| G6 | Gateway (C1) + signer (C2) + SDKs | Q5 |
| G7 | Hosted runtime: WASM sandbox, host ABI, vault, microVM backend | Q10, Q11 |
| G8 | Assurance: policy engine, replay, attestation (A3 per R2) | Q13 |
| G9 | BFT ordering + federation (R1, R3) | Q14, Q15 |
| G10 | Economy: payments, job escrow, reputation (legal L-1; market/token L-2/L-3) | Q16 |
| G11 | Governance, compliance, console, ops | Q17 |
| G12 | Hardening: external audit, game days, launch | Q18 |

## Commercial track (hypotheses; see addendum D)
| Step | Goal | Kill / continue |
|---|---|---|
| M0 | 10 design-partner conversations (banking/fintech, agent platforms, an insurer, an auditor) | ≥3 willing to pilot, or revisit thesis |
| M1 | Open-source spec + verifier published; reference vectors | external implementation verifies a receipt |
| M2 | First paid pilot of hosted log + evidence pack | signed pilot |
| M3 | Compliance mappings (EU AI Act Art. 12, NIST agent guidance, SEC/FINRA) | reviewed by a practitioner |
| M4 | Standards participation (SCITT agent-receipt profile) | draft or interop test accepted |

## Financial objectives v2 (assumptions; see addendum J §3)
| Horizon | Objective | Kill / pivot |
|---|---|---|
| M3 | ≥10 conversations, ≥3 willing to pilot | <3 → revisit segment |
| M6 | verifier published; ≥2 paid pilots; AARM mapping submitted; ≥1 external witness operator | 0 paid → thesis review |
| M9 | hosted runtime beta with ≥2 design partners | none → revisit runtime go-to-market |
| M12 | base ≈ US$500k ARR (evidence 250k + runtime 180k + packs 96k); stretch ≈ US$1.2M | <US$100k → change segment/mix |
| Trading | learning goals only; capital cap per MAS M11 | gate failure → stop strategy |

## MAS track — see `docs/arc42/MAS_ARC42_v4.md`
| Gate | Deliverable | Done when |
|---|---|---|
| T0 | Data pipeline + simulator; experiment registry; DSR/PBO tooling | reproducible backtest; validated against known fills |
| T1 | `mas-risk` + `mas-exec` + `vc-signer` integration (paper, evidence emitted) | 0 unevidenced orders; kill switch tested (needs platform G6) |
| T2 | LP-1 paper ≥ 60 days | markouts/fills within tolerance |
| T3 | Legal gate + micro-live ≥ 90 days | clean reconciliation; Sharpe CI lower bound > 0 |
| T4 | Scale in steps; add FC-1; move agents to hosted runtime (C3) | capacity tests; platform G7 |
| T5 | Research strategies (LP-2, AR-1, AR-2, CR-1) through S0–S2; additional venue adapters behind legal gates | per-strategy gates |
| T6 | External-capital mandates via jobs/marketplace | S4 + legal L-1/L-2 + platform G10 |
Rule: company revenue never depends on MAS P&L (Addendum H).

## Update 2026-10-31
- G2/G4 progress: vc-store, vc-seq verified; vc-sdk, vc-verify built and reviewed once (see `docs/reports/verifier_w1_store_seq.md`).
- vc-witness built (C6, tlog-cosignature/v1): persist-before-cosign, rollback refusal, equivocation freeze with signed-note evidence, exact-signed-text cosigning (self-review finding fixed). 11 tests. Independent vector author + adversarial verifier still to do.
- vc-server (hosted multi-tenant log + console, production slice 1) built: tenants with hashed API keys, per-tenant durable sequencer, ingest, checkpoints, inclusion proofs, dashboard API and console; 3 integration tests. Next: witness service, pack export, quotas, accounts, metrics.
- vc-server: evidence-pack export (/v1/pack) and trust anchors (/v1/trust). Finding: vc-verify accepts plain witness signatures only, while vc-witness issues C2SP tlog-cosignature/v1 lines; the verifier needs cosignature/v1 support (top next item), after which witnessed packs from a hosted log pass end to end. Unwitnessed packs correctly fail the witness-quorum check.
- Round 21: vc-log verifies tlog-cosignature/v1 witness lines (own key-id type, timestamped message; cannot stand in for the operator signature); vc-server adds witness registration, /v1/cosignatures, /v1/checkpoint/consistency and witnessed pack export (end-to-end test: witnessed pack verifies; unwitnessed fails closed). Addendum M: competitor map, moat ranking, mobile plan. Correction: update 20 shipped with a failing server test (fail-closed assertion did not apply); fixed here.
- Round 22: vc-verify check approval_order (separation of duties judged by log order: outcome needs earlier approval from a different agent); 4 new tests in vc-pack. Next: pinned policy for which intents must be gated, action-hash binding, passkey approval service and phone UI, witness daemon, vc-anchor.
- Round 23: policy gated_classes; passkey approval service + /approve phone page (software authenticator and real Chromium virtual authenticator tests); vc-witnessd daemon + witness-loop.sh (real-process smoke: witnessed pack passes vcv); vc-core ChainState::next_for. Next: offline passkey verification in vc-verify, vc-anchor, per-user accounts, WASM verify.
- Round 24: mas-research gains implication.rs (implication arbitrage), gate.rs (Hoeffding selective-prediction gate), quoting.rs fixes (inventory normalised by limit, post-only clamp), plus previously unshipped arb.rs and eprocess.rs; 73 tests, clippy clean. Addendum I round 7 records the breakout thesis and hypotheses H6-H10.
- Round 25: mas-research gains growth.rs (drawdown-bounded Kelly multiple, growth calculator, uncertainty shrink; Monte Carlo test of the bound) and shield.rs (deterministic runtime risk shield, 20,000-case invariant test); 83 tests, clippy clean. Addendum I round 8: aggression bounded by evidence, AI kept outside the enforcing loop, capacity as the real wall. Unread (rate-limited): arXiv 2609.19705, TradeTrap.
- Round 26: new crate mas-data (strict Polymarket book and event parsers, tamper-evident recordings, rebalancing scan, CLI discover/record/verify/scan, fake-server end-to-end test). Next: run the recorder in a Codespace, add a trades recorder for fill replay, curated implication-chain file, then quote-engine replay (H6/H7).
- Round 22: `vcw` witness client (keygen, run loop, file-backed durable state); real-HTTP test with two independent witnesses incl. a consistency-proof round; pack follows the newest fully witnessed checkpoint. Follow-up: scoped witness tokens.
- MAS T0 started: `crates/mas-research` (Sharpe/PSR/DSR/min track record, hash-chained experiment registry).
- MAS Addendum I (verified performance as the headline) proposed in `docs/arc42/MAS_ADDENDUM_I_VERIFIED_PERFORMANCE.md`.
- Round 27: Verified Performance Card (mas-research::card), claim standard + challenge; see Addendum I round 10.
- Round 28: paper engine + independent audit (mas-research::paper), mas-data trades recorder; Addendum I round 11.
- Round 29: collector runs from the console (mas-data::runner, vc-server trading routes/panel); card no longer proves anything on an empty ledger.
- Round 30: pilot hardening: failed-auth throttle, deploy/ (systemd, env, backup), PILOT_RUNBOOK and PILOT_GAPS.
- Round 31: final pre-checkpoint hardening: RFC 3161 anchoring, admin audit log, /readyz, paper replay in console, verify-all.sh, DILIGENCE.md.
