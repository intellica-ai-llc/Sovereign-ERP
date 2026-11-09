//! End to end on the synthetic Maximo-shaped database: dictionary tier, inference tier, reconciler, audit.
//!
//! The database is synthetic (see `sov_profiler::synth`), so these checks show the machinery is
//! consistent with relationships known by construction; they do not predict a customer's database.
//! Test code: failures surface as panics and the measurements are printed on purpose.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout
)]

use sov_profiler::audit::{balance_audit, BalanceSpec};
use sov_profiler::discovery::discover;
use sov_profiler::reconcile::{reconcile, Cardinality, Provenance, Reconciled};
use sov_profiler::synth::{maximo_shaped, Synthetic, TruthLink};
use std::collections::{BTreeSet, HashMap};

type Key = (String, BTreeSet<(String, String)>, String);

fn truth_key(t: &TruthLink) -> Key {
    (
        t.fk_table.to_owned(),
        t.fk_cols
            .iter()
            .zip(&t.ref_cols)
            .map(|(f, r)| ((*f).to_owned(), (*r).to_owned()))
            .collect(),
        t.ref_table.to_owned(),
    )
}

fn link_key(l: &Reconciled) -> Key {
    (
        l.fk_table.clone(),
        l.fk_cols
            .iter()
            .cloned()
            .zip(l.ref_cols.iter().cloned())
            .collect(),
        l.ref_table.clone(),
    )
}

struct Run {
    synth: Synthetic,
    links: HashMap<Key, Reconciled>,
}

fn run(seed: u64) -> Run {
    let synth = maximo_shaped(seed);
    let inferred = discover(&synth.tables);
    let findings = reconcile(&synth.tables, &synth.dictionary, &inferred);
    assert_eq!(
        findings.unsupported.len(),
        synth.unsupported_clauses,
        "unsupported clauses are reported exactly"
    );
    assert!(
        findings.unchecked.is_empty(),
        "every dictionary link can be checked: {:?}",
        findings.unchecked
    );
    let links = findings
        .links
        .into_iter()
        .map(|l| (link_key(&l), l))
        .collect();
    Run { synth, links }
}

/// Seeds checked. Every property below must hold for each, so a pass is not one lucky draw.
const SEEDS: [u64; 4] = [1, 2, 3, 4];

#[test]
fn dictionary_tier_resolves_every_declared_relationship_with_the_right_direction() {
    for seed in SEEDS {
        let r = run(seed);
        for t in r.synth.truth.iter().filter(|t| t.in_dictionary) {
            let l = r
                .links
                .get(&truth_key(t))
                .unwrap_or_else(|| panic!("seed {seed}: declared link missing: {t:?}"));
            assert!(
                matches!(l.provenance, Provenance::Dictionary | Provenance::Both),
                "seed {seed}: {t:?}"
            );
        }
    }
}

#[test]
fn reconciled_links_are_all_real_and_orphans_are_counted_exactly() {
    for seed in SEEDS {
        let r = run(seed);
        let truth_keys: BTreeSet<Key> = r.synth.truth.iter().map(truth_key).collect();
        for l in r.links.values() {
            assert!(
                truth_keys.contains(&link_key(l)),
                "seed {seed}: not a real relationship: {l:?}"
            );
        }
        // Injected orphans: found on declared links, exactly; clean everywhere else a link was found.
        let injected: HashMap<(&str, &str), usize> = r
            .synth
            .orphans
            .iter()
            .map(|(t, c, n)| ((*t, *c), *n))
            .collect();
        let reported = |fk: &str, col: &str| {
            r.links
                .values()
                .find(|l| l.fk_table == fk && l.fk_cols.iter().any(|c| c == col))
                .map(|l| l.orphan_rows)
        };
        assert_eq!(
            reported("WORKORDER", "ASSETNUM"),
            Some(injected[&("WORKORDER", "ASSETNUM")]),
            "seed {seed}"
        );
        assert_eq!(
            reported("MATUSETRANS", "REFWO"),
            Some(injected[&("MATUSETRANS", "REFWO")]),
            "seed {seed}"
        );
        assert_eq!(
            reported("SR", "ASSETNUM"),
            Some(injected[&("SR", "ASSETNUM")]),
            "seed {seed}"
        );
        let dirty = r.links.values().filter(|l| l.orphan_rows > 0).count();
        assert_eq!(
            dirty, 3,
            "seed {seed}: only the three injected, found links are dirty"
        );
    }
}

