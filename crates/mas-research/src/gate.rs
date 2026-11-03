//! Selective-prediction gate: trade only when the edge survives the forecaster's own measured unreliability.
//! The forecaster's probabilities are bucketed (equal-width bins). For each bin we keep how often the event really
//! happened. A trade on forecast f against market price m is allowed only if the CONFIDENCE BOUND of the bin's true
//! frequency still beats m by more than costs: for a buy, lower bound > m + costs; for a sell, upper bound < m - costs.
//! Bound: Hoeffding, eps = sqrt(ln(2/delta) / (2n)); valid when outcomes within a bin are exchangeable (the same
//! assumption conformal methods make). Thin bins are refused outright. This is what stops a confident-sounding
//! model (KalshiBench: reasoning raises confidence, not accuracy) from trading on illusory edge.

pub const BINS: usize = 10;

#[derive(Clone, Debug)]
pub struct Gate {
    n: [u32; BINS],
    wins: [u32; BINS],
    delta: f64,
    min_n: u32,
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Buy {
        lcb_edge: f64,
    },
    Sell {
        lcb_edge: f64,
    },
    /// No trade, with the reason.
    Skip(&'static str),
}

fn bin(p: f64) -> usize {
    ((p * BINS as f64) as usize).min(BINS - 1)
}

impl Gate {
    /// `history`: (forecast probability, outcome). `delta` in (0,1): per-decision error rate of the bound.
    pub fn fit(history: &[(f64, bool)], delta: f64, min_n: u32) -> Option<Gate> {
        if !(delta > 0.0 && delta < 1.0) || min_n == 0 {
            return None;
        }
        let mut g = Gate {
            n: [0; BINS],
            wins: [0; BINS],
            delta,
            min_n,
        };
        for &(p, y) in history {
            if !(0.0..=1.0).contains(&p) || !p.is_finite() {
                return None;
            }
            let b = bin(p);
            g.n[b] += 1;
            g.wins[b] += u32::from(y);
        }
        Some(g)
    }

    /// Decide on forecast `f` versus market price `m` with all-in cost `cost` per share (fees, spread crossed, slippage).
    pub fn decide(&self, f: f64, m: f64, cost: f64) -> Verdict {
        if !(f.is_finite() && m.is_finite() && cost.is_finite())
            || !(0.0..=1.0).contains(&f)
            || !(0.0..=1.0).contains(&m)
            || cost < 0.0
        {
            return Verdict::Skip("invalid input");
        }
        let b = bin(f);
        let n = self.n[b];
        if n < self.min_n {
            return Verdict::Skip("too little history in this probability bin");
        }
        let freq = f64::from(self.wins[b]) / f64::from(n);
        let eps = ((2.0 / self.delta).ln() / (2.0 * f64::from(n))).sqrt();
        let (lo, hi) = ((freq - eps).max(0.0), (freq + eps).min(1.0));
        if f > m {
            let e = lo - m - cost;
            if e > 0.0 {
                return Verdict::Buy { lcb_edge: e };
            }
        } else if f < m {
            let e = m - hi - cost;
            if e > 0.0 {
                return Verdict::Sell { lcb_edge: e };
            }
        }
        Verdict::Skip("edge does not survive the forecaster's measured unreliability")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 400 forecasts in the 0.7 bin of which 280 came true (a calibrated bin), 400 in the 0.3 bin of which 120.
    fn hist() -> Vec<(f64, bool)> {
        let mut h = vec![];
        for i in 0..400 {
            h.push((0.75, i < 280));
            h.push((0.35, i < 140));
        }
        h
    }

    #[test]
    fn trades_only_when_the_lower_bound_clears_price_and_cost() {
        let g = Gate::fit(&hist(), 0.05, 100).unwrap();
        // freq 0.70, eps = sqrt(ln(40)/800) = 0.0679 -> lcb 0.632
        assert!(matches!(g.decide(0.75, 0.55, 0.01), Verdict::Buy { lcb_edge } if lcb_edge > 0.05));
        assert!(matches!(g.decide(0.75, 0.63, 0.01), Verdict::Skip(_)));
        // sell side: bin 0.35 has freq 0.35, ucb 0.418
        assert!(
            matches!(g.decide(0.35, 0.50, 0.01), Verdict::Sell { lcb_edge } if lcb_edge > 0.05)
        );
        assert!(matches!(g.decide(0.35, 0.42, 0.01), Verdict::Skip(_)));
    }

    #[test]
    fn overconfident_forecaster_is_not_trusted() {
        // says 0.9 but is right only 60% of the time
        let h: Vec<(f64, bool)> = (0..500).map(|i| (0.9, i < 300)).collect();
        let g = Gate::fit(&h, 0.05, 100).unwrap();
        // a naive trader sees +0.3 edge against 0.60; the gate sees the real 0.60 and refuses
        assert!(matches!(g.decide(0.9, 0.60, 0.0), Verdict::Skip(_)));
        assert!(matches!(g.decide(0.9, 0.45, 0.0), Verdict::Buy { .. }));
    }

    #[test]
    fn thin_bins_bad_inputs_and_bad_fits_are_refused() {
        let g = Gate::fit(&hist(), 0.05, 100).unwrap();
        assert!(
            matches!(g.decide(0.95, 0.5, 0.0), Verdict::Skip(r) if r.contains("little history"))
        );
        assert!(matches!(g.decide(f64::NAN, 0.5, 0.0), Verdict::Skip(_)));
        assert!(matches!(g.decide(0.75, 1.5, 0.0), Verdict::Skip(_)));
        assert!(matches!(g.decide(0.75, 0.5, -0.1), Verdict::Skip(_)));
        assert!(Gate::fit(&[(1.5, true)], 0.05, 10).is_none());
        assert!(Gate::fit(&[], 0.0, 10).is_none());
        assert!(Gate::fit(&[], 0.05, 0).is_none());
    }

    #[test]
    fn simulated_false_trade_rate_stays_below_delta() {
        // forecaster calibrated at 0.5; "market" at 0.5-: how often does the gate buy when there is NO edge?
        // With exchangeable outcomes the bound should wrongly clear a price at the true frequency <= delta of the time.
        let mut x: u64 = 12345;
        let mut next = || {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (x >> 33) as f64 / (1u64 << 31) as f64
        };
        let (mut wrong, trials) = (0, 400);
        for _ in 0..trials {
            let h: Vec<(f64, bool)> = (0..200).map(|_| (0.55, next() < 0.55)).collect();
            let g = Gate::fit(&h, 0.05, 50).unwrap();
            // true frequency is 0.55; a market priced exactly at 0.55 offers no edge
            if matches!(g.decide(0.56, 0.55, 0.0), Verdict::Buy { .. }) {
                wrong += 1;
            }
        }
        assert!(f64::from(wrong) / f64::from(trials) <= 0.05, "{wrong}");
    }
}
