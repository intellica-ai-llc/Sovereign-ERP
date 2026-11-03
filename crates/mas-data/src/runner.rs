//! Background collector: discovers markets, then records order books and public trades into hash-chained files.
//! Started and stopped from the operator console (vc-server) or tests. Network access is injected as a `Fetch`
//! function so the whole loop is testable without a network. It only READS public endpoints; nothing here can
//! place an order. Safety rails: one session at a time, a disk cap per session, interruptible sleeps, and a drop guard
//! so a crash can never leave the status stuck at "running".
use crate::{
    make_rec, parse_book, parse_events, parse_trades, rec_to_line, EventInfo, MarketInfo, GENESIS,
};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const GAMMA: &str = "https://gamma-api.polymarket.com";
pub const CLOB: &str = "https://clob.polymarket.com";
pub const DATA_API: &str = "https://data-api.polymarket.com";
const MAX_BODY: u64 = 4 * 1024 * 1024;

pub type Fetch = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

pub fn http_get(url: &str) -> Result<String, String> {
    let r = ureq::get(url)
        .set("User-Agent", "mas-data/0.1 (research recorder)")
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| e.to_string())?;
    let mut s = String::new();
    r.into_reader()
        .take(MAX_BODY)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    Ok(s)
}

pub fn events_to_json(ev: &[EventInfo]) -> Value {
    json!(ev.iter().map(|e| json!({"slug":e.slug,"neg_risk":e.neg_risk,"markets":e.markets.iter().map(|m| json!({
        "question":m.question,"condition_id":m.condition_id,"yes_token":m.yes_token,"no_token":m.no_token,"end":m.end,
        "rewards_min_size":m.rewards_min_size,"rewards_max_spread":m.rewards_max_spread})).collect::<Vec<_>>()})).collect::<Vec<_>>())
}

pub fn events_from_json(text: &str) -> Result<Vec<EventInfo>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let st = |m: &Value, k: &str| m[k].as_str().unwrap_or("").to_string();
    v.as_array()
        .ok_or("events file must be an array")?
        .iter()
        .map(|e| {
            Ok(EventInfo {
                slug: st(e, "slug"),
                neg_risk: e["neg_risk"].as_bool().unwrap_or(false),
                markets: e["markets"]
                    .as_array()
                    .ok_or("markets")?
                    .iter()
                    .map(|m| MarketInfo {
                        question: st(m, "question"),
                        condition_id: st(m, "condition_id"),
                        yes_token: st(m, "yes_token"),
                        no_token: st(m, "no_token"),
                        end: st(m, "end"),
                        rewards_min_size: m["rewards_min_size"].as_f64(),
                        rewards_max_spread: m["rewards_max_spread"].as_f64(),
                    })
                    .collect(),
            })
        })
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct Config {
    pub gamma: String,
    pub clob: String,
    pub data: String,
    /// Events to follow (most active first).
    pub limit: u32,
    /// Rounds to record; 0 = keep going until stopped.
    pub rounds: u64,
    pub interval_secs: u64,
    /// Pause between requests (politeness to the public API).
    pub pace_ms: u64,
    /// Stop the session when it has written this many bytes.
    pub max_bytes: u64,
    pub out_dir: PathBuf,
}

