//! vc-seq: the sequencer core (ARC42 v9 §5.1, FR-L0-07).
//!
//! A submission is a COSE_Sign1 statement. It is accepted only if: the signature verifies under a registered
//! issuer key (strict COSE rules), the payload is exactly canonical RFC 8785 JSON, the statement is bound to the
//! issuer's agent id and does not claim more completeness than the issuer class allows, and the per-agent chain
//! rules hold. Accepted statements are appended durably (entry bundle first) before a registration is returned.
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde_json::Value;
use std::collections::HashMap;
use vc_core::canonical::{canonicalize_str, CanonError};
use vc_core::chain::{ChainError, ChainState};
use vc_core::hash::{leaf_hash, to_hex, Hash};
use vc_cose::CoseError;
use vc_log::checkpoint::{note, sign_line, Checkpoint};
use vc_store::{StoreError, TileLog, TileStore};

/// Maximum accepted statement size (COSE bytes); bounded by the tile entry framing (u16 length prefix).
pub const MAX_STATEMENT_BYTES: usize = u16::MAX as usize;

#[derive(Debug, thiserror::Error)]
pub enum SeqError {
    #[error("statement too large")]
    TooLarge,
    #[error("cose: {0}")]
    Cose(#[from] CoseError),
    #[error("payload is not canonical RFC 8785 JSON")]
    NotCanonical,
    #[error("payload is not valid I-JSON: {0}")]
    Json(#[from] CanonError),
    #[error("chain rule violated: {0}")]
    Chain(#[from] ChainError),
    #[error("statement agent id does not match the issuer's registered agent")]
    AgentBinding,
    #[error("statement claims completeness {claimed} above issuer maximum {max}")]
    Completeness { claimed: String, max: String },
    #[error("statement claims assurance {claimed} above issuer maximum {max}")]
    Assurance { claimed: String, max: String },
    #[error("statement violates the schema: {0}")]
    Schema(String),
    #[error("invalid sequencer configuration: {0}")]
    Config(String),
    #[error("sequencer is poisoned (state could not be reconciled with the log; reopen): {0}")]
    Poisoned(String),
    #[error("storage: {0}")]
    Store(#[from] StoreError),
}

/// A registered issuer: key, the single agent id it may speak for, and its completeness (C0..C3) and assurance
/// (A0..A3) ceilings, each 0..=3.
#[derive(Clone)]
pub struct Issuer {
    pub key: VerifyingKey,
    pub agent_id: String,
    pub max_completeness: u8,
    pub max_assurance: u8,
}

#[derive(Default, Clone)]
pub struct Registry {
    by_kid: HashMap<Vec<u8>, Issuer>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    /// Register (or replace) an issuer; returns the previous registration for this kid, if any.
    pub fn register(&mut self, kid: &[u8], issuer: Issuer) -> Option<Issuer> {
        self.by_kid.insert(kid.to_vec(), issuer)
    }
    fn lookup(&self, kid: &[u8]) -> Option<&Issuer> {
        self.by_kid.get(kid)
    }
}

/// Durable statements by leaf hash: index and statement id.
type ByLeaf = HashMap<Hash, (u64, Hash)>;

/// What a client receives when a statement is registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub index: u64,
    pub leaf_hash: Hash,
    pub statement_id: String,
    /// True if this exact statement had already been registered (idempotent retry).
    pub duplicate: bool,
}

pub struct Sequencer<S: TileStore> {
    log: TileLog<S>,
    chain: ChainState,
    registry: Registry,
    /// Durable statements by leaf hash (byte-identical retries): index and statement id.
    by_leaf: ByLeaf,
    poisoned: Option<String>,
    operator: SigningKey,
    operator_name: String,
    origin: String,
}

fn completeness_rank(s: &str) -> Option<u8> {
    match s {
        "C0" => Some(0),
        "C1" => Some(1),
        "C2" => Some(2),
        "C3" => Some(3),
        _ => None,
    }
}

fn assurance_rank(s: &str) -> Option<u8> {
    match s {
        "A0" => Some(0),
        "A1" => Some(1),
        "A2" => Some(2),
        "A3" => Some(3),
        _ => None,
    }
}

fn from_hex(s: &str) -> Option<Hash> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, c) in s.as_bytes().chunks(2).enumerate() {
        let d = |x: u8| (x as char).to_digit(16).map(|v| v as u8);
        out[i] = d(c[0])? * 16 + d(c[1])?;
    }
    Some(out)
}

impl<S: TileStore> Sequencer<S> {
    /// Open a sequencer over a store. Replays all entries (each verified against the tile hashes) to rebuild chain
    /// state; trusts its own log for signatures and registry (the registry may have rotated since).
    pub fn open(
        store: S,
        registry: Registry,
        operator: SigningKey,
        operator_name: &str,
        origin: &str,
    ) -> Result<Self, SeqError> {
        if !vc_log::checkpoint::valid_name(operator_name) {
            return Err(SeqError::Config(format!(
                "operator name {operator_name:?} is not a valid signed-note key name"
            )));
        }
        if origin.is_empty() || origin.trim() != origin || origin.chars().any(char::is_control) {
            return Err(SeqError::Config(format!(
                "origin {origin:?} cannot be carried by a checkpoint note"
            )));
        }
        let log = TileLog::open(store)?;
        let mut s = Sequencer {
            log,
            chain: ChainState::new(),
            registry,
            by_leaf: HashMap::new(),
            poisoned: None,
            operator,
            operator_name: operator_name.to_string(),
            origin: origin.to_string(),
        };
        let (chain, by_leaf) = s.rebuild()?;
        s.chain = chain;
        s.by_leaf = by_leaf;
        Ok(s)
    }

