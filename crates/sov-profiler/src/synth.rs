//! A deterministic, Maximo-shaped synthetic database with known ground truth.
//!
//! **Synthetic, built from IBM's documented conventions, not from any real installation.** It exists
//! because no real Maximo dictionary is available: it lets both discovery tiers and the reconciler be
//! tested against relationships whose truth is known by construction. A good score here shows the
//! machinery is consistent, not that it will work on a customer's database.
//!
//! Modelled on purpose: composite keys that repeat across sites (`ASSETNUM`+`SITEID`), a three-column
//! key, self-referencing hierarchies, relationships under different column names (`REFWO` to `WONUM`),
//! undeclared links, reverse-navigation dictionary rows, unparseable dictionary clauses, a polymorphic
//! reference (`OWNER`/`OWNERTABLE`), dense integer surrogate ids, orphan rows, and balance drift.

use crate::maximo::RelationshipRow;
use crate::table::{Column, Table};
use sov_sim::Rng;
use std::collections::HashSet;

/// A relationship that exists in the data by construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TruthLink {
    /// Table holding the foreign key.
    pub fk_table: &'static str,
    /// Foreign-key columns.
    pub fk_cols: Vec<&'static str>,
    /// Referenced table.
    pub ref_table: &'static str,
    /// Referenced columns, aligned with `fk_cols`.
    pub ref_cols: Vec<&'static str>,
    /// Whether the data dictionary (`MAXRELATIONSHIP`) declares it.
    pub in_dictionary: bool,
}

/// The generated database and everything known about it.
#[derive(Clone, Debug)]
pub struct Synthetic {
    /// Every table, dictionary included.
    pub tables: Vec<Table>,
    /// The `MAXRELATIONSHIP` rows.
    pub dictionary: Vec<RelationshipRow>,
    /// Relationships present in the data.
    pub truth: Vec<TruthLink>,
    /// How many dictionary rows are outside the supported clause subset.
    pub unsupported_clauses: usize,
    /// Orphan rows injected: (table, column, count).
    pub orphans: Vec<(&'static str, &'static str, usize)>,
    /// Stored balances deliberately made wrong: (item, location, site).
    pub drifted_balances: Vec<[String; 3]>,
}

type Row = Vec<Option<String>>;

struct Builder {
    name: &'static str,
    cols: Vec<&'static str>,
    rows: Vec<Row>,
}

impl Builder {
    fn new(name: &'static str, cols: &[&'static str]) -> Self {
        Self {
            name,
            cols: cols.to_vec(),
            rows: Vec::new(),
        }
    }
    fn push(&mut self, cells: Vec<Option<String>>) {
        debug_assert_eq!(cells.len(), self.cols.len());
        self.rows.push(cells);
    }
    fn build(self) -> Table {
        let columns = self
            .cols
            .iter()
            .enumerate()
            .map(|(i, name)| Column {
                name: (*name).to_owned(),
                values: self.rows.iter().map(|r| r[i].clone()).collect(),
            })
            .collect();
        Table {
            name: self.name.to_owned(),
            columns,
        }
    }
}

// A terse cell constructor used hundreds of times: the `Option` is the table's cell type.
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
fn v(x: impl ToString) -> Option<String> {
    Some(x.to_string())
}

fn pick(rng: &mut Rng, n: usize) -> usize {
    usize::try_from(rng.below(u64::try_from(n.max(1)).unwrap_or(1))).unwrap_or(0)
}

