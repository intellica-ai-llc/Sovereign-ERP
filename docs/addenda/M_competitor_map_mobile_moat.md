# Addendum M: competitor map (2026-11-01), mobile, and moat features

Sources: Provenant competitive map (github.com/JaredKlopstein/provenant issue 168), IETF draft-noa-scitt-ai-agent-receipt-01, a passkey-approval write-up on dev.to (goodroom). Competitor claims are as published by them or by that map; not independently tested.

## Who is out there and what they admit
| Product | Model | Stated gap we beat |
|---|---|---|
| Agent Receipts / Obsigna | W3C VC + Ed25519 + hash chain | no log, no witnesses, own envelope |
| agent-custody | Cedar policy then Merkle log; free 10k appends/mo, then US$50/mo per 1M | SDK path self-attested |
| Pipelock | firewall emitting signed decision receipts, US$49/mo to US$25k/yr | receipts are a side feature |
| Action State EvidenceBook | COSE + RFC 9162 log, free public witness of digests | a valid receipt does not prove completeness |
| Asqav | ML-DSA-65 signer, OpenTimestamps, RFC 3161 | its RFC 3161 token is signed by Asqav itself |
| ExecLayer | pre-execution authority receipts, US$10k to 30k per month | a governance kernel, not a log |
| CertifiedData | payment receipts, from US$49/mo | payments only |
| AI Identity | HMAC chain, session attestations US$79 to 599/mo | session level, not per action |
| Noa SCITT draft | JCS + Ed25519 + COSE | issuer-chosen timestamps; tail deletion invisible without checkpoint; no non-equivocation; admits it cannot show approval preceded action |
| Sello (paper) | receiver-attested receipts | research only |

## What nobody has together (our moat, in order of strength)
1. Completeness: epoch seals plus witnessed checkpoints expose a hidden tail. Others say "no absolute guarantees" or "cannot detect".
2. Non-equivocation: independent witnesses, cosigned in the C2SP format, with signed split-view evidence. Done in vc-witness; verifier accepts cosignatures (round 21).
3. Order by log position, never by claimed time: "approval before action" is provable from log order (Noa admits it cannot).
4. Verification never needs us: offline `vcv`, open spec, CC0 vectors.
5. Receiver-attested receipts (Sello idea): the venue or counterparty signs what it saw (attestation kind) so a lying agent cannot hide a fill. Planned: C2 venue attestations for the MAS signer.

## Gaps to close next (ranked)
G-A real third-party time: vc-anchor with RFC 3161 from independent TSAs plus OpenTimestamps (Asqav's TSA token is self-signed; ours must not be).
G-B approvals: passkey-backed human approval bound to the SHA-256 of the canonical action, recorded as an `approval` statement; mobile web app (PWA) approves in two taps.
G-C witness daemon with the add-checkpoint HTTP API, plus a `vcw` helper that runs the fetch, cosign, post loop.
G-D interop adapters: import Agent Receipts, export SCITT receipts and AAT sessions, OpenTelemetry to statements.
G-E pricing anchor: free tier at least as generous as agent-custody (10k statements/mo); paid tiers by retained evidence and witnesses, not by seat.
G-F pack export in the console (download and "verify in browser" via WASM).

## Mobile
- Phase 1 (cheap): the console is already responsive; add a PWA manifest so it installs on a phone home screen.
- Phase 2: push-style approvals: an `approval` request appears in the PWA, the user approves with a passkey (WebAuthn user verification), the server records a signed approval statement referencing the action hash. The signature covers action hash, audience, risk level, unique id and a short validity window; a reused or mismatched proof is rejected (pattern from the passkey write-up; their limits: only as strong as the enforcement point, and the executed payload must hash identically).
- Phase 3: native app only if customers ask; the PWA covers approvals, alerts and "is my agent still sealing?" views.
- Alerts that matter: a silent agent (missed epoch seal), an equivocation proof, a witness gone quiet, a policy breach, a quota nearing its limit.

## Round 22: approval_order check (shipped in vc-verify)
Convention (statement extensions, no schema change):
- An intent that needs a human or second party sets `ext.requires_approval: true`.
- An approver (a different registered agent id, for example the approval service acting for a person) writes an `approval` statement with `ext.approval.action_id` = the intent's statement id and `ext.approval.decision` = "approve" or "deny".
- Check `approval_order` (only present when a gated intent exists): every outcome of a gated intent must be preceded IN LOG ORDER by the latest approval for it being "approve", from an agent other than the intent's issuer. A missing approval, a late approval, a denial, a later denial after approval, or self-approval each fail the pack.
- Why it matters: the Noa draft states approval time cannot be ordered against action time. Here order comes from the append-only log, so it is provable, not claimed.
- Not yet: the passkey (WebAuthn) approval service and phone UI that produce these statements; the action hash binding (`ext.approval.action_hash`) check; policy to say which intents MUST be gated (today the issuer marks them; a pinned policy list comes next).

## Round 23 (shipped)
- Policy `gated_classes`: a pinned list of intent action classes that ALWAYS need approval, even if the issuer did not mark them (pins can only add). Test: same pack passes unpinned, fails pinned.
- Passkey approval service in vc-server (WebAuthn ES256, attestation none): enrol, pending list derived from the log, challenge bound to (origin, action id, decision, nonce), assertion verified (origin, rpIdHash, UP and UV flags, signature, counter must increase, single-use challenge), then an `approval` statement carrying the full assertion is appended, signed by a per-tenant approval-service key. Tested with a software authenticator AND in real Chromium with a virtual authenticator against the /approve page.
- Phone page /approve with web manifest (installable). Accounts are still tenant-key based; per-user accounts and roles are next.
- Witness daemon (vc-witnessd, C2SP add-checkpoint API, atomic fsynced state) and scripts/witness-loop.sh; verified with real processes: log + witness + loop + vcv gives an all-green witnessed pack.
- Action binding: approvals reference the intent's statement id, which is the hash of the entire canonical statement, so approval of one action cannot be reused for another.
- Still open: offline verification of the embedded passkey assertion by vc-verify (needs approver public keys in pinned trust); vc-anchor (RFC 3161 / OpenTimestamps); per-user accounts; WASM verify page.
