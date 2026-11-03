//! Operator controls for the data-collection pipeline, driven from the console. Admin-token only (the collector is
//! one process-wide resource, not a tenant feature). Read-only against public market endpoints: nothing here can
//! place an order. The card shown is computed from the paper ledger; with no ledger yet it honestly lists every
//! check as NOT YET with the reason.
use super::{bearer, err, h256, App};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use mas_data::runner::{self, Config, Fetch, Handle};
use mas_research::card::{self, Audit, CardInput, Rules};
use mas_research::registry::Registry;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MAX_SCAN_BYTES: u64 = 64 * 1024 * 1024;

pub struct Trading {
    pub(crate) dir: PathBuf,
    handle: Mutex<Option<Handle>>,
    fetch: Mutex<Fetch>,
}

impl Trading {
    pub(crate) fn new(dir: PathBuf) -> Trading {
        Trading {
            dir,
            handle: Mutex::new(None),
            fetch: Mutex::new(Arc::new(runner::http_get)),
        }
    }
    pub(crate) fn is_running(&self) -> bool {
        self.handle
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|h| h.status().running)
    }
    /// Replace the network function (tests, or a proxy-aware client).
    pub(crate) fn set_fetch(&self, f: Fetch) {
        *self.fetch.lock().unwrap() = f;
    }
    pub(crate) fn start(&self, limit: u32, rounds: u64, interval_secs: u64) -> Result<(), String> {
        let mut g = self.handle.lock().unwrap();
        if g.as_ref().is_some_and(|h| h.status().running) {
            return Err("a collection session is already running".into());
        }
        let mut cfg = Config::new(self.dir.clone());
        (cfg.limit, cfg.rounds, cfg.interval_secs) = (limit, rounds, interval_secs);
        let fetch = self.fetch.lock().unwrap().clone();
        *g = Some(runner::start(cfg, fetch)?);
        Ok(())
    }
    pub(crate) fn stop(&self) -> bool {
        match self.handle.lock().unwrap().as_ref() {
            Some(h) if h.status().running => {
                h.stop();
                true
            }
            _ => false,
        }
    }
}

impl App {
    /// Test hook / proxy support: replace the function used to reach the public market endpoints.
    pub fn set_market_fetch(&self, f: Fetch) {
        self.trading.set_fetch(f);
    }
    /// Start collecting with defaults (used by VC_COLLECT=1 on always-on hosts).
    pub fn start_collector_default(&self) -> Result<(), String> {
        self.collect_expected
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.trading.start(20, 0, 300)
    }
    /// Tell /readyz whether a running collector is required (tests).
    pub fn expect_collector(&self, yes: bool) {
        self.collect_expected
            .store(yes, std::sync::atomic::Ordering::SeqCst);
    }
}

fn is_admin(app: &App, headers: &HeaderMap) -> bool {
    matches!(bearer(headers), Some(t) if h256(t) == app.admin_hash)
}

#[derive(Deserialize, Default)]
struct StartReq {
    limit: Option<u32>,
    rounds: Option<u64>,
    interval_secs: Option<u64>,
}

pub(crate) async fn start(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_admin(&app, &headers) {
        return err(StatusCode::UNAUTHORIZED, "admin token required");
    }
    let r: StartReq = if body.is_empty() {
        StartReq::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => return err(StatusCode::BAD_REQUEST, "invalid JSON body"),
        }
    };
    let (limit, rounds, interval) = (
        r.limit.unwrap_or(20),
        r.rounds.unwrap_or(0),
        r.interval_secs.unwrap_or(300),
    );
    if !(1..=100).contains(&limit) || !(30..=86_400).contains(&interval) || rounds > 100_000 {
        return err(
            StatusCode::BAD_REQUEST,
            "limit 1-100, interval_secs 30-86400, rounds 0-100000 (0 = until stopped)",
        );
    }
    match app.trading.start(limit, rounds, interval) {
        Ok(()) => Json(json!({"started": true})).into_response(),
        Err(e) if e.contains("already running") => err(StatusCode::CONFLICT, &e),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

pub(crate) async fn stop(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if !is_admin(&app, &headers) {
        return err(StatusCode::UNAUTHORIZED, "admin token required");
    }
    Json(json!({"stopping": app.trading.stop()})).into_response()
}

fn files(dir: &std::path::Path) -> Vec<Value> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            Some((
                e.file_name().to_str()?.to_string(),
                e.metadata().ok()?.len(),
            ))
        })
        .collect();
    v.sort();
    v.into_iter()
        .map(|(n, s)| json!({"name": n, "bytes": s}))
        .collect()
}

