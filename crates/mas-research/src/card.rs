//! The Verified Performance Card: the only way we are allowed to talk about performance.
//!
//! A card is a pure function of (return ledger, trial registry, audit facts). Every line is a `Claim` that is either
//! PROVEN by a check that ran or NOT YET with the reason; there is no way to print a number without its check. The card
//! serialises canonically (sorted keys, values rounded to 6 places) and is identified by the SHA-256 of that text, so
//! it can be committed to the witnessed evidence log, and ANY third party holding the same inputs can recompute it
//! byte for byte (`verify_card`). What a card can say is limited by what it proved: no superlatives, no "guaranteed",
//! no number without its check. Superlatives ("the best") are unfalsifiable; instead the card ends with a challenge
//! anyone can accept: publish a card of your own, computed the same way, and compare.
use crate::claims::Claim;
use crate::eprocess::{Direction, EProcess};
use crate::registry::Registry;
use crate::{moments, norm_inv_cdf, sharpe};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct Audit {
    pub orders_total: u64,
    /// Orders that have a registered intent statement in the log BEFORE the order (must equal `orders_total`).
    pub orders_with_intent: u64,
    /// Highest observed exposure as a fraction of the hard capital cap (must be <= 1).
    pub max_exposure_frac: f64,
    /// Times a limit was bypassed or overridden (must be 0).
    pub limit_overrides: u32,
    pub kill_tests_passed: u32,
    pub days_since_last_kill_test: u32,
    pub reconciliation_breaks: u32,
    /// Reconciliations against the venue actually performed. Paper trading has no venue, so this stays 0 and the
    /// `books_reconcile_to_venue` claim cannot be proven: a card built from paper data can never read Verified.
    pub reconciliations_done: u32,
    pub live_days: u32,
    /// The drawdown the risk shield was configured to kill at (fraction of peak equity).
    pub drawdown_bound: f64,
}

#[derive(Clone, Debug)]
pub struct CardInput<'a> {
    pub strategy: &'a str,
    pub period: &'a str,
    /// Net per-period returns (after fees, rewards and slippage) as fractions of deployed capital.
    pub returns: &'a [f64],
    /// Largest absolute per-period return the strategy could ever book (known bound, used by the e-process).
    pub scale: f64,
    pub registry: &'a Registry,
    pub trial: &'a str,
    pub audit: &'a Audit,
    /// Evidence-log head hash the ledger was committed under (binds the card to a witnessed record).
    pub log_head: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub min_periods: usize,
    pub min_live_days: u32,
    pub sharpe_conf: f64,
    pub min_dsr: f64,
    pub e_alpha: f64,
    pub max_days_since_kill_test: u32,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            min_periods: 90,
            min_live_days: 90,
            sharpe_conf: 0.95,
            min_dsr: 0.95,
            e_alpha: 0.01,
            max_days_since_kill_test: 30,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Some required evidence is missing.
    Unverified,
    /// Every safety claim proven; performance not yet proven.
    Candidate,
    /// Every safety and every performance claim proven.
    Verified,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub strategy: String,
    pub period: String,
    pub log_head: String,
    pub claims: Vec<Claim>,
    pub standing: Standing,
}

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

pub fn max_drawdown(returns: &[f64]) -> Option<f64> {
    if returns.iter().any(|r| !r.is_finite() || *r <= -1.0) {
        return None;
    }
    let (mut eq, mut peak, mut dd) = (1.0f64, 1.0f64, 0.0f64);
    for r in returns {
        eq *= 1.0 + r;
        peak = peak.max(eq);
        dd = dd.max((peak - eq) / peak);
    }
    Some(dd)
}

fn proven(name: &'static str, value: f64, evidence: String) -> Claim {
    Claim::Proven {
        name,
        value: r6(value),
        evidence,
    }
}
fn not_yet(name: &'static str, reason: impl Into<String>) -> Claim {
    Claim::NotYet {
        name,
        reason: reason.into(),
    }
}

