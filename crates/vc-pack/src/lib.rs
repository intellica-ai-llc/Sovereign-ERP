//! Evidence-pack builder (C5). Collects agent statements, builds the log, has the operator and
//! witnesses sign one checkpoint, and emits a pack that `vc-verify` / `vcv` accepts.
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vc_core::canonical::canonicalize;
use vc_core::chain::statement_id;
use vc_core::hash::leaf_hash;
use vc_log::checkpoint::{note, try_sign_line, Checkpoint};

const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, thiserror::Error)]
pub enum PackBuildError {
    #[error("unknown agent {0}")]
    UnknownAgent(String),
    #[error("empty pack")]
    Empty,
    #[error("invalid statement: {0}")]
    Statement(String),
    #[error("invalid key name")]
    BadName,
}

struct AgentState {
    sk: SigningKey,
    seq: u64,
    prev: String,
}

pub struct PackBuilder {
    origin: String,
    operator: (String, SigningKey),
    witnesses: Vec<(String, SigningKey)>,
    min_witnesses: u32,
    agents: BTreeMap<String, AgentState>,
    coses: Vec<Vec<u8>>,
}

impl PackBuilder {
    pub fn new(origin: &str, operator: (&str, SigningKey)) -> Self {
        Self {
            origin: origin.into(),
            operator: (operator.0.into(), operator.1),
            witnesses: vec![],
            min_witnesses: 0,
            agents: BTreeMap::new(),
            coses: vec![],
        }
    }
    pub fn witness(mut self, name: &str, sk: SigningKey) -> Self {
        self.witnesses.push((name.into(), sk));
        self.min_witnesses = self.witnesses.len() as u32;
        self
    }
    pub fn min_witnesses(mut self, n: u32) -> Self {
        self.min_witnesses = n;
        self
    }
    pub fn agent(&mut self, id: &str, sk: SigningKey) {
        self.agents.insert(
            id.into(),
            AgentState {
                sk,
                seq: 0,
                prev: ZERO.into(),
            },
        );
    }
    pub fn len(&self) -> usize {
        self.coses.len()
    }
    pub fn is_empty(&self) -> bool {
        self.coses.is_empty()
    }
    pub fn next_seq(&self, agent: &str) -> Option<u64> {
        self.agents.get(agent).map(|a| a.seq)
    }

    /// Append one statement for `agent`; `extra` fields (action, pair, ext, ...) are merged in.
    /// Returns the statement id (use it as `pair.intent_id` for the matching outcome).
    pub fn statement(
        &mut self,
        agent: &str,
        kind: &str,
        ts: &str,
        extra: Value,
    ) -> Result<String, PackBuildError> {
        let a = self
            .agents
            .get_mut(agent)
            .ok_or_else(|| PackBuildError::UnknownAgent(agent.into()))?;
        let mut v = json!({"v":2,"kind":kind,"agent":{"id":agent},"seq":a.seq,"prev":a.prev,
            "ts":ts,"completeness":"C1","assurance":["A0"]});
        let extra = extra
            .as_object()
            .ok_or_else(|| PackBuildError::Statement("extra must be an object".into()))?;
        for (k, x) in extra {
            v[k] = x.clone();
        }
        vc_verify_free_check(&v)?;
        let id = statement_id(&v).map_err(|e| PackBuildError::Statement(e.to_string()))?;
        let payload = canonicalize(&v).map_err(|e| PackBuildError::Statement(e.to_string()))?;
        self.coses.push(vc_cose::sign1(
            &a.sk,
            agent.as_bytes(),
            "application/vc-statement+json",
            &payload,
        ));
        a.seq += 1;
        a.prev = id.clone();
        Ok(id)
    }

    /// Epoch seal: a lifecycle statement whose count equals the agent's statement count so far.
    pub fn seal(&mut self, agent: &str, epoch: &str, ts: &str) -> Result<String, PackBuildError> {
        let count = self
            .agents
            .get(agent)
            .ok_or_else(|| PackBuildError::UnknownAgent(agent.into()))?
            .seq;
        self.statement(
            agent,
            "lifecycle",
            ts,
            json!({"ext":{"seal":{"epoch":epoch,"count":count}}}),
        )
    }

