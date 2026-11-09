//! Stored-versus-derived balance audit: the first slice of equivalence checking.
//!
//! Many operational systems store a running balance (inventory on hand, account balance) next to the
//! transactions that should produce it. Re-deriving the balance from the transactions and comparing
//! tests two things at once: the source system is internally consistent, and a migration preserved
//! business state and not merely rows. Comparing rows alone (a data diff) cannot see either.
//!
//! Amounts are exact fixed-point (six decimals) in `i128`; there is no floating point.

use crate::table::Table;
use std::collections::BTreeMap;
use std::fmt;

/// Which tables and columns make up a balance and its transactions.
#[derive(Clone, Copy, Debug)]
pub struct BalanceSpec<'a> {
    /// The transaction table.
    pub transactions: &'a Table,
    /// Columns identifying the balance a transaction belongs to.
    pub tx_keys: &'a [&'a str],
    /// The signed amount column of a transaction.
    pub tx_amount: &'a str,
    /// The table of stored balances.
    pub balances: &'a Table,
    /// Columns identifying a stored balance, aligned with `tx_keys`.
    pub bal_keys: &'a [&'a str],
    /// The stored balance column.
    pub bal_amount: &'a str,
}

/// A stored balance that differs from the sum of its transactions. Amounts are scaled by 1,000,000.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    /// The balance key values.
    pub key: Vec<String>,
    /// What the balance table says.
    pub stored: i128,
    /// What the transactions add up to.
    pub derived: i128,
}

/// The audit result. Lists are sorted by key, so a report is reproducible.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditReport {
    /// Stored balances examined.
    pub balances_checked: usize,
    /// Stored balances that disagree with their transactions (including a non-zero balance with none).
    pub drifts: Vec<Drift>,
    /// Keys with transactions that add up to a non-zero amount but no stored balance.
    pub unstored: Vec<(Vec<String>, i128)>,
}

/// Why an audit could not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditError {
    /// A named column is not in its table.
    MissingColumn(String),
    /// An amount is not a decimal number with at most six decimals (row is 0-based).
    BadAmount {
        /// The table the amount is in.
        table: String,
        /// 0-based row.
        row: usize,
        /// The text found.
        text: String,
    },
    /// Two stored balances share a key, so the comparison is ambiguous.
    DuplicateBalanceKey(Vec<String>),
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AuditError {}

/// Parses `-12.5` style decimals into millionths. At most six decimals; no exponent, no separators.
fn parse_amount(text: &str) -> Option<i128> {
    let (neg, body) = text
        .strip_prefix('-')
        .map_or((false, text.strip_prefix('+').unwrap_or(text)), |b| {
            (true, b)
        });
    let (whole, frac) = body.split_once('.').unwrap_or((body, ""));
    if whole.is_empty() && frac.is_empty()
        || frac.len() > 6
        || !whole
            .bytes()
            .chain(frac.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole: i128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac_scaled: i128 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<6}").parse().ok()?
    };
    let magnitude = whole.checked_mul(1_000_000)?.checked_add(frac_scaled)?;
    Some(if neg { -magnitude } else { magnitude })
}

fn column<'a>(t: &'a Table, name: &str) -> Result<&'a [Option<String>], AuditError> {
    t.columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
        .map(|c| c.values.as_slice())
        .ok_or_else(|| AuditError::MissingColumn(format!("{}.{name}", t.name)))
}

fn key_at(cols: &[&[Option<String>]], row: usize) -> Option<Vec<String>> {
    cols.iter().map(|c| c[row].clone()).collect()
}

