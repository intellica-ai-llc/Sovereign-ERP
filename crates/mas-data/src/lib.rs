//! Market data for the trading track. Pure parsing and scanning; all network access lives in the `mas-data` binary.
//! Formats follow Polymarket's public documentation (GET /book -> OrderBookSummary; Gamma events/markets). Field
//! names for Gamma were taken from a third-party client's type definitions and the CLOB book from the official OpenAPI
//! page; neither has been checked against the live API from this environment (no network route), so the first real
//! run in a Codespace is the validation, and every parser refuses anything it does not recognise instead of guessing.
pub mod replay;
pub mod runner;
pub use runner::{events_from_json, events_to_json, http_get};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const MAX_LEVELS: usize = 2000;

#[derive(Debug, PartialEq)]
pub enum DataError {
    Json(String),
    Field(&'static str),
    Crossed,
    TooDeep,
    Chain(usize),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub price: f64,
    pub size: f64,
}

#[derive(Clone, Debug)]
pub struct Book {
    pub token: String,
    pub market: String,
    pub ts_ms: u64,
    pub hash: String,
    /// Best first (highest price first).
    pub bids: Vec<Level>,
    /// Best first (lowest price first).
    pub asks: Vec<Level>,
    pub tick: f64,
    pub min_size: f64,
    pub neg_risk: bool,
}

impl Book {
    pub fn best_bid(&self) -> Option<Level> {
        self.bids.first().copied()
    }
    pub fn best_ask(&self) -> Option<Level> {
        self.asks.first().copied()
    }
}

fn s<'a>(v: &'a Value, k: &'static str) -> Result<&'a str, DataError> {
    v.get(k).and_then(Value::as_str).ok_or(DataError::Field(k))
}

fn num(x: &str, k: &'static str) -> Result<f64, DataError> {
    x.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or(DataError::Field(k))
}

fn levels(v: &Value, k: &'static str, desc: bool) -> Result<Vec<Level>, DataError> {
    let arr = v
        .get(k)
        .and_then(Value::as_array)
        .ok_or(DataError::Field(k))?;
    if arr.len() > MAX_LEVELS {
        return Err(DataError::TooDeep);
    }
    let mut out = Vec::with_capacity(arr.len());
    for l in arr {
        let price = num(s(l, "price")?, "price")?;
        let size = num(s(l, "size")?, "size")?;
        if !(price > 0.0 && price <= 1.0 && size > 0.0) {
            return Err(DataError::Field("level out of range"));
        }
        out.push(Level { price, size });
    }
    // The documentation says best-first, but never rely on the sender's ordering.
    out.sort_by(|a, b| {
        if desc {
            b.price.total_cmp(&a.price)
        } else {
            a.price.total_cmp(&b.price)
        }
    });
    Ok(out)
}

/// Parse a GET /book response. A crossed book (best bid >= best ask) is refused: it is stale or corrupt.
pub fn parse_book(raw: &str) -> Result<Book, DataError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| DataError::Json(e.to_string()))?;
    let b = Book {
        token: s(&v, "asset_id")?.to_string(),
        market: s(&v, "market")?.to_string(),
        ts_ms: s(&v, "timestamp")?
            .parse::<u64>()
            .map_err(|_| DataError::Field("timestamp"))?,
        hash: s(&v, "hash")?.to_string(),
        bids: levels(&v, "bids", true)?,
        asks: levels(&v, "asks", false)?,
        tick: num(s(&v, "tick_size")?, "tick_size")?,
        min_size: num(s(&v, "min_order_size")?, "min_order_size")?,
        neg_risk: v
            .get("neg_risk")
            .and_then(Value::as_bool)
            .ok_or(DataError::Field("neg_risk"))?,
    };
    if b.token.is_empty() || !(b.tick > 0.0 && b.tick < 0.5) {
        return Err(DataError::Field("token or tick_size"));
    }
    if let (Some(x), Some(y)) = (b.best_bid(), b.best_ask()) {
        if x.price >= y.price {
            return Err(DataError::Crossed);
        }
    }
    Ok(b)
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarketInfo {
    pub question: String,
    pub condition_id: String,
    pub yes_token: String,
    pub no_token: String,
    pub end: String,
    pub rewards_min_size: Option<f64>,
    pub rewards_max_spread: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventInfo {
    pub slug: String,
    pub neg_risk: bool,
    pub markets: Vec<MarketInfo>,
}

/// Gamma encodes some list fields as JSON text inside a JSON string; accept either form.
fn string_list(v: &Value) -> Option<Vec<String>> {
    match v {
        Value::String(t) => serde_json::from_str::<Vec<String>>(t).ok(),
        Value::Array(a) => a.iter().map(|x| x.as_str().map(str::to_string)).collect(),
        _ => None,
    }
}

fn f(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|t| t.parse().ok()))
        .filter(|x| x.is_finite())
}

