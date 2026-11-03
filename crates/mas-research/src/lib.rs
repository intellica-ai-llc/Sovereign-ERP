//! Statistical honesty tools for strategy evaluation (MAS stage gates S0-S2).
//!
//! Formulas follow Bailey and Lopez de Prado, "The Deflated Sharpe Ratio" (2014). Sharpe ratios here are PER
//! OBSERVATION (not annualised): `sr` is mean/stdev of the per-period returns, `n` is the number of periods.
//! Every function is total: invalid input returns `None`, never a number that could pass a gate by accident.

pub mod arb;
pub mod card;
pub mod claims;
pub mod eprocess;
pub mod gate;
pub mod growth;
pub mod implication;
pub mod paper;
pub mod quoting;
pub mod recalibrate;
pub mod registry;
pub mod replay;
pub mod rewards;
pub mod scoring;
pub mod shield;
pub mod sim;
pub mod sizing;
pub mod toxicity;

use std::f64::consts::{PI, SQRT_2};

const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

/// Standard normal CDF, accurate to about 1e-12 (Taylor series for erf near zero, continued fraction for the tail).
pub fn norm_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    0.5 * erfc(-x / SQRT_2)
}

fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let r = if z < 2.5 {
        // erf(z) = 2/sqrt(pi) * sum (-1)^n z^(2n+1) / (n! (2n+1))
        let mut term = z;
        let mut sum = z;
        for n in 1..200 {
            term *= -z * z / n as f64;
            let add = term / (2 * n + 1) as f64;
            sum += add;
            if add.abs() < 1e-17 * sum.abs() {
                break;
            }
        }
        1.0 - 2.0 / PI.sqrt() * sum
    } else {
        let mut t = z;
        for k in (1..=80).rev() {
            t = z + (k as f64 / 2.0) / t;
        }
        (-z * z).exp() / (PI.sqrt() * t)
    };
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

/// Inverse standard normal CDF (Acklam's algorithm, relative error < 1.2e-9, then one Halley refinement).
pub fn norm_inv_cdf(p: f64) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let plow = 0.02425;
    let x = if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - plow {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    let e = norm_cdf(x) - p;
    let u = e * (2.0 * PI).sqrt() * (x * x / 2.0).exp();
    Some(x - u / (1.0 + x * u / 2.0))
}

/// Sample moments of a return series: (mean, stdev, skewness, kurtosis). Kurtosis is NON-excess (normal = 3).
/// `None` for fewer than 4 observations, non-finite values or zero variance.
pub fn moments(r: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let n = r.len();
    if n < 4 || r.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let nf = n as f64;
    let mean = r.iter().sum::<f64>() / nf;
    let m2 = r.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nf;
    if m2 <= 0.0 {
        return None;
    }
    let m3 = r.iter().map(|x| (x - mean).powi(3)).sum::<f64>() / nf;
    let m4 = r.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / nf;
    let sd = (m2 * nf / (nf - 1.0)).sqrt();
    Some((mean, sd, m3 / m2.powf(1.5), m4 / (m2 * m2)))
}

/// Per-observation Sharpe ratio (mean / sample stdev).
pub fn sharpe(r: &[f64]) -> Option<f64> {
    let (m, sd, _, _) = moments(r)?;
    Some(m / sd)
}

