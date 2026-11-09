//! FK-stripping benchmark runner (arc42 Appendix C spike 3, Q31).
//!
//! Usage: `cargo run --release -p sov-profiler --example fkbench -- <dataset-dir> [--tune]`
//! where the dataset directory comes from `bench/ofbiz/convert.py`.
//!
//! The profiler sees only tables of values. Declared relationships are used only to score.
//! Weights are tuned on the TRAIN modules and every headline number is on the held-out TEST modules.
//! Benchmark tooling: printing, unwrap and panic are how it reports and fails.
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use sov_profiler::discovery::{candidates, select, Candidate, Link, Params, Selection, Weights};
use sov_profiler::eval::{evaluate, parse_truth, Report};
use sov_profiler::table::Table;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

const TRAIN: &[&str] = &[
    "party",
    "product",
    "content",
    "humanres",
    "marketing",
    "manufacturing",
    "framework",
];

fn load(dir: &Path) -> (Vec<Table>, HashMap<String, String>) {
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
    let tables = files
        .iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            Table::from_tsv(&name, &fs::read_to_string(p).unwrap())
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        })
        .collect();
    (tables, modules)
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

/// arc42 Appendix J: inclusion into a unique parent, the child must use a meaningful share of the
/// parent's key space, and a child column set with more than one such parent goes to review (dropped).
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

fn row(label: &str, r: &Report) {
    println!(
        "{label:<34} predicted {:>5}  precision {:.3}  recall {:.3}  F1 {:.3}   table-level P {:.3} R {:.3}   recall(with data) {:.3}  ambiguous {}",
        r.predicted, r.precision(), r.recall(), r.f1(), r.precision_table(), r.recall_table(), r.recall_with_data(), r.ambiguous
    );
}

fn fields(w: &mut Weights) -> Vec<(&'static str, &mut f64)> {
    vec![
        ("bias", &mut w.bias),
        ("containment", &mut w.containment),
        ("coverage", &mut w.coverage),
        ("support", &mut w.support),
        ("parent_support", &mut w.parent_support),
        ("parent_position", &mut w.parent_position),
        ("name_col", &mut w.name_col),
        ("name_tab", &mut w.name_tab),
        ("shape", &mut w.shape),
        ("dominated", &mut w.dominated),
        ("self_ref", &mut w.self_ref),
        ("composite", &mut w.composite),
    ]
}

fn tune(
    cands: &[Candidate],
    tables: &[Table],
    truth: &[sov_profiler::eval::TruthFk],
    scope: &HashSet<String>,
    sel: &Selection,
    no_names: bool,
) -> Weights {
    let mut w = Weights::default();
    if no_names {
        w.name_col = 0.0;
        w.name_tab = 0.0;
    }
    let f1 = |w: &Weights| evaluate(tables, &select(cands, w, sel), truth, scope).f1();
    let mut best = f1(&w);
    for pass in 0..4 {
        for idx in 0..fields(&mut w).len() {
            if no_names && fields(&mut w)[idx].0.starts_with("name_") {
                continue;
            }
            let current = *fields(&mut w)[idx].1;
            let mut best_v = current;
            for step in -14..=14 {
                let v = f64::from(step) * 0.5;
                *fields(&mut w)[idx].1 = v;
                let s = f1(&w);
                if s > best + 1e-12 {
                    best = s;
                    best_v = v;
                }
            }
            *fields(&mut w)[idx].1 = best_v;
        }
        println!("  tuning pass {pass}: train F1 {best:.4}");
    }
    w
}

