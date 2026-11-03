//! Growth-optimal sizing with a drawdown guarantee, and an honest growth calculator.
//!
//! Theory (continuous-time, small-bet approximation; Thorp, MacLean-Ziemba, Breiman; Boyd et al. 2016 for the
//! constrained version): betting a fraction k of the Kelly stake has long-run log growth g(k) = (k - k^2/2) * mu^2 / var
//! per period, i.e. a fraction k(2-k) of full-Kelly growth, and the probability that wealth EVER falls to a fraction x of
//! its starting value is x^(2/k - 1). Half Kelly gives 75% of the growth and a 1-in-8 chance of ever halving; full Kelly
//! gives 50% of the chance of ever halving. Both are approximations: real bets are discrete, edges are estimated, and
//! estimation error makes the true drawdown risk HIGHER, which is why `shrink_for_uncertainty` exists.
//! Growth is only as fast as the edge allows: the calculator shows what a given Sharpe ratio can and cannot do.

/// Probability that wealth ever falls to `x` (0<x<1) of its starting value at Kelly multiple `k` (0<k).
/// For k >= 2 the strategy has no positive growth and ruin is certain in this model: returns 1.
pub fn drawdown_prob(k: f64, x: f64) -> Option<f64> {
    if !(k.is_finite() && k > 0.0 && x.is_finite() && x > 0.0 && x < 1.0) {
        return None;
    }
    if k >= 2.0 {
        return Some(1.0);
    }
    Some(x.powf(2.0 / k - 1.0).min(1.0))
}

/// Largest Kelly multiple (capped at 1, never above full Kelly) whose probability of ever drawing down to `x` of
/// starting wealth is at most `alpha`.
pub fn kelly_multiple_for_drawdown(x: f64, alpha: f64) -> Option<f64> {
    if !(x.is_finite() && x > 0.0 && x < 1.0 && alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
        return None;
    }
    // x^(2/k - 1) = alpha  =>  k = 2 / (1 + ln(alpha)/ln(x))
    let k = 2.0 / (1.0 + alpha.ln() / x.ln());
    Some(k.clamp(0.0, 1.0))
}

/// Share of full-Kelly growth kept at multiple k (0 < k <= 2).
pub fn growth_fraction(k: f64) -> Option<f64> {
    (k.is_finite() && k > 0.0 && k <= 2.0).then_some(k * (2.0 - k))
}

/// Expected log growth per period at Kelly multiple k, for per-period mean excess return `mu` and variance `var`
/// of the unit-leverage strategy.
pub fn log_growth(mu: f64, var: f64, k: f64) -> Option<f64> {
    (mu.is_finite() && var.is_finite() && var > 0.0 && k.is_finite() && k >= 0.0)
        .then(|| (k - k * k / 2.0) * mu * mu / var)
}

/// Periods needed to double wealth at the given log growth rate (None if growth <= 0).
pub fn doubling_periods(g: f64) -> Option<f64> {
    (g.is_finite() && g > 0.0).then(|| std::f64::consts::LN_2 / g)
}

/// Heuristic shrink for an estimated edge: mu^2 / (mu^2 + se^2). Noisy edges are bet much smaller; an edge
/// indistinguishable from zero (se >> mu) is bet almost not at all. (Approximation in the spirit of Baker-McHale's
/// optimal betting under parameter uncertainty; the exact factor depends on the estimator.)
pub fn shrink_for_uncertainty(mu_hat: f64, se: f64) -> Option<f64> {
    (mu_hat.is_finite() && se.is_finite() && se >= 0.0).then(|| {
        let m2 = mu_hat * mu_hat;
        if m2 + se * se == 0.0 {
            0.0
        } else {
            m2 / (m2 + se * se)
        }
    })
}