    /// The pack JSON. `policy` is embedded as-is.
    pub fn build(&self, policy: Value) -> Result<Value, PackBuildError> {
        if self.coses.is_empty() {
            return Err(PackBuildError::Empty);
        }
        let leaves: Vec<_> = self.coses.iter().map(|c| leaf_hash(c)).collect();
        let cp = Checkpoint {
            origin: self.origin.clone(),
            size: leaves.len() as u64,
            root: vc_log::root(&leaves),
        };
        let text = cp.text();
        let mut lines = vec![try_sign_line(&text, &self.operator.0, &self.operator.1)
            .map_err(|_| PackBuildError::BadName)?];
        for (n, k) in &self.witnesses {
            lines.push(try_sign_line(&text, n, k).map_err(|_| PackBuildError::BadName)?);
        }
        let signed = note(&text, &lines);
        let entries: Vec<Value> = self
            .coses
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let proof = vc_log::inclusion_proof(&leaves, i).expect("index in range");
                json!({"statement_cose_hex": hex::encode(c),
                    "receipt": {"v":1,"origin":self.origin,"leaf_index":i,"tree_size":leaves.len(),
                        "leaf_hash":hex::encode(leaves[i]),
                        "inclusion_proof":proof.iter().map(hex::encode).collect::<Vec<_>>(),
                        "checkpoint":signed}})
            })
            .collect();
        Ok(json!({"v":1,"trust":self.trust(),"entries":entries,"policy":policy}))
    }

    /// Trust anchors as a relying party would pin them (separate from the pack).
    pub fn trust(&self) -> Value {
        let pk = |k: &SigningKey| hex::encode(k.verifying_key().to_bytes());
        json!({"origin":self.origin,
            "operator":{"name":self.operator.0,"pubkey_hex":pk(&self.operator.1)},
            "witnesses":self.witnesses.iter().map(|(n,k)| json!({"name":n,"pubkey_hex":pk(k)})).collect::<Vec<_>>(),
            "min_witnesses":self.min_witnesses,
            "issuers":self.agents.iter().map(|(id,a)| json!({"kid_hex":hex::encode(id),"pubkey_hex":pk(&a.sk),"max_completeness":"C2"})).collect::<Vec<_>>()})
    }
}

