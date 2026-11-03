//! Tamper-evident log of operator actions (tenant creation, collector start/stop, failed admin logins).
//! Each line chains to the previous by SHA-256, so an edited, reordered or removed line is detected by anyone who
//! holds an earlier head. Bodies and tokens are never recorded: only method, path and outcome.
use super::{bearer, err, h256, now_secs, App};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub struct AuditLog {
    path: PathBuf,
    state: Mutex<(u64, String)>,
}

fn link(prev: &str, seq: u64, t: u64, action: &str, detail: &str) -> String {
    let mut d = Sha256::new();
    d.update(format!("{prev}\n{seq}\n{t}\n{action}\n{detail}").as_bytes());
    hex::encode(d.finalize())
}

/// Verify a whole log; Err(line index) names the first bad line.
pub fn verify(text: &str) -> Result<(usize, String), usize> {
    let (mut prev, mut n) = (GENESIS.to_string(), 0usize);
    for (i, line) in text.lines().enumerate() {
        let v: Value = serde_json::from_str(line).map_err(|_| i)?;
        let (Some(seq), Some(t), Some(a), Some(d), Some(p), Some(h)) = (
            v["seq"].as_u64(),
            v["t"].as_u64(),
            v["action"].as_str(),
            v["detail"].as_str(),
            v["prev"].as_str(),
            v["h"].as_str(),
        ) else {
            return Err(i);
        };
        if seq != i as u64 || p != prev || h != link(p, seq, t, a, d) {
            return Err(i);
        }
        prev = h.to_string();
        n += 1;
    }
    Ok((n, prev))
}

impl AuditLog {
    pub(crate) fn open(dir: &std::path::Path) -> AuditLog {
        let path = dir.join("admin-audit.jsonl");
        let st = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| verify(&t).ok())
            .unwrap_or((0, GENESIS.to_string()));
        AuditLog {
            path,
            state: Mutex::new((st.0 as u64, st.1)),
        }
    }
    /// Append one entry. Failure to write is reported to stderr, never silently ignored.
    pub(crate) fn append(&self, action: &str, detail: &str) {
        let mut g = self.state.lock().unwrap();
        let (seq, prev) = (g.0, g.1.clone());
        let t = now_secs();
        let h = link(&prev, seq, t, action, detail);
        let line =
            json!({"seq":seq,"t":t,"action":action,"detail":detail,"prev":prev,"h":h}).to_string();
        let ok = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| writeln!(f, "{line}").and_then(|()| f.sync_data()));
        match ok {
            Ok(()) => *g = (seq + 1, h),
            Err(e) => eprintln!("AUDIT WRITE FAILED: {e}"),
        }
    }
}

pub(crate) async fn list(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    match bearer(&headers) {
        Some(t) if h256(t) == app.admin_hash => {}
        _ => return err(StatusCode::UNAUTHORIZED, "admin token required"),
    }
    let path = app.audit.path.clone();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (valid, bad_line, head, count) = match verify(&text) {
        Ok((n, h)) => (true, Value::Null, h, n),
        Err(i) => (false, json!(i), String::new(), text.lines().count()),
    };
    let entries: Vec<Value> = text
        .lines()
        .rev()
        .take(200)
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Json(json!({"valid": valid, "bad_line": bad_line, "head": head, "count": count, "latest_first": entries}))
        .into_response()
}