    /// Rebuild chain state from the durable log into fresh values (the live state is untouched on error).
    fn rebuild(&self) -> Result<(ChainState, ByLeaf), SeqError> {
        let mut chain = ChainState::new();
        let mut by_leaf = HashMap::new();
        self.log.scan(0, |i, e| {
            let (_, payload) = vc_cose::peek_payload(e)?;
            let v: Value = serde_json::from_slice(&payload).map_err(|_| SeqError::NotCanonical)?;
            let c = chain.check(i as usize, &v)?;
            let id = from_hex(c.id()).ok_or(SeqError::NotCanonical)?;
            by_leaf.insert(leaf_hash(e), (i, id));
            chain.commit(c);
            Ok::<(), SeqError>(())
        })?;
        Ok((chain, by_leaf))
    }

    /// Register (or replace) an issuer on a live sequencer (hosted agent onboarding). Statements already
    /// in the log are unaffected; the new registration applies to later submissions.
    pub fn register_issuer(&mut self, kid: &[u8], issuer: Issuer) -> Option<Issuer> {
        self.registry.register(kid, issuer)
    }

    /// (seq, prev) the next statement of `agent` must carry (from the reconciled chain state).
    pub fn next_for(&self, agent: &str) -> (u64, String) {
        self.chain.next_for(agent)
    }

    pub fn size(&self) -> u64 {
        self.log.size()
    }

    pub fn log(&self) -> &TileLog<S> {
        &self.log
    }

    /// Open intents of the durable log; empty while poisoned (state unknown until reconciled).
    pub fn open_intents(&self) -> Vec<String> {
        if self.poisoned.is_some() {
            return Vec::new();
        }
        self.chain.open_intents()
    }

    /// Validate one submission against the current state without changing anything.
    fn validate(
        &self,
        cose: &[u8],
        index_hint: usize,
    ) -> Result<(vc_core::chain::Checked, Value), SeqError> {
        if cose.len() > MAX_STATEMENT_BYTES {
            return Err(SeqError::TooLarge);
        }
        let reg = &self.registry;
        let (kid, payload) = vc_cose::verify1(cose, |kid| reg.lookup(kid).map(|i| i.key))?;
        let issuer = reg.lookup(&kid).ok_or(CoseError::Kid)?.clone();
        let text = std::str::from_utf8(&payload).map_err(|_| SeqError::NotCanonical)?;
        let canon = canonicalize_str(text)?;
        if canon != payload {
            return Err(SeqError::NotCanonical);
        }
        let v: Value = serde_json::from_slice(&payload).map_err(|_| SeqError::NotCanonical)?;
        if v.pointer("/agent/id").and_then(Value::as_str) != Some(issuer.agent_id.as_str()) {
            return Err(SeqError::AgentBinding);
        }
        // completeness / assurance: schema-typed, and not above the issuer class ceiling (ARC42 s5.2 rule 6)
        if let Some(c) = v.get("completeness") {
            let rank = c.as_str().and_then(completeness_rank);
            match rank {
                Some(r) if r <= issuer.max_completeness => {}
                _ => {
                    return Err(SeqError::Completeness {
                        claimed: c.to_string(),
                        max: format!("C{}", issuer.max_completeness),
                    })
                }
            }
        }
        if let Some(a) = v.get("assurance") {
            let items = a
                .as_array()
                .ok_or_else(|| SeqError::Schema("assurance must be an array of A0..A3".into()))?;
            for item in items {
                match item.as_str().and_then(assurance_rank) {
                    None => {
                        return Err(SeqError::Schema(format!(
                            "assurance item {item} is not one of A0..A3"
                        )))
                    }
                    Some(r) if r > issuer.max_assurance => {
                        return Err(SeqError::Assurance {
                            claimed: item.to_string(),
                            max: format!("A{}", issuer.max_assurance),
                        })
                    }
                    Some(_) => {}
                }
            }
        }
        let checked = self.chain.check(index_hint, &v)?;
        Ok((checked, v))
    }

    /// Register one statement (durable before return).
    pub fn submit(&mut self, cose: &[u8]) -> Result<Registration, SeqError> {
        self.submit_batch(&[cose]).pop().expect("one result")
    }

