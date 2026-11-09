//! Statistical discovery of undeclared relationships (the inference tier, ADR eng-0007).
//!
//! Method, following the literature (`HoPF`; `Auto-BI`) and adding provenance:
//! 1. **Profile** every column: null count, distinct values, value kind, length range, name tokens.
//! 2. **Generate candidates once**: unary inclusion dependencies into unique columns, and composite
//!    (two-column) inclusion dependencies into unique column pairs. This is the expensive step.
//! 3. **Score** each candidate with explainable features (containment, key-space coverage, support,
//!    name similarity, shape, position, "closer parent exists"). Weights are data, so they can be
//!    tuned and every decision can be explained.
//! 4. **Resolve globally**: each set of child columns keeps its best parent; a close runner-up marks
//!    the link ambiguous so a human reviews it instead of the system guessing.
//!
//! Everything is deterministic: the same tables give the same candidates in the same order.

use crate::table::Table;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Settings for candidate generation.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Minimum share of a child's non-null rows whose values exist in the parent. Below 1.0 so that
    /// a few orphan rows in a real database do not hide a relationship.
    pub min_containment: f64,
    /// Composite search keeps this many best child columns per parent key position.
    pub top_k: usize,
    /// Search two-column parent keys too.
    pub composite: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            min_containment: 0.98,
            top_k: 3,
            composite: true,
        }
    }
}

/// The evidence for one candidate, each in `0.0..=1.0` unless noted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Features {
    /// Share of the child's non-null rows found in the parent.
    pub containment: f64,
    /// Share of the parent's distinct keys the child actually uses.
    pub coverage: f64,
    /// How many distinct child values back the claim (more is stronger).
    pub support: f64,
    /// How many rows the parent table has (a one-row parent proves little).
    pub parent_support: f64,
    /// Leading position of the parent key columns in their table (keys usually come first).
    pub parent_position: f64,
    /// Similarity between the child column name and the parent column name.
    pub name_col: f64,
    /// Share of the parent table name's words that appear in the child column name.
    pub name_tab: f64,
    /// Agreement of value kind and length range.
    pub shape: f64,
    /// `1.0` when another candidate parent is a strict subset of this one (a closer parent exists).
    pub dominated: f64,
    /// `1.0` when child and parent are the same table.
    pub self_ref: f64,
    /// `1.0` for a two-column candidate.
    pub composite: f64,
    /// `1.0` when the child column is itself unique (a key looking like a foreign key).
    pub child_unique: f64,
    /// For integer parents: how densely their values fill their range (a dense sequence includes
    /// other small integer columns by chance). Zero for non-integer parents.
    pub parent_density: f64,
}

/// A possible relationship with its evidence. Indices refer to the `tables` slice given to [`candidates`].
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// Child table index.
    pub child_table: usize,
    /// Child column indices.
    pub child_cols: Vec<usize>,
    /// Parent table index.
    pub parent_table: usize,
    /// Parent key column indices, aligned with `child_cols`.
    pub parent_cols: Vec<usize>,
    /// The evidence.
    pub features: Features,
}

/// Linear weights over [`Features`], plus a bias. The score is `bias + sum(weight * feature)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    /// Constant term.
    pub bias: f64,
    /// Weight of [`Features::containment`].
    pub containment: f64,
    /// Weight of [`Features::coverage`].
    pub coverage: f64,
    /// Weight of [`Features::support`].
    pub support: f64,
    /// Weight of [`Features::parent_support`].
    pub parent_support: f64,
    /// Weight of [`Features::parent_position`].
    pub parent_position: f64,
    /// Weight of [`Features::name_col`].
    pub name_col: f64,
    /// Weight of [`Features::name_tab`].
    pub name_tab: f64,
    /// Weight of [`Features::shape`].
    pub shape: f64,
    /// Weight of [`Features::dominated`].
    pub dominated: f64,
    /// Weight of [`Features::self_ref`].
    pub self_ref: f64,
    /// Weight of [`Features::composite`].
    pub composite: f64,
    /// Weight of [`Features::child_unique`].
    pub child_unique: f64,
    /// Weight of [`Features::parent_density`].
    pub parent_density: f64,
}

