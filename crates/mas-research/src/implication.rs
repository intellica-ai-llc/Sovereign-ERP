//! Logical-implication arbitrage between two markets (and chains of them).
//! If A implies B ("BTC above 110k on Friday" implies "BTC above 100k on Friday"), then price(A) <= price(B) must
//! hold. When bid(A) exceeds ask(B) after fees and a leg-risk haircut, selling A and buying B can never lose at
//! settlement: the outcomes are (A,B) in {(1,1),(0,1),(0,0)} and the payoff is bid(A) - ask(B) + [B and not A].
//! The CALLER must supply a true implication (never infer it from text: the AFT 2025 study found most LLM-flagged
//! dependencies were false positives). Both markets must settle on the same source and time.
use crate::rewards::fee_equivalent;

#[derive(Clone, Copy, Debug)]
pub struct Quote {
    pub bid: f64,
    pub ask: f64,
    pub depth: f64,
}

#[derive(Debug, PartialEq)]
pub struct Violation {
    /// Index of the stronger claim A (sell YES) and the weaker claim B (buy YES) in the input chain.
    pub sell: usize,
    pub buy: usize,
    pub shares: f64,
    /// Worst-case profit per share across all legal outcomes, after fees and haircut.
    pub edge: f64,
}

fn ok(q: &Quote) -> bool {
    q.bid.is_finite()
        && q.ask.is_finite()
        && q.depth.is_finite()
        && q.depth > 0.0
        && q.bid >= 0.0
        && q.ask <= 1.0
        && q.bid <= q.ask
}

/// Worst-case edge of selling `a` and buying `b` when a implies b.
pub fn edge(a: &Quote, b: &Quote, fee_rate: f64, leg_haircut: f64) -> Option<f64> {
    if !(ok(a) && ok(b) && fee_rate >= 0.0 && leg_haircut >= 0.0) {
        return None;
    }
    let got = a.bid - fee_equivalent(1.0, fee_rate, a.bid);
    let paid = b.ask + fee_equivalent(1.0, fee_rate, b.ask);
    Some(got - paid - 2.0 * leg_haircut)
}

/// `chain[i]` implies `chain[j]` for every i > j (strictest claim last, e.g. rising thresholds). Returns the
/// violating pair with the best worst-case edge that is at least `min_edge`, or None.
pub fn best_violation(
    chain: &[Quote],
    fee_rate: f64,
    leg_haircut: f64,
    min_edge: f64,
) -> Option<Violation> {
    if min_edge.is_nan() || min_edge <= 0.0 {
        return None;
    }
    let mut best: Option<Violation> = None;
    for i in 1..chain.len() {
        for j in 0..i {
            if let Some(e) = edge(&chain[i], &chain[j], fee_rate, leg_haircut) {
                if e >= min_edge && best.as_ref().is_none_or(|b| e > b.edge) {
                    best = Some(Violation {
                        sell: i,
                        buy: j,
                        shares: chain[i].depth.min(chain[j].depth),
                        edge: e,
                    });
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    fn q(bid: f64, ask: f64) -> Quote {
        Quote {
            bid,
            ask,
            depth: 100.0,
        }
    }

    #[test]
    fn finds_the_inversion_and_its_worst_case_edge() {
        // 120k above 0.40 bid, 100k above only 0.30 ask: impossible ordering
        let chain = [q(0.28, 0.30), q(0.38, 0.40), q(0.40, 0.42)];
        let v = best_violation(&chain, 0.0, 0.0, 0.02).unwrap();
        assert_eq!((v.sell, v.buy), (2, 0));
        assert!((v.edge - 0.10).abs() < 1e-9);
        assert_eq!(v.shares, 100.0);
    }

    #[test]
    fn fees_and_leg_risk_can_remove_the_edge() {
        let chain = [q(0.28, 0.30), q(0.33, 0.34)];
        assert!(best_violation(&chain, 0.0, 0.0, 0.01).is_some());
        assert!(best_violation(&chain, 0.05, 0.0, 0.01).is_none());
        assert!(best_violation(&chain, 0.0, 0.02, 0.01).is_none());
    }

    #[test]
    fn coherent_chain_and_bad_inputs_give_nothing() {
        assert!(best_violation(&[q(0.40, 0.42), q(0.30, 0.32)], 0.0, 0.0, 0.01).is_none());
        assert!(best_violation(&[q(0.5, 0.4), q(0.2, 0.3)], 0.0, 0.0, 0.01).is_none());
        assert!(best_violation(&[q(f64::NAN, 0.4), q(0.2, 0.3)], 0.0, 0.0, 0.01).is_none());
        assert!(best_violation(&[q(0.3, 0.4), q(0.5, 0.6)], 0.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn worst_case_payoff_is_nonnegative_in_every_legal_outcome() {
        // brute-force the three legal settlements for a profitable pair
        let (a, b) = (q(0.50, 0.52), q(0.30, 0.32));
        let e = edge(&a, &b, 0.0, 0.0).unwrap();
        assert!(e > 0.0);
        for (ya, yb) in [(1.0, 1.0), (0.0, 1.0), (0.0, 0.0)] {
            let pnl = a.bid - ya - b.ask + yb;
            assert!(pnl >= e - 1e-12, "{ya} {yb} {pnl}");
        }
    }
}
