# eng-0014: SHA-256 implemented in-house, hashing only

- Status: accepted
- Date: 2026-11-04

## Context
Reproducible, content-addressed reports need a cryptographic hash. eng-0003 keeps the trusted base free of third-party code until an ADR approves it, and the first approved dependency will be a signature library (eng-0005). Hashing is a different problem from signing: SHA-256 is a fixed, public function with published test vectors, and every implementation can be checked against others.

## Decision
`sov-evidence` implements SHA-256 itself and **nothing else**: no signatures, no keys, no randomness, no encryption. It is verified by FIPS 180-4 examples, the million-`a` vector, vectors from Python's `hashlib` at every block-boundary length (0, 1, 3, 55, 56, 57, 63, 64, 65, 119, 120, 127, 128, 129, 191, 192, 200, 1000), and a test that every possible streaming split gives the same digest. It is not constant-time and must never touch secrets. Signing, when added, uses an audited library; the in-house hash may then be replaced by that library's.

## Consequences
Reports and downloads can be hashed with no external tool, including by `xtask` and the `sov digest` command. The code is about 150 lines and entirely covered by known answers. Reviewers who reject any in-house cryptographic code should treat this as the first item to swap.
