//! Scoring discovered links against declared relationships (arc42 Q31).
//!
//! Recall is reported against three denominators, so a number cannot hide a limit:
//! every declared relationship, those whose child columns hold data, and those the data
//! actually supports (child tuples found in the parent). A relationship the data does not support
//! cannot be found by any value-based method, so it is not held against the method, but it is shown.

use crate::discovery::Link;
use crate::table::Table;
use std::collections::{BTreeSet, HashMap, HashSet};

/// One declared relationship (ground truth).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TruthFk {
    /// Child table name.
    pub child_table: String,
    /// Child column names.
    pub child_cols: Vec<String>,
    /// Parent table name.
    pub parent_table: String,
    /// Parent column names, aligned with `child_cols`.
    pub parent_cols: Vec<String>,
    /// How the source declared it (`one`, `one-nofk`, `many-reverse`); empty when not recorded.
    pub kind: String,
}

/// Reads the benchmark's `truth.tsv` (header, then `child_table child_cols parent_table parent_cols ...`;
/// column lists are comma separated).
///
/// # Errors
///
/// A line with fewer than four tab-separated fields.
pub fn parse_truth(tsv: &str) -> Result<Vec<TruthFk>, String> {
    let mut out = Vec::new();
    for (i, line) in tsv.lines().enumerate().skip(1) {
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!(
                "line {}: expected at least 4 fields, found {}",
                i + 1,
                f.len()
            ));
        }
        let list = |s: &str| s.split(',').map(str::to_owned).collect::<Vec<_>>();
        out.push(TruthFk {
            child_table: f[0].to_owned(),
            child_cols: list(f[1]),
            parent_table: f[2].to_owned(),
            parent_cols: list(f[3]),
            kind: f.get(5).map(|k| (*k).to_owned()).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// Counts and the metrics derived from them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Report {
    /// Links predicted for in-scope child tables.
    pub predicted: usize,
    /// Of those, predictions flagged ambiguous.
    pub ambiguous: usize,
    /// Predictions identical to a declared relationship (tables, columns and their pairing).
    pub tp_strict: usize,
    /// Predictions matching a declared relationship on child table, child columns and parent table.
    pub tp_table: usize,
    /// Declared relationships with an in-scope child table.
    pub truth_total: usize,
    /// ... whose child columns hold data and whose parent table is not empty.
    pub truth_with_data: usize,
    /// ... whose child tuples are found in the parent (at least 98 percent).
    pub truth_supported: usize,
    /// Supported relationships found exactly.
    pub found_strict: usize,
    /// Supported relationships found at table level.
    pub found_table: usize,
    /// Relationships with data found exactly (any support level).
    pub found_with_data_strict: usize,
}

fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        f64::from(u32::try_from(n).unwrap_or(u32::MAX))
            / f64::from(u32::try_from(d).unwrap_or(u32::MAX))
    }
}

impl Report {
    /// Exact-match precision: predictions that are declared relationships.
    #[must_use]
    pub fn precision(&self) -> f64 {
        ratio(self.tp_strict, self.predicted)
    }

    /// Exact-match recall over relationships the data supports.
    #[must_use]
    pub fn recall(&self) -> f64 {
        ratio(self.found_strict, self.truth_supported)
    }

    /// Table-level precision (parent column choice ignored).
    #[must_use]
    pub fn precision_table(&self) -> f64 {
        ratio(self.tp_table, self.predicted)
    }

    /// Table-level recall over supported relationships.
    #[must_use]
    pub fn recall_table(&self) -> f64 {
        ratio(self.found_table, self.truth_supported)
    }

    /// Exact-match recall over every relationship whose columns hold data.
    #[must_use]
    pub fn recall_with_data(&self) -> f64 {
        ratio(self.found_with_data_strict, self.truth_with_data)
    }