impl Weights {
    /// Evidence only from inclusion: every candidate scores the same. The "inclusion alone" baseline.
    #[must_use]
    pub fn inclusion_only() -> Self {
        Self {
            bias: 1.0,
            containment: 0.0,
            coverage: 0.0,
            support: 0.0,
            parent_support: 0.0,
            parent_position: 0.0,
            name_col: 0.0,
            name_tab: 0.0,
            shape: 0.0,
            dominated: 0.0,
            self_ref: 0.0,
            composite: 0.0,
            child_unique: 0.0,
            parent_density: 0.0,
        }
    }

    /// The score of a candidate.
    #[must_use]
    pub fn score(&self, f: &Features) -> f64 {
        self.bias
            + self.containment * f.containment
            + self.coverage * f.coverage
            + self.support * f.support
            + self.parent_support * f.parent_support
            + self.parent_position * f.parent_position
            + self.name_col * f.name_col
            + self.name_tab * f.name_tab
            + self.shape * f.shape
            + self.dominated * f.dominated
            + self.self_ref * f.self_ref
            + self.composite * f.composite
            + self.child_unique * f.child_unique
            + self.parent_density * f.parent_density
    }
}

impl Default for Weights {
    /// Weights fitted by coordinate descent on the `OFBiz` train modules plus Chinook, Northwind and Sakila
    /// (`fkloco --weights-only`); first fitted on `OFBiz` train modules alone, against the
    /// conservative (forward-declared only) ground truth. See `docs/research/fk-discovery.md`.
    fn default() -> Self {
        Self {
            bias: -4.0,
            containment: -1.5,
            coverage: 0.0,
            support: 2.0,
            parent_support: 1.5,
            parent_position: 1.0,
            name_col: 2.5,
            name_tab: 3.0,
            shape: 0.5,
            dominated: -1.0,
            self_ref: 2.0,
            composite: -2.0,
            child_unique: -1.5,
            parent_density: 0.0,
        }
    }
}

impl Weights {
    /// Weights for sources whose names carry no meaning (`T0001`, `C01`). Name features are switched
    /// off and the column-order cue is not used. Fitted on the train modules of the `OFBiz` benchmark
    /// rewritten with opaque names and shuffled columns (held-out F1 about 0.67; see the research note).
    #[must_use]
    pub fn opaque() -> Self {
        Self {
            bias: 1.5,
            containment: -1.5,
            coverage: -2.0,
            support: 6.5,
            parent_support: 3.5,
            parent_position: 0.0,
            name_col: 0.0,
            name_tab: 0.0,
            shape: -0.5,
            dominated: 0.0,
            self_ref: -1.5,
            composite: -6.5,
            child_unique: -1.5,
            parent_density: 0.0,
        }
    }

    /// The profile that fits this source: [`Weights::default`] when column names are meaningful,
    /// [`Weights::opaque`] when they are not (see [`name_informativeness`]).
    #[must_use]
    pub fn auto(tables: &[Table]) -> Self {
        if name_informativeness(tables) < 0.5 {
            Self::opaque()
        } else {
            Self::default()
        }
    }
}

/// Share of columns whose name holds at least one word of three or more letters (`ASSETNUM`,
/// `partyId`), as opposed to codes such as `C01` or `F3`. Near 1.0 for readable schemas, near 0.0
/// for opaque ones. Zero for an empty input.
#[must_use]
pub fn name_informativeness(tables: &[Table]) -> f64 {
    let (mut total, mut readable) = (0usize, 0usize);
    for c in tables.iter().flat_map(|t| &t.columns) {
        total += 1;
        readable += usize::from(
            tokens(&c.name)
                .iter()
                .any(|w| w.len() >= 3 && w.bytes().all(|b| b.is_ascii_lowercase())),
        );
    }
    frac(readable, total)
}

/// How candidates become links.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    /// Minimum score to accept the best candidate of a child column set.
    pub tau: f64,
    /// A runner-up within this margin of the best marks the link ambiguous.
    pub delta: f64,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            tau: 0.0,
            delta: 0.25,
        }
    }
}

/// A discovered relationship.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// Child table index.
    pub child_table: usize,
    /// Child column indices.
    pub child_cols: Vec<usize>,
    /// Parent table index.
    pub parent_table: usize,
    /// Parent column indices, aligned with `child_cols`.
    pub parent_cols: Vec<usize>,
    /// Score of this link.
    pub score: f64,
    /// A close runner-up exists, so a human should confirm which parent is meant.
    pub ambiguous: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Digits,
    Numeric,
    Text,
}

