# eng-0006: Pilot SKU, first source and platform

- Status: accepted (the design partner's constraints may amend it)
- Date: 2026-10-29

## Context
Nothing is shippable until a binary does one thing for one source on one platform. Research findings that shape the choice:
- IBM Maximo 7.6.x extended support was due to end on 30 September 2026 (IBM community post), so installations are migrating to the Maximo Application Suite (MAS). IBM's own migration path starts by duplicating or backing up the 7.6 production database.
- MAS Manage supports Db2, Oracle and SQL Server. IBM-managed Maximo supports Db2 only and does not permit direct access to production databases, even read-only; a reporting server is required.
- The Maximo database declares no foreign keys; relationships live in `MAXRELATIONSHIP` (see eng-0007).
- No reliable public figures exist for installed-base share by database engine.

## Decision
- **SKU:** a **Source Profiler and Equivalence Report** for IBM Maximo (7.6.x and MAS Manage). Read-only. No write-back. It learns the schema and relationships, builds the governed semantic model, and proves equivalence by replay. This is the "learns your database on day one" promise, and it helps a customer **plan their MAS migration** today.
- **Input modes, in priority order:** (1) restored backup or offline export, (2) reporting replica, (3) live read-only connection. Mode 1 works for every customer, including IBM-managed ones, and never touches production.
- **Engine order:** set by the design partner's engine. Default order Db2, Oracle, SQL Server. Engine-specific code is confined to a `SourceDialect` boundary (catalogue queries and driver); the dictionary-first logic is engine-independent because the `MAX*` tables are identical across engines.
- **Platform:** Linux x86-64, as a static tarball and an OCI image. Windows and macOS are deferred until a pilot customer needs them.
- **Pilot terms** (duration, fee) are a business decision for the founder; the recommendation is a free, fixed-term design-partner pilot (60 to 90 days) with written success criteria, to be set in the pilot agreement.

## Consequences
The first binary is narrow and honest. The engine decision waits on the design partner but costs little to defer.
