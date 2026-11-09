# eng-0001: Record engineering decisions

- Status: accepted
- Date: 2026-10-29

## Context
Architecture decisions live in arc42 section 9. Decisions about how the repository is built and run need a home that is reviewable and versioned with the code.

## Decision
Record each as a MADR file in `docs/adr/` with the `eng-` prefix. Accepted records are immutable; changes supersede them.

## Consequences
A short file per decision; the index in `docs/adr/README.md` is updated in the same PR.
