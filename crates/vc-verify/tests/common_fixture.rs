//! Shared fixture for builder_* tests (FR-L0-10 / FR-L0-11): builds a fully valid signed pack with vc-cose,
//! vc-log (tree + checkpoint note) and an operator plus witnesses. Not a test file by itself.
#![allow(dead_code)]
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use vc_core::canonical::canonicalize;
use vc_core::hash::{sha256, to_hex};
use vc_log::checkpoint::{note, sign_line, Checkpoint};
use vc_verify::*;

pub const ORIGIN: &str = "log.example/test";
pub const KID: &[u8] = b"issuer-1";

pub fn sk(n: u8) -> SigningKey {
    SigningKey::from_bytes(&[n; 32])
}
pub fn pk(k: &SigningKey) -> Hex32 {
    Hex32(k.verifying_key().to_bytes())
}

#[derive(Clone)]
pub struct Cfg {
    pub origin: &'static str,
    pub completeness: &'static str,
    pub assurance: &'static str,
    pub open_tail: bool,
    pub issuer_max: Completeness,
}
impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            origin: ORIGIN,
            completeness: "C2",
            assurance: "A1",
            open_tail: false,
            issuer_max: Completeness::C2,
        }
    }
}

pub fn statement(
    agent: &str,
    seq: u64,
    prev: &str,
    kind: &str,
    intent: Option<&str>,
    cfg: &Cfg,
) -> (Vec<u8>, String) {
    let mut v = json!({"v":2,"kind":kind,"agent":{"id":agent},"seq":seq,"prev":prev,
        "ts":"2026-01-01T00:00:00Z","completeness":cfg.completeness,"assurance":[cfg.assurance]});
    if let Some(i) = intent {
        v["pair"] = json!({"intent_id": i});
    }
    let payload = canonicalize(&v).unwrap();
    let id = to_hex(&sha256(&payload));
    (
        vc_cose::sign1(&sk(1), KID, "application/json", &payload),
        id,
    )
}

pub fn statements(cfg: &Cfg) -> Vec<Vec<u8>> {
    let z = "0".repeat(64);
    let (a0, a0id) = statement("agent-a", 0, &z, "intent", None, cfg);
    let (b0, b0id) = statement("agent-b", 0, &z, "intent", None, cfg);
    let (a1, a1id) = statement("agent-a", 1, &a0id, "outcome", Some(&a0id), cfg);
    let (b1, _) = statement("agent-b", 1, &b0id, "outcome", Some(&b0id), cfg);
    let (a2, a2id) = statement("agent-a", 2, &a1id, "intent", None, cfg);
    let mut v = vec![a0, b0, a1, b1, a2];
    if !cfg.open_tail {
        v.push(statement("agent-a", 3, &a2id, "outcome", Some(&a2id), cfg).0);
    }
    v
}

pub fn cp_note(origin: &str, size: u64, root: [u8; 32], signers: &[(&str, &SigningKey)]) -> String {
    let text = Checkpoint {
        origin: origin.into(),
        size,
        root,
    }
    .text();
    let lines: Vec<String> = signers
        .iter()
        .map(|(n, k)| sign_line(&text, n, k))
        .collect();
    note(&text, &lines)
}

pub fn trust(cfg: &Cfg) -> Trust {
    Trust {
        origin: ORIGIN.into(),
        operator: NamedKey {
            name: "operator".into(),
            pubkey_hex: pk(&sk(2)),
        },
        witnesses: vec![
            NamedKey {
                name: "w1".into(),
                pubkey_hex: pk(&sk(3)),
            },
            NamedKey {
                name: "w2".into(),
                pubkey_hex: pk(&sk(4)),
            },
            NamedKey {
                name: "w3".into(),
                pubkey_hex: pk(&sk(5)),
            },
        ],
        min_witnesses: 2,
        issuers: vec![Issuer {
            kid_hex: Hex(KID.to_vec()),
            pubkey_hex: pk(&sk(1)),
            max_completeness: cfg.issuer_max,
        }],
    }
}

/// Build a pack whose checkpoint is signed by `signers` (name, key).
pub fn build_with(cfg: &Cfg, signers: &[(&str, &SigningKey)]) -> EvidencePack {
    let cose = statements(cfg);
    let refs: Vec<&[u8]> = cose.iter().map(Vec::as_slice).collect();
    let leaves = vc_log::leaves_from_entries(&refs);
    let root = vc_log::root(&leaves);
    let n = cose.len() as u64;
    let checkpoint = cp_note(cfg.origin, n, root, signers);
    let entries = cose
        .iter()
        .enumerate()
        .map(|(i, c)| Entry {
            statement_cose_hex: Hex(c.clone()),
            receipt: Receipt {
                v: 1,
                origin: cfg.origin.into(),
                leaf_index: i as u64,
                tree_size: n,
                leaf_hash: Hex32(leaves[i]),
                inclusion_proof: vc_log::inclusion_proof(&leaves, i)
                    .unwrap()
                    .into_iter()
                    .map(Hex32)
                    .collect(),
                checkpoint: checkpoint.clone(),
            },
        })
        .collect();
    EvidencePack {
        v: 1,
        trust: trust(cfg),
        entries,
        policy: Policy {
            min_completeness: Completeness::C1,
            min_assurance: Assurance::A0,
            max_open_intents: 0,
            gated_classes: vec![],
        },
    }
}

pub fn good_signers() -> Vec<(&'static str, SigningKey)> {
    vec![("operator", sk(2)), ("w1", sk(3)), ("w2", sk(4))]
}

pub fn build(cfg: &Cfg) -> EvidencePack {
    let s = good_signers();
    let r: Vec<(&str, &SigningKey)> = s.iter().map(|(n, k)| (*n, k)).collect();
    build_with(cfg, &r)
}

pub fn pack() -> EvidencePack {
    build(&Cfg::default())
}

pub fn to_json(p: &EvidencePack) -> String {
    serde_json::to_string(p).unwrap()
}

pub fn as_value(p: &EvidencePack) -> Value {
    serde_json::to_value(p).unwrap()
}

/// Change one base64 character in the signature part of the note line signed under `name`.
pub fn corrupt_sig(note: &str, name: &str) -> String {
    let prefix = format!("\u{2014} {name} ");
    let mut out = String::new();
    for line in note.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix(&prefix) {
            let mut b: Vec<char> = rest.chars().collect();
            b[20] = if b[20] == 'A' { 'B' } else { 'A' };
            out.push_str(&prefix);
            out.extend(b);
        } else {
            out.push_str(line);
        }
    }
    assert_ne!(out, note, "signature line for {name} not found");
    out
}

pub fn set_checkpoint(p: &mut EvidencePack, f: impl Fn(&str) -> String) {
    for e in &mut p.entries {
        e.receipt.checkpoint = f(&e.receipt.checkpoint);
    }
}

/// Assert the report is not ok and that exactly the named check (at least) failed.
pub fn fails(r: &Report, check: &str) {
    assert!(!r.ok, "report must not be ok: {}", r.to_json());
    let c = r
        .check(check)
        .unwrap_or_else(|| panic!("no check {check}: {}", r.to_json()));
    assert!(!c.ok, "check {check} should fail: {}", r.to_json());
}
pub fn passes(r: &Report, check: &str) {
    let c = r
        .check(check)
        .unwrap_or_else(|| panic!("no check {check}: {}", r.to_json()));
    assert!(c.ok, "check {check} should pass: {}", r.to_json());
}
