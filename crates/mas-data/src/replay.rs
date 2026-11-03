//! Paper replay: feeds RECORDED order books and public trades through the paper engine with a baseline quoting
//! policy, producing the ledger a Verified Performance Card is built from.
//!
//! What this is and is not: the policy is a deliberately simple baseline (join the best bid when the spread is wide
//! enough), here to prove the harness, not to claim alpha. Fills are pessimistic (see `mas_research::paper`).
//! Positions are marked to the mid of the latest recorded book (no resolution data), so returns are mark-to-market,
//! not realised. There is no venue, so no reconciliation and no kill-switch test is ever recorded: a card built from
//! a replay can never read Verified, by design.
use crate::{merge_trades, parse_book, parse_trades, Book, DataError, Rec, Trade};
use mas_research::card::{self, Card, CardInput, Rules};
use mas_research::paper::{self, Engine, Ev};
use mas_research::registry::Registry;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Params {
    pub capital: f64,
    pub cap: f64,
    pub queue_factor: f64,
    pub size: f64,
    pub min_spread_ticks: f64,
    pub mid_lo: f64,
    pub mid_hi: f64,
    pub period_secs: u64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            capital: 1000.0,
            cap: 200.0,
            queue_factor: 0.5,
            size: 20.0,
            min_spread_ticks: 2.0,
            mid_lo: 0.10,
            mid_hi: 0.90,
            period_secs: 86_400,
        }
    }
}

impl Params {
    /// Canonical text registered in the trial registry BEFORE a run.
    pub fn canonical(&self) -> String {
        format!(
            "lp1-baseline;capital={};cap={};queue={};size={};min_spread_ticks={};mid=[{},{}];period_secs={}",
            self.capital, self.cap, self.queue_factor, self.size, self.min_spread_ticks, self.mid_lo, self.mid_hi, self.period_secs
        )
    }
}

#[derive(Debug)]
pub struct Replay {
    pub events: Vec<Ev>,
    pub returns: Vec<f64>,
    pub books_used: usize,
    pub trades_used: usize,
    pub first_ms: u64,
    pub last_ms: u64,
}

pub fn run(books: &[Rec], trade_recs: &[Rec], p: &Params) -> Result<Replay, String> {
    let mut snaps: Vec<(u64, Book)> = vec![];
    for r in books {
        if let Ok(b) = parse_book(&r.raw) {
            snaps.push((r.t_ms, b));
        }
    }
    snaps.sort_by_key(|(t, _)| *t);
    if snaps.is_empty() {
        return Err("no valid books in the recording".into());
    }
    let mut all: Vec<Trade> = vec![];
    for r in trade_recs {
        all.extend(parse_trades(&r.raw).map_err(|e: DataError| e.to_string())?);
    }
    let trades = merge_trades(all);
    let mut e = Engine::new(p.capital, p.cap, p.queue_factor).map_err(|x| format!("{x:?}"))?;
    let (first_ms, mut last_ms) = (snaps[0].0, snaps[0].0);
    let period_ms = p.period_secs.max(1) * 1000;
    let mut cur_period = 0u64;
    let mut mids: HashMap<String, f64> = HashMap::new();
    let mut ti = 0usize;
    let mut used = 0usize;
    for (t, b) in &snaps {
        let t = *t;
        // trades that printed since the previous snapshot meet the orders placed at that snapshot
        while ti < trades.len() && trades[ti].ts * 1000 <= t {
            let tr = &trades[ti];
            e.on_trade(tr.ts * 1000, &tr.token, tr.price, tr.size, !tr.taker_buy);
            ti += 1;
            used += 1;
        }
        let period = (t - first_ms) / period_ms;
        if period != cur_period {
            let marks: Vec<(&str, f64)> = mids.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            e.close_period(t, &marks).map_err(|x| format!("{x:?}"))?;
            cur_period = period;
        }
        last_ms = t;
        let (Some(bid), Some(ask)) = (b.best_bid(), b.best_ask()) else {
            continue;
        };
        let mid = (bid.price + ask.price) / 2.0;
        mids.insert(b.token.clone(), mid);
        for id in e.open_ids_for(&b.token) {
            let _ = e.cancel(t, id);
        }
        if ask.price - bid.price >= p.min_spread_ticks * b.tick - 1e-12
            && mid >= p.mid_lo
            && mid <= p.mid_hi
        {
            let _ = e.quote(t, &b.token, bid.price, p.size.max(b.min_size)); // refusals (cap, cash) just skip
        }
    }
    let marks: Vec<(&str, f64)> = mids.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    e.close_period(last_ms, &marks)
        .map_err(|x| format!("{x:?}"))?;
    let events = e.events().to_vec();
    Ok(Replay {
        returns: paper::returns(&events),
        events,
        books_used: snaps.len(),
        trades_used: used,
        first_ms,
        last_ms,
    })
}

