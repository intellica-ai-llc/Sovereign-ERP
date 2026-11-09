//! Source profiler (arc42 Appendix C spike 3, ADR eng-0007).
//!
//! Learns how a source database fits together. The first tier is **dictionary-first**: when the
//! source describes its own relationships, read that. IBM Maximo is the motivating case, because
//! its database declares no foreign keys; relationships live in the `MAXRELATIONSHIP` data
//! dictionary table as SQL where-clauses. The second tier ([`discovery`]) finds undeclared links
//! statistically (inclusion dependencies, uniqueness, names, global resolution), to validate and
//! extend the first.
//!
//! Everything here is pure: it takes rows read by a connector and returns structured results, so
//! it can be tested and model-checked without any database.

pub mod audit;
pub mod discovery;
pub mod eval;
pub mod maximo;
pub mod reconcile;
pub mod synth;
pub mod table;