struct Prof<'a> {
    values: &'a [Option<String>],
    nonnull: usize,
    set: HashSet<&'a str>,
    unique: bool,
    kind: Kind,
    min_len: usize,
    max_len: usize,
    tokens: Vec<String>,
    /// Smallest and largest value when every value is a non-negative integer.
    range: Option<(u64, u64)>,
}

fn frac(n: usize, d: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(d.max(1)).unwrap_or(u32::MAX))
}

/// `ln(1 + n) / ln(1 + cap)`, clamped to at most 1.
fn log_scaled(n: usize, cap: f64) -> f64 {
    let x = f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    ((1.0 + x).ln() / (1.0 + cap).ln()).min(1.0)
}

/// Words of an identifier, split on non-alphanumerics and lower-to-upper case changes.
fn tokens(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_ascii_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch.to_ascii_lowercase());
        prev_lower = ch.is_ascii_lowercase();
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    let sa: HashSet<&String> = a.iter().collect();
    let sb: HashSet<&String> = b.iter().collect();
    let inter = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    frac(inter, union)
}

fn profile(t: &Table) -> Vec<Prof<'_>> {
    let rows = t.row_count();
    t.columns
        .iter()
        .map(|c| {
            let mut set: HashSet<&str> = HashSet::new();
            let (mut nonnull, mut all_digits, mut all_numeric) = (0usize, true, true);
            let (mut min_len, mut max_len) = (usize::MAX, 0usize);
            let mut range: Option<(u64, u64)> = None;
            for v in c.values.iter().flatten() {
                nonnull += 1;
                set.insert(v.as_str());
                let digits = !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit());
                all_digits &= digits;
                all_numeric &= !v.is_empty() && v.parse::<f64>().is_ok();
                if digits {
                    if let Ok(n) = v.parse::<u64>() {
                        range = Some(range.map_or((n, n), |(lo, hi)| (lo.min(n), hi.max(n))));
                    }
                }
                min_len = min_len.min(v.len());
                max_len = max_len.max(v.len());
            }
            let kind = if nonnull == 0 || !all_numeric {
                Kind::Text
            } else if all_digits {
                Kind::Digits
            } else {
                Kind::Numeric
            };
            Prof {
                values: &c.values,
                nonnull,
                unique: rows > 0 && nonnull == rows && set.len() == nonnull,
                set,
                kind,
                min_len: if nonnull == 0 { 0 } else { min_len },
                max_len,
                tokens: tokens(&c.name),
                range: if all_digits && nonnull > 0 {
                    range
                } else {
                    None
                },
            }
        })
        .collect()
}

fn shape(a: &Prof<'_>, b: &Prof<'_>) -> f64 {
    let kind = if a.kind == b.kind { 1.0 } else { 0.0 };
    let lo = a.min_len.max(b.min_len);
    let hi = a.max_len.min(b.max_len);
    let span = a.max_len.max(b.max_len) - a.min_len.min(b.min_len) + 1;
    let overlap = if hi >= lo {
        frac(hi - lo + 1, span)
    } else {
        0.0
    };
    0.5 * kind + 0.5 * overlap
}

/// Share of `child`'s non-null rows whose value is in `parent`; `None` when it falls below `theta`.
fn containment(child: &Prof<'_>, parent: &HashSet<&str>, theta: f64) -> Option<f64> {
    if child.nonnull == 0 {
        return None;
    }
    let allowed = child.nonnull - ceil_count(theta, child.nonnull);
    let mut misses = 0usize;
    for v in child.values.iter().flatten() {
        if !parent.contains(v.as_str()) {
            misses += 1;
            if misses > allowed {
                return None;
            }
        }
    }
    Some(frac(child.nonnull - misses, child.nonnull))
}

/// Smallest whole number of rows that meets share `theta` of `n`.
fn ceil_count(theta: f64, n: usize) -> usize {
    let mut k = 0usize;
    while k < n && frac(k, n) < theta {
        k += 1;
    }
    k
}

/// Share of an integer column's range that is occupied by distinct values; `0.0` when not integer.
fn density(p: &Prof<'_>) -> f64 {
    p.range.map_or(0.0, |(lo, hi)| {
        let span = usize::try_from(hi - lo)
            .unwrap_or(usize::MAX)
            .saturating_add(1);
        frac(p.set.len(), span).min(1.0)
    })
}

fn position(cols: &[usize]) -> f64 {
    let max = cols.iter().copied().max().unwrap_or(0);
    1.0 / (1.0 + frac(max.saturating_sub(cols.len() - 1), 1))
}

