//! Runtime shield: the deterministic gate every proposal from any AI component must pass before it can become an
//! order. No model, prompt, news item or tool output can talk it out of a limit: it is plain arithmetic over state that
//! only the execution environment writes (survey recommendation: "read-only ground-truth state" and "hard constraints
//! on the path from inference to action"; documented attacks on LLM trading agents work by corrupting inputs, never by
//! changing this code). Anything non-finite, stale or unknown is denied. A kill is sticky until a human resets it.

#[derive(Clone, Debug)]
pub struct Limits {
    pub min_order: f64,
    pub max_order: f64,
    pub max_market_exposure: f64,
    pub max_event_exposure: f64,
    pub max_total_exposure: f64,
    /// Loss in the current day (realized plus unrealized) that stops new risk, in dollars.
    pub daily_loss_limit: f64,
    /// Fall from peak equity (fraction) that kills the strategy.
    pub max_drawdown: f64,
    pub max_orders_per_min: u32,
    pub max_data_age_secs: f64,
    /// No NEW exposure inside this many seconds of resolution.
    pub min_secs_to_resolution: f64,
}

#[derive(Clone, Debug)]
pub struct State {
    pub total_exposure: f64,
    pub event_exposure: f64,
    pub market_exposure: f64,
    pub day_pnl: f64,
    pub equity: f64,
    pub peak_equity: f64,
    pub orders_last_min: u32,
    pub data_age_secs: f64,
    pub killed: bool,
}

