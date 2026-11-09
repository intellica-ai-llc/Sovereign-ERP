# eng-0015: The evaluation build and how it is distributed

- Status: accepted
- Date: 2026-11-04

## Context
Nothing customers can run existed. Sending a pilot recipient or investor the repository alone is not an evaluation. A narrow, honest artefact is better than a broad, unfinished one.

## Decision
- **Artefact:** the `sov` command-line tool (`profile`, `audit`, `demo`, `digest`) in a Linux tarball with the evaluation notice, evaluation guide, security and privacy statement, and a CycloneDX bill of materials, plus `SHA256SUMS`. Input is exported tab-separated files; there are no live connectors.
- **Honesty rules:** every output says it is an evaluation build; `docs/STATUS.md` is authoritative; reports disclose truncation; the demo is labelled synthetic; nothing claims results on real Maximo data.
- **Build:** `cargo xtask dist` produces a reproducible archive (sorted names, zero timestamps) and smoke-tests the packaged binary. CI builds it on every push and keeps it as a downloadable artefact. A tag-triggered release workflow adds provenance and Sigstore signatures; **it has not been run** and is documented as such.
- **Distribution:** private, to named recipients under a reviewed agreement; the portal front end is hosted behind authentication (see `site/README.md`).
- **Platform:** Linux x86-64 only. Windows and macOS builds are deferred.

## Consequences
We can put something real in a partner's hands without overstating it. Before wider distribution: counsel's review of the notice and agreement, a run of the release workflow on a private fork, and at least one real Maximo export through the tool.
