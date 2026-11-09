//! Maximo `MAXRELATIONSHIP` where-clauses as structured joins.
//!
//! In the data dictionary a relationship has a *parent* object, a *child* object and a SQL where
//! clause. Bare identifiers in the clause are columns of the child; `:name` is an attribute of the
//! parent record being navigated from. The supported subset is a conjunction (`AND`) of
//! equalities. Anything else is reported as [`Unsupported`] with the reason, never guessed at.
//!
//! The parent/child naming describes navigation, not key direction or cardinality; those are
//! decided later from data statistics.

use std::fmt;

/// One row of the `MAXRELATIONSHIP` dictionary table, as a connector reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationshipRow {
    /// Relationship name, unique within the parent object.
    pub name: String,
    /// Object the relationship navigates from.
    pub parent: String,
    /// Object the relationship navigates to.
    pub child: String,
    /// The SQL where clause as stored.
    pub where_clause: String,
}

/// A parent attribute equated to a child column (`child_column = :parent_attr`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnPair {
    /// Attribute of the parent record, upper-cased.
    pub parent_attr: String,
    /// Column of the child object, upper-cased.
    pub child_column: String,
}

/// A literal a clause compares against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    /// A quoted string, without the quotes and with `''` unescaped.
    Text(String),
    /// A numeric literal exactly as written.
    Number(String),
}

/// A constant restriction on the child (`child_column = 'ACTIVE'`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// Child column, upper-cased.
    pub child_column: String,
    /// Value it must equal.
    pub literal: Literal,
}

/// The join a supported clause expresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Join {
    /// Parent-attribute to child-column equalities, in clause order, duplicates removed. Never empty.
    pub pairs: Vec<ColumnPair>,
    /// Constant restrictions on the child, in clause order.
    pub filters: Vec<Filter>,
}

/// Why a clause is outside the supported subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// The clause is blank.
    Empty,
    /// The relationship row names no parent or child object.
    MissingObject,
    /// Contains `OR`.
    Disjunction,
    /// Contains `SELECT` or `EXISTS`.
    Subquery,
    /// Calls a function.
    FunctionCall,
    /// Uses parentheses for grouping.
    Grouping,
    /// Uses an operator other than `=` (the operator is named).
    NonEquality(String),
    /// Tests for `NULL`.
    NullTest,
    /// Uses a system variable such as `:&USERNAME&`.
    SystemVariable,
    /// A predicate compares two parent attributes.
    TwoParentAttributes,
    /// A predicate compares two child columns.
    TwoChildColumns,
    /// A predicate constrains only the parent or only constants.
    NoChildColumn,
    /// A column is qualified with an object other than the child.
    ForeignQualifier(String),
    /// No predicate links a parent attribute to a child column.
    NoJoinPredicate,
    /// Malformed text, with what is wrong.
    Syntax(String),
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty where clause"),
            Self::MissingObject => write!(f, "relationship has no parent or child object"),
            Self::Disjunction => write!(f, "contains OR"),
            Self::Subquery => write!(f, "contains a subquery"),
            Self::FunctionCall => write!(f, "calls a function"),
            Self::Grouping => write!(f, "uses parentheses"),
            Self::NonEquality(op) => write!(f, "uses operator {op}"),
            Self::NullTest => write!(f, "tests for NULL"),
            Self::SystemVariable => write!(f, "uses a system variable"),
            Self::TwoParentAttributes => write!(f, "compares two parent attributes"),
            Self::TwoChildColumns => write!(f, "compares two child columns"),
            Self::NoChildColumn => write!(f, "has a predicate without a child column"),
            Self::ForeignQualifier(q) => write!(f, "column qualified with {q}, not the child"),
            Self::NoJoinPredicate => write!(f, "no predicate links parent to child"),
            Self::Syntax(why) => write!(f, "syntax: {why}"),
        }
    }
}