pub fn build(i: &CardInput, rules: &Rules) -> Card {
    let n = i.returns.len();
    let a = i.audit;
    let mut perf = vec![];
    let mut safety = vec![];

    // ---- performance ----
    let enough = n >= rules.min_periods;
    perf.push(if !enough {
        not_yet(
            "net_log_return",
            format!("{n} of {} required periods", rules.min_periods),
        )
    } else if i.returns.iter().any(|r| !r.is_finite() || *r <= -1.0) {
        not_yet("net_log_return", "ledger contains an invalid return")
    } else {
        proven(
            "net_log_return",
            i.returns.iter().map(|r| (1.0 + r).ln()).sum::<f64>(),
            format!("{n} periods, net of fees, rewards and slippage"),
        )
    });
    perf.push(match (enough, moments(i.returns), sharpe(i.returns)) {
        (true, Some((_, _, skew, kurt)), Some(sr)) => {
            let v = 1.0 - skew * sr + (kurt - 1.0) / 4.0 * sr * sr;
            let z = norm_inv_cdf(rules.sharpe_conf);
            match (z, v > 0.0) {
                (Some(z), true) => {
                    let lcb = sr - z * (v / (n as f64 - 1.0)).sqrt();
                    if lcb > 0.0 {
                        proven("sharpe_lower_bound", lcb, format!("per-period Sharpe {sr:.4}, {:.0}% one-sided bound, skew and kurtosis adjusted", rules.sharpe_conf * 100.0))
                    } else {
                        not_yet("sharpe_lower_bound", format!("lower bound {lcb:.4} is not above zero (Sharpe {sr:.4})"))
                    }
                }
                _ => not_yet("sharpe_lower_bound", "variance term invalid"),
            }
        }
        (false, ..) => not_yet("sharpe_lower_bound", "too few periods"),
        _ => not_yet("sharpe_lower_bound", "returns degenerate"),
    });
    perf.push(if !enough {
        not_yet("deflated_sharpe", "too few periods")
    } else if i.registry.verify_chain().is_err() {
        not_yet("deflated_sharpe", "trial registry chain does not verify")
    } else {
        match i.registry.deflated(i.trial, i.returns) {
            Some(d) if d >= rules.min_dsr => proven(
                "deflated_sharpe",
                d,
                format!(
                    "registry head {}, {} trials in the family",
                    &i.registry.head()[..16],
                    i.registry.trials_in(&family_of(i.registry, i.trial))
                ),
            ),
            Some(d) => not_yet(
                "deflated_sharpe",
                format!("{d:.3} is below {}", rules.min_dsr),
            ),
            None => not_yet(
                "deflated_sharpe",
                "trial not registered, not recorded, or evidence too thin",
            ),
        }
    });
    perf.push(match EProcess::new(Direction::Profit, i.scale, 0.5) {
        Err(_) => not_yet("anytime_valid_profit_evidence", "invalid scale"),
        Ok(mut e) => {
            if !enough {
                not_yet("anytime_valid_profit_evidence", "too few periods")
            } else if i.returns.iter().try_for_each(|r| e.update(*r)).is_err() {
                not_yet(
                    "anytime_valid_profit_evidence",
                    "a return exceeds the declared bound",
                )
            } else if e.crossed(rules.e_alpha) {
                proven(
                    "anytime_valid_profit_evidence",
                    e.log_wealth(),
                    format!(
                        "e-value {:.3e} crossed 1/{} (valid at any stopping time)",
                        e.e_value(),
                        rules.e_alpha
                    ),
                )
            } else {
                not_yet(
                    "anytime_valid_profit_evidence",
                    format!(
                        "e-value {:.3e} has not reached {:.0}",
                        e.e_value(),
                        1.0 / rules.e_alpha
                    ),
                )
            }
        }
    });

    // ---- safety ----
    safety.push(
        if a.orders_total > 0 && a.orders_with_intent == a.orders_total {
            proven(
                "every_order_logged_first",
                1.0,
                format!(
                    "{} of {} orders had an intent in the log before the order",
                    a.orders_with_intent, a.orders_total
                ),
            )
        } else if a.orders_total == 0 {
            not_yet("every_order_logged_first", "no orders yet")
        } else {
            not_yet(
                "every_order_logged_first",
                format!(
                    "{} of {} orders lacked a prior intent",
                    a.orders_total.saturating_sub(a.orders_with_intent),
                    a.orders_total
                ),
            )
        },
    );
    safety.push(if a.orders_total == 0 {
        not_yet("capital_cap_respected", "no orders yet")
    } else if a.max_exposure_frac.is_finite()
        && a.max_exposure_frac <= 1.0
        && a.limit_overrides == 0
    {
        proven(
            "capital_cap_respected",
            a.max_exposure_frac,
            format!(
                "peak exposure {:.1}% of the cap, {} overrides",
                a.max_exposure_frac * 100.0,
                a.limit_overrides
            ),
        )
    } else {
        not_yet(
            "capital_cap_respected",
            format!(
                "peak exposure {:.3} of cap with {} override(s)",
                a.max_exposure_frac, a.limit_overrides
            ),
        )
    });
    safety.push(match max_drawdown(i.returns) {
        _ if !enough => not_yet(
            "drawdown_within_bound",
            format!("{n} of {} required periods", rules.min_periods),
        ),
        Some(dd) if dd <= a.drawdown_bound && a.drawdown_bound > 0.0 && a.drawdown_bound < 1.0 => {
            proven(
                "drawdown_within_bound",
                dd,
                format!(
                    "max drawdown {:.2}% against a shield kill at {:.0}%",
                    dd * 100.0,
                    a.drawdown_bound * 100.0
                ),
            )
        }
        Some(dd) => not_yet(
            "drawdown_within_bound",
            format!(
                "max drawdown {:.2}% exceeds the {:.0}% bound",
                dd * 100.0,
                a.drawdown_bound * 100.0
            ),
        ),
        None => not_yet("drawdown_within_bound", "invalid ledger"),
    });
    safety.push(
        if a.kill_tests_passed > 0 && a.days_since_last_kill_test <= rules.max_days_since_kill_test
        {
            proven(
                "kill_switch_tested_recently",
                f64::from(a.kill_tests_passed),
                format!(
                    "{} passed, last {} day(s) ago",
                    a.kill_tests_passed, a.days_since_last_kill_test
                ),
            )
        } else {
            not_yet(
                "kill_switch_tested_recently",
                format!(
                    "{} passed; last {} day(s) ago (max {})",
                    a.kill_tests_passed,
                    a.days_since_last_kill_test,
                    rules.max_days_since_kill_test
                ),
            )
        },
    );
    safety.push(if a.reconciliations_done == 0 {
        not_yet(
            "books_reconcile_to_venue",
            "no reconciliation against a venue has been performed (paper trading has no venue)",
        )
    } else if a.reconciliation_breaks == 0 && a.live_days > 0 {
        proven(
            "books_reconcile_to_venue",
            f64::from(a.reconciliations_done),
            format!("{} reconciliations, zero breaks", a.reconciliations_done),
        )
    } else {
        not_yet(
            "books_reconcile_to_venue",
            format!(
                "{} break(s), {} live day(s)",
                a.reconciliation_breaks, a.live_days
            ),
        )
    });
    safety.push(if a.live_days >= rules.min_live_days {
        proven(
            "live_track_length",
            f64::from(a.live_days),
            format!("{} live days", a.live_days),
        )
    } else {
        not_yet(
            "live_track_length",
            format!("{} of {} live days", a.live_days, rules.min_live_days),
        )
    });

    let all = |v: &[Claim]| v.iter().all(Claim::is_proven);
    let standing = match (all(&safety), all(&perf)) {
        (true, true) => Standing::Verified,
        (true, false) => Standing::Candidate,
        _ => Standing::Unverified,
    };
    let mut claims = perf;
    claims.extend(safety);
    Card {
        strategy: i.strategy.into(),
        period: i.period.into(),
        log_head: i.log_head.into(),
        claims,
        standing,
    }
}