fn link(
    fk_table: &'static str,
    fk_cols: &[&'static str],
    ref_table: &'static str,
    ref_cols: &[&'static str],
    in_dictionary: bool,
) -> TruthLink {
    TruthLink {
        fk_table,
        fk_cols: fk_cols.to_vec(),
        ref_table,
        ref_cols: ref_cols.to_vec(),
        in_dictionary,
    }
}

/// The relationships built into the data, in the order of the module documentation.
#[allow(clippy::too_many_lines)] // a data table, clearest as one list
fn truth_links() -> Vec<TruthLink> {
    let mut links = vec![
        link("SITE", &["ORGID"], "ORGANIZATION", &["ORGID"], true),
        link("LABOR", &["PERSONID"], "PERSON", &["PERSONID"], true),
        link("LABOR", &["ORGID"], "ORGANIZATION", &["ORGID"], true),
        link("COMPANIES", &["ORGID"], "ORGANIZATION", &["ORGID"], true),
        link("LOCATIONS", &["SITEID"], "SITE", &["SITEID"], true),
        link(
            "LOCATIONS",
            &["PARENT", "SITEID"],
            "LOCATIONS",
            &["LOCATION", "SITEID"],
            true,
        ),
        link(
            "ASSET",
            &["LOCATION", "SITEID"],
            "LOCATIONS",
            &["LOCATION", "SITEID"],
            true,
        ),
        link(
            "ASSET",
            &["PARENT", "SITEID"],
            "ASSET",
            &["ASSETNUM", "SITEID"],
            true,
        ),
        link(
            "ASSET",
            &["VENDOR", "ORGID"],
            "COMPANIES",
            &["COMPANY", "ORGID"],
            true,
        ),
        link(
            "INVENTORY",
            &["ITEMNUM", "ITEMSETID"],
            "ITEM",
            &["ITEMNUM", "ITEMSETID"],
            true,
        ),
        link(
            "INVENTORY",
            &["LOCATION", "SITEID"],
            "LOCATIONS",
            &["LOCATION", "SITEID"],
            true,
        ),
        link(
            "INVBALANCES",
            &["ITEMNUM", "LOCATION", "SITEID"],
            "INVENTORY",
            &["ITEMNUM", "LOCATION", "SITEID"],
            true,
        ),
        link(
            "INVTRANS",
            &["ITEMNUM", "STORELOC", "SITEID"],
            "INVENTORY",
            &["ITEMNUM", "LOCATION", "SITEID"],
            true,
        ),
        link(
            "WORKORDER",
            &["ASSETNUM", "SITEID"],
            "ASSET",
            &["ASSETNUM", "SITEID"],
            true,
        ),
        link(
            "WORKORDER",
            &["LOCATION", "SITEID"],
            "LOCATIONS",
            &["LOCATION", "SITEID"],
            true,
        ),
        link(
            "WORKORDER",
            &["PARENT", "SITEID"],
            "WORKORDER",
            &["WONUM", "SITEID"],
            true,
        ),
        link("WORKORDER", &["REPORTEDBY"], "PERSON", &["PERSONID"], false),
        link(
            "LABTRANS",
            &["LABORCODE", "ORGID"],
            "LABOR",
            &["LABORCODE", "ORGID"],
            true,
        ),
        link(
            "LABTRANS",
            &["REFWO", "SITEID"],
            "WORKORDER",
            &["WONUM", "SITEID"],
            false,
        ),
        link(
            "MATUSETRANS",
            &["ITEMNUM", "STORELOC", "SITEID"],
            "INVENTORY",
            &["ITEMNUM", "LOCATION", "SITEID"],
            true,
        ),
        link(
            "MATUSETRANS",
            &["REFWO", "SITEID"],
            "WORKORDER",
            &["WONUM", "SITEID"],
            true,
        ),
        link(
            "PO",
            &["VENDOR", "ORGID"],
            "COMPANIES",
            &["COMPANY", "ORGID"],
            true,
        ),
        link(
            "POLINE",
            &["PONUM", "SITEID"],
            "PO",
            &["PONUM", "SITEID"],
            true,
        ),
        link("POLINE", &["ITEMNUM"], "ITEM", &["ITEMNUM"], false),
        link(
            "SR",
            &["ASSETNUM", "SITEID"],
            "ASSET",
            &["ASSETNUM", "SITEID"],
            true,
        ),
        link("SR", &["REPORTEDBY"], "PERSON", &["PERSONID"], false),
        link(
            "SR",
            &["LOCATION", "SITEID"],
            "LOCATIONS",
            &["LOCATION", "SITEID"],
            false,
        ),
    ];
    // Links that hold by construction without being designed one by one: every site-level table
    // references SITE, and so on. They are real relationships and are scored as such.
    for t in [
        "ASSET",
        "INVENTORY",
        "INVBALANCES",
        "INVTRANS",
        "WORKORDER",
        "LABTRANS",
        "MATUSETRANS",
        "PO",
        "POLINE",
        "SR",
    ] {
        links.push(link(t, &["SITEID"], "SITE", &["SITEID"], false));
    }
    for t in ["LOCATIONS", "ASSET", "WORKORDER", "PO", "LABTRANS"] {
        links.push(link(t, &["ORGID"], "ORGANIZATION", &["ORGID"], false));
    }
    for t in ["INVENTORY", "INVBALANCES", "INVTRANS", "MATUSETRANS"] {
        links.push(link(t, &["ITEMNUM"], "ITEM", &["ITEMNUM"], false));
    }
    links.push(link(
        "LABTRANS",
        &["LABORCODE"],
        "LABOR",
        &["LABORCODE"],
        false,
    ));
    links.push(link(
        "INVBALANCES",
        &["LOCATION", "SITEID"],
        "LOCATIONS",
        &["LOCATION", "SITEID"],
        false,
    ));
    links
}

/// Dictionary rows: forward navigation (the object holding the key navigates to the referenced object)
/// for every declared link, reverse navigation for some, and a few clauses outside the supported subset.
fn dictionary_rows(truth: &[TruthLink]) -> (Vec<RelationshipRow>, usize) {
    let mut rows = Vec::new();
    for (i, t) in truth.iter().enumerate().filter(|(_, t)| t.in_dictionary) {
        let forward: Vec<String> = t
            .ref_cols
            .iter()
            .zip(&t.fk_cols)
            .map(|(r, f)| format!("{} = :{}", r.to_lowercase(), f.to_lowercase()))
            .collect();
        rows.push(RelationshipRow {
            name: t.ref_table.to_owned(),
            parent: t.fk_table.to_owned(),
            child: t.ref_table.to_owned(),
            where_clause: forward.join(" and "),
        });
        if i % 3 == 0 {
            let reverse: Vec<String> = t
                .fk_cols
                .iter()
                .zip(&t.ref_cols)
                .map(|(f, r)| format!("{} = :{}", f.to_lowercase(), r.to_lowercase()))
                .collect();
            rows.push(RelationshipRow {
                name: format!("{}_LIST", t.fk_table),
                parent: t.ref_table.to_owned(),
                child: t.fk_table.to_owned(),
                where_clause: reverse.join(" and "),
            });
        }
    }
    let odd = [
        (
            "ACTIVEWO",
            "ASSET",
            "WORKORDER",
            "status in ('APPR','INPRG') and assetnum = :assetnum and siteid = :siteid",
        ),
        (
            "OWNERPERSON",
            "WORKORDER",
            "PERSON",
            "personid = :owner and :ownertable = 'PERSON'",
        ),
        (
            "OPENSR",
            "ASSET",
            "SR",
            "(status = 'NEW' or status = 'QUEUED') and assetnum = :assetnum",
        ),
        (
            "UPPERNAME",
            "PERSON",
            "LABOR",
            "upper(personid) = :personid",
        ),
    ];
    for (name, parent, child, clause) in odd {
        rows.push(RelationshipRow {
            name: name.to_owned(),
            parent: parent.to_owned(),
            child: child.to_owned(),
            where_clause: clause.to_owned(),
        });
    }
    (rows, odd.len())
}

fn dictionary_table(rows: &[RelationshipRow]) -> Table {
    let col = |name: &str, f: &dyn Fn(&RelationshipRow) -> String| Column {
        name: name.to_owned(),
        values: rows.iter().map(|r| Some(f(r))).collect(),
    };
    Table {
        name: "MAXRELATIONSHIP".to_owned(),
        columns: vec![
            col("NAME", &|r| r.name.clone()),
            col("PARENT", &|r| r.parent.clone()),
            col("CHILD", &|r| r.child.clone()),
            col("WHERECLAUSE", &|r| r.where_clause.clone()),
        ],
    }
}

const SITES: [(&str, &str); 4] = [
    ("S1", "ORG1"),
    ("S2", "ORG1"),
    ("S3", "ORG2"),
    ("S4", "ORG2"),
];

/// Generates the database. The same `seed` always gives the same result.
#[must_use]
#[allow(clippy::too_many_lines)] // a generator reads best as one top-to-bottom story
pub fn maximo_shaped(seed: u64) -> Synthetic {
    let mut rng = Rng::new(seed);
    let mut orphans: Vec<(&'static str, &'static str, usize)> = Vec::new();
    let mut surrogate = 0u32;
    let mut next_id = || {
        surrogate += 1;
        surrogate
    };

    let mut org = Builder::new("ORGANIZATION", &["ORGID", "NAME"]);
    for o in ["ORG1", "ORG2"] {
        org.push(vec![v(o), v(format!("Organisation {o}"))]);
    }
    let mut site = Builder::new("SITE", &["SITEID", "ORGID", "DESCRIPTION"]);
    for (s, o) in SITES {
        site.push(vec![v(s), v(o), v(format!("Site {s}"))]);
    }
    let mut person = Builder::new("PERSON", &["PERSONID", "DISPLAYNAME", "STATUS"]);
    for i in 1..=40 {
        person.push(vec![
            v(format!("P{i:04}")),
            v(format!("Person {i}")),
            v("ACTIVE"),
        ]);
    }
    let mut labor = Builder::new("LABOR", &["LABORCODE", "ORGID", "PERSONID"]);
    for i in 1..=30 {
        labor.push(vec![
            v(format!("LC{i:03}")),
            v(["ORG1", "ORG2"][i % 2]),
            v(format!("P{:04}", 1 + i)),
        ]);
    }
    let mut companies = Builder::new("COMPANIES", &["COMPANY", "ORGID", "NAME"]);
    for i in 1..=20 {
        companies.push(vec![
            v(format!("C{:04}", 1 + (i - 1) / 2)),
            v(["ORG1", "ORG2"][i % 2]),
            v(format!("Company {i}")),
        ]);
    }
    let mut item = Builder::new("ITEM", &["ITEMNUM", "ITEMSETID", "DESCRIPTION"]);
    for i in 1..=120 {
        item.push(vec![
            v(format!("I{i:04}")),
            v("SET1"),
            v(format!("Item {i}")),
        ]);
    }

    let mut locations = Builder::new(
        "LOCATIONS",
        &["LOCATION", "SITEID", "ORGID", "PARENT", "LOCATIONSID"],
    );
    let mut asset = Builder::new(
        "ASSET",
        &[
            "ASSETNUM", "SITEID", "ORGID", "LOCATION", "PARENT", "VENDOR", "ASSETID",
        ],
    );
    let mut inventory = Builder::new(
        "INVENTORY",
        &["ITEMNUM", "LOCATION", "SITEID", "ITEMSETID", "INVENTORYID"],
    );
    let mut invbal = Builder::new("INVBALANCES", &["ITEMNUM", "LOCATION", "SITEID", "CURBAL"]);
    let mut invtrans = Builder::new(
        "INVTRANS",
        &["ITEMNUM", "STORELOC", "SITEID", "QUANTITY", "INVTRANSID"],
    );
    let mut workorder = Builder::new(
        "WORKORDER",
        &[
            "WONUM",
            "SITEID",
            "ORGID",
            "ASSETNUM",
            "LOCATION",
            "PARENT",
            "REPORTEDBY",
            "STATUS",
            "OWNER",
            "OWNERTABLE",
            "WORKORDERID",
        ],
    );
    let mut matuse = Builder::new(
        "MATUSETRANS",
        &["ITEMNUM", "STORELOC", "SITEID", "REFWO", "MATUSETRANSID"],
    );
    let mut po = Builder::new("PO", &["PONUM", "SITEID", "ORGID", "VENDOR", "POID"]);
    let mut poline = Builder::new(
        "POLINE",
        &["PONUM", "SITEID", "POLINENUM", "ITEMNUM", "POLINEID"],
    );
    let mut sr = Builder::new(
        "SR",
        &[
            "TICKETID",
            "ASSETNUM",
            "SITEID",
            "LOCATION",
            "REPORTEDBY",
            "SRID",
        ],
    );
    let mut drifted = Vec::new();

    let mut ticket = 0u32;
    for (s, o) in SITES {
        let locs: Vec<String> = (1..=25).map(|i| format!("L{i:03}")).collect();
        for (i, l) in locs.iter().enumerate() {
            let parent = if i > 0 && pick(&mut rng, 10) < 3 {
                v(&locs[pick(&mut rng, i)])
            } else {
                None
            };
            locations.push(vec![v(l), v(s), v(o), parent, v(next_id())]);
        }
        let assets: Vec<String> = (1..=75).map(|i| format!("A{i:04}")).collect();
        let vendors: Vec<String> = (1..=10).map(|i| format!("C{i:04}")).collect();
        for (i, a) in assets.iter().enumerate() {
            let parent = if i > 0 && pick(&mut rng, 10) < 2 {
                v(&assets[pick(&mut rng, i)])
            } else {
                None
            };
            asset.push(vec![
                v(a),
                v(s),
                v(o),
                v(&locs[pick(&mut rng, locs.len())]),
                parent,
                v(&vendors[pick(&mut rng, 10)]),
                v(next_id()),
            ]);
        }
        let mut stock: Vec<(String, String)> = Vec::new();
        let mut taken: HashSet<(String, String)> = HashSet::new();
        while stock.len() < 100 {
            let key = (
                format!("I{:04}", 1 + pick(&mut rng, 120)),
                locs[pick(&mut rng, locs.len())].clone(),
            );
            if taken.insert(key.clone()) {
                stock.push(key);
            }
        }
        for (it, loc) in &stock {
            inventory.push(vec![v(it), v(loc), v(s), v("SET1"), v(next_id())]);
            let mut balance: i64 = 0;
            for _ in 0..(2 + pick(&mut rng, 4)) {
                let q: i64 = if pick(&mut rng, 3) == 0 {
                    -i64::try_from(1 + pick(&mut rng, 6)).unwrap_or(1)
                } else {
                    i64::try_from(5 + pick(&mut rng, 40)).unwrap_or(5)
                };
                balance += q;
                invtrans.push(vec![v(it), v(loc), v(s), v(q), v(next_id())]);
            }
            invbal.push(vec![v(it), v(loc), v(s), v(balance)]);
        }
        let mut wonums: Vec<String> = Vec::new();
        for i in 0..300 {
            let wonum = format!("{}", 1001 + i);
            let parent = if i > 0 && pick(&mut rng, 10) == 0 {
                v(&wonums[pick(&mut rng, wonums.len())])
            } else {
                None
            };
            let (owner, table) = if pick(&mut rng, 2) == 0 {
                (format!("P{:04}", 1 + pick(&mut rng, 40)), "PERSON")
            } else {
                (format!("PG{:02}", 1 + pick(&mut rng, 5)), "PERSONGROUP")
            };
            workorder.push(vec![
                v(&wonum),
                v(s),
                v(o),
                if pick(&mut rng, 5) == 0 {
                    None
                } else {
                    v(&assets[pick(&mut rng, assets.len())])
                },
                if pick(&mut rng, 10) < 3 {
                    None
                } else {
                    v(&locs[pick(&mut rng, locs.len())])
                },
                parent,
                v(format!("P{:04}", 1 + pick(&mut rng, 40))),
                v(["WAPPR", "APPR", "INPRG", "COMP", "CLOSE"][pick(&mut rng, 5)]),
                v(owner),
                v(table),
                v(next_id()),
            ]);
            wonums.push(wonum);
        }
        for _ in 0..250 {
            let (it, loc) = &stock[pick(&mut rng, stock.len())];
            matuse.push(vec![
                v(it),
                v(loc),
                v(s),
                v(&wonums[pick(&mut rng, wonums.len())]),
                v(next_id()),
            ]);
        }
        for i in 1..=40 {
            let ponum = format!("PO{i:04}");
            let vendor = format!("C{:04}", 1 + pick(&mut rng, 10));
            po.push(vec![v(&ponum), v(s), v(o), v(&vendor), v(next_id())]);
            for line in 1..=3 {
                poline.push(vec![
                    v(&ponum),
                    v(s),
                    v(line),
                    v(format!("I{:04}", 1 + pick(&mut rng, 120))),
                    v(next_id()),
                ]);
            }
        }
        for _ in 0..100 {
            ticket += 1;
            sr.push(vec![
                v(format!("SR{ticket:05}")),
                if pick(&mut rng, 10) < 3 {
                    None
                } else {
                    v(&assets[pick(&mut rng, assets.len())])
                },
                v(s),
                if pick(&mut rng, 2) == 0 {
                    None
                } else {
                    v(&locs[pick(&mut rng, locs.len())])
                },
                v(format!("P{:04}", 1 + pick(&mut rng, 40))),
                v(next_id()),
            ]);
        }
    }

    let mut labtrans = Builder::new(
        "LABTRANS",
        &["LABORCODE", "ORGID", "REFWO", "SITEID", "LABTRANSID"],
    );
    for _ in 0..1500 {
        let lc = 1 + pick(&mut rng, 30);
        let (s, _) = SITES[pick(&mut rng, 4)];
        labtrans.push(vec![
            v(format!("LC{lc:03}")),
            v(["ORG1", "ORG2"][lc % 2]),
            v(1001 + pick(&mut rng, 300)),
            v(s),
            v(next_id()),
        ]);
    }

    // Orphans: a handful of rows whose key matches nothing, in a few relationships.
    let mut inject = |b: &mut Builder, col: &'static str, bogus: &str, count: usize| {
        let idx = b.cols.iter().position(|c| *c == col).unwrap_or(0);
        let mut done = 0;
        for r in b
            .rows
            .iter_mut()
            .filter(|r| r[idx].is_some())
            .step_by(97)
            .take(count)
        {
            r[idx] = v(bogus);
            done += 1;
        }
        orphans.push((b.name, col, done));
    };
    // On declared links (the dictionary tier must report them) and on undeclared ones.
    inject(&mut workorder, "ASSETNUM", "A9999", 3);
    inject(&mut matuse, "REFWO", "9999", 4);
    inject(&mut sr, "ASSETNUM", "A9998", 2);
    inject(&mut workorder, "REPORTEDBY", "P9999", 3);
    inject(&mut labtrans, "REFWO", "9999", 4);

    // Drift: three stored balances disagree with the transactions that should produce them.
    for idx in [7usize, 120, 333] {
        if let Some(row) = invbal.rows.get_mut(idx) {
            let old: i64 = row[3].as_deref().and_then(|x| x.parse().ok()).unwrap_or(0);
            row[3] = v(old + 5);
            drifted.push([
                row[0].clone().unwrap_or_default(),
                row[1].clone().unwrap_or_default(),
                row[2].clone().unwrap_or_default(),
            ]);
        }
    }

    let truth = truth_links();
    let (dictionary, unsupported_clauses) = dictionary_rows(&truth);
    let mut tables: Vec<Table> = [
        org, site, person, labor, companies, item, locations, asset, inventory, invbal, invtrans,
        workorder, labtrans, matuse, po, poline, sr,
    ]
    .into_iter()
    .map(Builder::build)
    .collect();
    tables.push(dictionary_table(&dictionary));
    Synthetic {
        tables,
        dictionary,
        truth,
        unsupported_clauses,
        orphans,
        drifted_balances: drifted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic_and_shaped_as_described() {
        let a = maximo_shaped(1);
        let b = maximo_shaped(1);
        assert_eq!(a.tables, b.tables);
        assert_ne!(a.tables, maximo_shaped(2).tables);
        assert_eq!(a.tables.len(), 18);
        assert_eq!(a.truth.len(), 48);
        assert_eq!(a.unsupported_clauses, 4);
        assert_eq!(a.drifted_balances.len(), 3);
        let wo = a.tables.iter().find(|t| t.name == "WORKORDER").unwrap();
        assert_eq!(wo.row_count(), 1200);
    }

    #[test]
    fn composite_keys_really_repeat_across_sites() {
        let s = maximo_shaped(1);
        let asset = s.tables.iter().find(|t| t.name == "ASSET").unwrap();
        let col = |n: &str| asset.columns.iter().find(|c| c.name == n).unwrap();
        let nums: HashSet<_> = col("ASSETNUM").values.iter().collect();
        assert_eq!(
            nums.len(),
            75,
            "ASSETNUM alone repeats across the four sites"
        );
        let pairs: HashSet<_> = col("ASSETNUM")
            .values
            .iter()
            .zip(&col("SITEID").values)
            .collect();
        assert_eq!(pairs.len(), 300, "ASSETNUM with SITEID is unique");
    }
}
