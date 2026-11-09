# eng-0005: Licence format and verification

- Status: accepted (format and behaviour); crate version pinned at implementation
- Date: 2026-10-29

## Context
Pilot customers run the product on their own infrastructure, often with no outbound internet. We need a licence key that works offline, expires, and carries limits. Comparable on-premises products (public documentation of FalkorDB Enterprise, Devin air-gapped, ChorusControl) converge on the same design: an offline-verified Ed25519 signature over a small claims payload, with the public key compiled into the binary.

## Decision
- **Token:** one compact signed token, `base64url(payload JSON) "." base64url(signature)`, **algorithm fixed to Ed25519 with no algorithm field and no negotiation**. This avoids the algorithm-confusion class of JWT bugs. Payload claims: `v` (format version, unknown versions rejected), `kid`, `license_id`, `product`, `customer`, `tier`, `iat`, `nbf`, `exp`, `limits`, `features`.
- **Verification** at every start, fully offline: signature over the exact payload bytes using strict verification (rejects malleable signatures), `kid` found in the embedded keyring, `product` matches, `nbf`/`exp` window with a 24-hour skew allowance, `v` known.
- **Keys:** root key offline on a hardware token; signing keys only in the issuance tool; several `kid`s so keys rotate; keyring and its checksum compiled in.
- **Expiry behaviour:** pilot licences are 30 to 90 days. After expiry the product enters a 14-day **read-only grace**, then refuses to start new ingestions. No phone-home, no revocation list: revocation is short expiry plus replacement.
- **No-key behaviour:** a 14-day evaluation window on the synthetic sample dataset only, never against a customer source.
- **Operator surface:** `sov license install|show` and `sov doctor`; a catalogue of `LIC0xx` codes; warnings 30 days before expiry; replacing a licence needs no restart.
- **Crypto dependency:** `ed25519-dalek` (verify-only, default features off, strict verification). Chosen over `ring` because we need one primitive, it is pure Rust, and the signing-API weakness RUSTSEC-2022-0093 was removed in 2.0. The exact version is pinned in the implementing PR after checking its minimum Rust version against `rust-toolchain.toml` (a toolchain bump may precede it), and `cargo-deny` and `cargo-audit` become CI gates in that same PR (eng-0003).

## Consequences
Works air-gapped. It is a contractual and honest-user control, not tamper-proofing: a determined attacker can patch a binary, so the pilot agreement carries real enforcement. The issuance tool and key ceremony live in a private repository.