/// Parse a Gamma events list. Markets that are closed, inactive, without an order book, or whose outcomes are not
/// exactly ["Yes","No"] with two token ids are skipped (never guessed). Events with no usable markets are dropped.
pub fn parse_events(raw: &str) -> Result<Vec<EventInfo>, DataError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| DataError::Json(e.to_string()))?;
    let arr = v.as_array().ok_or(DataError::Field("events"))?;
    let mut out = vec![];
    for e in arr {
        let slug = e
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let neg_risk = e.get("negRisk").and_then(Value::as_bool).unwrap_or(false);
        let mut markets = vec![];
        for m in e
            .get("markets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let live = m.get("active").and_then(Value::as_bool) == Some(true)
                && m.get("closed").and_then(Value::as_bool) != Some(true)
                && m.get("enableOrderBook").and_then(Value::as_bool) == Some(true);
            let toks = m.get("clobTokenIds").and_then(string_list);
            let outs = m.get("outcomes").and_then(string_list);
            let (Some(toks), Some(outs)) = (toks, outs) else {
                continue;
            };
            if !live
                || toks.len() != 2
                || outs != ["Yes", "No"]
                || toks.iter().any(String::is_empty)
            {
                continue;
            }
            markets.push(MarketInfo {
                question: m
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                condition_id: m
                    .get("conditionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                yes_token: toks[0].clone(),
                no_token: toks[1].clone(),
                end: m
                    .get("endDateIso")
                    .or_else(|| m.get("endDate"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                rewards_min_size: m.get("rewardsMinSize").and_then(f),
                rewards_max_spread: m.get("rewardsMaxSpreads").and_then(f),
            });
        }
        if !markets.is_empty() {
            out.push(EventInfo {
                slug,
                neg_risk,
                markets,
            });
        }
    }
    Ok(out)
}

/// One recorded line: the raw response bytes are kept verbatim and hash-chained, so a recording is tamper-evident
/// and its head hash can be committed to the evidence log before any analysis (no cherry-picking of data).
#[derive(Clone, Debug, PartialEq)]
pub struct Rec {
    pub seq: u64,
    pub round: u64,
    pub t_ms: u64,
    pub raw: String,
    pub prev: String,
    pub h: String,
}

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn link(seq: u64, round: u64, t_ms: u64, raw: &str, prev: &str) -> String {
    let mut d = Sha256::new();
    d.update(format!("{prev}\n{seq}\n{round}\n{t_ms}\n").as_bytes());
    d.update(raw.as_bytes());
    hex::encode(d.finalize())
}

pub fn make_rec(seq: u64, round: u64, t_ms: u64, raw: &str, prev: &str) -> Rec {
    Rec {
        seq,
        round,
        t_ms,
        raw: raw.to_string(),
        prev: prev.to_string(),
        h: link(seq, round, t_ms, raw, prev),
    }
}

pub fn rec_to_line(r: &Rec) -> String {
    serde_json::json!({"seq":r.seq,"round":r.round,"t":r.t_ms,"raw":r.raw,"prev":r.prev,"h":r.h})
        .to_string()
}

/// Read and verify a whole recording. Any edit, reorder, deletion in the middle or splice is an error naming the line.
/// (Truncation of the tail is detectable only against a head hash committed elsewhere: see `head`.)
pub fn read_recording(text: &str) -> Result<Vec<Rec>, DataError> {
    let mut out: Vec<Rec> = vec![];
    for (i, line) in text.lines().enumerate() {
        let v: Value = serde_json::from_str(line).map_err(|_| DataError::Chain(i))?;
        let g = |k: &str| v.get(k);
        let (Some(seq), Some(round), Some(t), Some(raw), Some(prev), Some(h)) = (
            g("seq").and_then(Value::as_u64),
            g("round").and_then(Value::as_u64),
            g("t").and_then(Value::as_u64),
            g("raw").and_then(Value::as_str),
            g("prev").and_then(Value::as_str),
            g("h").and_then(Value::as_str),
        ) else {
            return Err(DataError::Chain(i));
        };
        let expect_prev = out.last().map_or(GENESIS, |r| r.h.as_str());
        if seq != i as u64 || prev != expect_prev || h != link(seq, round, t, raw, prev) {
            return Err(DataError::Chain(i));
        }
        out.push(Rec {
            seq,
            round,
            t_ms: t,
            raw: raw.to_string(),
            prev: prev.to_string(),
            h: h.to_string(),
        });
    }
    Ok(out)
}

/// Head hash and length: commit these to the evidence log when a recording is closed.
pub fn head(recs: &[Rec]) -> (String, usize) {
    (
        recs.last().map_or(GENESIS.to_string(), |r| r.h.clone()),
        recs.len(),
    )
}

#[derive(Debug, PartialEq)]
pub struct Opportunity {
    pub round: u64,
    pub event: String,
    pub side_buy_all: bool,
    pub shares: f64,
    pub edge: f64,
}

#[derive(Debug, Default, PartialEq)]
pub struct ScanSummary {
    pub rounds: usize,
    pub books_ok: usize,
    pub books_bad: usize,
    pub events_complete: usize,
    pub opportunities: Vec<Opportunity>,
}

/// Rebalancing-arbitrage scan over negRisk events. An event is evaluated in a round only if EVERY listed market
/// has a valid, two-sided YES book in that round. NegRisk events can contain placeholder or "other" outcomes the
/// listing does not show, so this reports candidates for human review; it never trades on inference.
pub fn scan_rebalance(
    recs: &[Rec],
    events: &[EventInfo],
    fee_rate: f64,
    leg_haircut: f64,
    min_edge: f64,
) -> ScanSummary {
    use mas_research::arb::{rebalance, Leg, Side};
    use std::collections::{BTreeMap, HashMap};
    let mut by_round: BTreeMap<u64, HashMap<String, Book>> = BTreeMap::new();
    let mut sum = ScanSummary::default();
    for r in recs {
        match parse_book(&r.raw) {
            Ok(b) => {
                sum.books_ok += 1;
                by_round
                    .entry(r.round)
                    .or_default()
                    .insert(b.token.clone(), b);
            }
            Err(_) => sum.books_bad += 1,
        }
    }
    sum.rounds = by_round.len();
    for (round, books) in &by_round {
        for e in events.iter().filter(|e| e.neg_risk && e.markets.len() >= 2) {
            let legs: Option<Vec<Leg>> = e
                .markets
                .iter()
                .map(|m| {
                    let b = books.get(&m.yes_token)?;
                    let (bid, ask) = (b.best_bid()?, b.best_ask()?);
                    Some(Leg {
                        bid: bid.price,
                        ask: ask.price,
                        depth: bid.size.min(ask.size),
                    })
                })
                .collect();
            let Some(legs) = legs else { continue };
            sum.events_complete += 1;
            if let Some(a) = rebalance(&legs, fee_rate, leg_haircut, min_edge) {
                sum.opportunities.push(Opportunity {
                    round: *round,
                    event: e.slug.clone(),
                    side_buy_all: a.side == Side::BuyAll,
                    shares: a.shares,
                    edge: a.edge,
                });
            }
        }
    }
    sum
}

/// One public trade print (Polymarket data API `GET /trades`). Needed to replay whether a resting order would have
/// filled. Field names follow the public docs and are unverified against the live API (see the module header).
#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub token: String,
    pub taker_buy: bool,
    pub price: f64,
    pub size: f64,
    pub ts: u64,
    pub tx: String,
}

pub fn parse_trades(raw: &str) -> Result<Vec<Trade>, DataError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| DataError::Json(e.to_string()))?;
    let arr = v.as_array().ok_or(DataError::Field("trades array"))?;
    arr.iter()
        .map(|t| {
            let side = t
                .get("side")
                .and_then(Value::as_str)
                .ok_or(DataError::Field("side"))?;
            let taker_buy = match side {
                "BUY" => true,
                "SELL" => false,
                _ => return Err(DataError::Field("side value")),
            };
            let f = |k: &'static str| {
                t.get(k)
                    .and_then(Value::as_f64)
                    .filter(|x| x.is_finite())
                    .ok_or(DataError::Field(k))
            };
            let (price, size) = (f("price")?, f("size")?);
            if !(price > 0.0 && price < 1.0 && size > 0.0) {
                return Err(DataError::Field("price or size range"));
            }
            Ok(Trade {
                token: t
                    .get("asset")
                    .and_then(Value::as_str)
                    .filter(|x| !x.is_empty())
                    .ok_or(DataError::Field("asset"))?
                    .to_string(),
                taker_buy,
                price,
                size,
                ts: t
                    .get("timestamp")
                    .and_then(Value::as_u64)
                    .ok_or(DataError::Field("timestamp"))?,
                tx: t
                    .get("transactionHash")
                    .and_then(Value::as_str)
                    .ok_or(DataError::Field("transactionHash"))?
                    .to_string(),
            })
        })
        .collect()
}

