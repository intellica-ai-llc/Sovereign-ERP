//! Statement chain rules (ARC42 v7 §5.2): per-agent seq/prev, intent/outcome pairing, orphans.
use crate::{
    canonical::{canonicalize, CanonError},
    hash::{sha256, to_hex},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    #[error("statement {0}: missing or invalid field `{1}`")]
    Field(usize, &'static str),
    #[error("statement {0}: seq {1} but expected {2}")]
    SeqGap(usize, u64, u64),
    #[error("statement {0}: prev hash does not match previous statement of this agent")]
    BadPrev(usize),
    #[error("statement {0}: outcome references unknown intent")]
    UnknownIntent(usize),
    #[error("statement {0}: duplicate outcome for intent")]
    DuplicateOutcome(usize),
    #[error("statement {0}: outcome from a different agent than its intent")]
    AgentMismatch(usize),
    #[error("statement {0}: schema violation: {1}")]
    Schema(usize, &'static str),
}

/// Identifier of a statement: hex SHA-256 of its canonical JSON bytes.
pub fn statement_id(v: &Value) -> Result<String, CanonError> {
    Ok(to_hex(&sha256(&canonicalize(v)?)))
}

const KINDS: [&str; 10] = [
    "intent",
    "outcome",
    "policy",
    "delegation",
    "artifact",
    "attestation",
    "lifecycle",
    "approval",
    "settlement",
    "governance",
];

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Schema constants from spec/statement.schema.json that the chain rules rely on.
/// (`seq` and `prev` forms are enforced by the chain checks below and reported as Field/SeqGap/BadPrev.)
fn check_schema(i: usize, s: &Value) -> Result<(), ChainError> {
    if s.get("v").and_then(Value::as_u64) != Some(2) {
        return Err(ChainError::Schema(i, "v must be 2"));
    }
    match s.get("kind").and_then(Value::as_str) {
        Some(k) if KINDS.contains(&k) => {}
        _ => return Err(ChainError::Schema(i, "unknown kind")),
    }
    match s.pointer("/agent/id").and_then(Value::as_str) {
        Some(id) if !id.is_empty() => {}
        _ => return Err(ChainError::Schema(i, "agent.id must be a non-empty string")),
    }
    if !s.get("ts").is_some_and(Value::is_string) {
        return Err(ChainError::Schema(i, "ts must be a string"));
    }
    if s.get("kind").and_then(Value::as_str) == Some("outcome")
        && !s
            .pointer("/pair/intent_id")
            .and_then(Value::as_str)
            .is_some_and(is_hex64)
    {
        return Err(ChainError::Schema(
            i,
            "outcome requires pair.intent_id (64 lowercase hex)",
        ));
    }
    Ok(())
}

/// Result of validating a whole log's worth of statements.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Intent ids with no outcome (orphans). Caller applies the timeout policy.
    pub open_intents: Vec<String>,
}

/// A statement that passed all checks against a `ChainState` and may be committed.
#[derive(Debug, Clone)]
pub struct Checked {
    agent: String,
    seq: u64,
    id: String,
    kind: String,
    intent_id: Option<String>,
}

impl Checked {
    /// The statement id (hex SHA-256 of canonical bytes).
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Incremental chain validator: `check` is pure, `commit` applies a checked statement.
/// The sequencer calls `check` before the durable append and `commit` after it succeeds.
#[derive(Debug, Default, Clone)]
pub struct ChainState {
    last: HashMap<String, (u64, String)>,
    intents: HashMap<String, String>, // intent id -> agent
    order: Vec<String>,
    closed: HashSet<String>,
}

impl ChainState {
    pub fn new() -> Self {
        Self::default()
    }

    /// (seq, prev) the agent's next statement must carry: (0, 64 zeros) for an agent not seen yet.
    pub fn next_for(&self, agent: &str) -> (u64, String) {
        match self.last.get(agent) {
            Some((seq, id)) => (seq + 1, id.clone()),
            None => (0, "0".repeat(64)),
        }
    }

    /// Check statement number `i` (its position, used in error values) against current state.
    pub fn check(&self, i: usize, s: &Value) -> Result<Checked, ChainError> {
        let agent = s
            .pointer("/agent/id")
            .and_then(Value::as_str)
            .ok_or(ChainError::Field(i, "agent.id"))?
            .to_string();
        let seq = s
            .get("seq")
            .and_then(Value::as_u64)
            .ok_or(ChainError::Field(i, "seq"))?;
        let prev = s
            .get("prev")
            .and_then(Value::as_str)
            .ok_or(ChainError::Field(i, "prev"))?;
        let kind = s
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(ChainError::Field(i, "kind"))?;
        check_schema(i, s)?;

        let (want_seq, want_prev) = match self.last.get(&agent) {
            None => (0, "0".repeat(64)),
            Some((n, id)) => (n + 1, id.clone()),
        };
        if seq != want_seq {
            return Err(ChainError::SeqGap(i, seq, want_seq));
        }
        if prev != want_prev {
            return Err(ChainError::BadPrev(i));
        }
        let id = statement_id(s).map_err(|_| ChainError::Field(i, "canonicalization"))?;

        let mut intent_id = None;
        if kind == "outcome" {
            let iid = s
                .pointer("/pair/intent_id")
                .and_then(Value::as_str)
                .ok_or(ChainError::Field(i, "pair.intent_id"))?;
            let owner = self.intents.get(iid).ok_or(ChainError::UnknownIntent(i))?;
            if *owner != agent {
                return Err(ChainError::AgentMismatch(i));
            }
            if self.closed.contains(iid) {
                return Err(ChainError::DuplicateOutcome(i));
            }
            intent_id = Some(iid.to_string());
        }
        Ok(Checked {
            agent,
            seq,
            id,
            kind: kind.to_string(),
            intent_id,
        })
    }

    /// Apply a statement that `check` accepted against this same state.
    pub fn commit(&mut self, c: Checked) {
        match c.kind.as_str() {
            "intent" => {
                self.intents.insert(c.id.clone(), c.agent.clone());
                self.order.push(c.id.clone());
            }
            "outcome" => {
                if let Some(iid) = c.intent_id {
                    self.closed.insert(iid);
                }
            }
            _ => {}
        }
        self.last.insert(c.agent, (c.seq, c.id));
    }

    /// Intents with no outcome yet, in log order.
    pub fn open_intents(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| !self.closed.contains(*id))
            .cloned()
            .collect()
    }

    /// Last (seq, id) committed for an agent.
    pub fn last_of(&self, agent: &str) -> Option<&(u64, String)> {
        self.last.get(agent)
    }
}

/// Validate statements in log order. The first statement of an agent must have
/// seq 0 and prev = 64 zeros; later ones seq = last+1 and prev = id of the last.
pub fn validate(stmts: &[Value]) -> Result<Report, ChainError> {
    let mut st = ChainState::new();
    for (i, s) in stmts.iter().enumerate() {
        let c = st.check(i, s)?;
        st.commit(c);
    }
    Ok(Report {
        open_intents: st.open_intents(),
    })
}