impl Config {
    pub fn new(out_dir: PathBuf) -> Self {
        Config {
            gamma: GAMMA.into(),
            clob: CLOB.into(),
            data: DATA_API.into(),
            limit: 20,
            rounds: 0,
            interval_secs: 300,
            pace_ms: 150,
            max_bytes: 500 * 1024 * 1024,
            out_dir,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub running: bool,
    pub phase: String,
    pub session: String,
    pub round: u64,
    pub rounds: u64,
    pub events: usize,
    pub neg_risk_events: usize,
    pub books_ok: u64,
    pub books_bad: u64,
    pub trade_polls_ok: u64,
    pub trade_polls_bad: u64,
    pub bytes: u64,
    pub books_head: String,
    pub trades_head: String,
    pub last_error: String,
    pub started_ms: u64,
    pub finished_ms: u64,
}

impl Status {
    pub fn to_json(&self) -> Value {
        json!({"running":self.running,"phase":self.phase,"session":self.session,"round":self.round,"rounds":self.rounds,
        "events":self.events,"neg_risk_events":self.neg_risk_events,"books_ok":self.books_ok,"books_bad":self.books_bad,
        "trade_polls_ok":self.trade_polls_ok,"trade_polls_bad":self.trade_polls_bad,"bytes":self.bytes,
        "books_head":self.books_head,"trades_head":self.trades_head,"last_error":self.last_error,
        "started_ms":self.started_ms,"finished_ms":self.finished_ms})
    }
}

pub struct Handle {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<Status>>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Handle {
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    /// Block until the collector thread has ended (tests, shutdown).
    pub fn wait(&self) {
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}

struct Guard(Arc<Mutex<Status>>);
impl Drop for Guard {
    fn drop(&mut self) {
        let mut s = self.0.lock().unwrap();
        s.running = false;
        s.finished_ms = now_ms();
        if s.phase == "recording" || s.phase == "discovering" || s.phase.is_empty() {
            s.phase = "ended unexpectedly".into();
        }
    }
}

fn nap(stop: &AtomicBool, ms: u64) {
    let mut left = ms;
    while left > 0 && !stop.load(Ordering::SeqCst) {
        let d = left.min(100);
        std::thread::sleep(std::time::Duration::from_millis(d));
        left -= d;
    }
}

struct Chain {
    f: std::fs::File,
    prev: String,
    seq: u64,
}

impl Chain {
    fn create(path: &std::path::Path) -> Result<Chain, String> {
        let f = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Chain {
            f,
            prev: GENESIS.into(),
            seq: 0,
        })
    }
    fn push(&mut self, round: u64, raw: &str) -> Result<usize, String> {
        let rec = make_rec(self.seq, round, now_ms(), raw, &self.prev);
        let line = rec_to_line(&rec);
        writeln!(self.f, "{line}").map_err(|e| e.to_string())?;
        self.f.sync_data().map_err(|e| e.to_string())?;
        self.prev = rec.h;
        self.seq += 1;
        Ok(line.len() + 1)
    }
}

/// Start a collection session in the background. Fails if the output directory cannot be created.
pub fn start(cfg: Config, fetch: Fetch) -> Result<Handle, String> {
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| format!("{}: {e}", cfg.out_dir.display()))?;
    let session = format!("{}", now_ms());
    let status = Arc::new(Mutex::new(Status {
        running: true,
        phase: "discovering".into(),
        session: session.clone(),
        rounds: cfg.rounds,
        started_ms: now_ms(),
        ..Status::default()
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let (st, sp) = (status.clone(), stop.clone());
    let join = std::thread::spawn(move || {
        let _g = Guard(st.clone());
        let set = |f: &dyn Fn(&mut Status)| f(&mut st.lock().unwrap());
        let fail = |m: String| {
            set(&|s| {
                s.last_error = m.clone();
                s.phase = "error".into();
            })
        };
        let url = format!(
            "{}/events?active=true&closed=false&limit={}&order=volume24hr&ascending=false",
            cfg.gamma, cfg.limit
        );
        let events = match fetch(&url).and_then(|b| parse_events(&b).map_err(|e| e.to_string())) {
            Ok(e) => e,
            Err(e) => return fail(format!("discover failed: {e}")),
        };
        let neg: Vec<&EventInfo> = events
            .iter()
            .filter(|e| e.neg_risk && e.markets.len() >= 2)
            .collect();
        let tokens: Vec<String> = events
            .iter()
            .filter(|e| e.neg_risk)
            .flat_map(|e| e.markets.iter().map(|m| m.yes_token.clone()))
            .collect();
        let conds: Vec<String> = events
            .iter()
            .flat_map(|e| e.markets.iter().map(|m| m.condition_id.clone()))
            .filter(|c| !c.is_empty())
            .collect();
        set(&|s| {
            s.events = events.len();
            s.neg_risk_events = neg.len();
        });
        if tokens.is_empty() {
            return fail("no negRisk markets found".into());
        }
        let ev_text = serde_json::to_string_pretty(&events_to_json(&events)).unwrap_or_default();
        let dir = &cfg.out_dir;
        let mut bytes = ev_text.len() as u64;
        let (mut books, mut trades) = match (
            std::fs::write(dir.join(format!("events-{session}.json")), &ev_text)
                .map_err(|e| e.to_string()),
            Chain::create(&dir.join(format!("books-{session}.jsonl"))),
            Chain::create(&dir.join(format!("trades-{session}.jsonl"))),
        ) {
            (Ok(()), Ok(b), Ok(t)) => (b, t),
            (Err(e), ..) | (_, Err(e), _) | (_, _, Err(e)) => return fail(e),
        };
        set(&|s| s.phase = "recording".into());
        let mut round = 0u64;
        'outer: loop {
            for t in &tokens {
                if sp.load(Ordering::SeqCst) {
                    break 'outer;
                }
                match fetch(&format!("{}/book?token_id={t}", cfg.clob))
                    .and_then(|raw| parse_book(&raw).map(|_| raw).map_err(|e| e.to_string()))
                {
                    Ok(raw) => match books.push(round, &raw) {
                        Ok(n) => {
                            bytes += n as u64;
                            let h = books.prev.clone();
                            set(&|s| {
                                s.books_ok += 1;
                                s.bytes = bytes;
                                s.books_head = h.clone();
                            });
                        }
                        Err(e) => return fail(format!("write failed: {e}")),
                    },
                    Err(e) => set(&|s| {
                        s.books_bad += 1;
                        s.last_error = format!("book {t}: {e}");
                    }),
                }
                if bytes >= cfg.max_bytes {
                    return fail(format!(
                        "disk cap of {} bytes reached; session stopped",
                        cfg.max_bytes
                    ));
                }
                nap(&sp, cfg.pace_ms);
            }
            for c in &conds {
                if sp.load(Ordering::SeqCst) {
                    break 'outer;
                }
                match fetch(&format!("{}/trades?market={c}&limit=500", cfg.data))
                    .and_then(|raw| parse_trades(&raw).map(|_| raw).map_err(|e| e.to_string()))
                {
                    Ok(raw) => match trades.push(round, &raw) {
                        Ok(n) => {
                            bytes += n as u64;
                            let h = trades.prev.clone();
                            set(&|s| {
                                s.trade_polls_ok += 1;
                                s.bytes = bytes;
                                s.trades_head = h.clone();
                            });
                        }
                        Err(e) => return fail(format!("write failed: {e}")),
                    },
                    Err(e) => set(&|s| {
                        s.trade_polls_bad += 1;
                        s.last_error = format!("trades {c}: {e}");
                    }),
                }
                if bytes >= cfg.max_bytes {
                    return fail(format!(
                        "disk cap of {} bytes reached; session stopped",
                        cfg.max_bytes
                    ));
                }
                nap(&sp, cfg.pace_ms);
            }
            round += 1;
            set(&|s| s.round = round);
            if cfg.rounds != 0 && round >= cfg.rounds {
                break;
            }
            nap(&sp, cfg.interval_secs.saturating_mul(1000));
        }
        let stopped = sp.load(Ordering::SeqCst);
        set(&|s| {
            s.phase = if stopped {
                "stopped".into()
            } else {
                "finished".into()
            }
        });
    });
    Ok(Handle {
        stop,
        status,
        join: Mutex::new(Some(join)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_recording;

    pub fn fake() -> Fetch {
        Arc::new(|u: &str| {
            if u.contains("/events?") {
                Ok(r#"[{"slug":"e1","negRisk":true,"markets":[
                  {"question":"a","conditionId":"c1","active":true,"closed":false,"enableOrderBook":true,"clobTokenIds":"[\"y1\",\"n1\"]","outcomes":"[\"Yes\",\"No\"]"},
                  {"question":"b","conditionId":"c2","active":true,"closed":false,"enableOrderBook":true,"clobTokenIds":"[\"y2\",\"n2\"]","outcomes":"[\"Yes\",\"No\"]"}]}]"#.into())
            } else if let Some(t) = u.split("token_id=").nth(1) {
                Ok(format!(
                    r#"{{"market":"m","asset_id":"{t}","timestamp":"1762000000000","hash":"h","bids":[{{"price":"0.28","size":"100"}}],"asks":[{{"price":"0.30","size":"100"}}],"min_order_size":"5","tick_size":"0.01","neg_risk":true}}"#
                ))
            } else if u.contains("/trades?") {
                if u.contains("c2") {
                    Ok("not json".into())
                } else {
                    Ok(r#"[{"side":"SELL","asset":"y1","size":5,"price":0.29,"timestamp":1700000000,"transactionHash":"0x1"}]"#.into())
                }
            } else {
                Err("unexpected url".into())
            }
        })
    }

    fn cfg(name: &str, rounds: u64) -> Config {
        let d = std::env::temp_dir().join(format!("mas-runner-{name}-{}", now_ms()));
        Config {
            rounds,
            interval_secs: 0,
            pace_ms: 0,
            ..Config::new(d)
        }
    }

    #[test]
    fn records_verified_chains_and_counts_bad_polls() {
        let c = cfg("ok", 3);
        let dir = c.out_dir.clone();
        let h = start(c, fake()).unwrap();
        h.wait();
        let s = h.status();
        assert!(!s.running && s.phase == "finished" && s.round == 3, "{s:?}");
        assert_eq!((s.books_ok, s.books_bad), (6, 0)); // 2 tokens x 3 rounds
        assert_eq!((s.trade_polls_ok, s.trade_polls_bad), (3, 3)); // c2 returns garbage and is refused
        let files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        let b = files
            .iter()
            .find(|p| p.to_string_lossy().contains("books-"))
            .unwrap();
        let recs = read_recording(&std::fs::read_to_string(b).unwrap()).unwrap();
        assert_eq!(recs.len(), 6);
        assert_eq!(recs.last().unwrap().h, s.books_head);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn stop_is_prompt_and_disk_cap_and_discovery_errors_are_reported() {
        // endless session with a long interval: stop must not wait for it
        let mut c = cfg("stop", 0);
        c.interval_secs = 3600;
        let dir = c.out_dir.clone();
        let h = start(c, fake()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let t0 = std::time::Instant::now();
        h.stop();
        h.wait();
        assert!(t0.elapsed().as_secs() < 3 && h.status().phase == "stopped");
        std::fs::remove_dir_all(dir).ok();
        // disk cap
        let mut c = cfg("cap", 0);
        c.max_bytes = 1500;
        let dir = c.out_dir.clone();
        let h = start(c, fake()).unwrap();
        h.wait();
        let s = h.status();
        assert!(
            s.phase == "error" && s.last_error.contains("disk cap") && !s.running,
            "{s:?}"
        );
        std::fs::remove_dir_all(dir).ok();
        // discovery failure
        let h = start(cfg("disc", 1), Arc::new(|_| Err("network down".into()))).unwrap();
        h.wait();
        let s = h.status();
        assert!(s.phase == "error" && s.last_error.contains("network down") && !s.running);
    }
}