/// Generates every unary and (optionally) two-column candidate that satisfies inclusion.
///
/// Candidates carry features but no decision; see [`select`].
#[must_use]
pub fn candidates(tables: &[Table], params: &Params) -> Vec<Candidate> {
    let profs: Vec<Vec<Prof<'_>>> = tables.iter().map(profile).collect();
    let rows: Vec<usize> = tables.iter().map(Table::row_count).collect();
    let table_tokens: Vec<Vec<String>> = tables.iter().map(|t| tokens(&t.name)).collect();
    let mut out = unary_candidates(tables, &profs, &rows, &table_tokens, params);
    mark_dominated(&mut out, &profs);
    if params.composite {
        out.extend(composite_candidates(
            tables,
            &profs,
            &rows,
            &table_tokens,
            params,
        ));
    }
    out
}

fn unary_candidates(
    tables: &[Table],
    profs: &[Vec<Prof<'_>>],
    rows: &[usize],
    table_tokens: &[Vec<String>],
    params: &Params,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (ti, child_profs) in profs.iter().enumerate() {
        if rows[ti] == 0 {
            continue;
        }
        for (ci, a) in child_profs.iter().enumerate() {
            if a.nonnull == 0 {
                continue;
            }
            for (si, parent_profs) in profs.iter().enumerate() {
                for (di, b) in parent_profs.iter().enumerate() {
                    if !b.unique || (si, di) == (ti, ci) {
                        continue;
                    }
                    let Some(c) = containment(a, &b.set, params.min_containment) else {
                        continue;
                    };
                    let in_parent = a.set.iter().filter(|v| b.set.contains(**v)).count();
                    out.push(Candidate {
                        child_table: ti,
                        child_cols: vec![ci],
                        parent_table: si,
                        parent_cols: vec![di],
                        features: Features {
                            containment: c,
                            coverage: frac(in_parent, b.set.len()),
                            support: log_scaled(a.set.len(), 16.0),
                            parent_support: log_scaled(rows[si], 16.0),
                            parent_position: position(&[di]),
                            name_col: jaccard(&a.tokens, &b.tokens),
                            name_tab: frac(
                                table_tokens[si]
                                    .iter()
                                    .filter(|w| a.tokens.contains(w))
                                    .count(),
                                table_tokens[si].len(),
                            ),
                            shape: shape(a, b),
                            dominated: 0.0,
                            self_ref: if si == ti { 1.0 } else { 0.0 },
                            composite: 0.0,
                            child_unique: if a.unique { 1.0 } else { 0.0 },
                            parent_density: density(b),
                        },
                    });
                }
            }
        }
    }
    let _ = tables;
    out
}

/// Marks a unary candidate when another candidate parent of the same child column is a strict subset
/// of its parent column: the closer (smaller) parent is usually the intended one.
fn mark_dominated(cands: &mut [Candidate], profs: &[Vec<Prof<'_>>]) {
    let mut by_child: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        by_child
            .entry((c.child_table, c.child_cols[0]))
            .or_default()
            .push(i);
    }
    let set_of = |t: usize, c: usize| &profs[t][c].set;
    let mut flags = vec![false; cands.len()];
    for group in by_child.values() {
        for &i in group {
            let big = (cands[i].parent_table, cands[i].parent_cols[0]);
            for &j in group {
                let small = (cands[j].parent_table, cands[j].parent_cols[0]);
                if small != big
                    && set_of(small.0, small.1).len() < set_of(big.0, big.1).len()
                    && set_of(small.0, small.1)
                        .iter()
                        .all(|v| set_of(big.0, big.1).contains(*v))
                {
                    flags[i] = true;
                    break;
                }
            }
        }
    }
    for (c, flag) in cands.iter_mut().zip(flags) {
        if flag {
            c.features.dominated = 1.0;
        }
    }
}

/// Two-column unique combinations of a table, over columns that are never null and not unique alone.
/// A two-column unique combination: the column indices and the set of value pairs.
type PairKey<'a> = (usize, usize, HashSet<(&'a str, &'a str)>);

/// Candidates of one child column set, with their scores.
type Scored<'a> = Vec<(f64, &'a Candidate)>;