impl std::error::Error for Unsupported {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tok {
    Ident(String),
    Qualified(String, String),
    Bind(String),
    Text(String),
    Number(String),
    Eq,
    And,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '#'
}

/// Splits a clause into tokens, classifying unsupported constructs on the way.
fn lex(clause: &str) -> Result<Vec<Tok>, Unsupported> {
    let chars: Vec<char> = clause.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '\'' {
            let (literal, after) = lex_string(&chars, i)?;
            toks.push(Tok::Text(literal));
            i = after;
        } else if c == ':' {
            let (tok, next) = lex_bind(&chars, i)?;
            toks.push(tok);
            i = next;
        } else if is_ident_start(c) {
            let (tok, next) = lex_word(&chars, i)?;
            toks.push(tok);
            i = next;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            toks.push(Tok::Number(chars[start..i].iter().collect()));
        } else if c == '(' || c == ')' {
            return Err(Unsupported::Grouping);
        } else if matches!(c, '=' | '<' | '>' | '!') {
            let (op, next) = lex_operator(&chars, i);
            if op != "=" {
                return Err(Unsupported::NonEquality(op));
            }
            toks.push(Tok::Eq);
            i = next;
        } else {
            return Err(Unsupported::Syntax(format!("unexpected character {c:?}")));
        }
    }
    Ok(toks)
}

fn lex_string(chars: &[char], start: usize) -> Result<(String, usize), Unsupported> {
    let mut text = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                text.push('\'');
                i += 2;
            } else {
                return Ok((text, i + 1));
            }
        } else {
            text.push(chars[i]);
            i += 1;
        }
    }
    Err(Unsupported::Syntax("unterminated string".to_owned()))
}

fn lex_bind(chars: &[char], start: usize) -> Result<(Tok, usize), Unsupported> {
    let mut i = start + 1;
    if chars.get(i) == Some(&'&') {
        return Err(Unsupported::SystemVariable);
    }
    let name_start = i;
    while i < chars.len() && is_ident_char(chars[i]) {
        i += 1;
    }
    if i == name_start || !is_ident_start(chars[name_start]) {
        return Err(Unsupported::Syntax("':' not followed by a name".to_owned()));
    }
    Ok((
        Tok::Bind(
            chars[name_start..i]
                .iter()
                .collect::<String>()
                .to_ascii_uppercase(),
        ),
        i,
    ))
}

fn lex_operator(chars: &[char], start: usize) -> (String, usize) {
    let mut op = String::from(chars[start]);
    if let Some(&next) = chars.get(start + 1) {
        let two = matches!((chars[start], next), ('<', '>' | '=') | ('>' | '!', '='));
        if two {
            op.push(next);
            return (op, start + 2);
        }
    }
    (op, start + 1)
}

fn lex_word(chars: &[char], start: usize) -> Result<(Tok, usize), Unsupported> {
    let mut i = start;
    while i < chars.len() && is_ident_char(chars[i]) {
        i += 1;
    }
    let word: String = chars[start..i].iter().collect();
    let upper = word.to_ascii_uppercase();
    match upper.as_str() {
        "AND" => return Ok((Tok::And, i)),
        "OR" => return Err(Unsupported::Disjunction),
        "SELECT" | "EXISTS" => return Err(Unsupported::Subquery),
        "IS" | "NULL" => return Err(Unsupported::NullTest),
        "LIKE" | "IN" | "BETWEEN" | "NOT" => return Err(Unsupported::NonEquality(upper)),
        _ => {}
    }
    let mut j = i;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if chars.get(j) == Some(&'(') {
        return Err(Unsupported::FunctionCall);
    }
    if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(|&c| is_ident_start(c)) {
        let col_start = i + 1;
        let mut k = col_start;
        while k < chars.len() && is_ident_char(chars[k]) {
            k += 1;
        }
        let column: String = chars[col_start..k].iter().collect();
        return Ok((Tok::Qualified(upper, column.to_ascii_uppercase()), k));
    }
    Ok((Tok::Ident(upper), i))
}

enum Operand {
    Child(String),
    Parent(String),
    Constant(Literal),
}

fn operand(tok: &Tok, child: &str) -> Result<Operand, Unsupported> {
    match tok {
        Tok::Ident(name) => Ok(Operand::Child(name.clone())),
        Tok::Qualified(q, name) if q == child => Ok(Operand::Child(name.clone())),
        Tok::Qualified(q, _) => Err(Unsupported::ForeignQualifier(q.clone())),
        Tok::Bind(name) => Ok(Operand::Parent(name.clone())),
        Tok::Text(t) => Ok(Operand::Constant(Literal::Text(t.clone()))),
        Tok::Number(n) => Ok(Operand::Constant(Literal::Number(n.clone()))),
        Tok::Eq | Tok::And => Err(Unsupported::Syntax(
            "operator where a value was expected".to_owned(),
        )),
    }
}

