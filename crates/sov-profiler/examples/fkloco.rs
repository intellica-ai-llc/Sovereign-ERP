//! Leave-one-corpus-out evaluation of relationship discovery (arc42 Q31, transfer across databases).
//!
//! Usage: `fkloco <dataset-dir> <dataset-dir>...` (each from a `bench/*/convert.py`).
//! For every corpus in turn: tune the weights on all the OTHER corpora, then score the held-out one.
//! A directory whose tables span the `OFBiz` modules is split by module (train modules for tuning,
//! test modules for scoring); every other corpus is used whole. Benchmark tooling: printing and
//! panicking are how it reports and fails.
#![allow(
    clippy::too_many_lines,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use sov_profiler::discovery::{candidates, select, Candidate, Link, Params, Selection, Weights};
use sov_profiler::eval::{evaluate, parse_truth, TruthFk};
use sov_profiler::table::Table;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

const OFBIZ_TRAIN: &[&str] = &[
    "party",
    "product",
    "content",
    "humanres",
    "marketing",
    "manufacturing",
    "framework",
];

struct Corpus {
    name: String,
    tables: Vec<Table>,
    truth: Vec<TruthFk>,
    cands: Vec<Candidate>,
    train: HashSet<String>,
    test: HashSet<String>,
    all: HashSet<String>,
}

fn load(dir: &Path) -> Corpus {
    let mut modules = HashMap::new();
    for line in fs::read_to_string(dir.join("tables.tsv")).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        modules.insert(f[0].to_owned(), f[1].to_owned());
    }
    let mut files: Vec<_> = fs::read_dir(dir.join("tables"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let tables: Vec<Table> = files
        .iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            Table::from_tsv(&name, &fs::read_to_string(p).unwrap())
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        })
        .collect();
    let truth = parse_truth(&fs::read_to_string(dir.join("truth.tsv")).unwrap())
        .unwrap()
        .into_iter()
        .filter(|t| t.kind != "many-reverse")
        .collect();
    let all: HashSet<String> = tables.iter().map(|t| t.name.clone()).collect();
    let is_ofbiz = modules.values().any(|m| m == "accounting");
    let (train, test) = if is_ofbiz {
        let tr: HashSet<String> = modules
            .iter()
            .filter(|(_, m)| OFBIZ_TRAIN.contains(&m.as_str()))
            .map(|(t, _)| t.clone())
            .collect();
        let te: HashSet<String> = all.difference(&tr).cloned().collect();
        (tr, te)
    } else {
        (all.clone(), all.clone())
    };
    let cands = candidates(&tables, &Params::default());
    let name = dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .trim_start_matches("out-")
        .to_owned();
    Corpus {
        name,
        tables,
        truth,
        cands,
        train,
        test,
        all,
    }
}

fn as_link(c: &Candidate) -> Link {
    Link {
        child_table: c.child_table,
        child_cols: c.child_cols.clone(),
        parent_table: c.parent_table,
        parent_cols: c.parent_cols.clone(),
        score: 0.0,
        ambiguous: false,
    }
}

fn appendix_j(cands: &[Candidate], min_share: f64) -> Vec<Link> {
    let mut groups: BTreeMap<(usize, &[usize]), Vec<&Candidate>> = BTreeMap::new();
    for c in cands.iter().filter(|c| c.features.coverage >= min_share) {
        groups
            .entry((c.child_table, c.child_cols.as_slice()))
            .or_default()
            .push(c);
    }
    groups
        .values()
        .filter(|g| g.len() == 1)
        .map(|g| as_link(g[0]))
        .collect()
}

fn fields(w: &mut Weights) -> Vec<&mut f64> {
    vec![
        &mut w.bias,
        &mut w.containment,
        &mut w.coverage,
        &mut w.support,
        &mut w.parent_support,
        &mut w.parent_position,
        &mut w.name_col,
        &mut w.name_tab,
        &mut w.shape,
        &mut w.dominated,
        &mut w.self_ref,
        &mut w.composite,
        &mut w.child_unique,
        &mut w.parent_density,
    ]
}

type TrainSet<'a> = Vec<(&'a Corpus, &'a HashSet<String>)>;

/// Mean F1 across the training sets, so a big corpus cannot drown the small ones.
fn macro_f1(sets: &TrainSet<'_>, w: &Weights, sel: &Selection) -> f64 {
    let total: f64 = sets
        .iter()
        .map(|(c, scope)| evaluate(&c.tables, &select(&c.cands, w, sel), &c.truth, scope).f1())
        .sum();
    total / f64::from(u32::try_from(sets.len()).unwrap())
}

fn tune(sets: &TrainSet<'_>, sel: &Selection) -> Weights {
    // Start from a neutral point (accept everything), never from the shipped weights, which were
    // themselves fitted on every corpus: starting there would leak the held-out database.
    let mut w = Weights::inclusion_only();
    let mut best = macro_f1(sets, &w, sel);
    for _ in 0..2 {
        for idx in 0..fields(&mut w).len() {
            let mut best_v = *fields(&mut w)[idx];
            for step in -8..=8 {
                let v = f64::from(step) * 0.5;
                *fields(&mut w)[idx] = v;
                let s = macro_f1(sets, &w, sel);
                if s > best + 1e-12 {
                    best = s;
                    best_v = v;
                }
            }
            *fields(&mut w)[idx] = best_v;
        }
    }
    w
}

