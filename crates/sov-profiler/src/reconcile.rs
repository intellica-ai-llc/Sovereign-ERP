//! Merging the two discovery tiers into one answer (ADR eng-0007).
//!
//! The dictionary says which tables navigate to which, but not which side holds the key, whether the
//! link is one-to-one or many-to-one, or whether the data agrees. Inference finds links nobody
//! declared. This module resolves every dictionary link against the data (direction, cardinality,
//! orphan rows), matches it with inferred links, and reports provenance: declared, inferred or both.

use crate::discovery::Link;
use crate::maximo::{extract, ColumnPair, Filter, Literal, RelationshipRow, Unsupported};
use crate::table::Table;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Share of a side's tuples that must be found in the other side for it to be called the foreign-key side.
/// Deliberately low: direction comes from which side is a unique key, and dirty data (orphan rows) is a
/// finding to report, not a reason to hide the relationship.
const MIN_DIRECTION_CONTAINMENT: f64 = 0.5;

/// Where a link came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Provenance {
    /// Only the data dictionary declares it.
    Dictionary,
    /// Only statistical inference found it.
    Inferred,
    /// Both agree.
    Both,
}

/// How the two sides relate, decided from the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cardinality {
    /// Many rows hold the key; the referenced side is unique.
    ManyToOne,
    /// Both sides are unique.
    OneToOne,
    /// The data does not settle direction or uniqueness (see [`Reconciled::orphan_rows`]).
    Unresolved,
}

/// One relationship with its evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Reconciled {
    /// Table holding the foreign key (upper case).
    pub fk_table: String,
    /// Foreign-key columns (upper case).
    pub fk_cols: Vec<String>,
    /// Referenced table (upper case).
    pub ref_table: String,
    /// Referenced columns, aligned with `fk_cols` (upper case).
    pub ref_cols: Vec<String>,
    /// Dictionary, inference or both.
    pub provenance: Provenance,
    /// Decided from the data.
    pub cardinality: Cardinality,
    /// Foreign-key tuples examined (rows where every key column is present).
    pub checked_rows: usize,
    /// Of those, rows whose key is absent from the referenced side.
    pub orphan_rows: usize,
    /// Names of the dictionary relationships that declare it.
    pub dictionary_names: Vec<String>,
    /// Inference flagged a close alternative parent.
    pub ambiguous: bool,
    /// `false` when the data could not tell which side holds the key (both sides unique and equally
    /// contained) and no other evidence decided; a person should confirm the direction.
    pub direction_certain: bool,
}

impl Reconciled {
    /// Share of examined rows with a matching referenced row (1.0 when none were examined).
    #[must_use]
    pub fn containment(&self) -> f64 {
        if self.checked_rows == 0 {
            1.0
        } else {
            let (ok, all) = (self.checked_rows - self.orphan_rows, self.checked_rows);
            f64::from(u32::try_from(ok).unwrap_or(u32::MAX))
                / f64::from(u32::try_from(all).unwrap_or(u32::MAX))
        }
    }
}

/// Everything found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Findings {
    /// Reconciled relationships, sorted for reproducibility.
    pub links: Vec<Reconciled>,
    /// Dictionary rows outside the supported clause subset, with the reason.
    pub unsupported: Vec<(RelationshipRow, Unsupported)>,
    /// Dictionary links that could not be checked against the data (relationship name, reason).
    pub unchecked: Vec<(String, String)>,
}

type Key = (String, BTreeSet<(String, String)>, String);

struct Index<'a> {
    tables: HashMap<String, &'a Table>,
}

impl<'a> Index<'a> {
    fn new(tables: &'a [Table]) -> Self {
        Self {
            tables: tables
                .iter()
                .map(|t| (t.name.to_ascii_uppercase(), t))
                .collect(),
        }
    }

    fn table(&self, name: &str) -> Result<&'a Table, String> {
        self.tables
            .get(&name.to_ascii_uppercase())
            .copied()
            .ok_or_else(|| format!("table {name} is not in the source"))
    }
}

fn col_index(t: &Table, name: &str) -> Result<usize, String> {
    t.columns
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("column {}.{name} is not in the source", t.name))
}

fn indices(t: &Table, names: &[String]) -> Result<Vec<usize>, String> {
    names.iter().map(|n| col_index(t, n)).collect()
}

/// Complete (no NULL) key tuples over the given rows.
fn tuples<'a>(t: &'a Table, cols: &[usize], rows: &[usize]) -> Vec<Vec<&'a str>> {
    rows.iter()
        .filter_map(|&r| {
            cols.iter()
                .map(|&c| t.columns[c].values[r].as_deref())
                .collect::<Option<Vec<_>>>()
        })
        .collect()
}

