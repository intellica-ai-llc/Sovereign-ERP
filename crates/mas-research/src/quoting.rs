//! Two-sided quotes for a binary contract that settles to 0 or 1.
//!
//! Adapts Avellaneda-Stoikov to prediction markets, following the structure of "Optimal Market Making in
//! Prediction Markets" (arXiv 2607.17991): price-dependent volatility (it vanishes at 0 and 1), a terminal
//! settlement penalty gamma_T * q^2 * p(1-p), and fill intensity that decays with distance from the mid.
//! This is a closed-form APPROXIMATION of that paper's HJB solution, used until the numeric solver is validated
//! against recorded fills. Every parameter is a pre-registered hypothesis; none is a measured fact.
//! Prices are in dollars on (0, 1); `q` is signed inventory in shares (positive = long YES).

#[derive(Clone, Copy, Debug)]
pub struct QuoteParams {
    /// Risk aversion for running inventory variance.
    pub gamma: f64,
    /// Terminal settlement-risk aversion.
    pub gamma_t: f64,
    /// Logit-space volatility per sqrt(second).
    pub sigma_logit: f64,
    /// Fill-intensity decay per dollar of distance from mid (larger = fills dry up faster).
    pub k: f64,
    /// Hard inventory limit in shares; at the limit the side that adds inventory is not quoted.
    pub max_inv: f64,
    pub tick: f64,
    /// Never rest a bid below this price (favourite-longshot hypothesis H6: cheap contracts are bought by
    /// takers with extreme beliefs; do not warehouse them). 0 disables.
    pub min_buy_price: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quotes {
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}

/// Reservation price: the mid shifted against inventory by running and settlement risk.
pub fn reservation(p: f64, q: f64, secs_left: f64, par: &QuoteParams) -> f64 {
    let vol = p * (1.0 - p) * par.sigma_logit;
    // inventory is normalised by the hard limit, so the shift at FULL inventory is
    // gamma * vol^2 * secs_left + gamma_t * p(1-p) dollars: a number a human can sanity-check
    p - (q / par.max_inv)
        * (par.gamma * vol * vol * secs_left.max(0.0) + par.gamma_t * p * (1.0 - p))
}

pub fn quote(p: f64, q: f64, secs_left: f64, par: &QuoteParams) -> Option<Quotes> {
    let ok = p.is_finite()
        && p > 0.0
        && p < 1.0
        && q.is_finite()
        && secs_left.is_finite()
        && par.gamma > 0.0
        && par.gamma_t >= 0.0
        && par.sigma_logit >= 0.0
        && par.k > 0.0
        && par.tick > 0.0
        && par.tick < 0.5
        && par.max_inv > 0.0
        && par.min_buy_price >= 0.0;
    if !ok {
        return None;
    }
    let vol = p * (1.0 - p) * par.sigma_logit;
    let r = reservation(p, q, secs_left, par);
    let spread = par.gamma * vol * vol * secs_left.max(0.0)
        + (2.0 / par.gamma) * (1.0 + par.gamma / par.k).ln();
    let (lo, hi) = (par.tick, 1.0 - par.tick);
    let bid_raw = r - spread / 2.0;
    let ask_raw = r + spread / 2.0;
    // post-only: never quote through the mid, however strong the inventory skew
    let below = ((p / par.tick - 1e-9).ceil() - 1.0) * par.tick;
    let above = ((p / par.tick + 1e-9).floor() + 1.0) * par.tick;
    let bid = ((bid_raw / par.tick).floor() * par.tick)
        .min(below)
        .clamp(lo, hi);
    let ask = ((ask_raw / par.tick).ceil() * par.tick)
        .max(above)
        .clamp(lo, hi);
    let mut b =
        (q < par.max_inv && bid_raw >= lo - par.tick && bid < p && bid >= par.min_buy_price)
            .then_some(bid);
    let mut a = (q > -par.max_inv && ask_raw <= hi + par.tick && ask > p).then_some(ask);
    if let (Some(x), Some(y)) = (b, a) {
        if x >= y {
            b = None;
            a = None;
        }
    }
    Some(Quotes { bid: b, ask: a })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn par() -> QuoteParams {
        QuoteParams {
            gamma: 2.0,
            gamma_t: 0.2,
            sigma_logit: 0.01,
            k: 20.0,
            max_inv: 500.0,
            tick: 0.01,
            min_buy_price: 0.0,
        }
    }

    #[test]
    fn flat_inventory_is_centred_and_positive_inventory_shifts_down() {
        let flat = quote(0.5, 0.0, 600.0, &par()).unwrap();
        let (b, a) = (flat.bid.unwrap(), flat.ask.unwrap());
        assert!(((b + a) / 2.0 - 0.5).abs() < 0.011, "{b} {a}");
        let long = quote(0.5, 300.0, 600.0, &par()).unwrap();
        assert!(long.bid.unwrap() < b && long.ask.unwrap() < a);
        let short = quote(0.5, -300.0, 600.0, &par()).unwrap();
        assert!(short.bid.unwrap() > b && short.ask.unwrap() > a);
    }

    #[test]
    fn spread_widens_with_time_left_and_is_wider_mid_than_near_the_edge() {
        let w = |p: f64, t: f64| {
            let r = quote(p, 0.0, t, &par()).unwrap();
            r.ask.unwrap() - r.bid.unwrap()
        };
        assert!(w(0.5, 36_000.0) > w(0.5, 60.0));
        assert!(w(0.5, 36_000.0) >= w(0.9, 36_000.0));
    }

    #[test]
    fn inventory_limits_remove_the_side_that_adds_risk() {
        let at_long = quote(0.5, 500.0, 600.0, &par()).unwrap();
        assert!(at_long.bid.is_none() && at_long.ask.is_some());
        let at_short = quote(0.5, -500.0, 600.0, &par()).unwrap();
        assert!(at_short.ask.is_none() && at_short.bid.is_some());
    }

    #[test]
    fn quotes_stay_inside_the_book_and_bad_inputs_are_refused() {
        for p in [0.02, 0.05, 0.5, 0.95, 0.98] {
            for q in [-400.0, 0.0, 400.0] {
                if let Some(Quotes { bid, ask }) = quote(p, q, 3600.0, &par()) {
                    for x in [bid, ask].into_iter().flatten() {
                        assert!((0.01..=0.99).contains(&x));
                    }
                    if let (Some(b), Some(a)) = (bid, ask) {
                        assert!(b < a);
                    }
                }
            }
        }
        assert!(quote(0.0, 0.0, 1.0, &par()).is_none());
        assert!(quote(f64::NAN, 0.0, 1.0, &par()).is_none());
        let mut bad = par();
        bad.gamma = 0.0;
        assert!(quote(0.5, 0.0, 1.0, &bad).is_none());
    }

    #[test]
    fn longshot_floor_stops_cheap_bids_only() {
        let mut p = par();
        p.min_buy_price = 0.10;
        let q = quote(0.06, 0.0, 600.0, &p).unwrap();
        assert!(q.bid.is_none());
        let q = quote(0.55, 0.0, 600.0, &p).unwrap();
        assert!(q.bid.is_some());
    }
}