    /// Harmonic mean of exact precision and recall.
    #[must_use]
    pub fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

type PairKey = (String, BTreeSet<(String, String)>, String);
type TableKey = (String, BTreeSet<String>, String);

fn pair_key(ct: &str, cc: &[String], pt: &str, pc: &[String]) -> PairKey {
    (
        ct.to_owned(),
        cc.iter().cloned().zip(pc.iter().cloned()).collect(),
        pt.to_owned(),
    )
}

fn table_key(ct: &str, cc: &[String], pt: &str) -> TableKey {
    (ct.to_owned(), cc.iter().cloned().collect(), pt.to_owned())
}

/// Share of the child's complete (all-columns non-null) tuples present in the parent; `None` when
/// a table or column is missing, or no child tuple is complete.
fn tuple_containment(by_name: &HashMap<&str, &Table>, fk: &TruthFk) -> Option<(f64, usize)> {
    let child = by_name.get(fk.child_table.as_str())?;
    let parent = by_name.get(fk.parent_table.as_str())?;
    let find = |t: &Table, names: &[String]| -> Option<Vec<usize>> {
        names
            .iter()
            .map(|n| t.columns.iter().position(|c| &c.name == n))
            .collect()
    };
    let (cc, pc) = (find(child, &fk.child_cols)?, find(parent, &fk.parent_cols)?);
    let tuples: HashSet<Vec<&str>> = (0..parent.row_count())
        .filter_map(|r| {
            pc.iter()
                .map(|&c| parent.columns[c].values[r].as_deref())
                .collect::<Option<Vec<_>>>()
        })
        .collect();
    let (mut nn, mut hits) = (0usize, 0usize);
    for r in 0..child.row_count() {
        if let Some(t) = cc
            .iter()
            .map(|&c| child.columns[c].values[r].as_deref())
            .collect::<Option<Vec<_>>>()
        {
            nn += 1;
            hits += usize::from(tuples.contains(&t));
        }
    }
    (nn > 0).then(|| (ratio(hits, nn), nn))
}

/// Scores `links` against `truth`, counting only relationships whose child table is in `scope`.
#[must_use]
pub fn evaluate(
    tables: &[Table],
    links: &[Link],
    truth: &[TruthFk],
    scope: &HashSet<String, impl std::hash::BuildHasher>,
) -> Report {
    let by_name: HashMap<&str, &Table> = tables.iter().map(|t| (t.name.as_str(), t)).collect();
    let name = |t: usize, cols: &[usize]| -> Vec<String> {
        cols.iter()
            .map(|&c| tables[t].columns[c].name.clone())
            .collect()
    };

    let truth: Vec<&TruthFk> = truth
        .iter()
        .filter(|t| scope.contains(&t.child_table))
        .collect();
    let strict: HashSet<PairKey> = truth
        .iter()
        .map(|t| {
            pair_key(
                &t.child_table,
                &t.child_cols,
                &t.parent_table,
                &t.parent_cols,
            )
        })
        .collect();
    let loose: HashSet<TableKey> = truth
        .iter()
        .map(|t| table_key(&t.child_table, &t.child_cols, &t.parent_table))
        .collect();

    let mut r = Report {
        truth_total: truth.len(),
        ..Report::default()
    };
    let mut predicted_strict: HashSet<PairKey> = HashSet::new();
    let mut predicted_table: HashSet<TableKey> = HashSet::new();
    for l in links
        .iter()
        .filter(|l| scope.contains(&tables[l.child_table].name))
    {
        r.predicted += 1;
        r.ambiguous += usize::from(l.ambiguous);
        let (ct, pt) = (&tables[l.child_table].name, &tables[l.parent_table].name);
        let (cc, pc) = (
            name(l.child_table, &l.child_cols),
            name(l.parent_table, &l.parent_cols),
        );
        let (sk, lk) = (pair_key(ct, &cc, pt, &pc), table_key(ct, &cc, pt));
        r.tp_strict += usize::from(strict.contains(&sk));
        r.tp_table += usize::from(loose.contains(&lk));
        predicted_strict.insert(sk);
        predicted_table.insert(lk);
    }
    for t in &truth {
        let has_data = by_name
            .get(t.parent_table.as_str())
            .is_some_and(|p| p.row_count() > 0);
        let tc = tuple_containment(&by_name, t);
        let with_data = has_data && tc.is_some();
        let supported = with_data && tc.is_some_and(|(c, _)| c >= 0.98);
        let hit_strict = predicted_strict.contains(&pair_key(
            &t.child_table,
            &t.child_cols,
            &t.parent_table,
            &t.parent_cols,
        ));
        let hit_table =
            predicted_table.contains(&table_key(&t.child_table, &t.child_cols, &t.parent_table));
        r.truth_with_data += usize::from(with_data);
        r.truth_supported += usize::from(supported);
        r.found_with_data_strict += usize::from(with_data && hit_strict);
        r.found_strict += usize::from(supported && hit_strict);
        r.found_table += usize::from(supported && hit_table);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::discover;

    #[test]
    fn parses_truth_and_rejects_short_lines() {
        let t = parse_truth("h\nA\ta,b\tB\tx,y\tmod\tone\n").unwrap();
        assert_eq!(t[0].child_cols, ["a", "b"]);
        assert_eq!(t[0].parent_cols, ["x", "y"]);
        assert!(parse_truth("h\nA\tb\n").is_err());
    }

    #[test]
    fn scores_a_perfect_and_an_empty_prediction() {
        let tables = vec![
            Table::from_tsv("Party", "partyId\nP1\nP2\nP3\n").unwrap(),
            Table::from_tsv("Invoice", "invoiceId\tpartyId\nI1\tP1\nI2\tP2\nI3\tP3\n").unwrap(),
        ];
        let truth = parse_truth("h\nInvoice\tpartyId\tParty\tpartyId\tm\tone\n").unwrap();
        let scope: HashSet<String> = ["Invoice".to_owned()].into();
        let r = evaluate(&tables, &discover(&tables), &truth, &scope);
        assert_eq!(
            (
                r.truth_total,
                r.truth_supported,
                r.found_strict,
                r.tp_strict
            ),
            (1, 1, 1, 1)
        );
        assert!((r.f1() - 1.0).abs() < 1e-9, "{r:?}");
        let none = evaluate(&tables, &[], &truth, &scope);
        assert_eq!((none.found_strict, none.predicted), (0, 0));
        assert!(none.f1().abs() < 1e-9);
    }

    #[test]
    fn unsupported_declared_links_are_not_counted_against_recall() {
        let tables = vec![
            Table::from_tsv("Party", "partyId\nP1\n").unwrap(),
            Table::from_tsv("Invoice", "invoiceId\tpartyId\nI1\tZZZ\n").unwrap(),
        ];
        let truth = parse_truth("h\nInvoice\tpartyId\tParty\tpartyId\tm\tone\n").unwrap();
        let scope: HashSet<String> = ["Invoice".to_owned()].into();
        let r = evaluate(&tables, &[], &truth, &scope);
        assert_eq!(
            (r.truth_total, r.truth_with_data, r.truth_supported),
            (1, 1, 0)
        );
    }
}