/// Prints false positives and missed (data-supported) relationships so errors can be studied.
fn show_errors(
    tables: &[Table],
    links: &[Link],
    truth: &[sov_profiler::eval::TruthFk],
    scope: &HashSet<String>,
) {
    let nm = |t: usize, cs: &[usize]| {
        cs.iter()
            .map(|&c| tables[t].columns[c].name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    let key = |ct: &str, cc: &str, pt: &str, pc: &str| format!("{ct}|{cc}|{pt}|{pc}");
    let truth_keys: HashSet<String> = truth
        .iter()
        .map(|t| {
            key(
                &t.child_table,
                &t.child_cols.join(","),
                &t.parent_table,
                &t.parent_cols.join(","),
            )
        })
        .collect();
    let truth_children: HashSet<String> = truth
        .iter()
        .map(|t| format!("{}|{}", t.child_table, t.child_cols.join(",")))
        .collect();
    let mut predicted_children = HashSet::new();
    let (mut fp_wrong_parent, mut fp_undeclared) = (Vec::new(), Vec::new());
    for l in links
        .iter()
        .filter(|l| scope.contains(&tables[l.child_table].name))
    {
        let (ct, cc) = (
            tables[l.child_table].name.as_str(),
            nm(l.child_table, &l.child_cols),
        );
        let (pt, pc) = (
            tables[l.parent_table].name.as_str(),
            nm(l.parent_table, &l.parent_cols),
        );
        predicted_children.insert(format!("{ct}|{cc}"));
        if !truth_keys.contains(&key(ct, &cc, pt, &pc)) {
            let line = format!(
                "{ct}.{cc} -> {pt}.{pc}  score {:.2}{}",
                l.score,
                if l.ambiguous { " (ambiguous)" } else { "" }
            );
            if truth_children.contains(&format!("{ct}|{cc}")) {
                fp_wrong_parent.push(line);
            } else {
                fp_undeclared.push(line);
            }
        }
    }
    println!("\nFALSE POSITIVES on TEST: {} wrong parent for a declared child, {} child columns with no declared relationship", fp_wrong_parent.len(), fp_undeclared.len());
    for l in fp_wrong_parent.iter().take(14) {
        println!("  wrong parent : {l}");
    }
    for l in fp_undeclared.iter().take(14) {
        println!("  undeclared   : {l}");
    }
    let mut missed = Vec::new();
    for t in truth.iter().filter(|t| scope.contains(&t.child_table)) {
        let k = format!("{}|{}", t.child_table, t.child_cols.join(","));
        if !links.iter().any(|l| {
            format!(
                "{}|{}",
                tables[l.child_table].name,
                nm(l.child_table, &l.child_cols)
            ) == k
        }) && !predicted_children.contains(&k)
        {
            missed.push(format!(
                "{}.{} -> {}.{}",
                t.child_table,
                t.child_cols.join(","),
                t.parent_table,
                t.parent_cols.join(",")
            ));
        }
    }
    println!("declared relationships with NO prediction for that child column(s) (includes those with no data): {}", missed.len());
}

// A linear benchmark script: splitting it up would only scatter the report.
#[allow(clippy::too_many_lines)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(args.get(1).expect("usage: fkbench <dataset-dir> [--tune]"));
    let do_tune = args.iter().any(|a| a == "--tune");
    let (tables, modules) = load(dir);
    let mut truth = parse_truth(&fs::read_to_string(dir.join("truth.tsv")).unwrap()).unwrap();
    if args.iter().any(|a| a == "--forward-only") {
        truth.retain(|t| t.kind != "many-reverse");
        println!("ground truth: forward declarations only (conservative)");
    } else {
        println!("ground truth: forward and parent-side (many) declarations");
    }
    let zero_shot = args.iter().any(|a| a == "--zero-shot");
    // Zero-shot: every table is TEST and no tuning happens, so the shipped weights meet a database
    // they have never seen.
    let scope_of = |train: bool| -> HashSet<String> {
        modules
            .iter()
            .filter(|(_, m)| {
                !zero_shot && TRAIN.contains(&m.as_str()) == train || zero_shot && !train
            })
            .map(|(t, _)| t.clone())
            .collect()
    };
    let (train, test) = (scope_of(true), scope_of(false));
    let nonempty = tables.iter().filter(|t| t.row_count() > 0).count();
    println!(
        "tables {} (non-empty {}), declared relationships {}",
        tables.len(),
        nonempty,
        truth.len()
    );

    let started = std::time::Instant::now();
    let cands = candidates(&tables, &Params::default());
    println!(
        "candidates {} generated in {:.1}s",
        cands.len(),
        started.elapsed().as_secs_f64()
    );

    let sel = Selection::default();
    let report = |label: &str, links: &[Link], scope: &HashSet<String>| {
        row(label, &evaluate(&tables, links, &truth, scope));
    };
    let all: Vec<Link> = cands.iter().map(as_link).collect();
    let tr = evaluate(&tables, &[], &truth, &train);
    let te = evaluate(&tables, &[], &truth, &test);
    println!(
        "\nTRAIN scope: declared {}, with data {}, data-supported {}",
        tr.truth_total, tr.truth_with_data, tr.truth_supported
    );
    println!(
        "TEST  scope: declared {}, with data {}, data-supported {}",
        te.truth_total, te.truth_with_data, te.truth_supported
    );

    let weights = if do_tune {
        println!("\ntuning on TRAIN modules only...");
        let w = tune(
            &cands,
            &tables,
            &truth,
            &train,
            &sel,
            args.iter().any(|a| a == "--no-names"),
        );
        println!("tuned weights: {w:?}");
        w
    } else {
        Weights::auto(&tables)
    };

    let best_share = [0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8]
        .into_iter()
        .max_by(|a, b| {
            let f = |s: f64| evaluate(&tables, &appendix_j(&cands, s), &truth, &train).f1();
            f(*a).total_cmp(&f(*b))
        })
        .unwrap();
    println!("\n== HELD-OUT TEST modules (accounting, order, shipment, workeffort) ==");
    report("inclusion only (all candidates)", &all, &test);
    report(
        &format!("arc42 App. J heuristic (share {best_share})"),
        &appendix_j(&cands, best_share),
        &test,
    );
    report(
        "sov-profiler, default weights",
        &select(&cands, &Weights::default(), &sel),
        &test,
    );
    if do_tune {
        report(
            "sov-profiler, tuned on TRAIN",
            &select(&cands, &weights, &sel),
            &test,
        );
    }
    println!("\n== TRAIN modules (for reference) ==");
    report("inclusion only (all candidates)", &all, &train);
    report(
        "sov-profiler, current weights",
        &select(&cands, &weights, &sel),
        &train,
    );

    let cut: Vec<Link> = select(&cands, &weights, &sel)
        .into_iter()
        .filter(|l| !l.ambiguous)
        .collect();
    if args.iter().any(|a| a == "--errors") {
        show_errors(&tables, &select(&cands, &weights, &sel), &truth, &test);
    }
    let gate = |flag: &str| -> Option<f64> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map(|v| v.parse().expect("number after gate flag"))
    };
    let final_report = evaluate(&tables, &select(&cands, &weights, &sel), &truth, &test);
    let (min_p, min_r) = (gate("--min-precision"), gate("--min-recall"));
    println!("\n== TEST, ambiguous links held back for human review ==");
    report("sov-profiler, confident only", &cut, &test);
    let mut failed = false;
    if let Some(m) = min_p {
        if final_report.precision() < m {
            println!(
                "GATE FAILED: held-out precision {:.3} < {m}",
                final_report.precision()
            );
            failed = true;
        }
    }
    if let Some(m) = min_r {
        if final_report.recall() < m {
            println!(
                "GATE FAILED: held-out recall {:.3} < {m}",
                final_report.recall()
            );
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
    if min_p.is_some() || min_r.is_some() {
        println!(
            "gate passed: held-out precision {:.3}, recall {:.3}",
            final_report.precision(),
            final_report.recall()
        );
    }
}