/// Parses a relationship where clause against its child object.
///
/// Names are normalised to upper case, as Maximo stores them.
///
/// # Errors
///
/// An [`Unsupported`] naming the first construct outside the supported subset (a conjunction of
/// `=` predicates between child columns, `:parent` attributes and constants), or the syntax error.
pub fn parse_where(clause: &str, child_object: &str) -> Result<Join, Unsupported> {
    let child = child_object.trim().to_ascii_uppercase();
    let toks = lex(clause)?;
    if toks.is_empty() {
        return Err(Unsupported::Empty);
    }
    let mut join = Join {
        pairs: Vec::new(),
        filters: Vec::new(),
    };
    for predicate in toks.split(|t| *t == Tok::And) {
        let [left, Tok::Eq, right] = predicate else {
            return Err(Unsupported::Syntax(
                "expected `value = value` between ANDs".to_owned(),
            ));
        };
        match (operand(left, &child)?, operand(right, &child)?) {
            (Operand::Child(c), Operand::Parent(p)) | (Operand::Parent(p), Operand::Child(c)) => {
                let pair = ColumnPair {
                    parent_attr: p,
                    child_column: c,
                };
                if !join.pairs.contains(&pair) {
                    join.pairs.push(pair);
                }
            }
            (Operand::Child(c), Operand::Constant(l))
            | (Operand::Constant(l), Operand::Child(c)) => {
                join.filters.push(Filter {
                    child_column: c,
                    literal: l,
                });
            }
            (Operand::Parent(_), Operand::Parent(_)) => {
                return Err(Unsupported::TwoParentAttributes)
            }
            (Operand::Child(_), Operand::Child(_)) => return Err(Unsupported::TwoChildColumns),
            _ => return Err(Unsupported::NoChildColumn),
        }
    }
    if join.pairs.is_empty() {
        return Err(Unsupported::NoJoinPredicate);
    }
    Ok(join)
}

/// A relationship successfully understood.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// Relationship name.
    pub name: String,
    /// Object navigated from, upper-cased.
    pub parent_object: String,
    /// Object navigated to, upper-cased.
    pub child_object: String,
    /// The join it expresses.
    pub join: Join,
}

/// The outcome of reading a whole `MAXRELATIONSHIP` table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extraction {
    /// Relationships understood, in input order.
    pub links: Vec<Link>,
    /// Relationships left out, each with the reason, in input order.
    pub unsupported: Vec<(RelationshipRow, Unsupported)>,
}

impl Extraction {
    /// How many rows were read.
    #[must_use]
    pub fn total(&self) -> usize {
        self.links.len() + self.unsupported.len()
    }
}

/// Extracts every understandable relationship; reports the rest instead of dropping them.
#[must_use]
pub fn extract(rows: &[RelationshipRow]) -> Extraction {
    let mut out = Extraction::default();
    for row in rows {
        let parent = row.parent.trim().to_ascii_uppercase();
        let child = row.child.trim().to_ascii_uppercase();
        let outcome = if parent.is_empty() || child.is_empty() {
            Err(Unsupported::MissingObject)
        } else {
            parse_where(&row.where_clause, &child)
        };
        match outcome {
            Ok(join) => out.links.push(Link {
                name: row.name.trim().to_ascii_uppercase(),
                parent_object: parent,
                child_object: child,
                join,
            }),
            Err(reason) => out.unsupported.push((row.clone(), reason)),
        }
    }
    out
}

#[cfg(test)]
// Test fixtures build text with `format!` and `push_str`: clearer than `write!` and fine here.
#[allow(clippy::format_push_string)]
mod tests {
    use super::*;
    use sov_sim::Rng;

    fn pair(parent: &str, child: &str) -> ColumnPair {
        ColumnPair {
            parent_attr: parent.to_owned(),
            child_column: child.to_owned(),
        }
    }

    #[test]
    fn composite_key_join() {
        let j = parse_where("assetnum = :assetnum and siteid = :siteid", "ASSET").unwrap();
        assert_eq!(
            j.pairs,
            vec![pair("ASSETNUM", "ASSETNUM"), pair("SITEID", "SITEID")]
        );
        assert_eq!(j.filters, Vec::new());
    }