/// Build the claim card for a replay: registers the parameters first, records the result, audits the event list.
pub fn make_card(r: &Replay, p: &Params, books_head: &str) -> Card {
    let mut reg = Registry::new();
    let id = reg.register("lp1", &p.canonical());
    let _ = reg.record(&id, &r.returns); // too few or degenerate periods: stays unrecorded, claim stays NotYet
    let audit = paper::audit(&r.events, p.cap, 0.2, r.last_ms);
    let max_ret = r.returns.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    card::build(
        &CardInput {
            strategy: "lp1-baseline",
            period: "paper replay of recorded data",
            returns: &r.returns,
            scale: (max_ret * 2.0).max(0.01),
            registry: &reg,
            trial: &id,
            audit: &audit,
            log_head: books_head,
        },
        &Rules::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{make_rec, GENESIS};

    fn book(token: &str, bid: &str, ask: &str) -> String {
        format!(
            r#"{{"market":"m","asset_id":"{token}","timestamp":"1","hash":"h","bids":[{{"price":"{bid}","size":"500"}}],"asks":[{{"price":"{ask}","size":"500"}}],"min_order_size":"5","tick_size":"0.01","neg_risk":true}}"#
        )
    }
    fn rec(seq: u64, t: u64, raw: &str) -> Rec {
        make_rec(seq, 0, t, raw, GENESIS)
    }
    fn trade(side: &str, price: f64, size: f64, ts: u64, tx: &str) -> String {
        format!(
            r#"[{{"side":"{side}","asset":"y1","size":{size},"price":{price},"timestamp":{ts},"transactionHash":"{tx}"}}]"#
        )
    }

    #[test]
    fn a_fill_needs_a_later_trade_and_is_marked_to_mid() {
        let p = Params {
            period_secs: 5,
            ..Params::default()
        };
        let books = [
            rec(0, 0, &book("y1", "0.40", "0.46")),
            rec(1, 10_000, &book("y1", "0.40", "0.46")),
        ];
        // a SELL at 0.40 one second after the first snapshot, plus one before it (must be ignored for fills)
        let trades = [rec(0, 0, &trade("SELL", 0.40, 100.0, 1, "0x1"))];
        let r = run(&books, &trades, &p).unwrap();
        assert_eq!((r.books_used, r.trades_used), (2, 1));
        // 20 shares filled at 0.40 (cost 8), marked at mid 0.43 (8.6): +0.6 on capital 1000
        assert!((r.returns[0] - 0.0006).abs() < 1e-9, "{:?}", r.returns);
        assert_eq!(r.returns.len(), 2);
        let a = paper::audit(&r.events, p.cap, 0.2, 10_000);
        assert!(
            a.orders_total >= 1
                && a.orders_total == a.orders_with_intent
                && a.reconciliations_done == 0
        );
    }

    #[test]
    fn buys_wrong_price_and_early_trades_never_fill() {
        let p = Params {
            period_secs: 5,
            ..Params::default()
        };
        let books = [
            rec(0, 5_000, &book("y1", "0.40", "0.46")),
            rec(1, 20_000, &book("y1", "0.40", "0.46")),
        ];
        for t in [
            trade("BUY", 0.40, 100.0, 7, "0xa"), // taker bought: not a fill for a resting buy
            trade("SELL", 0.41, 100.0, 7, "0xb"), // above our price
            trade("SELL", 0.40, 100.0, 3, "0xc"), // before we quoted
        ] {
            let r = run(&books, &[rec(0, 0, &t)], &p).unwrap();
            assert!(
                r.returns.iter().all(|x| x.abs() < 1e-12),
                "{t} -> {:?}",
                r.returns
            );
        }
    }

    #[test]
    fn narrow_spreads_and_extreme_prices_are_not_quoted_and_bad_input_errors() {
        let p = Params::default();
        let books = [
            rec(0, 0, &book("y1", "0.40", "0.41")),
            rec(1, 1000, &book("y1", "0.02", "0.06")),
        ];
        let r = run(&books, &[], &p).unwrap();
        assert!(!r.events.iter().any(|e| matches!(e, Ev::Order { .. })));
        assert!(run(&[], &[], &p).is_err());
        assert!(run(&books, &[rec(0, 0, "not json")], &p).is_err());
    }

    #[test]
    fn a_replay_card_can_never_read_verified() {
        let p = Params {
            period_secs: 1,
            ..Params::default()
        };
        // 200 one-second periods of a market that keeps paying the baseline: still not Verified (no venue)
        let books: Vec<Rec> = (0..200)
            .map(|i| rec(i, i * 1000, &book("y1", "0.40", "0.46")))
            .collect();
        let trades: Vec<Rec> = (0..200)
            .map(|i| rec(i, 0, &trade("SELL", 0.40, 100.0, i + 1, &format!("0x{i}"))))
            .collect();
        let r = run(&books, &trades, &p).unwrap();
        let c = make_card(&r, &p, "headhash");
        assert_ne!(c.standing, card::Standing::Verified);
        let rec_claim = c
            .claims
            .iter()
            .find(|x| x.name() == "books_reconcile_to_venue")
            .unwrap();
        assert!(!rec_claim.is_proven());
        assert!(card::public_statement(&c).contains("NOT YET ESTABLISHED"));
    }
}