/// Standard error denominator term: 1 - skew*SR + (kurt-1)/4 * SR^2. Must be positive.
fn variance_term(sr: f64, skew: f64, kurt: f64) -> Option<f64> {
    let v = 1.0 - skew * sr + (kurt - 1.0) / 4.0 * sr * sr;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Probabilistic Sharpe Ratio: probability that the true Sharpe exceeds `benchmark`, given `n` observations.
pub fn psr(sr: f64, benchmark: f64, n: usize, skew: f64, kurt: f64) -> Option<f64> {
    if n < 2 || !sr.is_finite() || !benchmark.is_finite() {
        return None;
    }
    let v = variance_term(sr, skew, kurt)?;
    Some(norm_cdf(
        (sr - benchmark) * ((n - 1) as f64).sqrt() / v.sqrt(),
    ))
}

/// Expected maximum Sharpe among `trials` independent trials whose Sharpe estimates have variance `var_sr`.
/// This is the deflation benchmark: the best of many random strategies looks good by luck alone.
pub fn expected_max_sharpe(var_sr: f64, trials: u32) -> Option<f64> {
    if trials == 0 || var_sr < 0.0 || !var_sr.is_finite() {
        return None;
    }
    if trials == 1 {
        return Some(0.0);
    }
    let n = trials as f64;
    let a = norm_inv_cdf(1.0 - 1.0 / n)?;
    let b = norm_inv_cdf(1.0 - 1.0 / (n * std::f64::consts::E))?;
    Some(var_sr.sqrt() * ((1.0 - EULER_GAMMA) * a + EULER_GAMMA * b))
}

/// Deflated Sharpe Ratio: PSR against the expected maximum of `trials` trials. The gate S0 needs this >= 0.95
/// at the TRUE trial count recorded in the experiment registry.
pub fn dsr(
    sr: f64,
    n: usize,
    skew: f64,
    kurt: f64,
    trials: u32,
    var_sr_across_trials: f64,
) -> Option<f64> {
    let bench = expected_max_sharpe(var_sr_across_trials, trials)?;
    psr(sr, bench, n, skew, kurt)
}

/// Minimum number of observations for the Sharpe to exceed `benchmark` at confidence `conf` (e.g. 0.95).
pub fn min_track_record(sr: f64, benchmark: f64, skew: f64, kurt: f64, conf: f64) -> Option<f64> {
    if !sr.is_finite() || !benchmark.is_finite() || sr <= benchmark {
        return None;
    }
    let z = norm_inv_cdf(conf)?;
    let v = variance_term(sr, skew, kurt)?;
    Some(1.0 + v * (z / (sr - benchmark)).powi(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} vs {b}");
    }

    #[test]
    fn normal_cdf_and_inverse_match_tables() {
        close(norm_cdf(0.0), 0.5, 1e-12);
        close(norm_cdf(1.96), 0.975_002, 2e-6);
        close(norm_cdf(-1.645), 0.049_985, 2e-6);
        close(norm_inv_cdf(0.975).unwrap(), 1.959_964, 1e-5);
        close(norm_inv_cdf(0.95).unwrap(), 1.644_854, 1e-5);
        close(norm_inv_cdf(0.001).unwrap(), -3.090_232, 1e-4);
        for p in [1e-6, 0.01, 0.2, 0.5, 0.8, 0.99, 1.0 - 1e-6] {
            close(norm_cdf(norm_inv_cdf(p).unwrap()), p, 1e-6);
        }
        assert!(
            norm_inv_cdf(0.0).is_none()
                && norm_inv_cdf(1.0).is_none()
                && norm_inv_cdf(f64::NAN).is_none()
        );
    }

    #[test]
    fn psr_is_half_at_benchmark_and_monotone() {
        close(psr(0.1, 0.1, 100, 0.0, 3.0).unwrap(), 0.5, 1e-9);
        let lo = psr(0.1, 0.0, 50, 0.0, 3.0).unwrap();
        let hi = psr(0.1, 0.0, 500, 0.0, 3.0).unwrap();
        assert!(hi > lo && hi > 0.95 && lo < hi);
    }

    #[test]
    fn deflation_punishes_many_trials() {
        // identical evidence, more trials tried -> lower confidence (the whole point of the registry)
        let a = dsr(0.15, 400, 0.0, 3.0, 1, 0.01).unwrap();
        let b = dsr(0.15, 400, 0.0, 3.0, 100, 0.01).unwrap();
        let c = dsr(0.15, 400, 0.0, 3.0, 10_000, 0.01).unwrap();
        assert!(a > b && b > c, "{a} {b} {c}");
        assert!(expected_max_sharpe(0.01, 0).is_none());
        close(expected_max_sharpe(0.01, 1).unwrap(), 0.0, 0.0);
    }

    #[test]
    fn min_track_record_matches_closed_form() {
        // Normal returns, benchmark 0: n = 1 + (1 + SR^2/2) (z/SR)^2
        let sr = 0.1;
        let want = 1.0 + (1.0 + sr * sr / 2.0) * (1.644_854_f64 / sr).powi(2);
        close(
            min_track_record(sr, 0.0, 0.0, 3.0, 0.95).unwrap(),
            want,
            0.05,
        );
        assert!(
            min_track_record(0.0, 0.0, 0.0, 3.0, 0.95).is_none(),
            "no edge needs infinite record"
        );
        assert!(min_track_record(-0.1, 0.0, 0.0, 3.0, 0.95).is_none());
    }

    #[test]
    fn invalid_inputs_never_produce_a_number() {
        assert!(moments(&[1.0, 2.0, 3.0]).is_none());
        assert!(moments(&[1.0, 1.0, 1.0, 1.0]).is_none());
        assert!(moments(&[1.0, f64::NAN, 2.0, 3.0]).is_none());
        assert!(sharpe(&[f64::INFINITY, 1.0, 2.0, 3.0]).is_none());
        assert!(psr(f64::NAN, 0.0, 100, 0.0, 3.0).is_none());
        assert!(psr(0.1, 0.0, 1, 0.0, 3.0).is_none());
        // fat-tail / skew combination that makes the variance term non-positive
        assert!(psr(5.0, 0.0, 100, 5.0, 3.0).is_none());
        assert!(dsr(0.1, 100, 0.0, 3.0, 10, f64::NAN).is_none());
    }

    #[test]
    fn sharpe_of_known_series() {
        let r = [0.01, -0.01, 0.02, 0.0, 0.01, 0.03];
        let (m, sd, _, _) = moments(&r).unwrap();
        close(m, 0.01, 1e-12);
        close(sharpe(&r).unwrap(), m / sd, 1e-12);
    }
}