    #[test]
    fn different_names_either_orientation_any_case_and_whitespace() {
        let j = parse_where(
            "  :LOCATION=Location\n\tAND   Siteid =:siteid ",
            "LOCATIONS",
        )
        .unwrap();
        assert_eq!(
            j.pairs,
            vec![pair("LOCATION", "LOCATION"), pair("SITEID", "SITEID")]
        );
        let j = parse_where("parentwo = :wonum", "WORKORDER").unwrap();
        assert_eq!(j.pairs, vec![pair("WONUM", "PARENTWO")]);
    }

    #[test]
    fn constant_filters_are_kept_apart_from_the_join() {
        let j = parse_where(
            "status = 'ACTIVE' and wonum = :wonum and 3 = priority",
            "WORKORDER",
        )
        .unwrap();
        assert_eq!(j.pairs, vec![pair("WONUM", "WONUM")]);
        assert_eq!(
            j.filters,
            vec![
                Filter {
                    child_column: "STATUS".into(),
                    literal: Literal::Text("ACTIVE".into())
                },
                Filter {
                    child_column: "PRIORITY".into(),
                    literal: Literal::Number("3".into())
                },
            ]
        );
    }

    #[test]
    fn quoted_text_never_confuses_the_parser() {
        let j = parse_where("note = 'a AND b :c (x) OR ''q'' = ' and id = :id", "T").unwrap();
        assert_eq!(
            j.filters[0].literal,
            Literal::Text("a AND b :c (x) OR 'q' = ".into())
        );
        assert_eq!(j.pairs, vec![pair("ID", "ID")]);
    }

    #[test]
    fn child_qualifier_accepted_other_qualifier_refused() {
        let j = parse_where("asset.assetnum = :assetnum", "asset").unwrap();
        assert_eq!(j.pairs, vec![pair("ASSETNUM", "ASSETNUM")]);
        assert_eq!(
            parse_where("other.x = :x", "ASSET"),
            Err(Unsupported::ForeignQualifier("OTHER".into()))
        );
    }

    #[test]
    fn duplicates_collapse() {
        let j = parse_where("a = :a and a = :a and :a = a", "T").unwrap();
        assert_eq!(j.pairs.len(), 1);
    }

    #[test]
    fn unsupported_constructs_are_named_not_guessed() {
        let cases: &[(&str, Unsupported)] = &[
            ("", Unsupported::Empty),
            ("   \n ", Unsupported::Empty),
            ("a = :a or b = :b", Unsupported::Disjunction),
            ("exists (select 1 from x)", Unsupported::Subquery),
            ("a = (select 1)", Unsupported::Grouping),
            ("upper(a) = :a", Unsupported::FunctionCall),
            ("(a = :a)", Unsupported::Grouping),
            ("a <> :a", Unsupported::NonEquality("<>".into())),
            ("a >= :a", Unsupported::NonEquality(">=".into())),
            ("a like 'x%'", Unsupported::NonEquality("LIKE".into())),
            ("a in ('x')", Unsupported::NonEquality("IN".into())),
            ("a is null", Unsupported::NullTest),
            ("a = :&USERNAME&", Unsupported::SystemVariable),
            (":a = :b", Unsupported::TwoParentAttributes),
            ("a = b", Unsupported::TwoChildColumns),
            (":a = 'x'", Unsupported::NoChildColumn),
            ("'x' = 'y'", Unsupported::NoChildColumn),
            ("status = 'ACTIVE'", Unsupported::NoJoinPredicate),
        ];
        for (clause, want) in cases {
            assert_eq!(
                parse_where(clause, "T").unwrap_err(),
                *want,
                "clause: {clause:?}"
            );
        }
    }

    #[test]
    fn malformed_text_is_a_syntax_error() {
        for clause in [
            "a = ",
            "= :a",
            "a = :a and",
            "and a = :a",
            "a = :a and and b = :b",
            "a = 'open",
            "a = :",
            "a = :1x",
            "a = @b",
            "a : b",
        ] {
            assert!(
                matches!(parse_where(clause, "T"), Err(Unsupported::Syntax(_))),
                "clause: {clause:?}"
            );
        }
    }