fn pair_keys<'a>(t: &'a Table, prof: &[Prof<'a>], rows: usize) -> Vec<PairKey<'a>> {
    if rows < 2 {
        return Vec::new();
    }
    let eligible: Vec<usize> = (0..t.columns.len())
        .filter(|&i| {
            prof[i].nonnull == rows
                && !prof[i].unique
                && prof[i].kind != Kind::Numeric
                && prof[i].max_len <= 64
        })
        .take(40)
        .collect();
    let mut keys = Vec::new();
    for (n, &i) in eligible.iter().enumerate() {
        for &j in &eligible[n + 1..] {
            let tuples: HashSet<(&str, &str)> = prof[i]
                .values
                .iter()
                .zip(prof[j].values)
                .filter_map(|(a, b)| Some((a.as_deref()?, b.as_deref()?)))
                .collect();
            if tuples.len() == rows {
                keys.push((i, j, tuples));
            }
        }
    }
    keys
}

fn composite_candidates(
    tables: &[Table],
    profs: &[Vec<Prof<'_>>],
    rows: &[usize],
    table_tokens: &[Vec<String>],
    params: &Params,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (si, s) in tables.iter().enumerate() {
        for (p1, p2, parent_tuples) in pair_keys(s, &profs[si], rows[si]) {
            for (ti, t) in tables.iter().enumerate() {
                if rows[ti] == 0 {
                    continue;
                }
                let best = |p: usize| -> Vec<(usize, f64)> {
                    let mut v: Vec<(usize, f64, f64)> = (0..t.columns.len())
                        .filter(|&a| (ti, a) != (si, p1) && (ti, a) != (si, p2))
                        .filter_map(|a| {
                            let c = containment(
                                &profs[ti][a],
                                &profs[si][p].set,
                                params.min_containment,
                            )?;
                            Some((a, c, jaccard(&profs[ti][a].tokens, &profs[si][p].tokens)))
                        })
                        .collect();
                    v.sort_by(|x, y| {
                        y.1.total_cmp(&x.1)
                            .then(y.2.total_cmp(&x.2))
                            .then(x.0.cmp(&y.0))
                    });
                    v.into_iter()
                        .take(params.top_k)
                        .map(|(a, _, n)| (a, n))
                        .collect()
                };
                let (l1, l2) = (best(p1), best(p2));
                for &(a1, n1) in &l1 {
                    for &(a2, n2) in &l2 {
                        if a1 == a2 {
                            continue;
                        }
                        let (pa1, pa2) = (&profs[ti][a1], &profs[ti][a2]);
                        let mut seen: HashSet<(&str, &str)> = HashSet::new();
                        let (mut nn, mut hits) = (0usize, 0usize);
                        for (x, y) in pa1.values.iter().zip(pa2.values) {
                            if let (Some(x), Some(y)) = (x.as_deref(), y.as_deref()) {
                                nn += 1;
                                if parent_tuples.contains(&(x, y)) {
                                    hits += 1;
                                    seen.insert((x, y));
                                }
                            }
                        }
                        if nn == 0 || frac(hits, nn) < params.min_containment {
                            continue;
                        }
                        let tok = |p: usize, a: &Prof<'_>| {
                            frac(
                                table_tokens[si]
                                    .iter()
                                    .filter(|w| a.tokens.contains(w))
                                    .count(),
                                table_tokens[si].len(),
                            )
                            .max(0.0 * frac(p, 1))
                        };
                        out.push(Candidate {
                            child_table: ti,
                            child_cols: vec![a1, a2],
                            parent_table: si,
                            parent_cols: vec![p1, p2],
                            features: Features {
                                containment: frac(hits, nn),
                                coverage: frac(seen.len(), parent_tuples.len()),
                                support: log_scaled(seen.len(), 16.0),
                                parent_support: log_scaled(rows[si], 16.0),
                                parent_position: position(&[p1, p2]),
                                name_col: f64::midpoint(n1, n2),
                                name_tab: f64::midpoint(tok(p1, pa1), tok(p2, pa2)),
                                shape: f64::midpoint(
                                    shape(pa1, &profs[si][p1]),
                                    shape(pa2, &profs[si][p2]),
                                ),
                                dominated: 0.0,
                                self_ref: if si == ti { 1.0 } else { 0.0 },
                                composite: 1.0,
                                child_unique: 0.0,
                                parent_density: 0.0,
                            },
                        });
                    }
                }
            }
        }
    }
    out
}

/// Chooses links from candidates: per child column set, the best-scoring candidate if it reaches
/// `tau`, flagged ambiguous when a runner-up is within `delta`. Output is sorted for determinism.
#[must_use]
pub fn select(cands: &[Candidate], weights: &Weights, sel: &Selection) -> Vec<Link> {
    let mut groups: BTreeMap<(usize, &[usize]), Scored<'_>> = BTreeMap::new();
    for c in cands {
        groups
            .entry((c.child_table, c.child_cols.as_slice()))
            .or_default()
            .push((weights.score(&c.features), c));
    }
    let mut links = Vec::new();
    for group in groups.values_mut() {
        group.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(a.1.parent_table.cmp(&b.1.parent_table))
                .then(a.1.parent_cols.cmp(&b.1.parent_cols))
        });
        let (score, best) = group[0];
        if score < sel.tau {
            continue;
        }
        let ambiguous = group
            .get(1)
            .is_some_and(|(second, _)| score - second <= sel.delta);
        links.push(Link {
            child_table: best.child_table,
            child_cols: best.child_cols.clone(),
            parent_table: best.parent_table,
            parent_cols: best.parent_cols.clone(),
            score,
            ambiguous,
        });
    }
    links
}

