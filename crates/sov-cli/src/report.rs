//! The profile report: assembled from the library results and rendered as text or JSON.

use crate::load::Loaded;
use sov_evidence::sha256;
use sov_profiler::audit::AuditReport;
use sov_profiler::discovery::{discover, name_informativeness};
use sov_profiler::maximo::{extract, RelationshipRow};
use sov_profiler::reconcile::{reconcile, Cardinality, Findings, Provenance, Reconciled};
use std::fmt::Write as _;

/// Everything the report states.
pub struct ProfileReport {
    pub tables: usize,
    pub rows: usize,
    pub truncated: Vec<String>,
    pub informativeness: f64,
    pub opaque_profile: bool,
    pub dictionary_rows: usize,
    pub dictionary_understood: usize,
    pub findings: Findings,
}

/// Runs discovery and reconciliation.
pub fn profile(loaded: &Loaded, dictionary: &[RelationshipRow]) -> ProfileReport {
    let informativeness = name_informativeness(&loaded.inferable);
    let inferred = discover(&loaded.inferable);
    let findings = reconcile(&loaded.inferable, dictionary, &inferred);
    let understood = extract(dictionary).links.len();
    ProfileReport {
        tables: loaded.inferable.len(),
        rows: loaded.rows,
        truncated: loaded.truncated.clone(),
        informativeness,
        opaque_profile: informativeness < 0.5,
        dictionary_rows: dictionary.len(),
        dictionary_understood: understood,
        findings,
    }
}

fn cols(c: &[String]) -> String {
    c.join(",")
}

fn provenance(p: Provenance) -> &'static str {
    match p {
        Provenance::Dictionary => "dictionary",
        Provenance::Inferred => "inferred",
        Provenance::Both => "both",
    }
}

fn cardinality(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ManyToOne => "many-to-one",
        Cardinality::OneToOne => "one-to-one",
        Cardinality::Unresolved => "unresolved",
    }
}

fn pct(part: usize, whole: usize) -> String {
    match (part.saturating_mul(1000) + whole / 2).checked_div(whole) {
        None => "0.0".to_owned(),
        Some(tenths) => format!("{}.{}", tenths / 10, tenths % 10),
    }
}

fn link_line(l: &Reconciled) -> String {
    let mut s = format!(
        "  {}({}) -> {}({})  [{}] [{}] orphans {}/{} ({}%)",
        l.fk_table,
        cols(&l.fk_cols),
        l.ref_table,
        cols(&l.ref_cols),
        provenance(l.provenance),
        cardinality(l.cardinality),
        l.orphan_rows,
        l.checked_rows,
        pct(l.orphan_rows, l.checked_rows)
    );
    if !l.dictionary_names.is_empty() {
        let _ = write!(s, " dictionary: {}", l.dictionary_names.join(","));
    }
    if l.ambiguous {
        s.push_str(" REVIEW: close alternative parent");
    }
    if !l.direction_certain {
        s.push_str(" REVIEW: key direction not settled by the data");
    }
    s
}

fn body(r: &ProfileReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Sovereign ERP source profile: {}", crate::VERSION);
    let _ = writeln!(out, "Input: {} tables, {} rows", r.tables, r.rows);
    for t in &r.truncated {
        let _ = writeln!(out, "NOTE: table {t} was cut to the row limit; results for it come from its first rows only");
    }
    let _ = writeln!(
        out,
        "Names: {:.2} of columns have readable names; using the {} weight profile",
        r.informativeness,
        if r.opaque_profile { "opaque" } else { "named" }
    );
    let f = &r.findings;
    let _ = writeln!(
        out,
        "Dictionary: {} relationships, {} understood, {} outside the supported subset, {} could not be checked",
        r.dictionary_rows,
        r.dictionary_understood,
        f.unsupported.len(),
        f.unchecked.len()
    );
    let count = |p: Provenance| f.links.iter().filter(|l| l.provenance == p).count();
    let _ =
        writeln!(
        out,
        "Relationships: {} (dictionary only {}, inferred only {}, both {}); {} flagged for review",
        f.links.len(),
        count(Provenance::Dictionary),
        count(Provenance::Inferred),
        count(Provenance::Both),
        f.links.iter().filter(|l| l.ambiguous || !l.direction_certain).count()
    );
    for l in &f.links {
        let _ = writeln!(out, "{}", link_line(l));
    }
    let dirty: Vec<&Reconciled> = f.links.iter().filter(|l| l.orphan_rows > 0).collect();
    let _ = writeln!(
        out,
        "Orphan rows: {} relationships have rows whose key matches nothing",
        dirty.len()
    );
    for l in dirty {
        let _ = writeln!(
            out,
            "  {}({}): {} of {} rows",
            l.fk_table,
            cols(&l.fk_cols),
            l.orphan_rows,
            l.checked_rows
        );
    }
    for (row, why) in &f.unsupported {
        let _ = writeln!(out, "Not understood: {}.{} ({why})", row.parent, row.name);
    }
    for (what, why) in &f.unchecked {
        let _ = writeln!(out, "Not checked: {what} ({why})");
    }
    out
}

