//! `sov demo`: a self-contained tour on a synthetic database. Deterministic by construction.

use crate::load::Loaded;
use crate::{report, CliError};
use sov_evidence::sha256;
use sov_period_controller::{Command, PeriodController, PeriodId};
use sov_profiler::audit::{balance_audit, BalanceSpec};
use sov_profiler::synth::{maximo_shaped, Synthetic};
use sov_profiler::table::Table;
use sov_sync::{EntryKind, Replica, ReplicaId};
use std::fmt::Write as _;
use std::path::Path;

fn tsv(t: &Table) -> String {
    let mut out = t
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join("\t");
    out.push('\n');
    for r in 0..t.row_count() {
        let row: Vec<&str> = t
            .columns
            .iter()
            .map(|c| c.values[r].as_deref().unwrap_or("\\N"))
            .collect();
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

fn export(tables: &[Table], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for t in tables {
        let path = dir.join(format!("{}.tsv", t.name));
        std::fs::write(&path, tsv(t)).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn section_profile(out: &mut String, synth: &Synthetic) {
    let _ = writeln!(out, "== 1. Relationship discovery and reconciliation (Maximo-shaped, 18 tables, no declared keys) ==");
    let inferable: Vec<Table> = synth
        .tables
        .iter()
        .filter(|t| t.name != "MAXRELATIONSHIP")
        .cloned()
        .collect();
    let rows = synth.tables.iter().map(Table::row_count).sum();
    let loaded = Loaded {
        all: synth.tables.clone(),
        inferable,
        dictionary: synth.dictionary.clone(),
        truncated: Vec::new(),
        rows,
    };
    out.push_str(&report::render_text(&report::profile(
        &loaded,
        &synth.dictionary,
    )));
}

fn section_audit(out: &mut String, synth: &Synthetic) -> Result<(), CliError> {
    let _ = writeln!(
        out,
        "\n== 2. Stored balances re-derived from transactions =="
    );
    let get = |n: &str| synth.tables.iter().find(|t| t.name == n);
    if let (Some(tx), Some(bal)) = (get("INVTRANS"), get("INVBALANCES")) {
        let spec = BalanceSpec {
            transactions: tx,
            tx_keys: &["ITEMNUM", "STORELOC", "SITEID"],
            tx_amount: "QUANTITY",
            balances: bal,
            bal_keys: &["ITEMNUM", "LOCATION", "SITEID"],
            bal_amount: "CURBAL",
        };
        let audit = balance_audit(&spec).map_err(|e| CliError::Failed(e.to_string()))?;
        out.push_str(&report::render_audit(&audit));
    }
    Ok(())
}

fn section_sync(out: &mut String) {
    let _ = writeln!(
        out,
        "\n== 3. Offline field work: a late entry is kept and flagged, never lost =="
    );
    let (mut desk, mut phone) = (Replica::new(ReplicaId(1)), Replica::new(ReplicaId(2)));
    for _ in 0..2 {
        let _ = desk.advance();
        phone.receive(desk.doc());
    }
    let _ = desk.advance();
    let _ = desk.advance();
    let entry = phone.add_entry(EntryKind::Labor, 90, "replaced bearing (recorded offline)");
    desk.receive(phone.doc());
    phone.receive(desk.doc());
    let _ = writeln!(
        out,
        "Desk status {:?}; phone status {:?}; replicas identical: {}",
        desk.doc().status().rank,
        phone.doc().status().rank,
        desk.doc() == phone.doc()
    );
    let _ = writeln!(
        out,
        "Entry {entry:?} kept: {} live entries; conflicts for review: {:?}",
        desk.doc().live_entries().len(),
        desk.doc().conflicts()
    );
}

fn section_periods(out: &mut String) {
    let _ = writeln!(
        out,
        "\n== 4. Period close rules are enforced, not suggested =="
    );
    let mut periods = PeriodController::new(3);
    let (p1, p2) = (PeriodId(1), PeriodId(2));
    let _ = writeln!(
        out,
        "Close period 2 first:  {:?}",
        periods
            .execute(&Command::BeginClose(p2))
            .map(|_| "accepted")
    );
    for cmd in [Command::BeginClose(p1), Command::Close(p1)] {
        let _ = writeln!(
            out,
            "{:<22} {:?}",
            format!("{cmd:?}"),
            periods.execute(&cmd).map(|_| "accepted")
        );
    }
    let _ = writeln!(
        out,
        "Reopen period 1 while it is still closing: {:?}",
        periods.execute(&Command::Reopen(p1)).map(|_| "accepted")
    );
}

pub fn run(export_to: Option<&Path>) -> Result<String, CliError> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}: demonstration on a SYNTHETIC database (generated, not real customer data)\n",
        crate::VERSION
    );
    let synth = maximo_shaped(1);
    if let Some(dir) = export_to {
        export(&synth.tables, dir)?;
        let _ = writeln!(
            out,
            "Exported {} tables to {} (try: sov profile {})\n",
            synth.tables.len(),
            dir.display(),
            dir.display()
        );
    }
    section_profile(&mut out, &synth);
    section_audit(&mut out, &synth)?;
    section_sync(&mut out);
    section_periods(&mut out);
    let digest = sha256(out.as_bytes());
    let _ = writeln!(out, "\nDemo digest (SHA-256 of the text above): {digest}");
    Ok(out)
}