    /// Register a batch with group commit: each statement is validated in order against the state left by the
    /// accepted ones before it; accepted statements are appended in one durable write. Results are per statement.
    /// A byte-identical retry (already durable, or earlier in the same batch) is acknowledged as a duplicate.
    pub fn submit_batch(&mut self, batch: &[&[u8]]) -> Vec<Result<Registration, SeqError>> {
        if self.poisoned.is_some() {
            // try to reconcile with the durable log again (the failure that poisoned us may have been transient)
            match self.rebuild() {
                Ok((chain, by_leaf)) => {
                    self.chain = chain;
                    self.by_leaf = by_leaf;
                    self.poisoned = None;
                }
                Err(e) => {
                    let m = e.to_string();
                    self.poisoned = Some(m.clone());
                    return batch
                        .iter()
                        .map(|_| Err(SeqError::Poisoned(m.clone())))
                        .collect();
                }
            }
        }
        type Acc<'a> = (usize, &'a [u8], vc_core::chain::Checked, Hash);
        let mut results: Vec<Option<Result<Registration, SeqError>>> =
            (0..batch.len()).map(|_| None).collect();
        let mut accepted: Vec<Acc> = Vec::new();
        let mut pending: HashMap<Hash, usize> = HashMap::new();
        let mut in_batch_dups: Vec<(usize, usize)> = Vec::new();
        let base = self.log.size();
        for (i, cose) in batch.iter().enumerate() {
            // cheapest check first: never parse or hash-canonicalise an oversize, unauthenticated payload
            if cose.len() > MAX_STATEMENT_BYTES {
                results[i] = Some(Err(SeqError::TooLarge));
                continue;
            }
            let lh = leaf_hash(cose);
            if let Some((idx, id)) = self.by_leaf.get(&lh) {
                results[i] = Some(Ok(Registration {
                    index: *idx,
                    leaf_hash: lh,
                    statement_id: to_hex(id),
                    duplicate: true,
                }));
                continue;
            }
            if let Some(&pos) = pending.get(&lh) {
                in_batch_dups.push((i, pos));
                continue;
            }
            let next_index = base + accepted.len() as u64;
            match self.validate(cose, next_index as usize) {
                Ok((checked, _)) => {
                    // make the accepted statement visible to later statements in this batch
                    self.chain.commit(checked.clone());
                    pending.insert(lh, accepted.len());
                    accepted.push((i, cose, checked, lh));
                }
                Err(e) => results[i] = Some(Err(e)),
            }
        }
        if !accepted.is_empty() {
            let entries: Vec<&[u8]> = accepted.iter().map(|a| a.1).collect();
            match self.log.append_batch(&entries) {
                Ok((first, _)) => {
                    let reg = |n: usize, dup: bool| {
                        let (_, _, checked, lh) = &accepted[n];
                        Registration {
                            index: first + n as u64,
                            leaf_hash: *lh,
                            statement_id: checked.id().to_string(),
                            duplicate: dup,
                        }
                    };
                    for (n, (i, _, checked, lh)) in accepted.iter().enumerate() {
                        if let Some(id) = from_hex(checked.id()) {
                            self.by_leaf.insert(*lh, (first + n as u64, id));
                        }
                        results[*i] = Some(Ok(reg(n, false)));
                    }
                    for (i, pos) in &in_batch_dups {
                        results[*i] = Some(Ok(reg(*pos, true)));
                    }
                }
                Err(e) => {
                    // Nothing became durable (TileLog::append_batch is transactional). Reconcile the speculative
                    // chain state with the log; if that is impossible, refuse everything until reopen.
                    let msg = e.to_string();
                    match self.rebuild() {
                        Ok((chain, by_leaf)) => {
                            self.chain = chain;
                            self.by_leaf = by_leaf;
                        }
                        Err(re) => {
                            // drop the speculative (non-durable) state so nothing stale can be observed
                            self.chain = ChainState::new();
                            self.by_leaf.clear();
                            self.poisoned = Some(re.to_string());
                        }
                    }
                    let failed = accepted
                        .iter()
                        .map(|a| a.0)
                        .chain(in_batch_dups.iter().map(|d| d.0));
                    for i in failed {
                        results[i] = Some(Err(SeqError::Store(StoreError::Io(
                            std::io::Error::other(msg.clone()),
                        ))));
                    }
                }
            }
        }
        results
            .into_iter()
            .map(|r| r.expect("every statement has a result"))
            .collect()
    }

    /// Sign a checkpoint for the current tree (operator signature only; witnesses cosign separately).
    /// Refused while the in-memory state could not be reconciled with the log (see `submit_batch`).
    pub fn checkpoint(&self) -> Result<(Checkpoint, String), SeqError> {
        if let Some(m) = &self.poisoned {
            return Err(SeqError::Poisoned(m.clone()));
        }
        let cp = Checkpoint {
            origin: self.origin.clone(),
            size: self.log.size(),
            root: self.log.root()?,
        };
        let text = cp.text();
        let n = note(
            &text,
            &[sign_line(&text, &self.operator_name, &self.operator)],
        );
        Ok((cp, n))
    }
}