fn all_rows(t: &Table) -> Vec<usize> {
    (0..t.row_count()).collect()
}

fn filtered_rows(t: &Table, filters: &[Filter]) -> Result<Vec<usize>, String> {
    let checks: Vec<(usize, &str)> = filters
        .iter()
        .map(|f| {
            let lit = match &f.literal {
                Literal::Text(s) | Literal::Number(s) => s.as_str(),
            };
            Ok((col_index(t, &f.child_column)?, lit))
        })
        .collect::<Result<_, String>>()?;
    Ok((0..t.row_count())
        .filter(|&r| {
            checks
                .iter()
                .all(|&(c, lit)| t.columns[c].values[r].as_deref() == Some(lit))
        })
        .collect())
}

fn share(hits: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        f64::from(u32::try_from(hits).unwrap_or(u32::MAX))
            / f64::from(u32::try_from(total).unwrap_or(u32::MAX))
    }
}

fn distinct(v: &[Vec<&str>]) -> usize {
    v.iter().collect::<HashSet<_>>().len()
}

struct Side<'a> {
    tuples: Vec<Vec<&'a str>>,
}

impl Side<'_> {
    fn unique(&self) -> bool {
        !self.tuples.is_empty() && distinct(&self.tuples) == self.tuples.len()
    }

    /// How many of `self`'s tuples exist in `other`.
    fn hits_in(&self, other: &Side<'_>) -> usize {
        let set: HashSet<&Vec<&str>> = other.tuples.iter().collect();
        self.tuples.iter().filter(|t| set.contains(t)).count()
    }
}

fn key(fk_table: &str, fk_cols: &[String], ref_table: &str, ref_cols: &[String]) -> Key {
    (
        fk_table.to_owned(),
        fk_cols
            .iter()
            .cloned()
            .zip(ref_cols.iter().cloned())
            .collect(),
        ref_table.to_owned(),
    )
}

fn upper(v: &[String]) -> Vec<String> {
    v.iter().map(|s| s.to_ascii_uppercase()).collect()
}

/// Resolves one dictionary link against the data: which side holds the key, and how well it holds.
fn resolve_dictionary(
    idx: &Index<'_>,
    parent: &str,
    child: &str,
    pairs: &[ColumnPair],
    filters: &[Filter],
    hubs: &HashMap<String, usize>,
) -> Result<Reconciled, String> {
    let (pt, ct) = (idx.table(parent)?, idx.table(child)?);
    let p_names: Vec<String> = pairs.iter().map(|p| p.parent_attr.clone()).collect();
    let c_names: Vec<String> = pairs.iter().map(|p| p.child_column.clone()).collect();
    let (pi, ci) = (indices(pt, &p_names)?, indices(ct, &c_names)?);
    let p_side = Side {
        tuples: tuples(pt, &pi, &all_rows(pt)),
    };
    let c_side = Side {
        tuples: tuples(ct, &ci, &filtered_rows(ct, filters)?),
    };

    let (p_in_c, c_in_p) = (p_side.hits_in(&c_side), c_side.hits_in(&p_side));
    let (p_cont, c_cont) = (
        share(p_in_c, p_side.tuples.len()),
        share(c_in_p, c_side.tuples.len()),
    );
    let (p_unique, c_unique) = (p_side.unique(), c_side.unique());

    // A foreign key points at a unique key. The child holds the key when the parent side is unique and
    // the child's tuples are mostly found in it; the parent holds it in the mirrored case.
    let child_can = p_unique && c_cont >= MIN_DIRECTION_CONTAINMENT;
    let parent_can = c_unique && p_cont >= MIN_DIRECTION_CONTAINMENT;
    // Both directions plausible and equally well supported: the data cannot tell. Fall back to the
    // table that other relationships point at (a hub is the referenced side); otherwise say so.
    let tie = child_can && parent_can && (c_cont - p_cont).abs() < 0.02;
    let hub = |t: &str| hubs.get(&t.to_ascii_uppercase()).copied().unwrap_or(0);
    let (hub_parent, hub_child) = (hub(parent), hub(child));
    let certain = !tie || hub_parent != hub_child;
    let child_holds = match (child_can, parent_can) {
        (true, false) => true,
        (false, true) => false,
        _ if tie && hub_parent != hub_child => hub_parent > hub_child,
        _ => c_cont >= p_cont,
    };
    let (ref_unique, fk_unique) = if child_holds {
        (p_unique, c_unique)
    } else {
        (c_unique, p_unique)
    };
    let cardinality = match (ref_unique, fk_unique) {
        (true, true) => Cardinality::OneToOne,
        (true, false) => Cardinality::ManyToOne,
        (false, _) => Cardinality::Unresolved,
    };
    let (fk_table, fk_cols, ref_table, ref_cols, checked, hits) = if child_holds {
        (child, c_names, parent, p_names, c_side.tuples.len(), c_in_p)
    } else {
        (parent, p_names, child, c_names, p_side.tuples.len(), p_in_c)
    };
    Ok(Reconciled {
        fk_table: fk_table.to_ascii_uppercase(),
        fk_cols: upper(&fk_cols),
        ref_table: ref_table.to_ascii_uppercase(),
        ref_cols: upper(&ref_cols),
        provenance: Provenance::Dictionary,
        cardinality,
        checked_rows: checked,
        orphan_rows: checked - hits,
        dictionary_names: Vec::new(),
        ambiguous: false,
        direction_certain: certain,
    })
}