/// Re-derives every stored balance from its transactions and reports the differences.
///
/// # Errors
///
/// A missing column, an unparseable amount, or duplicate stored-balance keys.
pub fn balance_audit(spec: &BalanceSpec<'_>) -> Result<AuditReport, AuditError> {
    let tx_key_cols: Vec<&[Option<String>]> = spec
        .tx_keys
        .iter()
        .map(|k| column(spec.transactions, k))
        .collect::<Result<_, _>>()?;
    let amounts = column(spec.transactions, spec.tx_amount)?;
    let mut derived: BTreeMap<Vec<String>, i128> = BTreeMap::new();
    for (row, amount) in amounts.iter().enumerate() {
        let (Some(text), Some(key)) = (amount.as_deref(), key_at(&tx_key_cols, row)) else {
            continue;
        };
        let value = parse_amount(text).ok_or_else(|| AuditError::BadAmount {
            table: spec.transactions.name.clone(),
            row,
            text: text.to_owned(),
        })?;
        *derived.entry(key).or_insert(0) += value;
    }

    let bal_key_cols: Vec<&[Option<String>]> = spec
        .bal_keys
        .iter()
        .map(|k| column(spec.balances, k))
        .collect::<Result<_, _>>()?;
    let stored_col = column(spec.balances, spec.bal_amount)?;
    let mut stored: BTreeMap<Vec<String>, i128> = BTreeMap::new();
    for (row, amount) in stored_col.iter().enumerate() {
        let (Some(text), Some(key)) = (amount.as_deref(), key_at(&bal_key_cols, row)) else {
            continue;
        };
        let value = parse_amount(text).ok_or_else(|| AuditError::BadAmount {
            table: spec.balances.name.clone(),
            row,
            text: text.to_owned(),
        })?;
        if stored.insert(key.clone(), value).is_some() {
            return Err(AuditError::DuplicateBalanceKey(key));
        }
    }

    let mut report = AuditReport {
        balances_checked: stored.len(),
        ..AuditReport::default()
    };
    for (key, &s) in &stored {
        let d = derived.get(key).copied().unwrap_or(0);
        if s != d {
            report.drifts.push(Drift {
                key: key.clone(),
                stored: s,
                derived: d,
            });
        }
    }
    for (key, &d) in &derived {
        if d != 0 && !stored.contains_key(key) {
            report.unstored.push((key.clone(), d));
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(name: &str, tsv: &str) -> Table {
        Table::from_tsv(name, tsv).unwrap()
    }

    #[test]
    fn parses_exact_decimals_and_rejects_the_rest() {
        assert_eq!(parse_amount("12"), Some(12_000_000));
        assert_eq!(parse_amount("-12.5"), Some(-12_500_000));
        assert_eq!(parse_amount("+0.000001"), Some(1));
        assert_eq!(parse_amount(".5"), Some(500_000));
        for bad in [
            "",
            "-",
            ".",
            "1.2345678",
            "1e3",
            "1,000",
            "abc",
            "--1",
            "1.2.3",
        ] {
            assert_eq!(parse_amount(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn reports_exactly_the_balances_that_disagree() {
        let tx = t("TX", "item\tqty\nA\t10\nA\t-3\nB\t5\nB\t5\nC\t2.5\nD\t7\n");
        let bal = t("BAL", "item\tbal\nA\t7\nB\t11\nC\t2.5\nE\t0\nF\t4\n");
        let r = balance_audit(&BalanceSpec {
            transactions: &tx,
            tx_keys: &["item"],
            tx_amount: "qty",
            balances: &bal,
            bal_keys: &["item"],
            bal_amount: "bal",
        })
        .unwrap();
        assert_eq!(r.balances_checked, 5);
        let keys: Vec<_> = r
            .drifts
            .iter()
            .map(|d| (d.key[0].as_str(), d.stored, d.derived))
            .collect();
        assert_eq!(
            keys,
            vec![("B", 11_000_000, 10_000_000), ("F", 4_000_000, 0)]
        );
        assert_eq!(r.unstored, vec![(vec!["D".to_owned()], 7_000_000)]);
    }

    #[test]
    fn errors_are_specific() {
        let tx = t("TX", "item\tqty\nA\tx\n");
        let bal = t("BAL", "item\tbal\nA\t1\n");
        let run = |keys: &[&str]| {
            balance_audit(&BalanceSpec {
                transactions: &tx,
                tx_keys: keys,
                tx_amount: "qty",
                balances: &bal,
                bal_keys: &["item"],
                bal_amount: "bal",
            })
        };
        assert!(matches!(run(&["nope"]), Err(AuditError::MissingColumn(_))));
        assert!(matches!(
            run(&["item"]),
            Err(AuditError::BadAmount { row: 0, .. })
        ));
        let dup = t("BAL", "item\tbal\nA\t1\nA\t2\n");
        let tx_ok = t("TX", "item\tqty\nA\t1\n");
        let s = BalanceSpec {
            transactions: &tx_ok,
            tx_keys: &["item"],
            tx_amount: "qty",
            balances: &dup,
            bal_keys: &["item"],
            bal_amount: "bal",
        };
        assert_eq!(
            balance_audit(&s),
            Err(AuditError::DuplicateBalanceKey(vec!["A".to_owned()]))
        );
    }
}