/// The text report, ending with the SHA-256 of everything above it.
pub fn render_text(r: &ProfileReport) -> String {
    let mut out = body(r);
    let digest = sha256(out.as_bytes());
    let _ = writeln!(out, "Report digest (SHA-256 of the text above): {digest}");
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_list(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|s| json_str(s))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// The report as JSON (same facts; the digest is of the text form).
pub fn render_json(r: &ProfileReport) -> String {
    let digest = sha256(body(r).as_bytes());
    let links: Vec<String> = r
        .findings
        .links
        .iter()
        .map(|l| {
            format!(
                "{{\"fk_table\":{},\"fk_cols\":{},\"ref_table\":{},\"ref_cols\":{},\"provenance\":{},\"cardinality\":{},\"checked_rows\":{},\"orphan_rows\":{},\"dictionary_names\":{},\"ambiguous\":{},\"direction_certain\":{}}}",
                json_str(&l.fk_table),
                json_list(&l.fk_cols),
                json_str(&l.ref_table),
                json_list(&l.ref_cols),
                json_str(provenance(l.provenance)),
                json_str(cardinality(l.cardinality)),
                l.checked_rows,
                l.orphan_rows,
                json_list(&l.dictionary_names),
                l.ambiguous,
                l.direction_certain
            )
        })
        .collect();
    let unsupported: Vec<String> = r
        .findings
        .unsupported
        .iter()
        .map(|(row, why)| {
            format!(
                "{{\"parent\":{},\"name\":{},\"reason\":{}}}",
                json_str(&row.parent),
                json_str(&row.name),
                json_str(&why.to_string())
            )
        })
        .collect();
    format!(
        "{{\"tool\":{},\"tables\":{},\"rows\":{},\"readable_names\":{:.2},\"dictionary_rows\":{},\"dictionary_understood\":{},\"links\":[{}],\"not_understood\":[{}],\"digest_sha256\":{}}}\n",
        json_str(crate::VERSION),
        r.tables,
        r.rows,
        r.informativeness,
        r.dictionary_rows,
        r.dictionary_understood,
        links.join(","),
        unsupported.join(","),
        json_str(&digest.to_hex())
    )
}

fn amount(v: i128) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    let (whole, frac) = (abs / 1_000_000, abs % 1_000_000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!(
            "{sign}{whole}.{}",
            format!("{frac:06}").trim_end_matches('0')
        )
    }
}

/// The balance audit as text.
pub fn render_audit(a: &AuditReport) -> String {
    let mut out = format!(
        "Balances checked: {}\nDrift: {} stored balances differ from their transactions\n",
        a.balances_checked,
        a.drifts.len()
    );
    for d in &a.drifts {
        let _ = writeln!(
            out,
            "  [{}] stored {} derived {} (difference {})",
            d.key.join(", "),
            amount(d.stored),
            amount(d.derived),
            amount(d.stored - d.derived)
        );
    }
    let _ = writeln!(
        out,
        "Transactions with no stored balance: {}",
        a.unstored.len()
    );
    for (key, derived) in &a.unstored {
        let _ = writeln!(out, "  [{}] derived {}", key.join(", "), amount(*derived));
    }
    let digest = sha256(out.as_bytes());
    let _ = writeln!(out, "Report digest (SHA-256 of the text above): {digest}");
    out
}