    #[test]
    fn extraction_keeps_every_row_accounted_for() {
        let row = |name: &str, parent: &str, child: &str, w: &str| RelationshipRow {
            name: name.into(),
            parent: parent.into(),
            child: child.into(),
            where_clause: w.into(),
        };
        let rows = vec![
            row(
                "asset",
                "workorder",
                "asset",
                "assetnum = :assetnum and siteid = :siteid",
            ),
            row("odd", "workorder", "asset", "assetnum = :assetnum or 1 = 1"),
            row("", "", "asset", "x = :x"),
        ];
        let e = extract(&rows);
        assert_eq!((e.links.len(), e.unsupported.len(), e.total()), (1, 2, 3));
        assert_eq!(e.links[0].name, "ASSET");
        assert_eq!(e.links[0].parent_object, "WORKORDER");
        assert_eq!(e.unsupported[0].1, Unsupported::Disjunction);
        assert_eq!(e.unsupported[1].1, Unsupported::MissingObject);
    }

    #[test]
    fn generated_conjunctions_round_trip() {
        let names = [
            "WONUM", "SITEID", "ORGID", "ASSETNUM", "LOCATION", "PARENT", "STATUS", "X1", "A_B",
        ];
        let mut rng = Rng::new(2026);
        for _ in 0..3000 {
            let n = 1 + usize::try_from(rng.below(4)).unwrap();
            let mut text = String::new();
            let mut want_pairs: Vec<ColumnPair> = Vec::new();
            let mut want_filters = 0;
            for k in 0..n {
                let child = names[usize::try_from(rng.below(names.len() as u64)).unwrap()];
                let parent = names[usize::try_from(rng.below(names.len() as u64)).unwrap()];
                let lower = rng.chance(1, 2);
                let c = if lower {
                    child.to_ascii_lowercase()
                } else {
                    child.to_owned()
                };
                let p = if lower {
                    parent.to_ascii_lowercase()
                } else {
                    parent.to_owned()
                };
                let sep = [" ", "  ", "\n", "\t"][usize::try_from(rng.below(4)).unwrap()];
                if k > 0 {
                    text.push_str(sep);
                    text.push_str(if rng.chance(1, 2) { "and" } else { "AND" });
                    text.push_str(sep);
                }
                match rng.below(3) {
                    0 => text.push_str(&format!("{c}{sep}={sep}:{p}")),
                    1 => text.push_str(&format!(":{p} = {c}")),
                    _ => {
                        text.push_str(&format!("{c} = 'v{k}'"));
                        want_filters += 1;
                        continue;
                    }
                }
                let pr = pair(parent, child);
                if !want_pairs.contains(&pr) {
                    want_pairs.push(pr);
                }
            }
            match parse_where(&text, "T") {
                Ok(j) => {
                    assert_eq!(j.pairs, want_pairs, "clause: {text:?}");
                    assert_eq!(j.filters.len(), want_filters, "clause: {text:?}");
                }
                Err(e) => assert!(
                    want_pairs.is_empty() && e == Unsupported::NoJoinPredicate,
                    "clause {text:?}: {e}"
                ),
            }
        }
    }

    #[test]
    fn injecting_an_unsupported_construct_is_always_caught() {
        let injections = [
            " or x = :x",
            " and (y = :y)",
            " and f(z) = :z",
            " and w <> :w",
            " and q is null",
        ];
        let mut rng = Rng::new(7);
        for _ in 0..500 {
            let inj = injections[usize::try_from(rng.below(injections.len() as u64)).unwrap()];
            let clause = format!("assetnum = :assetnum{inj}");
            assert!(
                parse_where(&clause, "ASSET").is_err(),
                "accepted {clause:?}"
            );
        }
    }

    #[test]
    fn arbitrary_text_never_panics() {
        let alphabet: Vec<char> = "abcXYZ019_:&'\"()=<>!. \n\t$#,;@%".chars().collect();
        let mut rng = Rng::new(99);
        for _ in 0..20_000 {
            let len = usize::try_from(rng.below(40)).unwrap();
            let s: String = (0..len)
                .map(|_| alphabet[usize::try_from(rng.below(alphabet.len() as u64)).unwrap()])
                .collect();
            let _ = parse_where(&s, "T");
        }
    }
}