#[test]
fn three_column_keys_are_resolved_by_the_dictionary_because_inference_cannot_yet() {
    let r = run(1);
    for (fk, refs) in [
        ("INVTRANS", "INVENTORY"),
        ("MATUSETRANS", "INVENTORY"),
        ("INVBALANCES", "INVENTORY"),
    ] {
        let l = r
            .links
            .values()
            .find(|l| l.fk_table == fk && l.ref_table == refs && l.fk_cols.len() == 3)
            .unwrap_or_else(|| panic!("{fk} missing"));
        assert_eq!(
            l.provenance,
            Provenance::Dictionary,
            "{fk}: inference handles two-column keys at most (known gap)"
        );
    }
    let one_to_one = r
        .links
        .values()
        .find(|l| l.fk_table == "INVBALANCES" && l.ref_table == "INVENTORY")
        .unwrap();
    assert_eq!(one_to_one.cardinality, Cardinality::OneToOne);
    assert!(
        one_to_one.direction_certain,
        "the hub tie-break settled a 1:1 whose direction the data cannot show"
    );
}

#[test]
fn combined_coverage_and_the_known_gaps_are_recorded() {
    let r = run(1);
    let total = r.synth.truth.len();
    let missing: Vec<String> = r
        .synth
        .truth
        .iter()
        .filter(|t| !r.links.contains_key(&truth_key(t)))
        .map(|t| format!("{}.{} -> {}", t.fk_table, t.fk_cols.join("+"), t.ref_table))
        .collect();
    println!(
        "coverage {}/{total}; missing: {missing:#?}",
        total - missing.len()
    );
    // Known gaps, kept visible: role-named references (REPORTEDBY to PERSON), different names on each
    // side (REFWO to WONUM) and ORGID into a two-row table. A fix should lower this number.
    assert!(
        total - missing.len() >= 40,
        "coverage regressed: {missing:?}"
    );
    assert!(
        missing.iter().all(|m| !m.contains("ASSETNUM")),
        "a name-matched composite link regressed: {missing:?}"
    );
}

#[test]
fn audit_finds_exactly_the_injected_drift() {
    let s = maximo_shaped(1);
    let get = |n: &str| s.tables.iter().find(|t| t.name == n).unwrap();
    let report = balance_audit(&BalanceSpec {
        transactions: get("INVTRANS"),
        tx_keys: &["ITEMNUM", "STORELOC", "SITEID"],
        tx_amount: "QUANTITY",
        balances: get("INVBALANCES"),
        bal_keys: &["ITEMNUM", "LOCATION", "SITEID"],
        bal_amount: "CURBAL",
    })
    .unwrap();
    println!(
        "balances checked {}  drifts {}  unstored {}",
        report.balances_checked,
        report.drifts.len(),
        report.unstored.len()
    );
    let found: BTreeSet<Vec<String>> = report.drifts.iter().map(|d| d.key.clone()).collect();
    let injected: BTreeSet<Vec<String>> = s.drifted_balances.iter().map(|k| k.to_vec()).collect();
    assert_eq!(
        found, injected,
        "exactly the injected drifts, no false alarms"
    );
    assert!(report
        .drifts
        .iter()
        .all(|d| d.stored - d.derived == 5_000_000));
    assert_eq!(report.unstored, Vec::new());
}