#[derive(Clone, Debug)]
pub struct Proposal {
    /// Dollars of additional exposure the order would add (negative = it reduces exposure).
    pub notional: f64,
    pub secs_to_resolution: f64,
    pub market_disputed: bool,
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    /// Allowed in full.
    Allow,
    /// Allowed only up to this many dollars of added exposure.
    Reduce(f64),
    Deny(&'static str),
    /// Deny and latch the kill switch (the caller must persist `killed = true`).
    Kill(&'static str),
}

fn finite(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite())
}

pub fn check(l: &Limits, s: &State, p: &Proposal) -> Verdict {
    if !finite(&[
        l.min_order,
        l.max_order,
        l.max_market_exposure,
        l.max_event_exposure,
        l.max_total_exposure,
        l.daily_loss_limit,
        l.max_drawdown,
        l.max_data_age_secs,
        l.min_secs_to_resolution,
        s.total_exposure,
        s.event_exposure,
        s.market_exposure,
        s.day_pnl,
        s.equity,
        s.peak_equity,
        s.data_age_secs,
        p.notional,
        p.secs_to_resolution,
    ]) || s.equity <= 0.0
        || s.peak_equity < s.equity
        || l.min_order < 0.0
        || l.max_order < l.min_order
    {
        return Verdict::Deny("invalid or non-finite input");
    }
    // Loss limits first: they latch.
    if s.equity <= s.peak_equity * (1.0 - l.max_drawdown) {
        return if p.notional < 0.0 {
            Verdict::Allow // always allow risk reduction
        } else {
            Verdict::Kill("maximum drawdown reached")
        };
    }
    if p.notional <= 0.0 {
        return Verdict::Allow; // reduces or does not add risk
    }
    if s.killed {
        return Verdict::Deny("strategy is killed; human reset required");
    }
    if -s.day_pnl >= l.daily_loss_limit {
        return Verdict::Deny("daily loss limit reached");
    }
    if s.data_age_secs > l.max_data_age_secs {
        return Verdict::Deny("market data is stale");
    }
    if p.market_disputed {
        return Verdict::Deny("market is disputed or escalated");
    }
    if p.secs_to_resolution < l.min_secs_to_resolution {
        return Verdict::Deny("too close to resolution for new exposure");
    }
    if s.orders_last_min >= l.max_orders_per_min {
        return Verdict::Deny("order rate limit");
    }
    let room = (l.max_total_exposure - s.total_exposure)
        .min(l.max_event_exposure - s.event_exposure)
        .min(l.max_market_exposure - s.market_exposure)
        .min(l.max_order);
    if room < l.min_order {
        return Verdict::Deny("no exposure headroom");
    }
    if p.notional <= room {
        Verdict::Allow
    } else {
        Verdict::Reduce(room)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lim() -> Limits {
        Limits {
            min_order: 5.0,
            max_order: 200.0,
            max_market_exposure: 500.0,
            max_event_exposure: 1000.0,
            max_total_exposure: 2500.0,
            daily_loss_limit: 150.0,
            max_drawdown: 0.20,
            max_orders_per_min: 30,
            max_data_age_secs: 5.0,
            min_secs_to_resolution: 3600.0,
        }
    }
    fn st() -> State {
        State {
            total_exposure: 0.0,
            event_exposure: 0.0,
            market_exposure: 0.0,
            day_pnl: 0.0,
            equity: 5000.0,
            peak_equity: 5000.0,
            orders_last_min: 0,
            data_age_secs: 1.0,
            killed: false,
        }
    }
    fn pr(n: f64) -> Proposal {
        Proposal {
            notional: n,
            secs_to_resolution: 86_400.0,
            market_disputed: false,
        }
    }

    #[test]
    fn allows_reduces_and_denies() {
        assert_eq!(check(&lim(), &st(), &pr(100.0)), Verdict::Allow);
        assert_eq!(check(&lim(), &st(), &pr(900.0)), Verdict::Reduce(200.0));
        let mut s = st();
        s.market_exposure = 480.0;
        assert_eq!(check(&lim(), &s, &pr(100.0)), Verdict::Reduce(20.0));
        s.market_exposure = 498.0;
        assert_eq!(
            check(&lim(), &s, &pr(100.0)),
            Verdict::Deny("no exposure headroom")
        );
    }

    #[test]
    fn each_hard_rule_denies() {
        let mut s = st();
        s.data_age_secs = 9.0;
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(m) if m.contains("stale")));
        let mut p = pr(10.0);
        p.market_disputed = true;
        assert!(matches!(check(&lim(), &st(), &p), Verdict::Deny(m) if m.contains("disputed")));
        let mut p = pr(10.0);
        p.secs_to_resolution = 60.0;
        assert!(matches!(check(&lim(), &st(), &p), Verdict::Deny(m) if m.contains("resolution")));
        let mut s = st();
        s.orders_last_min = 30;
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(m) if m.contains("rate")));
        let mut s = st();
        s.day_pnl = -150.0;
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(m) if m.contains("daily")));
    }

    #[test]
    fn drawdown_kills_but_risk_reduction_is_always_allowed() {
        let mut s = st();
        s.equity = 3900.0; // 22% below peak
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Kill(_)));
        assert_eq!(check(&lim(), &s, &pr(-50.0)), Verdict::Allow);
        let mut s = st();
        s.killed = true;
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(m) if m.contains("killed")));
        assert_eq!(check(&lim(), &s, &pr(-10.0)), Verdict::Allow);
    }

    #[test]
    fn garbage_in_means_deny() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(check(&lim(), &st(), &pr(bad)), Verdict::Deny(_)));
            let mut s = st();
            s.equity = bad;
            assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(_)));
            let mut l = lim();
            l.max_total_exposure = bad;
            assert!(matches!(check(&l, &st(), &pr(10.0)), Verdict::Deny(_)));
        }
        let mut s = st();
        s.equity = -1.0;
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(_)));
        let mut s = st();
        s.peak_equity = 100.0; // peak below equity is impossible state
        assert!(matches!(check(&lim(), &s, &pr(10.0)), Verdict::Deny(_)));
    }

    #[test]
    fn invariant_no_allowed_order_ever_breaches_a_cap() {
        let l = lim();
        let mut x: u64 = 7;
        let mut r = |m: f64| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 33) as f64 / (1u64 << 31) as f64 * m
        };
        for _ in 0..20_000 {
            let s = State {
                total_exposure: r(3000.0),
                event_exposure: r(1200.0),
                market_exposure: r(600.0),
                day_pnl: r(300.0) - 200.0,
                equity: 3000.0 + r(3000.0),
                peak_equity: 6000.0,
                orders_last_min: r(40.0) as u32,
                data_age_secs: r(8.0),
                killed: r(1.0) > 0.9,
            };
            let p = Proposal {
                notional: r(500.0),
                secs_to_resolution: r(90_000.0),
                market_disputed: r(1.0) > 0.9,
            };
            let add = match check(&l, &s, &p) {
                Verdict::Allow => p.notional,
                Verdict::Reduce(a) => a,
                _ => continue,
            };
            assert!(add <= l.max_order + 1e-9);
            assert!(
                s.total_exposure + add <= l.max_total_exposure + 1e-9
                    || s.total_exposure > l.max_total_exposure
            );
            assert!(
                s.event_exposure + add <= l.max_event_exposure + 1e-9
                    || s.event_exposure > l.max_event_exposure
            );
            assert!(
                s.market_exposure + add <= l.max_market_exposure + 1e-9
                    || s.market_exposure > l.max_market_exposure
            );
            assert!(!s.killed && s.data_age_secs <= l.max_data_age_secs && !p.market_disputed);
        }
    }
}