/// Convenience: candidates, then selection, with the weight profile that fits the source.
#[must_use]
pub fn discover(tables: &[Table]) -> Vec<Link> {
    select(
        &candidates(tables, &Params::default()),
        &Weights::auto(tables),
        &Selection::default(),
    )
}

#[cfg(test)]
// Test fixtures build text with `format!` and `push_str`: clearer than `write!` and fine here.
#[allow(clippy::format_push_string)]
mod tests {
    use super::*;

    /// Mechanics tests state their own weights: the shipped defaults are calibrated on real data and
    /// rightly distrust links backed by three or four rows.
    fn permissive() -> Weights {
        Weights {
            name_col: 1.0,
            containment: 1.0,
            ..Weights::inclusion_only()
        }
    }

    fn find(t: &[Table], w: &Weights) -> Vec<Link> {
        select(
            &candidates(t, &Params::default()),
            w,
            &Selection {
                tau: 0.5,
                delta: 0.25,
            },
        )
    }

    fn table(name: &str, tsv: &str) -> Table {
        Table::from_tsv(name, tsv).unwrap()
    }

    fn party_world() -> Vec<Table> {
        vec![
            table(
                "Party",
                "partyId\tname\nP1\tann\nP2\tbob\nP3\tcy\nP4\tdee\n",
            ),
            table(
                "Invoice",
                "invoiceId\tpartyId\tamount\nI1\tP1\t10\nI2\tP2\t20\nI3\tP2\t5\nI4\tP4\t7\n",
            ),
            table(
                "Note",
                "noteId\ttext\nN1\thello\nN2\tworld\nN3\tagain\nN4\tbye\n",
            ),
        ]
    }

    fn name(tables: &[Table], l: &Link) -> (String, Vec<String>, String, Vec<String>) {
        let cols = |t: usize, cs: &[usize]| {
            cs.iter()
                .map(|&c| tables[t].columns[c].name.clone())
                .collect()
        };
        (
            tables[l.child_table].name.clone(),
            cols(l.child_table, &l.child_cols),
            tables[l.parent_table].name.clone(),
            cols(l.parent_table, &l.parent_cols),
        )
    }

    #[test]
    fn finds_a_plain_foreign_key_and_nothing_else() {
        let t = party_world();
        let links = discover(&t);
        let found: Vec<_> = links.iter().map(|l| name(&t, l)).collect();
        assert!(
            found.contains(&(
                "Invoice".into(),
                vec!["partyId".into()],
                "Party".into(),
                vec!["partyId".into()]
            )),
            "{found:?}"
        );
        assert!(
            !found.iter().any(|f| f.0 == "Note"),
            "unrelated table linked: {found:?}"
        );
    }

    #[test]
    fn finds_a_composite_foreign_key() {
        let t = vec![
            table("Site", "orgId\tsiteId\nO1\tS1\nO1\tS2\nO2\tS1\nO2\tS3\n"),
            table(
                "Asset",
                "assetNum\torgId\tsiteId\nA1\tO1\tS1\nA2\tO1\tS2\nA3\tO2\tS1\nA4\tO2\tS1\n",
            ),
        ];
        let found: Vec<_> = find(&t, &permissive())
            .iter()
            .map(|l| name(&t, l))
            .collect();
        let want = (
            "Asset".into(),
            vec!["orgId".into(), "siteId".into()],
            "Site".into(),
            vec!["orgId".into(), "siteId".into()],
        );
        assert!(found.contains(&want), "{found:?}");
    }