fn family_of(reg: &Registry, id: &str) -> String {
    reg.entries()
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.family.clone())
        .unwrap_or_default()
}

/// Canonical text of a card (stable across machines): this is what is hashed and committed.
pub fn to_json(c: &Card) -> String {
    let claims: Vec<Value> = c
        .claims
        .iter()
        .map(|x| match x {
            Claim::Proven {
                name,
                value,
                evidence,
            } => {
                json!({"name": name, "status": "proven", "value": r6(*value), "evidence": evidence})
            }
            Claim::NotYet { name, reason } => {
                json!({"name": name, "status": "not_yet", "reason": reason})
            }
        })
        .collect();
    json!({"v":1,"strategy":c.strategy,"period":c.period,"log_head":c.log_head,
        "standing": format!("{:?}", c.standing).to_lowercase(), "claims": claims})
    .to_string()
}

pub fn card_id(c: &Card) -> String {
    hex::encode(Sha256::digest(to_json(c).as_bytes()))
}

/// Recompute a card from the same inputs and compare it with a published one, byte for byte.
pub fn verify_card(published: &str, i: &CardInput, rules: &Rules) -> bool {
    to_json(&build(i, rules)) == published
}

/// The only wording the card allows. It states proven numbers and lists what is NOT yet established. It cannot say
/// "best", "most", "highest", "guaranteed" or "risk-free": none of those is something a check can prove.
pub fn public_statement(c: &Card) -> String {
    let mut s = String::new();
    let get = |n: &str| c.claims.iter().find(|x| x.name() == n);
    s.push_str(&format!(
        "Strategy {} ({}). Standing: {:?}.\n",
        c.strategy, c.period, c.standing
    ));
    for x in &c.claims {
        if let Claim::Proven {
            name,
            value,
            evidence,
        } = x
        {
            s.push_str(&format!("PROVEN {name} = {value}: {evidence}\n"));
        }
    }
    let missing: Vec<&str> = c
        .claims
        .iter()
        .filter(|x| !x.is_proven())
        .map(Claim::name)
        .collect();
    if missing.is_empty() {
        s.push_str("Every check on this card passed.\n");
    } else {
        s.push_str(&format!("NOT YET ESTABLISHED: {}.\n", missing.join(", ")));
    }
    let _ = get;
    s.push_str(&format!("Committed under evidence-log head {}. Recompute it from the same inputs; if it differs, it is wrong.\n", c.log_head));
    s.push_str("Challenge: publish a card of your own, computed the same way, and compare.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) as f64 / (1u64 << 31) as f64
        }
        fn normal(&mut self) -> f64 {
            let (u, v) = (self.next().max(1e-12), self.next());
            (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
        }
    }

    fn series(seed: u64, n: usize, mean: f64, sd: f64) -> Vec<f64> {
        let mut r = Rng(seed);
        (0..n).map(|_| mean + sd * r.normal()).collect()
    }

    fn good_audit() -> Audit {
        Audit {
            orders_total: 1200,
            orders_with_intent: 1200,
            max_exposure_frac: 0.62,
            limit_overrides: 0,
            kill_tests_passed: 3,
            days_since_last_kill_test: 9,
            reconciliation_breaks: 0,
            reconciliations_done: 5,
            live_days: 120,
            drawdown_bound: 0.2,
        }
    }

    /// Registry with `others` abandoned trials of the same family, plus the strategy's own recorded trial.
    fn registry(returns: &[f64], others: usize) -> (Registry, String) {
        let mut reg = Registry::new();
        for k in 0..others {
            let id = reg.register("lp1", &format!("variant={k}"));
            reg.record(&id, &series(100 + k as u64, returns.len(), 0.0, 0.01))
                .unwrap();
        }
        let id = reg.register("lp1", "final");
        reg.record(&id, returns).unwrap();
        (reg, id)
    }

    fn input<'a>(r: &'a [f64], reg: &'a Registry, id: &'a str, a: &'a Audit) -> CardInput<'a> {
        CardInput {
            strategy: "lp1",
            period: "2027-01-01..2027-04-30",
            returns: r,
            scale: 0.05,
            registry: reg,
            trial: id,
            audit: a,
            log_head: "ab12",
        }
    }

    #[test]
    fn an_honest_profitable_strategy_earns_verified() {
        let r = series(7, 400, 0.004, 0.01);
        let (reg, id) = registry(&r, 5);
        let a = good_audit();
        let c = build(&input(&r, &reg, &id, &a), &Rules::default());
        let bad: Vec<_> = c
            .claims
            .iter()
            .filter(|x| !x.is_proven())
            .map(|x| format!("{x:?}"))
            .collect();
        assert_eq!(c.standing, Standing::Verified, "{bad:#?}");
        let said = public_statement(&c);
        for w in [
            "best",
            "most",
            "highest",
            "guarantee",
            "risk-free",
            "safest",
            "record",
        ] {
            assert!(
                !said.to_lowercase().contains(w),
                "forbidden word {w}: {said}"
            );
        }
        assert!(said.contains("Challenge:") && said.contains("Every check"));
    }

    #[test]
    fn luck_after_many_hidden_trials_is_not_verified() {
        // same returns, but the registry honestly shows 3000 abandoned variants: deflation removes the claim
        let r = series(7, 400, 0.0006, 0.01);
        let (reg_few, id_few) = registry(&r, 3);
        let a = good_audit();
        let few = build(&input(&r, &reg_few, &id_few, &a), &Rules::default());
        let mut reg_many = Registry::new();
        for k in 0..3000 {
            let id = reg_many.register("lp1", &format!("v{k}"));
            reg_many
                .record(&id, &series(5000 + k as u64, r.len(), 0.0, 0.01))
                .unwrap();
        }
        let id = reg_many.register("lp1", "final");
        reg_many.record(&id, &r).unwrap();
        let many = build(&input(&r, &reg_many, &id, &a), &Rules::default());
        let d = |c: &Card| {
            c.claims
                .iter()
                .find(|x| x.name() == "deflated_sharpe")
                .unwrap()
                .is_proven()
        };
        assert!(d(&few) || !d(&many)); // never easier with more trials
        assert!(!d(&many));
        assert_ne!(many.standing, Standing::Verified);
    }

    #[test]
    fn each_safety_failure_blocks_verified_and_is_named() {
        let r = series(7, 400, 0.004, 0.01);
        let (reg, id) = registry(&r, 5);
        let cases: Vec<(&str, Audit)> = vec![
            (
                "every_order_logged_first",
                Audit {
                    orders_with_intent: 1199,
                    ..good_audit()
                },
            ),
            (
                "capital_cap_respected",
                Audit {
                    max_exposure_frac: 1.01,
                    ..good_audit()
                },
            ),
            (
                "capital_cap_respected",
                Audit {
                    limit_overrides: 1,
                    ..good_audit()
                },
            ),
            (
                "drawdown_within_bound",
                Audit {
                    drawdown_bound: 0.001,
                    ..good_audit()
                },
            ),
            (
                "kill_switch_tested_recently",
                Audit {
                    days_since_last_kill_test: 31,
                    ..good_audit()
                },
            ),
            (
                "books_reconcile_to_venue",
                Audit {
                    reconciliation_breaks: 1,
                    ..good_audit()
                },
            ),
            (
                "books_reconcile_to_venue",
                Audit {
                    reconciliations_done: 0,
                    ..good_audit()
                },
            ),
            (
                "live_track_length",
                Audit {
                    live_days: 30,
                    ..good_audit()
                },
            ),
        ];
        for (name, a) in cases {
            let c = build(&input(&r, &reg, &id, &a), &Rules::default());
            assert_ne!(c.standing, Standing::Verified, "{name}");
            assert!(
                c.claims.iter().any(|x| x.name() == name && !x.is_proven()),
                "{name}"
            );
        }
    }

    #[test]
    fn noise_and_thin_ledgers_prove_nothing() {
        let a = good_audit();
        let noise = series(9, 400, 0.0, 0.01);
        let (reg, id) = registry(&noise, 5);
        let c = build(&input(&noise, &reg, &id, &a), &Rules::default());
        assert_ne!(c.standing, Standing::Verified);
        assert!(!c
            .claims
            .iter()
            .find(|x| x.name() == "anytime_valid_profit_evidence")
            .unwrap()
            .is_proven());
        let thin = series(7, 40, 0.01, 0.01);
        let (reg2, id2) = registry(&thin, 1);
        let c = build(&input(&thin, &reg2, &id2, &a), &Rules::default());
        assert!(c.claims.iter().filter(|x| !x.is_proven()).count() >= 4);
        assert_eq!(c.standing, Standing::Unverified); // too thin even for the drawdown claim
    }

    #[test]
    fn card_is_canonical_recomputable_and_tamper_evident() {
        let r = series(7, 400, 0.004, 0.01);
        let (reg, id) = registry(&r, 5);
        let a = good_audit();
        let i = input(&r, &reg, &id, &a);
        let c = build(&i, &Rules::default());
        let (j1, j2) = (to_json(&c), to_json(&build(&i, &Rules::default())));
        assert_eq!(j1, j2);
        assert_eq!(card_id(&c).len(), 64);
        assert!(verify_card(&j1, &i, &Rules::default()));
        // editing any proven number, or the standing, fails recomputation
        assert!(!verify_card(
            &j1.replacen("\"standing\":\"verified\"", "\"standing\":\"candidate\"", 1),
            &i,
            &Rules::default()
        ));
        assert!(!verify_card(
            &j1.replacen("proven", "not_yet", 1),
            &i,
            &Rules::default()
        ));
        // a different ledger (one return nudged) yields a different card id
        let mut r2 = r.clone();
        r2[10] += 0.001;
        let (reg2, id2) = registry(&r2, 5);
        let c2 = build(&input(&r2, &reg2, &id2, &a), &Rules::default());
        assert_ne!(card_id(&c), card_id(&c2));
    }

    #[test]
    fn an_empty_ledger_proves_nothing_at_all() {
        let a = Audit {
            orders_total: 0,
            orders_with_intent: 0,
            max_exposure_frac: 0.0,
            limit_overrides: 0,
            kill_tests_passed: 0,
            days_since_last_kill_test: u32::MAX,
            reconciliation_breaks: 0,
            reconciliations_done: 0,
            live_days: 0,
            drawdown_bound: 0.2,
        };
        let reg = Registry::new();
        let c = build(&input(&[], &reg, "", &a), &Rules::default());
        assert_eq!(c.standing, Standing::Unverified);
        let proven: Vec<_> = c
            .claims
            .iter()
            .filter(|x| x.is_proven())
            .map(|x| x.name())
            .collect();
        assert!(proven.is_empty(), "vacuously proven: {proven:?}");
    }

    #[test]
    fn max_drawdown_matches_hand_calculation() {
        // +10%, -20%, +5%: peak 1.10, trough 0.88 -> 20%
        assert!((max_drawdown(&[0.10, -0.20, 0.05]).unwrap() - 0.20).abs() < 1e-12);
        assert_eq!(max_drawdown(&[0.01, 0.02]).unwrap(), 0.0);
        assert!(max_drawdown(&[-1.0]).is_none());
        assert!(max_drawdown(&[f64::NAN]).is_none());
    }
}
