//! Anytime-valid evidence for trading claims (betting e-processes; Shafer, Vovk, Ramdas, Grünwald).
//!
//! Why: a fixed "60 days then test" gate is invalid if anyone peeks, and wasteful if the answer is clear early.
//! A betting e-process is a nonnegative wealth process that cannot grow in expectation under the null; by Ville's
//! inequality P(wealth ever reaches 1/alpha) <= alpha, so it may be read after EVERY trade, stopped at any moment,
//! and still keeps its error guarantee. The same wealth process doubles as a Kelly-style stake rule, so capital can
//! be released in proportion to evidence (`evidence_fraction`) instead of on a calendar date.
//!
//! Two directions are tracked: PROFIT (H0: mean return <= 0) to earn capital, and LOSS (H0: mean return >= 0) to
//! kill a strategy early. Observations must lie in [-scale, scale]; out-of-range values are errors, never clipped
//! (clipping would bias the test in the strategy's favour).
//! `e_bh` controls the false discovery rate across many strategies under arbitrary dependence (Wang & Ramdas).

#[derive(Debug, PartialEq)]
pub enum EError {
    BadConfig,
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Evidence that the mean return is positive.
    Profit,
    /// Evidence that the mean return is negative.
    Loss,
}

#[derive(Clone, Debug)]
pub struct EProcess {
    dir: Direction,
    scale: f64,
    cap: f64,
    n: u64,
    sum: f64,
    sumsq: f64,
    log_w: f64,
}

impl EProcess {
    /// `scale`: the largest absolute per-period return you can ever observe (known bound, e.g. max loss as a
    /// fraction of capital). `cap`: largest stake fraction, in (0, 0.5].
    pub fn new(dir: Direction, scale: f64, cap: f64) -> Result<Self, EError> {
        if !(scale.is_finite() && scale > 0.0 && cap.is_finite() && cap > 0.0 && cap <= 0.5) {
            return Err(EError::BadConfig);
        }
        Ok(Self {
            dir,
            scale,
            cap,
            n: 0,
            sum: 0.0,
            sumsq: 0.0,
            log_w: 0.0,
        })
    }

    fn sign(&self) -> f64 {
        match self.dir {
            Direction::Profit => 1.0,
            Direction::Loss => -1.0,
        }
    }

    /// Predictable stake: depends only on past observations (shrunk toward 0 early on).
    fn stake(&self) -> f64 {
        let m = self.sum / (self.n as f64 + 1.0);
        let v = (self.sumsq + 1.0) / (self.n as f64 + 1.0);
        (self.sign() * m / v).clamp(0.0, self.cap)
    }

    pub fn update(&mut self, x: f64) -> Result<(), EError> {
        if !x.is_finite() || x.abs() > self.scale {
            return Err(EError::OutOfRange);
        }
        let y = x / self.scale;
        let lam = self.stake();
        self.log_w += (1.0 + lam * self.sign() * y).ln(); // lam <= 0.5 and |y| <= 1 so the factor is >= 0.5
        self.n += 1;
        self.sum += y;
        self.sumsq += y * y;
        Ok(())
    }

    pub fn n(&self) -> u64 {
        self.n
    }
    pub fn log_wealth(&self) -> f64 {
        self.log_w
    }
    /// The e-value so far (capped to avoid overflow in reports).
    pub fn e_value(&self) -> f64 {
        self.log_w.min(700.0).exp()
    }
    /// True once the null is rejected at level `alpha` (valid at every look, any stopping rule).
    pub fn crossed(&self, alpha: f64) -> bool {
        alpha > 0.0 && alpha < 1.0 && self.log_w >= (1.0 / alpha).ln()
    }
    /// Evidence as a 0..1 fraction of what is needed to reject at `alpha`: the capital ladder input.
    pub fn evidence_fraction(&self, alpha: f64) -> f64 {
        if !(alpha > 0.0 && alpha < 1.0) {
            return 0.0;
        }
        (self.log_w / (1.0 / alpha).ln()).clamp(0.0, 1.0)
    }
}

/// Capital to release to a strategy: `cap` times the profit evidence fraction, and zero as soon as the loss
/// e-process has crossed `kill_alpha`. Capital rises only as evidence accumulates and never on a calendar date.
pub fn capital_ladder(
    profit: &EProcess,
    loss: &EProcess,
    alpha: f64,
    kill_alpha: f64,
    cap: f64,
) -> f64 {
    if !(cap.is_finite() && cap >= 0.0) || loss.crossed(kill_alpha) {
        return 0.0;
    }
    cap * profit.evidence_fraction(alpha)
}