fn vc_verify_free_check(v: &Value) -> Result<(), PackBuildError> {
    if v["agent"]["id"].as_str().is_none_or(str::is_empty) {
        return Err(PackBuildError::Statement("agent id empty".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn k(n: u8) -> SigningKey {
        SigningKey::from_bytes(&[n; 32])
    }
    fn builder() -> PackBuilder {
        let mut b = PackBuilder::new("vc.example/t", ("op", k(100)))
            .witness("w1", k(101))
            .witness("w2", k(102));
        b.agent("a", k(1));
        b
    }
    fn policy() -> Value {
        json!({"min_completeness":"C1","min_assurance":"A0","max_open_intents":0})
    }

    #[test]
    fn built_pack_verifies_and_matches_pinned_trust() {
        let mut b = builder();
        let i = b
            .statement("a", "intent", "2026-11-01T00:00:00Z", json!({}))
            .unwrap();
        b.statement(
            "a",
            "outcome",
            "2026-11-01T00:00:01Z",
            json!({"pair":{"intent_id":i}}),
        )
        .unwrap();
        b.seal("a", "e1", "2026-11-01T00:00:02Z").unwrap();
        let p = b.build(policy()).unwrap();
        let bytes = serde_json::to_vec(&p).unwrap();
        let pins = vc_verify::Pins {
            trust: Some(serde_json::from_value(b.trust()).unwrap()),
            policy: Some(serde_json::from_value(policy()).unwrap()),
            require_seals: true,
        };
        let r = vc_verify::verify_pack_bytes(&bytes, Some(&pins));
        assert!(r.ok, "{:#?}", r.checks);
    }

    #[test]
    fn empty_unknown_agent_and_bad_names_are_errors() {
        assert!(matches!(
            builder().build(policy()),
            Err(PackBuildError::Empty)
        ));
        assert!(matches!(
            builder().statement("zz", "intent", "2026-11-01T00:00:00Z", json!({})),
            Err(PackBuildError::UnknownAgent(_))
        ));
        let mut b = PackBuilder::new("o", ("bad name", k(100)));
        b.agent("a", k(1));
        b.statement("a", "intent", "2026-11-01T00:00:00Z", json!({}))
            .unwrap();
        assert!(matches!(b.build(policy()), Err(PackBuildError::BadName)));
    }

    #[test]
    fn open_intent_fails_policy() {
        let mut b = builder();
        b.statement("a", "intent", "2026-11-01T00:00:00Z", json!({}))
            .unwrap();
        let p = b.build(policy()).unwrap();
        let r = vc_verify::verify_pack_bytes(&serde_json::to_vec(&p).unwrap(), None);
        assert!(!r.ok);
    }

    fn gated(b: &mut PackBuilder) -> String {
        b.statement(
            "a",
            "intent",
            "2026-11-01T00:00:00Z",
            json!({"ext":{"requires_approval":true}}),
        )
        .unwrap()
    }
    fn approval(b: &mut PackBuilder, who: &str, id: &str, dec: &str) {
        b.statement(
            who,
            "approval",
            "2026-11-01T00:00:01Z",
            json!({"ext":{"approval":{"action_id":id,"decision":dec}}}),
        )
        .unwrap();
    }
    fn outcome(b: &mut PackBuilder, id: &str) {
        b.statement(
            "a",
            "outcome",
            "2026-11-01T00:00:02Z",
            json!({"pair":{"intent_id":id}}),
        )
        .unwrap();
    }
    fn names(b: &PackBuilder) -> (bool, Vec<String>) {
        let p = b.build(policy()).unwrap();
        let r = vc_verify::verify_pack_bytes(&serde_json::to_vec(&p).unwrap(), None);
        let bad = r
            .checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| c.name.clone())
            .collect();
        (
            r.checks.iter().any(|c| c.name == "approval_order" && c.ok),
            bad,
        )
    }
    fn two() -> PackBuilder {
        let mut b = builder();
        b.agent("human", k(2));
        b
    }

    #[test]
    fn approval_before_outcome_passes() {
        let mut b = two();
        let i = gated(&mut b);
        approval(&mut b, "human", &i, "approve");
        outcome(&mut b, &i);
        let (ran, bad) = names(&b);
        assert!(ran && !bad.iter().any(|n| n == "approval_order"), "{bad:?}");
    }

    #[test]
    fn outcome_without_or_before_approval_fails() {
        let mut b = two();
        let i = gated(&mut b);
        outcome(&mut b, &i);
        assert!(names(&b).1.contains(&"approval_order".to_string()));
        let mut b = two();
        let i = gated(&mut b);
        outcome(&mut b, &i);
        approval(&mut b, "human", &i, "approve"); // too late: log order decides, not timestamps
        assert!(names(&b).1.contains(&"approval_order".to_string()));
    }

    #[test]
    fn denial_and_self_approval_fail() {
        let mut b = two();
        let i = gated(&mut b);
        approval(&mut b, "human", &i, "deny");
        outcome(&mut b, &i);
        assert!(names(&b).1.contains(&"approval_order".to_string()));
        let mut b = two();
        let i = gated(&mut b);
        approval(&mut b, "a", &i, "approve");
        outcome(&mut b, &i);
        assert!(names(&b).1.contains(&"approval_order".to_string()));
        // approve, then a later denial, then outcome: the latest decision wins
        let mut b = two();
        let i = gated(&mut b);
        approval(&mut b, "human", &i, "approve");
        approval(&mut b, "human", &i, "deny");
        outcome(&mut b, &i);
        assert!(names(&b).1.contains(&"approval_order".to_string()));
    }

    #[test]
    fn ungated_intents_do_not_add_the_check() {
        let mut b = two();
        let i = b
            .statement("a", "intent", "2026-11-01T00:00:00Z", json!({}))
            .unwrap();
        outcome(&mut b, &i);
        assert!(!names(&b).0);
    }

    #[test]
    fn pinned_gated_class_forces_approval_even_if_issuer_did_not_mark_it() {
        let mut b = two();
        let i = b
            .statement(
                "a",
                "intent",
                "2026-11-01T00:00:00Z",
                json!({"ext":{"action_class":"payment"}}),
            )
            .unwrap();
        outcome(&mut b, &i);
        let p = b.build(policy()).unwrap();
        let bytes = serde_json::to_vec(&p).unwrap();
        // without a pinned class list the issuer's omission goes unnoticed
        assert!(vc_verify::verify_pack_bytes(&bytes, None).ok);
        // the relying party pins "payment" as gated: the same pack now fails, naming the check
        let mut pol = policy();
        pol["gated_classes"] = json!(["payment"]);
        let pins = vc_verify::Pins {
            trust: Some(serde_json::from_value(b.trust()).unwrap()),
            policy: Some(serde_json::from_value(pol).unwrap()),
            require_seals: false,
        };
        let r = vc_verify::verify_pack_bytes(&bytes, Some(&pins));
        assert!(!r.ok);
        assert!(r.checks.iter().any(|c| c.name == "approval_order" && !c.ok));
        assert!(r.checks.iter().any(|c| c.name == "policy_pinned" && !c.ok));
    }
}