/// Final Kelly multiple: the drawdown-bounded multiple times the uncertainty shrink. Never exceeds 1.
pub fn plan_multiple(
    mu_hat: f64,
    se: f64,
    max_drawdown_x: f64,
    max_drawdown_prob: f64,
) -> Option<f64> {
    if mu_hat <= 0.0 {
        return Some(0.0); // no demonstrated positive edge: no growth bet
    }
    Some(
        kelly_multiple_for_drawdown(max_drawdown_x, max_drawdown_prob)?
            * shrink_for_uncertainty(mu_hat, se)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn textbook_values() {
        // full Kelly: P(ever halve) = 0.5; half Kelly: 1/8; quarter Kelly: (0.5)^7
        assert!(close(drawdown_prob(1.0, 0.5).unwrap(), 0.5));
        assert!(close(drawdown_prob(0.5, 0.5).unwrap(), 0.125));
        assert!(close(drawdown_prob(0.25, 0.5).unwrap(), 0.5f64.powi(7)));
        assert_eq!(drawdown_prob(2.0, 0.5), Some(1.0));
        assert!(close(growth_fraction(0.5).unwrap(), 0.75));
        assert!(close(growth_fraction(1.0).unwrap(), 1.0));
        assert!(close(growth_fraction(2.0).unwrap(), 0.0));
    }

    #[test]
    fn multiple_for_drawdown_inverts_the_probability() {
        // "at most a 5% chance of ever losing half" -> about a quarter-Kelly-ish multiple
        let k = kelly_multiple_for_drawdown(0.5, 0.05).unwrap();
        assert!(close(drawdown_prob(k, 0.5).unwrap(), 0.05), "{k}");
        assert!(k < 0.4 && k > 0.3, "{k}");
        // a loose tolerance never lets the multiple exceed full Kelly
        assert_eq!(kelly_multiple_for_drawdown(0.5, 0.9), Some(1.0));
        assert!(kelly_multiple_for_drawdown(1.0, 0.5).is_none());
        assert!(kelly_multiple_for_drawdown(0.5, 0.0).is_none());
    }

    #[test]
    fn growth_is_limited_by_edge_not_by_ambition() {
        // annual Sharpe 1 (an excellent strategy): full-Kelly log growth = SR^2/2 = 50%/yr
        let g1 = log_growth(1.0, 1.0, 1.0).unwrap();
        assert!(close(g1, 0.5));
        // a Sharpe-0.2 strategy cannot get close to "exponential" however it is sized
        let g = log_growth(0.2, 1.0, 1.0).unwrap();
        assert!(close(g, 0.02));
        assert!(doubling_periods(g).unwrap() > 34.0); // 34.7 years
                                                      // over-betting destroys growth: k=2 gives zero, k=3 is negative
        assert!(close(log_growth(1.0, 1.0, 2.0).unwrap(), 0.0));
        assert!(log_growth(1.0, 1.0, 3.0).unwrap() < 0.0);
        assert!(doubling_periods(-0.1).is_none());
    }

    #[test]
    fn noisy_edges_are_bet_small_and_missing_edges_not_at_all() {
        assert!(close(shrink_for_uncertainty(0.1, 0.0).unwrap(), 1.0));
        assert!(close(shrink_for_uncertainty(0.1, 0.1).unwrap(), 0.5));
        assert!(shrink_for_uncertainty(0.01, 0.1).unwrap() < 0.01);
        assert_eq!(plan_multiple(-0.1, 0.01, 0.5, 0.05), Some(0.0));
        let sure = plan_multiple(0.1, 0.0, 0.5, 0.05).unwrap();
        let noisy = plan_multiple(0.1, 0.1, 0.5, 0.05).unwrap();
        assert!(noisy < sure && close(noisy, sure * 0.5));
        assert!(plan_multiple(f64::NAN, 0.0, 0.5, 0.05).is_none());
    }

    #[test]
    fn simulation_respects_the_drawdown_bound() {
        // Monte Carlo of a repeated favourable binary bet at multiple k: the bound must hold (it is an approximation
        // for small bets, so use small bets: p = 0.52 even-odds, Kelly f* = 0.04; k = 0.5)
        let (p, k, x) = (0.52f64, 0.5f64, 0.5f64);
        let f = k * (2.0 * p - 1.0);
        let mut s: u64 = 99;
        let mut rnd = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64
        };
        let (paths, steps) = (2000, 4000);
        let mut hit = 0;
        for _ in 0..paths {
            let mut w = 1.0f64;
            for _ in 0..steps {
                w *= if rnd() < p { 1.0 + f } else { 1.0 - f };
                if w <= x {
                    hit += 1;
                    break;
                }
            }
        }
        let freq = f64::from(hit) / f64::from(paths);
        let bound = drawdown_prob(k, x).unwrap(); // 0.125
        assert!(freq <= bound + 0.03, "observed {freq} vs bound {bound}");
    }
}
