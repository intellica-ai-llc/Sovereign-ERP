# eng-0009: Per-source weight profiles and cross-corpus evaluation

- Status: accepted
- Date: 2026-10-31

## Context
eng-0008 fixed the inference method and a held-out gate on one ERP. Testing it elsewhere showed two things: (1) integer surrogate keys create coincidental inclusions that `OFBiz`'s text ids never exercised; (2) weights fitted where names are meaningful collapse (recall 0.08) on schemas whose names are not, so a single global weight vector is wrong. A first leave-one-corpus-out run also leaked the held-out corpus through its starting weights.

## Decision
- Two shipped weight profiles, **named** (`Weights::default`) and **opaque** (`Weights::opaque`, name features off, no column-order cue), chosen per source by `Weights::auto` from `name_informativeness`. `discover` uses `auto`.
- Features `child_unique` and `parent_density` join the model to handle surrogate integer keys.
- Evaluation protocol: **leave-one-corpus-out** with tuning started from a neutral point, never from shipped weights; tuning objective is mean F1 across training corpora so large corpora cannot drown small ones. `cargo xtask bench-transfer` runs it (manual, slow) with a floor on mean held-out F1; `cargo xtask bench-fk` stays the fast CI gate.
- Corpora are downloaded at run time and pinned by commit (`OFBiz`) or SHA-256 (SQLite databases); none is committed.
- Weights are refitted only by `fkloco --weights-only`, and the research note records the corpora used.

## Consequences
Scores quoted for the product must name the protocol: within-corpus held-out modules (0.84), leave-one-corpus-out (mean 0.86 over four, range 0.77 to 0.98) or opaque names (about 0.67 to 0.70). The corpora are small and the opaque profile rests on one corpus, so weights will be refitted as larger corpora and, eventually, real Maximo dictionaries become available.