    #[test]
    fn every_link_really_satisfies_inclusion() {
        let t = party_world();
        let params = Params::default();
        for l in select(
            &candidates(&t, &params),
            &Weights::default(),
            &Selection::default(),
        ) {
            let child = &t[l.child_table];
            let parent = &t[l.parent_table];
            let mut nn = 0usize;
            let mut hits = 0usize;
            for r in 0..child.row_count() {
                let key: Option<Vec<&str>> = l
                    .child_cols
                    .iter()
                    .map(|&c| child.columns[c].values[r].as_deref())
                    .collect();
                if let Some(key) = key {
                    nn += 1;
                    let found = (0..parent.row_count()).any(|pr| {
                        l.parent_cols
                            .iter()
                            .zip(&key)
                            .all(|(&pc, k)| parent.columns[pc].values[pr].as_deref() == Some(*k))
                    });
                    hits += usize::from(found);
                }
            }
            assert!(frac(hits, nn) >= params.min_containment, "{l:?}");
        }
    }

    #[test]
    fn tolerates_a_few_orphans_but_not_many() {
        let mut parent = String::from("id\n");
        let mut child = String::from("rowId\tid\n");
        for i in 0..100 {
            parent.push_str(&format!("K{i}\n"));
            child.push_str(&format!("R{i}\tK{}\n", i % 50));
        }
        child.push_str("R100\tNOPE\n");
        let ok = vec![table("Parent", &parent), table("Child", &child)];
        assert!(
            discover(&ok)
                .iter()
                .any(|l| ok[l.child_table].name == "Child"),
            "one orphan in 101 rows must not hide the link"
        );
        for i in 0..20 {
            child.push_str(&format!("X{i}\tNOPE{i}\n"));
        }
        let bad = vec![table("Parent", &parent), table("Child", &child)];
        assert!(
            !discover(&bad)
                .iter()
                .any(|l| bad[l.child_table].name == "Child"),
            "20% orphans must hide it"
        );
    }

    #[test]
    fn output_is_deterministic() {
        let t = party_world();
        assert_eq!(discover(&t), discover(&t));
    }

    #[test]
    fn a_close_runner_up_is_flagged_ambiguous() {
        let t = vec![
            table("Alpha", "k\nA\nB\nC\n"),
            table("Beta", "k\nA\nB\nC\n"),
            table("Child", "id\tk\n1\tA\n2\tB\n3\tC\n"),
        ];
        let links = find(&t, &permissive());
        assert!(
            links
                .iter()
                .any(|l| t[l.child_table].name == "Child" && l.ambiguous),
            "{links:?}"
        );
    }

    #[test]
    fn name_informativeness_tells_readable_from_opaque() {
        assert!(name_informativeness(&party_world()) > 0.9);
        let opaque = vec![
            table("T0001", "C01\tC02\nP1\tx\n"),
            table("T0002", "C01\tC03\nP1\t9\n"),
        ];
        assert!(name_informativeness(&opaque) < 0.1);
        assert!(name_informativeness(&[]).abs() < 1e-12);
        assert_eq!(Weights::auto(&party_world()), Weights::default());
        assert_eq!(Weights::auto(&opaque), Weights::opaque());
    }

    #[test]
    fn opaque_names_still_find_a_key_with_data_evidence_alone() {
        let t = vec![
            table("T0001", "C01\tC02\nP1\tann\nP2\tbob\nP3\tcy\nP4\tdee\nP5\teve\nP6\tfay\n"),
            table("T0002", "C01\tC02\tC03\nI1\tP1\t10\nI2\tP2\t20\nI3\tP2\t5\nI4\tP4\t7\nI5\tP5\t9\nI6\tP6\t2\n"),
        ];
        let links = discover(&t);
        assert!(
            links
                .iter()
                .any(|l| l.child_table == 1 && l.child_cols == [1] && l.parent_table == 0),
            "{links:?}"
        );
    }

    #[test]
    fn name_tokens_split_camel_and_snake_case() {
        assert_eq!(tokens("fromPartyId"), ["from", "party", "id"]);
        assert_eq!(tokens("PARTY_ID"), ["party", "id"]);
        assert_eq!(tokens("partyID"), ["party", "id"]);
    }
}