fn line(label: &str, c: &Corpus, links: &[Link]) {
    let r = evaluate(&c.tables, links, &c.truth, &c.test);
    println!(
        "  {label:<38} predicted {:>5}  P {:.3}  R {:.3}  F1 {:.3}  ambiguous {:>3}",
        r.predicted,
        r.precision(),
        r.recall(),
        r.f1(),
        r.ambiguous
    );
}

fn main() {
    let corpora: Vec<Corpus> = std::env::args()
        .skip(1)
        .filter(|a| {
            !a.starts_with("--")
                && a.parse::<f64>().is_err()
                && !["ofbiz-bench", "chinook", "northwind", "sakila"].contains(&a.as_str())
        })
        .map(|d| load(Path::new(&d)))
        .collect();
    assert!(corpora.len() >= 2, "need at least two corpora");
    let sel = Selection::default();
    for c in &corpora {
        let r = evaluate(&c.tables, &[], &c.truth, &c.test);
        println!(
            "corpus {:<10} tables {:>3}  candidates {:>6}  declared {:>4}  data-supported {:>4}",
            c.name,
            c.tables.len(),
            c.cands.len(),
            r.truth_total,
            r.truth_supported
        );
    }
    let weights_only = std::env::args().any(|a| a == "--weights-only");
    let mut sums = (0.0, 0.0, 0.0, 0usize);
    let only = std::env::args().skip_while(|a| a != "--only").nth(1);
    for target in corpora
        .iter()
        .filter(|c| !weights_only && only.as_ref().is_none_or(|o| *o == c.name))
    {
        let mut sets: TrainSet<'_> = Vec::new();
        for c in corpora.iter().filter(|c| c.name != target.name) {
            sets.push((c, &c.all));
        }
        if target.train != target.test {
            sets.push((target, &target.train));
        }
        let w = tune(&sets, &sel);
        let share = [0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 0.9]
            .into_iter()
            .max_by(|a, b| {
                let f = |s: f64| {
                    sets.iter()
                        .map(|(c, sc)| {
                            evaluate(&c.tables, &appendix_j(&c.cands, s), &c.truth, sc).f1()
                        })
                        .sum::<f64>()
                };
                f(*a).total_cmp(&f(*b))
            })
            .unwrap();
        println!(
            "\nHELD OUT: {}   (tuned on {})",
            target.name,
            sets.iter()
                .map(|(c, _)| c.name.as_str())
                .collect::<Vec<_>>()
                .join(" + ")
        );
        line(
            "inclusion only",
            target,
            &target.cands.iter().map(as_link).collect::<Vec<_>>(),
        );
        line(
            &format!("arc42 App. J (share {share}, tuned fairly)"),
            target,
            &appendix_j(&target.cands, share),
        );
        line(
            "sov-profiler, shipped weights",
            target,
            &select(&target.cands, &Weights::auto(&target.tables), &sel),
        );
        let tuned = select(&target.cands, &w, &sel);
        line("sov-profiler, tuned on the others", target, &tuned);
        let r = evaluate(&target.tables, &tuned, &target.truth, &target.test);
        sums = (
            sums.0 + r.precision(),
            sums.1 + r.recall(),
            sums.2 + r.f1(),
            sums.3 + 1,
        );
    }
    if sums.3 > 0 {
        let n = f64::from(u32::try_from(sums.3).unwrap());
        let mean_f1 = sums.2 / n;
        println!(
            "\nMEAN over held-out corpora: P {:.3}  R {:.3}  F1 {mean_f1:.3}",
            sums.0 / n,
            sums.1 / n
        );
        let floor = std::env::args()
            .skip_while(|a| a != "--min-mean-f1")
            .nth(1)
            .and_then(|v| v.parse::<f64>().ok());
        if let Some(floor) = floor {
            assert!(
                mean_f1 >= floor,
                "mean held-out F1 {mean_f1:.3} fell below the floor {floor:.3}"
            );
            println!("gate passed: mean held-out F1 {mean_f1:.3} >= {floor:.3}");
        }
    }

    if !weights_only {
        return;
    }
    let mut all_sets: TrainSet<'_> = Vec::new();
    for c in &corpora {
        all_sets.push((c, if c.train == c.test { &c.all } else { &c.train }));
    }
    let w = tune(&all_sets, &sel);
    println!("\nWEIGHTS tuned on every corpus (OFBiz: train modules only): {w:?}");
    for c in corpora.iter().filter(|c| c.train != c.test) {
        line(
            &format!("{} held-out modules, those weights", c.name),
            c,
            &select(&c.cands, &w, &sel),
        );
    }
}