fn scan_latest(dir: &std::path::Path) -> Value {
    let latest = |prefix: &str| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.starts_with(prefix))
            .max()
    };
    let (Some(b), Some(ev)) = (latest("books-"), latest("events-")) else {
        return json!({"error": "no recording yet"});
    };
    let bp = dir.join(&b);
    if std::fs::metadata(&bp).map_or(true, |m| m.len() > MAX_SCAN_BYTES) {
        return json!({"error": "recording too large to scan in the console; use the mas-data CLI"});
    }
    let go = || -> Result<Value, String> {
        let recs =
            mas_data::read_recording(&std::fs::read_to_string(&bp).map_err(|e| e.to_string())?)
                .map_err(|e| format!("recording INVALID: {e}"))?;
        let events = runner::events_from_json(
            &std::fs::read_to_string(dir.join(&ev)).map_err(|e| e.to_string())?,
        )?;
        let s = mas_data::scan_rebalance(&recs, &events, 0.04, 0.01, 0.02);
        let best = s.opportunities.iter().map(|o| o.edge).fold(0.0, f64::max);
        let (head, n) = mas_data::head(&recs);
        Ok(
            json!({"file": b, "records": n, "head": head, "rounds": s.rounds, "books_ok": s.books_ok,
            "events_complete": s.events_complete, "opportunities": s.opportunities.len(), "best_edge": best,
            "assumptions": "fee rate 4%, 1% haircut, minimum edge 2%"}),
        )
    };
    go().unwrap_or_else(|e| json!({"error": e}))
}

/// The card as it stands today: no paper ledger exists yet, so every check is listed with why it is not proven.
fn current_card() -> Value {
    let reg = Registry::new();
    let audit = Audit {
        orders_total: 0,
        orders_with_intent: 0,
        max_exposure_frac: 0.0,
        limit_overrides: 0,
        kill_tests_passed: 0,
        days_since_last_kill_test: u32::MAX,
        reconciliation_breaks: 0,
        reconciliations_done: 0,
        live_days: 0,
        drawdown_bound: 0.2,
    };
    let input = CardInput {
        strategy: "lp1",
        period: "no ledger yet",
        returns: &[],
        scale: 0.05,
        registry: &reg,
        trial: "",
        audit: &audit,
        log_head: "none",
    };
    let c = card::build(&input, &Rules::default());
    let mut v: Value = serde_json::from_str(&card::to_json(&c)).unwrap_or(Value::Null);
    v["statement"] = Value::String(card::public_statement(&c));
    v
}

/// Replay the newest recording through the paper engine and build the card from it.
fn paper_latest(dir: &std::path::Path) -> Value {
    let latest = |prefix: &str| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.starts_with(prefix))
            .max()
    };
    let Some(b) = latest("books-") else {
        return json!({"error": "no recording yet"});
    };
    let rd = |name: &str| -> Result<Vec<mas_data::Rec>, String> {
        let p = dir.join(name);
        if std::fs::metadata(&p).map_or(true, |m| m.len() > MAX_SCAN_BYTES) {
            return Err(format!(
                "{name} missing or too large for the console; use `mas-data paper`"
            ));
        }
        mas_data::read_recording(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{name} INVALID: {e}"))
    };
    let go = || -> Result<Value, String> {
        let books = rd(&b)?;
        let trades = match latest("trades-") {
            Some(t) => rd(&t)?,
            None => vec![],
        };
        let p = mas_data::replay::Params::default();
        let r = mas_data::replay::run(&books, &trades, &p)?;
        let (bh, _) = mas_data::head(&books);
        let c = mas_data::replay::make_card(&r, &p, &bh);
        let mut v: Value = serde_json::from_str(&card::to_json(&c)).unwrap_or(Value::Null);
        v["statement"] = Value::String(card::public_statement(&c));
        v["card_id"] = Value::String(card::card_id(&c));
        v["source"] = json!({"books": b, "periods": r.returns.len(), "books_used": r.books_used,
            "trades_applied": r.trades_used, "params": p.canonical(),
            "note": "baseline policy, fills pessimistic, marked to mid; a paper card can never be Verified"});
        Ok(v)
    };
    go().unwrap_or_else(|e| json!({"error": e}))
}

pub(crate) async fn status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if !is_admin(&app, &headers) {
        return err(StatusCode::UNAUTHORIZED, "admin token required");
    }
    let collector = app
        .trading
        .handle
        .lock()
        .unwrap()
        .as_ref()
        .map(|h| h.status().to_json());
    let dir = app.trading.dir.clone();
    let want_scan = q.get("scan").is_some_and(|v| v == "1");
    let want_paper = q.get("paper").is_some_and(|v| v == "1");
    let r = tokio::task::spawn_blocking(move || {
        json!({"files": files(&dir), "scan": if want_scan { scan_latest(&dir) } else { Value::Null },
            "paper": if want_paper { paper_latest(&dir) } else { Value::Null }})
    })
    .await;
    let Ok(mut out) = r else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "status failed");
    };
    out["collector"] = collector.unwrap_or(Value::Null);
    out["card"] = current_card();
    Json(out).into_response()
}