/// Measures an inferred link directly: it is already oriented child to parent.
fn measure(idx: &Index<'_>, l: &InferredNames) -> Result<Reconciled, String> {
    let (ft, rt) = (idx.table(&l.fk_table)?, idx.table(&l.ref_table)?);
    let (fi, ri) = (indices(ft, &l.fk_cols)?, indices(rt, &l.ref_cols)?);
    let fk = Side {
        tuples: tuples(ft, &fi, &all_rows(ft)),
    };
    let rf = Side {
        tuples: tuples(rt, &ri, &all_rows(rt)),
    };
    let hits = fk.hits_in(&rf);
    Ok(Reconciled {
        fk_table: l.fk_table.to_ascii_uppercase(),
        fk_cols: upper(&l.fk_cols),
        ref_table: l.ref_table.to_ascii_uppercase(),
        ref_cols: upper(&l.ref_cols),
        provenance: Provenance::Inferred,
        cardinality: if !rf.unique() {
            Cardinality::Unresolved
        } else if fk.unique() {
            Cardinality::OneToOne
        } else {
            Cardinality::ManyToOne
        },
        checked_rows: fk.tuples.len(),
        orphan_rows: fk.tuples.len() - hits,
        dictionary_names: Vec::new(),
        ambiguous: l.ambiguous,
        direction_certain: true,
    })
}

struct InferredNames {
    fk_table: String,
    fk_cols: Vec<String>,
    ref_table: String,
    ref_cols: Vec<String>,
    ambiguous: bool,
}

/// Reconciles the dictionary with inference over the same tables.
///
/// `inferred` must come from [`crate::discovery`] run on `tables` (its indices refer to that slice).
#[must_use]
pub fn reconcile(tables: &[Table], dictionary: &[RelationshipRow], inferred: &[Link]) -> Findings {
    let idx = Index::new(tables);
    let extraction = extract(dictionary);
    let mut found: BTreeMap<Key, Reconciled> = BTreeMap::new();
    let mut out = Findings {
        unsupported: extraction.unsupported.clone(),
        ..Findings::default()
    };

    // Pass one resolves what the data settles; the tables other links point at become hubs, which then
    // break ties in pass two (a 1:1 relationship has no direction in the data alone).
    let mut hubs: HashMap<String, usize> = HashMap::new();
    for pass in 0..2 {
        found.clear();
        out.unchecked.clear();
        for l in &extraction.links {
            match resolve_dictionary(
                &idx,
                &l.parent_object,
                &l.child_object,
                &l.join.pairs,
                &l.join.filters,
                &hubs,
            ) {
                Ok(r) => {
                    let k = key(&r.fk_table, &r.fk_cols, &r.ref_table, &r.ref_cols);
                    let entry = found.entry(k).or_insert(r);
                    if !entry.dictionary_names.contains(&l.name) {
                        entry.dictionary_names.push(l.name.clone());
                    }
                }
                Err(why) => out
                    .unchecked
                    .push((format!("{}.{}", l.parent_object, l.name), why)),
            }
        }
        if pass == 0 {
            for r in found
                .values()
                .filter(|r| r.cardinality == Cardinality::ManyToOne)
            {
                *hubs.entry(r.ref_table.clone()).or_insert(0) += 1;
            }
        }
    }

    for l in inferred {
        let name = |t: usize, cols: &[usize]| -> Vec<String> {
            cols.iter()
                .map(|&c| tables[t].columns[c].name.to_ascii_uppercase())
                .collect()
        };
        let names = InferredNames {
            fk_table: tables[l.child_table].name.to_ascii_uppercase(),
            fk_cols: name(l.child_table, &l.child_cols),
            ref_table: tables[l.parent_table].name.to_ascii_uppercase(),
            ref_cols: name(l.parent_table, &l.parent_cols),
            ambiguous: l.ambiguous,
        };
        let k = key(
            &names.fk_table,
            &names.fk_cols,
            &names.ref_table,
            &names.ref_cols,
        );
        if let Some(existing) = found.get_mut(&k) {
            existing.provenance = Provenance::Both;
            existing.ambiguous |= l.ambiguous;
        } else if let Ok(r) = measure(&idx, &names) {
            found.insert(k, r);
        }
    }
    out.links = found.into_values().collect();
    out
}

