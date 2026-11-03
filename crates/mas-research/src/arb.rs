//! Rebalancing arbitrage inside one mutually exclusive, exhaustive outcome set (a "NegRisk" group): exactly one
//! outcome pays 1. Buying every outcome for less than 1 (after fees and a leg-risk haircut), or selling every outcome
//! for more than 1, locks in profit IF all legs fill. Legs are not atomic on-chain, so a haircut per leg prices the
//! chance that only some fill (the AFT 2025 study measured about US$40M extracted on Polymarket in a year,
//! mostly by faster operators). The caller must guarantee the set is exclusive and exhaustive (never infer it).
use crate::rewards::fee_equivalent;

#[derive(Clone, Copy, Debug)]
pub struct Leg {
    pub bid: f64,
    pub ask: f64,
    /// Shares available at that price.
    pub depth: f64,
}

#[derive(Debug, PartialEq)]
pub enum Side {
    BuyAll,
    SellAll,
}

#[derive(Debug)]
pub struct Arb {
    pub side: Side,
    pub shares: f64,
    /// Expected profit per share set after fees and haircut.
    pub edge: f64,
}

fn valid(l: &Leg) -> bool {
    l.bid.is_finite()
        && l.ask.is_finite()
        && l.depth.is_finite()
        && l.depth > 0.0
        && l.bid >= 0.0
        && l.ask <= 1.0
        && l.bid <= l.ask
}

/// Best rebalancing opportunity with edge >= `min_edge` per share set, or None.
/// `fee_rate`: taker fee rate for the category; `leg_haircut`: per-leg cost for partial-fill risk.
pub fn rebalance(legs: &[Leg], fee_rate: f64, leg_haircut: f64, min_edge: f64) -> Option<Arb> {
    if legs.len() < 2
        || !legs.iter().all(valid)
        || !(fee_rate >= 0.0 && leg_haircut >= 0.0 && min_edge > 0.0)
    {
        return None;
    }
    let shares = legs.iter().map(|l| l.depth).fold(f64::INFINITY, f64::min);
    let hair = leg_haircut * legs.len() as f64;
    let buy_cost: f64 = legs
        .iter()
        .map(|l| l.ask + fee_equivalent(1.0, fee_rate, l.ask))
        .sum();
    let sell_in: f64 = legs
        .iter()
        .map(|l| l.bid - fee_equivalent(1.0, fee_rate, l.bid))
        .sum();
    let buy_edge = 1.0 - buy_cost - hair;
    let sell_edge = sell_in - 1.0 - hair;
    let best = if buy_edge >= sell_edge {
        (Side::BuyAll, buy_edge)
    } else {
        (Side::SellAll, sell_edge)
    };
    (best.1 >= min_edge).then_some(Arb {
        side: best.0,
        shares,
        edge: best.1,
    })
}

/// Coherent quoting: given desired bids for an exclusive, exhaustive set, scale them down so they sum to at most
/// 1 - margin. Otherwise our own resting bids can be hit on every leg for a guaranteed loss (we would be the arb).
pub fn coherent_bids(bids: &[f64], margin: f64) -> Vec<f64> {
    let s: f64 = bids.iter().sum();
    if !(s.is_finite() && (0.0..1.0).contains(&margin))
        || bids.iter().any(|b| !b.is_finite() || *b < 0.0)
    {
        return vec![0.0; bids.len()];
    }
    if s <= 1.0 - margin {
        return bids.to_vec();
    }
    let f = (1.0 - margin) / s;
    bids.iter().map(|b| b * f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(bid: f64, ask: f64, depth: f64) -> Leg {
        Leg { bid, ask, depth }
    }

    #[test]
    fn buy_all_when_asks_sum_well_below_one() {
        let legs = [
            l(0.28, 0.30, 100.0),
            l(0.30, 0.32, 50.0),
            l(0.28, 0.30, 80.0),
        ];
        let a = rebalance(&legs, 0.0, 0.0, 0.02).unwrap();
        assert_eq!(a.side, Side::BuyAll);
        assert!((a.edge - 0.08).abs() < 1e-9);
        assert_eq!(a.shares, 50.0);
    }

    #[test]
    fn sell_all_when_bids_sum_above_one() {
        let legs = [
            l(0.40, 0.42, 10.0),
            l(0.35, 0.37, 10.0),
            l(0.30, 0.32, 10.0),
        ];
        assert_eq!(
            rebalance(&legs, 0.0, 0.0, 0.02).unwrap().side,
            Side::SellAll
        );
    }

    #[test]
    fn fees_and_leg_risk_can_erase_the_edge() {
        let legs = [
            l(0.28, 0.30, 100.0),
            l(0.30, 0.32, 100.0),
            l(0.28, 0.30, 100.0),
        ];
        assert!(rebalance(&legs, 0.05, 0.0, 0.05).is_none());
        assert!(rebalance(&legs, 0.0, 0.03, 0.02).is_none());
    }

    #[test]
    fn bad_inputs_and_efficient_books_give_nothing() {
        assert!(rebalance(&[l(0.5, 0.5, 1.0)], 0.0, 0.0, 0.01).is_none());
        assert!(rebalance(&[l(0.6, 0.5, 1.0), l(0.4, 0.5, 1.0)], 0.0, 0.0, 0.01).is_none());
        assert!(rebalance(&[l(0.49, 0.51, 1.0), l(0.49, 0.51, 1.0)], 0.0, 0.0, 0.01).is_none());
        assert!(rebalance(&[l(0.4, 0.5, 0.0), l(0.4, 0.5, 1.0)], 0.0, 0.0, 0.01).is_none());
    }

    #[test]
    fn coherent_bids_never_sum_above_one_minus_margin() {
        let b = coherent_bids(&[0.5, 0.4, 0.3], 0.02);
        assert!((b.iter().sum::<f64>() - 0.98).abs() < 1e-9);
        assert_eq!(coherent_bids(&[0.3, 0.3], 0.02), vec![0.3, 0.3]);
        assert_eq!(coherent_bids(&[f64::NAN], 0.0), vec![0.0]);
    }
}
