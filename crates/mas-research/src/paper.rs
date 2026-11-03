//! Paper-trading engine and independent audit: the source of the ledger a Verified Performance Card is built from.
//!
//! Safety by construction: only maker BUY orders exist (a view against an outcome is a buy of the other token), every
//! order needs an intent first, exposure can never exceed the hard cap, and fills are deliberately pessimistic: a
//! resting buy fills only from a LATER public trade that sold at or below our price, takes at most `queue_factor`
//! of that trade's size, and is charged at OUR price (no price improvement). Maker rebates and liquidity rewards are
//! not counted. Every action is an `Ev`; `audit` and `returns` recompute everything from the event list alone, so a
//! third party can check the engine's claims (and catch a log from which an intent was removed).
use crate::card::Audit;
use std::collections::HashMap;

const DAY_MS: u64 = 86_400_000;

#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    Intent {
        id: u64,
        t: u64,
        token: String,
        price: f64,
        size: f64,
    },
    Order {
        id: u64,
        t: u64,
    },
    Fill {
        id: u64,
        t: u64,
        qty: f64,
    },
    Cancel {
        id: u64,
        t: u64,
    },
    Resolve {
        t: u64,
        token: String,
        payout: f64,
    },
    Period {
        t: u64,
        ret: f64,
    },
    KillTest {
        t: u64,
        passed: bool,
    },
    Reconcile {
        t: u64,
        ok: bool,
    },
    Override {
        t: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    BadInput,
    CapExceeded,
    NoCash,
    UnknownOrder,
    MissingMark,
}

pub struct Engine {
    capital: f64,
    cap: f64,
    queue_factor: f64,
    cash: f64,
    last_equity: f64,
    next_id: u64,
    ev: Vec<Ev>,
    // id -> (token, price, remaining, placed_at)
    open: HashMap<u64, (String, f64, f64, u64)>,
    pos: HashMap<String, (f64, f64)>, // token -> (qty, cost)
}

impl Engine {
    /// `capital`: money the strategy is measured against. `cap`: hard exposure limit (<= capital).
    /// `queue_factor` in (0, 1]: share of each qualifying public trade we are assumed to receive.
    pub fn new(capital: f64, cap: f64, queue_factor: f64) -> Result<Self, Refused> {
        let ok = capital.is_finite()
            && capital > 0.0
            && cap.is_finite()
            && cap > 0.0
            && cap <= capital
            && queue_factor > 0.0
            && queue_factor <= 1.0;
        if !ok {
            return Err(Refused::BadInput);
        }
        Ok(Self {
            capital,
            cap,
            queue_factor,
            cash: capital,
            last_equity: capital,
            next_id: 0,
            ev: vec![],
            open: HashMap::new(),
            pos: HashMap::new(),
        })
    }

    fn open_notional(&self) -> f64 {
        self.open.values().map(|(_, p, r, _)| p * r).sum()
    }
    pub fn exposure(&self) -> f64 {
        self.open_notional() + self.pos.values().map(|(_, c)| c).sum::<f64>()
    }
    pub fn events(&self) -> &[Ev] {
        &self.ev
    }

