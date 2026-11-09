# Relationship discovery on a real ERP: method, results, limits

Status: first real-data evaluation (arc42 Appendix C spike 3, Q31). Reproduce with `cargo xtask bench-fk`.

## Question
When a database declares no foreign keys, how well can relationships be recovered from the data alone, on a real ERP rather than a synthetic one? arc42 Appendix J answered on a synthetic 11-table database and said so: "it does not predict performance on real schemas".

## Data
Apache `OFBiz` (Apache-2.0), pinned at commit `1234bc367afd381424a11ba8a66ec3ee721ea179`. `bench/ofbiz/convert.py` turns its entity definitions and bundled demo and seed data into tables and ground truth:

- 812 tables, 390 with data, about 13,000 rows in total (the demo data is small and hand-authored).
- 2,312 declared relationships: 1,898 `one`, 231 `one-nofk`, 183 parent-side `many` declarations, many composite.
- The profiler sees **only table and column names and values**: no keys, no relations. Declared relationships are used only to score.
- Data files are upserts, so rows are merged by primary key as the loader would. (My first conversion appended instead of merging and showed 245 supported relationships pointing at "duplicate" parents. That was a fidelity bug in the benchmark, found by diagnosing the recall ceiling and fixed before any tuning.)

## Method (`sov-profiler::discovery`)
1. Profile columns: nulls, distinct values, value kind, length range, name words.
2. Generate candidates once: unary inclusion dependencies into unique columns, and two-column inclusion dependencies into unique column pairs, tolerating up to 2 percent orphan rows.
3. Score with explainable linear features: containment, key-space coverage, support, parent-table size, key position, column-name and table-name similarity, shape, "a closer parent exists", self reference, composite.
4. Resolve per child column set: keep the best parent, flag a runner-up within a margin as ambiguous for human review.

Weights were tuned by coordinate descent on **training modules only** (party, product, content, humanres, marketing, manufacturing, framework). Every headline number is on the **held-out test modules** (accounting, order, shipment, workeffort), scored against the conservative truth (forward declarations only).

## Results (held-out test modules)

| Method | Predicted | Precision | Recall | F1 |
|---|---|---|---|---|
| Inclusion dependencies only | 2,926 | 0.12 | 0.94 | 0.22 |
| arc42 Appendix J heuristic, reimplemented (share filter and ambiguity drop) | 178 | 0.53 | 0.25 | 0.34 |
| `sov-profiler`, tuned on the `OFBiz` train modules alone | 386 | 0.84 | 0.85 | 0.85 |
| `sov-profiler`, shipped default weights (fitted on all corpora, see below) | 366 | **0.86** | **0.82** | **0.84** |
| same, ambiguous links held for review (371 kept) | 371 | 0.87 | 0.84 | 0.85 |
| `sov-profiler`, name evidence switched off, re-tuned | 389 | 0.70 | 0.72 | 0.71 |
| same, confident links only (183 held for review) | 206 | 0.85 | 0.46 | 0.60 |

Recall is over relationships the data supports (child tuples found in the parent, 380 of 1,048 declared in scope; the rest have empty or inconsistent data, so no value-based method can see them). Recall over every relationship that has data is about 0.79. With weights tuned on the `OFBiz` train modules alone, train F1 is 0.91, so the generalisation gap is about 0.06. Counting parent-side declarations as truth changes the test numbers by 0.002 or less.

## Transfer to other databases (leave-one-corpus-out)

Weights tuned on one ERP could be an artefact of it, so three public SQLite databases with declared keys were added (`bench/sqlite/convert.py`; Chinook, Northwind with 626,000 rows, Sakila; pinned by SHA-256). Each corpus is scored in turn by weights tuned on **the others**, starting from a neutral point (`cargo xtask bench-transfer`, slow, manual):

| Held-out corpus | Appendix J, share tuned fairly (F1) | `sov-profiler` precision | recall | F1 |
|---|---|---|---|---|
| `OFBiz` test modules | 0.10 | 0.73 | 0.82 | 0.77 |
| Chinook | 0.67 | 0.77 | 0.91 | 0.83 |
| Northwind | **0.95** | 1.00 | 0.73 | 0.84 |
| Sakila | 0.32 | 0.96 | 1.00 | **0.98** |
| **Mean** | 0.51 | 0.86 | 0.86 | **0.86** |