/// e-BH: indices of hypotheses rejected with false discovery rate <= alpha under ANY dependence.
/// Invalid e-values (NaN, negative) count as 0.
pub fn e_bh(e: &[f64], alpha: f64) -> Vec<usize> {
    let m = e.len();
    if m == 0 || !(alpha > 0.0 && alpha < 1.0) {
        return vec![];
    }
    let mut idx: Vec<usize> = (0..m).collect();
    let val = |i: usize| {
        if e[i].is_finite() && e[i] > 0.0 {
            e[i]
        } else if e[i] == f64::INFINITY {
            f64::MAX
        } else {
            0.0
        }
    };
    idx.sort_by(|&a, &b| {
        val(b)
            .partial_cmp(&val(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut k_star = 0;
    for k in 1..=m {
        if val(idx[k - 1]) >= m as f64 / (alpha * k as f64) {
            k_star = k;
        }
    }
    let mut out: Vec<usize> = idx[..k_star].to_vec();
    out.sort_unstable();
    out
}

/// Arithmetic mean of e-values: always a valid e-value, whatever the dependence (use it to merge evidence
/// about the same null from overlapping data).
pub fn e_average(e: &[f64]) -> f64 {
    if e.is_empty() {
        return 1.0;
    }
    e.iter()
        .map(|v| if v.is_finite() && *v > 0.0 { *v } else { 0.0 })
        .sum::<f64>()
        / e.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn u(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Fraction of paths that EVER cross while being checked after every observation.
    fn crossing_rate(drift: f64, dir: Direction, paths: u32, steps: u32) -> f64 {
        let mut r = Rng(0x9E3779B97F4A7C15);
        let mut hit = 0;
        for _ in 0..paths {
            let mut e = EProcess::new(dir, 1.0, 0.5).unwrap();
            for _ in 0..steps {
                let x = (r.u() * 2.0 - 1.0 + drift).clamp(-1.0, 1.0);
                e.update(x).unwrap();
                if e.crossed(0.05) {
                    hit += 1;
                    break;
                }
            }
        }
        hit as f64 / paths as f64
    }

    #[test]
    fn type_one_error_holds_under_continuous_peeking() {
        // zero-mean returns, looked at after every trade for 600 trades
        assert!(crossing_rate(0.0, Direction::Profit, 2000, 600) <= 0.07);
        assert!(crossing_rate(0.0, Direction::Loss, 2000, 600) <= 0.07);
    }

    #[test]
    fn real_edge_is_found_and_the_wrong_direction_is_not() {
        assert!(crossing_rate(0.15, Direction::Profit, 300, 600) > 0.9);
        assert!(crossing_rate(0.15, Direction::Loss, 300, 600) <= 0.07);
        assert!(crossing_rate(-0.15, Direction::Loss, 300, 600) > 0.9);
    }

    #[test]
    fn out_of_range_and_bad_config_are_errors_not_clipped() {
        assert!(EProcess::new(Direction::Profit, 0.0, 0.5).is_err());
        assert!(EProcess::new(Direction::Profit, 1.0, 0.9).is_err());
        let mut e = EProcess::new(Direction::Profit, 0.02, 0.5).unwrap();
        assert_eq!(e.update(0.05), Err(EError::OutOfRange));
        assert_eq!(e.update(f64::NAN), Err(EError::OutOfRange));
        assert_eq!(e.n(), 0);
        assert!(e.update(0.02).is_ok());
    }

    #[test]
    fn capital_ladder_rises_with_evidence_and_dies_on_kill() {
        let mut r = Rng(42);
        let (mut p, mut l) = (
            EProcess::new(Direction::Profit, 1.0, 0.5).unwrap(),
            EProcess::new(Direction::Loss, 1.0, 0.5).unwrap(),
        );
        let mut last = 0.0;
        let mut rising = true;
        for _ in 0..400 {
            let x = (r.u() * 2.0 - 1.0 + 0.2).clamp(-1.0, 1.0);
            p.update(x).unwrap();
            l.update(x).unwrap();
            let c = capital_ladder(&p, &l, 0.05, 0.05, 5000.0);
            rising &= c + 1e-9 >= 0.0 && c <= 5000.0;
            last = c;
        }
        assert!(rising && last > 4000.0, "{last}");
        // a losing strategy gets zero capital
        let (mut p, mut l) = (
            EProcess::new(Direction::Profit, 1.0, 0.5).unwrap(),
            EProcess::new(Direction::Loss, 1.0, 0.5).unwrap(),
        );
        for _ in 0..400 {
            let x = (r.u() * 2.0 - 1.0 - 0.3).clamp(-1.0, 1.0);
            p.update(x).unwrap();
            l.update(x).unwrap();
        }
        assert_eq!(capital_ladder(&p, &l, 0.05, 0.05, 5000.0), 0.0);
    }

    #[test]
    fn e_bh_rejects_strong_and_ignores_weak_and_invalid() {
        let e = [1000.0, 1.2, 500.0, f64::NAN, -3.0, 0.4];
        assert_eq!(e_bh(&e, 0.1), vec![0, 2]);
        assert!(e_bh(&[1.0, 2.0, 3.0], 0.05).is_empty());
        assert!(e_bh(&[], 0.05).is_empty());
        assert!(e_bh(&[100.0], 1.5).is_empty());
        assert!((e_average(&[10.0, 0.0]) - 5.0).abs() < 1e-12);
    }
}