    /// Log the intent, then place a resting maker buy. Refuses (and logs nothing) if it would break the cap or cash.
    pub fn quote(&mut self, t: u64, token: &str, price: f64, size: f64) -> Result<u64, Refused> {
        if token.is_empty()
            || !(price.is_finite() && price > 0.0 && price < 1.0 && size.is_finite() && size > 0.0)
        {
            return Err(Refused::BadInput);
        }
        let notional = price * size;
        if self.exposure() + notional > self.cap + 1e-9 {
            return Err(Refused::CapExceeded);
        }
        if self.cash - self.open_notional() < notional - 1e-9 {
            return Err(Refused::NoCash);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.ev.push(Ev::Intent {
            id,
            t,
            token: token.into(),
            price,
            size,
        });
        self.ev.push(Ev::Order { id, t });
        self.open.insert(id, (token.into(), price, size, t));
        Ok(id)
    }

    /// Ids of our resting orders on a token (so a strategy can cancel and re-quote).
    pub fn open_ids_for(&self, token: &str) -> Vec<u64> {
        let mut v: Vec<u64> = self
            .open
            .iter()
            .filter(|(_, o)| o.0 == token)
            .map(|(id, _)| *id)
            .collect();
        v.sort_unstable();
        v
    }

    pub fn cancel(&mut self, t: u64, id: u64) -> Result<(), Refused> {
        self.open.remove(&id).ok_or(Refused::UnknownOrder)?;
        self.ev.push(Ev::Cancel { id, t });
        Ok(())
    }

    /// A public trade printed. `taker_sold`: the aggressor hit the bids (the only way a resting buy gets filled).
    pub fn on_trade(&mut self, t: u64, token: &str, price: f64, size: f64, taker_sold: bool) {
        if !taker_sold || !(price.is_finite() && size.is_finite() && size > 0.0) {
            return;
        }
        let mut ids: Vec<u64> = self.open.keys().copied().collect();
        ids.sort_unstable();
        let mut budget = size * self.queue_factor;
        for id in ids {
            if budget <= 0.0 {
                break;
            }
            let (tok, p, rem, placed) = self.open[&id].clone();
            if tok != token || t <= placed || price > p + 1e-12 {
                continue;
            }
            let qty = rem.min(budget);
            budget -= qty;
            self.cash -= qty * p;
            let e = self.pos.entry(tok).or_insert((0.0, 0.0));
            e.0 += qty;
            e.1 += qty * p;
            if rem - qty <= 1e-12 {
                self.open.remove(&id);
            } else if let Some(o) = self.open.get_mut(&id) {
                o.2 = rem - qty;
            }
            self.ev.push(Ev::Fill { id, t, qty });
        }
    }

    /// Market resolved: `payout` is 1.0 for the winning token, 0.0 otherwise. Open orders on it are cancelled.
    pub fn resolve(&mut self, t: u64, token: &str, payout: f64) -> Result<(), Refused> {
        if payout != 0.0 && payout != 1.0 {
            return Err(Refused::BadInput);
        }
        self.open.retain(|_, o| o.0 != token);
        if let Some((q, _)) = self.pos.remove(token) {
            self.cash += q * payout;
        }
        self.ev.push(Ev::Resolve {
            t,
            token: token.into(),
            payout,
        });
        Ok(())
    }

    /// Close a reporting period at the given marks; returns the net period return on `capital`.
    pub fn close_period(&mut self, t: u64, marks: &[(&str, f64)]) -> Result<f64, Refused> {
        let mut eq = self.cash;
        for (tok, (q, _)) in &self.pos {
            let m = marks
                .iter()
                .find(|(k, _)| k == tok)
                .ok_or(Refused::MissingMark)?
                .1;
            if !(m.is_finite() && (0.0..=1.0).contains(&m)) {
                return Err(Refused::BadInput);
            }
            eq += q * m;
        }
        let ret = (eq - self.last_equity) / self.capital;
        self.last_equity = eq;
        self.ev.push(Ev::Period { t, ret });
        Ok(ret)
    }

    pub fn kill_test(&mut self, t: u64, passed: bool) {
        self.ev.push(Ev::KillTest { t, passed });
    }
    pub fn reconcile(&mut self, t: u64, ok: bool) {
        self.ev.push(Ev::Reconcile { t, ok });
    }
}

pub fn returns(events: &[Ev]) -> Vec<f64> {
    events
        .iter()
        .filter_map(|e| {
            if let Ev::Period { ret, .. } = e {
                Some(*ret)
            } else {
                None
            }
        })
        .collect()
}

/// Recompute the audit facts from the event list alone. `cap` is the declared hard limit.
pub fn audit(events: &[Ev], cap: f64, drawdown_bound: f64, now_ms: u64) -> Audit {
    let mut intents: HashMap<u64, (String, f64)> = HashMap::new();
    let mut open: HashMap<u64, (String, f64, f64)> = HashMap::new(); // id -> (token, price, remaining)
    let mut pos: HashMap<String, f64> = HashMap::new(); // token -> cost
    let (mut orders, mut with_intent, mut overrides, mut kills, mut breaks) =
        (0u64, 0u64, 0u32, 0u32, 0u32);
    let mut recon_done = 0u32;
    let (mut last_kill, mut first, mut last, mut peak) = (None::<u64>, u64::MAX, 0u64, 0.0f64);
    for e in events {
        let t = match e {
            Ev::Intent { t, .. }
            | Ev::Order { t, .. }
            | Ev::Fill { t, .. }
            | Ev::Cancel { t, .. }
            | Ev::Resolve { t, .. }
            | Ev::Period { t, .. }
            | Ev::KillTest { t, .. }
            | Ev::Reconcile { t, .. }
            | Ev::Override { t } => *t,
        };
        first = first.min(t);
        last = last.max(t);
        match e {
            Ev::Intent {
                id,
                token,
                price,
                size,
                ..
            } => {
                intents.insert(*id, (token.clone(), *price));
                // staged under a reserved key until the matching order event arrives
                open.insert(u64::MAX - *id, (token.clone(), *price, *size));
            }
            Ev::Order { id, .. } => {
                orders += 1;
                if let Some((tok, p)) = intents.get(id) {
                    with_intent += 1;
                    if let Some((_, _, size)) = open.remove(&(u64::MAX - *id)) {
                        open.insert(*id, (tok.clone(), *p, size));
                    }
                }
            }
            Ev::Fill { id, qty, .. } => {
                if let Some((tok, p, rem)) = open.get_mut(id) {
                    let q = qty.min(*rem);
                    *rem -= q;
                    *pos.entry(tok.clone()).or_insert(0.0) += q * *p;
                }
            }
            Ev::Cancel { id, .. } => {
                open.remove(id);
            }
            Ev::Resolve { token, .. } => {
                open.retain(|_, o| &o.0 != token);
                pos.remove(token);
            }
            Ev::KillTest { t, passed } => {
                if *passed {
                    kills += 1;
                    last_kill = Some(last_kill.map_or(*t, |k| k.max(*t)));
                }
            }
            Ev::Reconcile { ok, .. } => {
                recon_done += 1;
                breaks += u32::from(!*ok);
            }
            Ev::Override { .. } => overrides += 1,
            Ev::Period { .. } => {}
        }
        let exposure: f64 = open
            .iter()
            .filter(|(k, _)| **k < u64::MAX / 2)
            .map(|(_, o)| o.1 * o.2)
            .sum::<f64>()
            + pos.values().sum::<f64>();
        peak = peak.max(exposure);
    }
    Audit {
        orders_total: orders,
        orders_with_intent: with_intent,
        max_exposure_frac: if cap > 0.0 { peak / cap } else { f64::INFINITY },
        limit_overrides: overrides,
        kill_tests_passed: kills,
        days_since_last_kill_test: last_kill.map_or(u32::MAX, |k| {
            (now_ms.saturating_sub(k) / DAY_MS).min(u32::MAX as u64) as u32
        }),
        reconciliation_breaks: breaks,
        reconciliations_done: recon_done,
        live_days: if first == u64::MAX {
            0
        } else {
            ((last - first) / DAY_MS).min(u32::MAX as u64) as u32
        },
        drawdown_bound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{build, CardInput, Rules, Standing};
    use crate::registry::Registry;

    fn eng() -> Engine {
        Engine::new(1000.0, 200.0, 0.5).unwrap()
    }

    #[test]
    fn fills_are_pessimistic() {
        let mut e = eng();
        let id = e.quote(1000, "YES", 0.40, 100.0).unwrap();
        e.on_trade(1000, "YES", 0.39, 500.0, true); // same instant as placement: no fill
        e.on_trade(2000, "YES", 0.41, 500.0, true); // above our price: no fill
        e.on_trade(2000, "YES", 0.39, 500.0, false); // taker bought, not sold: no fill
        e.on_trade(2000, "NO", 0.39, 500.0, true); // wrong token
        assert_eq!(e.exposure(), 40.0);
        e.on_trade(3000, "YES", 0.39, 60.0, true); // we get at most half: 30 shares, at OUR price 0.40
        assert!((e.pos["YES"].0 - 30.0).abs() < 1e-9 && (e.pos["YES"].1 - 12.0).abs() < 1e-9);
        assert!((e.cash - 988.0).abs() < 1e-9);
        assert!(e.cancel(4000, id).is_ok() && e.cancel(4000, id) == Err(Refused::UnknownOrder));
    }

    #[test]
    fn the_cap_and_cash_cannot_be_exceeded() {
        let mut e = eng();
        assert!(e.quote(1, "A", 0.5, 300.0).is_ok()); // 150
        assert_eq!(e.quote(2, "B", 0.5, 120.0), Err(Refused::CapExceeded)); // 150 + 60 > 200
        assert!(e.quote(3, "B", 0.5, 100.0).is_ok()); // exactly 200
        for bad in [
            (0.0, 1.0),
            (1.0, 1.0),
            (0.5, 0.0),
            (f64::NAN, 1.0),
            (0.5, f64::INFINITY),
        ] {
            assert_eq!(e.quote(4, "C", bad.0, bad.1), Err(Refused::BadInput));
        }
        assert!(Engine::new(100.0, 200.0, 0.5).is_err() && Engine::new(100.0, 50.0, 0.0).is_err());
        let a = audit(e.events(), 200.0, 0.2, 10);
        assert!(
            (a.max_exposure_frac - 1.0).abs() < 1e-9
                && a.orders_total == 2
                && a.orders_with_intent == 2
        );
    }

    #[test]
    fn resolution_pays_and_period_returns_account_for_everything() {
        let mut e = eng();
        e.quote(1, "YES", 0.40, 100.0).unwrap();
        e.on_trade(2, "YES", 0.40, 400.0, true); // 100 shares filled (budget 200)
        assert_eq!(e.close_period(3, &[]), Err(Refused::MissingMark));
        let r = e.close_period(3, &[("YES", 0.40)]).unwrap();
        assert!(r.abs() < 1e-12); // marked at cost: no change
        e.resolve(4, "YES", 1.0).unwrap(); // +60 profit
        let r = e.close_period(5, &[]).unwrap();
        assert!((r - 0.06).abs() < 1e-12, "{r}");
        assert_eq!(e.resolve(6, "YES", 0.5), Err(Refused::BadInput));
        assert_eq!(returns(e.events()).len(), 2);
    }

    #[test]
    fn audit_catches_a_log_with_an_intent_removed_and_overrides() {
        let mut e = eng();
        e.quote(1, "A", 0.4, 50.0).unwrap();
        e.quote(2, "B", 0.4, 50.0).unwrap();
        e.kill_test(3, true);
        e.reconcile(4, true);
        let clean = audit(e.events(), 200.0, 0.2, 5 * DAY_MS);
        assert!(
            clean.orders_total == 2
                && clean.orders_with_intent == 2
                && clean.kill_tests_passed == 1
        );
        assert_eq!(clean.days_since_last_kill_test, 4);
        let mut forged: Vec<Ev> = e.events().to_vec();
        forged.remove(0); // delete the first intent
        let a = audit(&forged, 200.0, 0.2, 5 * DAY_MS);
        assert_eq!((a.orders_total, a.orders_with_intent), (2, 1));
        forged.push(Ev::Override { t: 9 });
        forged.push(Ev::Reconcile { t: 9, ok: false });
        let a = audit(&forged, 200.0, 0.2, 5 * DAY_MS);
        assert_eq!((a.limit_overrides, a.reconciliation_breaks), (1, 1));
        assert_eq!(
            audit(&[], 200.0, 0.2, 1).days_since_last_kill_test,
            u32::MAX
        );
    }

    #[test]
    fn engine_to_card_pipeline_is_honest_about_a_short_run() {
        let mut e = eng();
        let mut x = 11u64;
        for day in 0..40u64 {
            let t = day * DAY_MS;
            let tok = format!("T{day}");
            e.quote(t, &tok, 0.45, 100.0).unwrap();
            e.on_trade(t + 1, &tok, 0.44, 400.0, true);
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            e.resolve(t + 2, &tok, if (x >> 33) % 100 < 52 { 1.0 } else { 0.0 })
                .unwrap();
            e.close_period(t + 3, &[]).unwrap();
        }
        e.kill_test(39 * DAY_MS, true);
        e.reconcile(39 * DAY_MS, true);
        let r = returns(e.events());
        let a = audit(e.events(), 200.0, 0.2, 40 * DAY_MS);
        assert_eq!((a.orders_total, a.orders_with_intent), (40, 40));
        assert!(a.max_exposure_frac <= 1.0);
        let mut reg = Registry::new();
        let id = reg.register("lp1", "queue=0.5;price=0.45");
        reg.record(&id, &r).unwrap();
        let c = build(
            &CardInput {
                strategy: "lp1",
                period: "paper",
                returns: &r,
                scale: 0.1,
                registry: &reg,
                trial: &id,
                audit: &a,
                log_head: "00",
            },
            &Rules::default(),
        );
        // 40 periods, 39 live days: nothing may be claimed
        assert_ne!(c.standing, Standing::Verified);
        assert!(c.claims.iter().filter(|x| !x.is_proven()).count() >= 4);
    }
}
