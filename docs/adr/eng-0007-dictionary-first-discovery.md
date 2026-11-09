# eng-0007: Dictionary-first relationship discovery

- Status: accepted
- Date: 2026-10-29

## Context
Maximo's database has no foreign keys. IBM's own integration notes say so, and describe relationships as SQL statements stored in `MAXRELATIONSHIP`; the data dictionary also holds `MAXOBJECT`, `MAXATTRIBUTE` and `MAXSYSINDEXES`. A profiler that only guesses relationships from data would throw away the best evidence available and be less accurate than the system's own metadata.

## Decision
Two tiers, in order:
1. **Dictionary tier.** Read the source's own metadata. For Maximo, parse each `MAXRELATIONSHIP` where clause (`sov-profiler::maximo`) into a structured join: parent attributes, child columns, constant filters. Clauses outside the supported subset (OR, subqueries, functions, grouping, non-equality, null tests, system variables) are **reported with the reason**, never guessed at or dropped.
2. **Inference tier** (not built yet). Statistical discovery (value inclusion, uniqueness, name and type similarity) to validate dictionary links, find undeclared and custom joins, and support non-Maximo sources.

Every link carries provenance (`declared`, `dictionary`, `inferred` with a score). Conflicts between tiers are surfaced to the user, not silently resolved. Direction and cardinality of a dictionary link are **not** taken from the parent/child naming (that is navigation); they come from the data profile.

## Consequences
Arc42 Appendix C spike 3 splits in two: the dictionary tier is measured by parse coverage on real dictionaries (needs a design partner's `MAXRELATIONSHIP` export, which is metadata, not business data); the FK-stripping benchmark on open-source ERP schemas measures the inference tier only.