/// Merge overlapping polls of the same market: one trade per (tx, token, ts, price, size), sorted by time then tx.
pub fn merge_trades(mut all: Vec<Trade>) -> Vec<Trade> {
    all.sort_by(|a, b| {
        (a.ts, &a.tx, &a.token)
            .cmp(&(b.ts, &b.tx, &b.token))
            .then(a.price.total_cmp(&b.price))
            .then(a.size.total_cmp(&b.size))
    });
    all.dedup();
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn book_json(token: &str, bid: &str, ask: &str, size: &str) -> String {
        format!(
            r#"{{"market":"0xabc","asset_id":"{token}","timestamp":"1762000000000","hash":"h1",
            "bids":[{{"price":"0.01","size":"5"}},{{"price":"{bid}","size":"{size}"}}],
            "asks":[{{"price":"0.99","size":"5"}},{{"price":"{ask}","size":"{size}"}}],
            "min_order_size":"5","tick_size":"0.01","neg_risk":true,"last_trade_price":"0.5"}}"#
        )
    }

    #[test]
    fn parses_and_sorts_a_book_whatever_order_it_arrives_in() {
        let b = parse_book(&book_json("t1", "0.40", "0.45", "100")).unwrap();
        assert_eq!(b.best_bid().unwrap().price, 0.40);
        assert_eq!(b.best_ask().unwrap().price, 0.45);
        assert_eq!(b.best_ask().unwrap().size, 100.0);
        assert!(b.neg_risk && b.tick == 0.01 && b.min_size == 5.0);
    }

    #[test]
    fn refuses_crossed_malformed_and_out_of_range_books() {
        assert_eq!(
            parse_book(&book_json("t", "0.50", "0.45", "10")).unwrap_err(),
            DataError::Crossed
        );
        assert!(parse_book("not json").is_err());
        assert!(parse_book("{}").is_err());
        assert!(parse_book(&book_json("t", "1.5", "1.6", "10")).is_err());
        assert!(parse_book(&book_json("t", "0.4", "0.45", "-3")).is_err());
        assert!(parse_book(&book_json("t", "0.4", "0.45", "NaN")).is_err());
        assert!(parse_book(&book_json("", "0.4", "0.45", "10")).is_err());
    }

    const EVENTS: &str = r#"[{"slug":"who-wins","negRisk":true,"markets":[
      {"question":"A?","conditionId":"c1","clobTokenIds":"[\"a-yes\",\"a-no\"]","outcomes":"[\"Yes\",\"No\"]","active":true,"closed":false,"enableOrderBook":true,"endDateIso":"2026-12-01","rewardsMinSize":50,"rewardsMaxSpreads":3.5},
      {"question":"B?","conditionId":"c2","clobTokenIds":["b-yes","b-no"],"outcomes":["Yes","No"],"active":true,"closed":false,"enableOrderBook":true},
      {"question":"C?","conditionId":"c3","clobTokenIds":"[\"c-yes\",\"c-no\"]","outcomes":"[\"Yes\",\"No\"]","active":true,"closed":true,"enableOrderBook":true},
      {"question":"D?","conditionId":"c4","clobTokenIds":"[\"d1\",\"d2\",\"d3\"]","outcomes":"[\"X\",\"Y\",\"Z\"]","active":true,"enableOrderBook":true}]},
      {"slug":"empty","negRisk":false,"markets":[]}]"#;

    #[test]
    fn events_keep_only_live_yes_no_markets_in_either_encoding() {
        let ev = parse_events(EVENTS).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].markets.len(), 2);
        assert_eq!(ev[0].markets[0].yes_token, "a-yes");
        assert_eq!(ev[0].markets[0].rewards_min_size, Some(50.0));
        assert_eq!(ev[0].markets[0].rewards_max_spread, Some(3.5));
        assert_eq!(ev[0].markets[1].yes_token, "b-yes");
        assert!(parse_events("{}").is_err());
    }

    fn recording(rounds: &[(&str, &str, &str)]) -> String {
        // each round: three tokens a-yes, b-yes, c-yes with identical bid/ask/size
        let mut prev = GENESIS.to_string();
        let (mut seq, mut lines) = (0u64, vec![]);
        for (r, (bid, ask, size)) in rounds.iter().enumerate() {
            for tok in ["a-yes", "b-yes", "c-yes"] {
                let rec = make_rec(
                    seq,
                    r as u64,
                    1000 + seq,
                    &tests::book_json(tok, bid, ask, size),
                    &prev,
                );
                prev = rec.h.clone();
                lines.push(rec_to_line(&rec));
                seq += 1;
            }
        }
        lines.join("\n") + "\n"
    }

    fn three_leg_event() -> Vec<EventInfo> {
        let m = |t: &str| MarketInfo {
            question: String::new(),
            condition_id: String::new(),
            yes_token: t.into(),
            no_token: String::new(),
            end: String::new(),
            rewards_min_size: None,
            rewards_max_spread: None,
        };
        vec![EventInfo {
            slug: "who-wins".into(),
            neg_risk: true,
            markets: vec![m("a-yes"), m("b-yes"), m("c-yes")],
        }]
    }

    #[test]
    fn recording_roundtrips_and_the_scan_finds_only_real_gaps() {
        // round 0: asks 0.30 x3 = 0.90 (buy-all edge 0.10); round 1: asks 0.34 x3 = 1.02 (nothing)
        let text = recording(&[("0.28", "0.30", "100"), ("0.32", "0.34", "100")]);
        let recs = read_recording(&text).unwrap();
        assert_eq!(recs.len(), 6);
        let s = scan_rebalance(&recs, &three_leg_event(), 0.0, 0.0, 0.02);
        assert_eq!(
            (s.rounds, s.books_ok, s.books_bad, s.events_complete),
            (2, 6, 0, 2)
        );
        assert_eq!(s.opportunities.len(), 1);
        assert_eq!(s.opportunities[0].round, 0);
        assert!(s.opportunities[0].side_buy_all);
        assert!((s.opportunities[0].edge - 0.10).abs() < 1e-9);
        // with fees and a leg haircut the same data shows nothing
        assert!(scan_rebalance(&recs, &three_leg_event(), 0.07, 0.02, 0.02)
            .opportunities
            .is_empty());
    }

    #[test]
    fn incomplete_rounds_are_not_scanned() {
        let text = recording(&[("0.28", "0.30", "100")]);
        let mut lines: Vec<&str> = text.lines().collect();
        lines.pop(); // lose the third leg
        let recs = read_recording(&(lines.join("\n") + "\n")).unwrap();
        let s = scan_rebalance(&recs, &three_leg_event(), 0.0, 0.0, 0.02);
        assert_eq!(s.events_complete, 0);
        assert!(s.opportunities.is_empty());
    }

    #[test]
    fn tampering_reordering_and_splicing_are_detected() {
        let text = recording(&[("0.28", "0.30", "100"), ("0.32", "0.34", "100")]);
        assert!(read_recording(&text).is_ok());
        // edit a price inside one raw book
        let edited = text.replacen("0.28", "0.27", 1);
        assert!(matches!(read_recording(&edited), Err(DataError::Chain(_))));
        // delete a middle line
        let mut l: Vec<&str> = text.lines().collect();
        l.remove(2);
        assert!(matches!(
            read_recording(&(l.join("\n") + "\n")),
            Err(DataError::Chain(2))
        ));
        // swap two lines
        let mut l: Vec<&str> = text.lines().collect();
        l.swap(1, 2);
        assert!(read_recording(&(l.join("\n") + "\n")).is_err());
        // the head hash commits to the whole recording
        let recs = read_recording(&text).unwrap();
        let (h, n) = head(&recs);
        assert_eq!(n, 6);
        assert_eq!(h, recs[5].h);
    }

    #[test]
    fn trades_parse_strictly_and_overlapping_polls_merge() {
        let raw = r#"[{"side":"SELL","asset":"Y1","size":50,"price":0.41,"timestamp":1700000002,"transactionHash":"0xb"},
                      {"side":"BUY","asset":"Y1","size":10,"price":0.42,"timestamp":1700000001,"transactionHash":"0xa"}]"#;
        let a = parse_trades(raw).unwrap();
        assert!(!a[0].taker_buy && a[1].taker_buy);
        let merged = merge_trades([a.clone(), a.clone()].concat());
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].tx, "0xa");
        for bad in [
            r#"{"x":1}"#,
            r#"[{"side":"HOLD","asset":"Y","size":1,"price":0.5,"timestamp":1,"transactionHash":"0x"}]"#,
            r#"[{"side":"BUY","asset":"Y","size":1,"price":1.5,"timestamp":1,"transactionHash":"0x"}]"#,
            r#"[{"side":"BUY","asset":"","size":1,"price":0.5,"timestamp":1,"transactionHash":"0x"}]"#,
            r#"[{"side":"BUY","asset":"Y","size":0,"price":0.5,"timestamp":1,"transactionHash":"0x"}]"#,
            r#"[{"side":"BUY","asset":"Y","size":1,"price":0.5,"transactionHash":"0x"}]"#,
        ] {
            assert!(parse_trades(bad).is_err(), "{bad}");
        }
    }
}