Findings:
1. **Integer surrogate keys are the main trap, as arc42 Appendix J warned.** Dense integer sequences contain each other by chance (`Album.AlbumId` inside `Track.TrackId`). Zero-shot, before any fix, Chinook scored F1 0.60. Two features that target the mechanism (the child column is itself unique; how densely the parent's integers fill their range) fixed most of it. `OFBiz` uses text ids, so its weights had never seen the problem.
2. **On a clean, tiny database the old heuristic is competitive.** Northwind (13 tables) is the one corpus where Appendix J wins (0.95 against 0.84), because there a "meaningful share of the key space" filter is a good rule. It fails everywhere with larger, messier schemas (0.10 on `OFBiz`).
3. **A method-validity error was found and fixed during this round.** The first leave-one-out run started tuning from the shipped weights, which had been fitted on all corpora, so the held-out database leaked in through the starting point (and one corpus degenerated to zero predictions). The runner now starts from a neutral point; the table above is that corrected run. The earlier, more flattering numbers are retracted.
4. The small corpora are tiny (11, 13 and 16 tables). The mean weights each corpus equally, so `OFBiz` (812 tables) counts as much as Chinook. Read the mean as indicative.

## Cryptic names and column order

`bench/ofbiz/obfuscate.py` rewrites `OFBiz` with every table `T0001...` and every column `C01...`, optionally also permuting column order (so "keys come first" is not a cue). Held-out test modules, weights re-tuned on the rewritten train modules:

| Variant | Precision | Recall | F1 | Ambiguous links |
|---|---|---|---|---|
| Readable names (shipped weights) | 0.86 | 0.82 | 0.84 | 22 |
| Opaque names | 0.69 | 0.70 | 0.70 | 128 |
| Opaque names and shuffled columns | 0.65 | 0.69 | 0.67 | 149 |
| arc42 Appendix J, either opaque variant | 0.53 | 0.25 | 0.34 | 0 |

1. **The weights must be chosen per source.** The readable-name weights applied unchanged to the opaque rewrite collapse to recall 0.08, because their bias demands name evidence that is not there. `Weights::auto` measures how many column names hold a word of three or more letters and picks the readable or the opaque profile (`name_informativeness`, tested on every corpus here).
2. **The positional cue is worth about 0.03 F1.** The opaque profile ships position-free, so it does not depend on `OFBiz` placing keys first.
3. **Opaque schemas need review capacity**: about 150 links of 410 go to a human, and keeping only confident links cuts recall to about 0.5. This is the cost the dictionary tier (eng-0007) exists to avoid.

## What this changes in arc42

1. **Inclusion alone is as bad on real data as Appendix J said.** Precision 0.12 here against 0.09 there. That finding stands.
2. **Appendix J's heuristic does not survive contact with a real schema.** It reported recall 0.96 on synthetic data; reimplemented here its recall is 0.25. Its "child must use a meaningful share of the parent's key space" filter discards the many real relationships where a child references a small part of a large parent table. The tuned scoring gives that coverage feature a **negative** weight. arc42 should replace the filter, not tune it.
3. **Name evidence is worth about 0.14 F1.** Without any name features the method still doubles the Appendix J heuristic, but the ambiguous queue grows from 15 links to 183, and keeping only the confident links cuts recall to 0.46. Cryptic schemas (SAP-style abbreviations, `T01`/`C1`) mean a much larger human review load. Plan reviewer time on that basis, and use the source's own dictionary first (eng-0007).
4. **The recall ceiling is the candidate generator.** Misses are structural: composite keys of three or more columns, and parents that are not unique (nulls, duplicates). Both are measurable and listed below.
5. **Almost all false positives are links no one declared.** Of 61 test errors under the first run, 2 were a wrong parent for a declared column; the rest were columns with no declared relationship, many of them plausible (`BudgetRole.roleTypeId` to `RoleType`). Precision is therefore a lower bound against the declared truth. The fix is human confirmation, not a higher threshold.
6. **Q31 is now a gate.** `cargo xtask bench-fk` fails below held-out precision 0.80 or recall 0.80 on the conservative truth, run in CI.

## Limits (read before quoting a number)
- One system, with readable names, small hand-authored demo data (about 13,000 rows), mostly single and two-column keys. Real Maximo data is larger, dirtier and has cryptic-but-structured names; no Maximo data was used.
- The shipped weights are fitted on four corpora (one ERP, three small sample databases). Leave-one-corpus-out gives F1 0.77 to 0.98; transfer to Maximo is unmeasured. The opaque-name runs are the nearest proxy for cryptic schemas. The opaque profile is fitted on a single corpus.
- Composite relationships of three or more columns are not searched; composite candidates are noisy (their weight tunes negative).
- Polymorphic references, soft deletes and customisation are absent from this corpus.
- Everything is exact set inclusion on in-memory tables. Million-row tables need sketches (below); that path is designed, not built or measured.
- The ground truth is incomplete (see 5). Precision numbers are conservative; recall numbers are exact for what is declared.
- The test modules are held out from tuning but not from the corpus: they share OFBiz's conventions with the training modules. A different ERP is the real test.

## Next research (not done)
- **Global schema-graph resolution** in the style of Auto-BI (k-MCA) in place of per-column resolution, to cut ambiguity.
- **Sketch-based inclusion** (MinHash or HyperLogLog with exact verification of survivors) for large tables, measured against exact results.
- **Three-column composite keys** and non-unique parents (near-unique with tolerance).
- **Larger ERP and EAM corpora** (ERPNext, Odoo, openMAINT, Dolibarr): the three extra corpora here are small and clean, so transfer to a messy 500-table system is still a single data point (`OFBiz`).
- **Maximo-shaped synthetic database** (composite site keys, custom undeclared links, polymorphic owners) with a reconciler that merges dictionary links and inferred links, and reports direction, cardinality and orphan rates from the data.
- **Oracle-checked adjudication** of ambiguous links: accounting and inventory invariants as a tie-breaker, as arc42 Appendix J showed for sign conventions. Any model-based adjudication must be local, optional and judged against those oracles.
- **Learning from reviewer decisions** to refit weights per source.