#[cfg(test)]
// Test fixtures build text with `format!` and `push_str`: clearer than `write!` and fine here.
#[allow(clippy::format_push_string)]
mod tests {
    use super::*;

    fn t(name: &str, tsv: &str) -> Table {
        Table::from_tsv(name, tsv).unwrap()
    }

    fn row(name: &str, parent: &str, child: &str, clause: &str) -> RelationshipRow {
        RelationshipRow {
            name: name.into(),
            parent: parent.into(),
            child: child.into(),
            where_clause: clause.into(),
        }
    }

    fn world() -> Vec<Table> {
        vec![
            t(
                "ASSET",
                "assetnum\tsiteid\nA1\tS1\nA2\tS1\nA1\tS2\nA3\tS2\n",
            ),
            t(
                "WORKORDER",
                "wonum\tassetnum\tsiteid\n1\tA1\tS1\n2\tA1\tS1\n3\tA3\tS2\n4\tA2\tS1\n5\tZZ\tS1\n",
            ),
        ]
    }

    #[test]
    fn forward_navigation_is_oriented_by_the_data() {
        // Workorder navigates to asset: the workorder holds the key, with one orphan row.
        let rows = [row(
            "ASSET",
            "WORKORDER",
            "ASSET",
            "assetnum = :assetnum and siteid = :siteid",
        )];
        let f = reconcile(&world(), &rows, &[]);
        assert_eq!(f.links.len(), 1);
        let l = &f.links[0];
        assert_eq!(
            (l.fk_table.as_str(), l.ref_table.as_str()),
            ("WORKORDER", "ASSET")
        );
        assert_eq!(l.cardinality, Cardinality::ManyToOne);
        assert_eq!((l.checked_rows, l.orphan_rows), (5, 1));
        assert_eq!(l.provenance, Provenance::Dictionary);
    }

    #[test]
    fn reverse_navigation_resolves_to_the_same_link_and_merges_names() {
        let rows = [
            row(
                "ASSET",
                "WORKORDER",
                "ASSET",
                "assetnum = :assetnum and siteid = :siteid",
            ),
            row(
                "WORKORDERS",
                "ASSET",
                "WORKORDER",
                "assetnum = :assetnum and siteid = :siteid",
            ),
        ];
        let f = reconcile(&world(), &rows, &[]);
        assert_eq!(f.links.len(), 1, "{:?}", f.links);
        assert_eq!(f.links[0].dictionary_names.len(), 2);
    }

    #[test]
    fn inferred_only_and_both_are_told_apart() {
        let tables = world();
        let inferred = crate::discovery::discover(&tables);
        let rows = [row(
            "ASSET",
            "WORKORDER",
            "ASSET",
            "assetnum = :assetnum and siteid = :siteid",
        )];
        let with_dict = reconcile(&tables, &rows, &inferred);
        let without = reconcile(&tables, &[], &inferred);
        assert!(
            with_dict
                .links
                .iter()
                .any(|l| l.provenance == Provenance::Both)
                || inferred.is_empty(),
            "{:?}",
            with_dict.links
        );
        assert!(without
            .links
            .iter()
            .all(|l| l.provenance == Provenance::Inferred));
    }

    #[test]
    fn missing_tables_and_unsupported_clauses_are_reported_not_hidden() {
        let rows = [
            row("X", "WORKORDER", "NOSUCH", "a = :a"),
            row("Y", "ASSET", "WORKORDER", "a = :a or b = :b"),
        ];
        let f = reconcile(&world(), &rows, &[]);
        assert_eq!(f.links, Vec::new());
        assert_eq!(f.unchecked.len(), 1);
        assert_eq!(f.unsupported.len(), 1);
    }

    #[test]
    fn constant_filters_restrict_the_child_side() {
        let tables = vec![
            t("PARENT", "id\nA\nB\n"),
            t(
                "CHILD",
                "pid\tstatus\nA\tACTIVE\nB\tACTIVE\nB\tOLD\nZ\tOLD\n",
            ),
        ];
        let rows = [row(
            "ACTIVEKIDS",
            "PARENT",
            "CHILD",
            "pid = :id and status = 'ACTIVE'",
        )];
        let l = &reconcile(&tables, &rows, &[]).links[0];
        // Only ACTIVE rows count: both parent ids appear, so the parent side is fully found in the child.
        assert_eq!(l.fk_table, "CHILD");
        assert_eq!(l.orphan_rows, 0, "the orphan Z row is outside the filter");
    }
}
