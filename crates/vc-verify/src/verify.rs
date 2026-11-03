//! Receipt and evidence-pack verification (FR-L0-11, FR-L0-10 verification side). Pure, deterministic, offline.
use crate::chain::{check_chain, ChainInput};
use crate::limits::*;
use crate::report::{aggregate, check_of, echo, Check, Report, Res};
use crate::schema::validate_statement;
use crate::types::*;
use ed25519_dalek::VerifyingKey;
use serde_json::Value;
use std::collections::HashMap;
use vc_core::hash::{leaf_hash, sha256, to_hex};
use vc_log::checkpoint::{verify_note, verify_note_for_origin, Checkpoint, KeyInfo};

const NOT_EVAL: &str = "not evaluated: ";

/// Trust anchors compiled to keys. Fails closed on any invalid key.
struct Compiled {
    origin: String,
    operator: KeyInfo,
    witnesses: Vec<KeyInfo>,
    min_witnesses: usize,
    issuers: Vec<(Vec<u8>, VerifyingKey, Completeness)>,
}

fn key(k: &NamedKey, what: &str) -> Result<KeyInfo, String> {
    let vk = VerifyingKey::from_bytes(&k.pubkey_hex.0)
        .map_err(|_| format!("{what} {}: not a valid Ed25519 key", echo(&k.name)))?;
    if vk.is_weak() {
        return Err(format!("{what} {}: weak (small-order) key", echo(&k.name)));
    }
    Ok(KeyInfo {
        name: k.name.clone(),
        key: vk,
    })
}

fn compile(t: &Trust) -> Result<Compiled, String> {
    t.validate_shape()?;
    let operator = key(&t.operator, "operator")?;
    let witnesses = t
        .witnesses
        .iter()
        .map(|w| key(w, "witness"))
        .collect::<Result<Vec<_>, _>>()?;
    let mut issuers: Vec<(Vec<u8>, VerifyingKey, Completeness)> = Vec::new();
    for i in &t.issuers {
        let vk = VerifyingKey::from_bytes(&i.pubkey_hex.0)
            .map_err(|_| "issuer: not a valid Ed25519 key".to_string())?;
        if vk.is_weak() {
            return Err("issuer: weak (small-order) key".into());
        }
        if issuers.iter().any(|(k, _, _)| *k == i.kid_hex.0) {
            return Err(format!(
                "issuer kid {} listed twice (ambiguous)",
                hex::encode(&i.kid_hex.0)
            ));
        }
        issuers.push((i.kid_hex.0.clone(), vk, i.max_completeness));
    }
    Ok(Compiled {
        origin: t.origin.clone(),
        operator,
        witnesses,
        min_witnesses: t.min_witnesses as usize,
        issuers,
    })
}

/// Parse the (unauthenticated) checkpoint text of a signed note, splitting exactly as `vc_log` does.
fn parse_note(note: &str) -> Result<Checkpoint, String> {
    let i = note
        .find("\n\n")
        .ok_or("checkpoint note has no blank line before signatures")?;
    Checkpoint::parse(&note[..=i]).map_err(|e| e.to_string())
}

struct CpEval {
    operator: Res,
    quorum: Res,
}

fn eval_checkpoint(note: &str, ct: &Compiled) -> CpEval {
    let parsed = parse_note(note);
    let operator = match verify_note(note, &ct.operator, &ct.witnesses, 0) {
        Err(e) => Err(e.to_string()),
        Ok(cp) => match &parsed {
            Ok(p) if *p == cp => {
                if cp.origin != ct.origin {
                    Err(format!(
                        "checkpoint origin {} is not the trusted origin",
                        echo(&cp.origin)
                    ))
                } else {
                    Ok(format!(
                        "operator {} signed checkpoint of size {}",
                        echo(&ct.operator.name),
                        cp.size
                    ))
                }
            }
            _ => Err("internal differential: checkpoint parse disagrees with vc-log".into()),
        },
    };
    let quorum = if ct.min_witnesses == 0 {
        Err("trust.min_witnesses is 0: a quorum of zero witnesses is no quorum".into())
    } else if let Err(e) = &operator {
        Err(format!(
            "{NOT_EVAL}operator signature not verified ({})",
            echo(e)
        ))
    } else {
        verify_note_for_origin(
            note,
            &ct.origin,
            &ct.operator,
            &ct.witnesses,
            ct.min_witnesses,
        )
        .map(|_| {
            format!(
                "at least {} independent witness keys cosigned, bound to origin",
                ct.min_witnesses
            )
        })
        .map_err(|e| e.to_string())
    };
    CpEval { operator, quorum }
}

/// An authenticated statement.
struct Stmt {
    value: Value,
    id: String,
    issuer_max: Completeness,
    completeness: Completeness,
    assurance: Assurance,
}

struct EntryEval {
    cose: Res,
    schema: Res,
    inclusion: Res,
    stmt: Option<Stmt>,
    leaf_index: u64,
    cp_key: String,
}

fn eval_entry(cose_bytes: &[u8], r: &Receipt, ct: &Compiled) -> EntryEval {
    // 1. COSE signature by a trusted issuer.
    let lookup = |kid: &[u8]| {
        ct.issuers
            .iter()
            .find(|(k, _, _)| k == kid)
            .map(|(_, v, _)| *v)
    };
    let verified = vc_cose::verify1(cose_bytes, lookup).map_err(|e| e.to_string());
    let cose = verified
        .as_ref()
        .map(|(kid, _)| format!("signed by trusted issuer kid {}", hex::encode(kid)))
        .map_err(Clone::clone);

    // 2. Statement payload: canonical (JCS) JSON conforming to the statement schema.
    let mut stmt = None;
    let schema = match &verified {
        Err(_) => Err(format!("{NOT_EVAL}signature not verified")),
        Ok((kid, payload)) => {
            parse_statement(payload).and_then(|(value, completeness, assurance)| {
                let issuer_max = ct
                    .issuers
                    .iter()
                    .find(|(k, _, _)| k == kid)
                    .map(|(_, _, m)| *m)
                    .ok_or("issuer vanished")?;
                stmt = Some(Stmt {
                    value,
                    id: to_hex(&sha256(payload)),
                    issuer_max,
                    completeness,
                    assurance,
                });
                Ok("canonical JSON conforming to statement schema".to_string())
            })
        }
    };

    // 3. Inclusion proof against the checkpoint's stated root (authenticity of the checkpoint is judged separately).
    let inclusion = eval_inclusion(cose_bytes, r, ct);
    EntryEval {
        cose,
        schema,
        inclusion,
        stmt,
        leaf_index: r.leaf_index,
        cp_key: r.checkpoint.clone(),
    }
}

fn parse_statement(payload: &[u8]) -> Result<(Value, Completeness, Assurance), String> {
    let text = std::str::from_utf8(payload).map_err(|_| "payload is not UTF-8".to_string())?;
    let canon = vc_core::canonical::canonicalize_str(text)
        .map_err(|e| format!("payload is not strict JSON: {e}"))?;
    if canon != payload {
        return Err("payload is not in RFC 8785 canonical form".into());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| format!("payload JSON: {e}"))?;
    validate_statement(&value).map_err(|e| echo(&e))?;
    let completeness = match value.get("completeness").and_then(Value::as_str) {
        None => Completeness::C0,
        Some(s) => Completeness::parse(s).ok_or("completeness not C0..C3")?,
    };
    let mut assurance = Assurance::A0;
    if let Some(a) = value.get("assurance").and_then(Value::as_array) {
        for x in a {
            assurance = assurance.max(
                x.as_str()
                    .and_then(Assurance::parse)
                    .ok_or("assurance not A0..A3")?,
            );
        }
    }
    Ok((value, completeness, assurance))
}

fn eval_inclusion(cose_bytes: &[u8], r: &Receipt, ct: &Compiled) -> Res {
    if r.origin != ct.origin {
        return Err(format!(
            "receipt origin {} is not the trusted origin",
            echo(&r.origin)
        ));
    }
    let leaf = leaf_hash(cose_bytes);
    if leaf != r.leaf_hash.0 {
        return Err("receipt leaf_hash is not SHA-256(0x00 || statement bytes)".into());
    }
    let cp = parse_note(&r.checkpoint)?;
    if cp.origin != r.origin {
        return Err("checkpoint origin differs from receipt origin".into());
    }
    if cp.size != r.tree_size {
        return Err(format!(
            "checkpoint size {} != receipt tree_size {}",
            cp.size, r.tree_size
        ));
    }
    let idx = usize::try_from(r.leaf_index)
        .map_err(|_| "leaf_index exceeds this platform's usize".to_string())?;
    let size = usize::try_from(r.tree_size)
        .map_err(|_| "tree_size exceeds this platform's usize".to_string())?;
    let path: Vec<[u8; 32]> = r.inclusion_proof.iter().map(|h| h.0).collect();
    vc_log::verify_inclusion(&leaf, idx, size, &path, &cp.root)
        .map(|_| format!("leaf {} included in tree of {}", r.leaf_index, r.tree_size))
        .map_err(|e| format!("inclusion proof: {e}"))
}

fn trust_check(t: &Result<Compiled, String>) -> Check {
    match t {
        Ok(_) => Check {
            name: "trust_anchors".into(),
            ok: true,
            detail: "operator, witness and issuer keys are valid".into(),
        },
        Err(e) => Check {
            name: "trust_anchors".into(),
            ok: false,
            detail: echo(e),
        },
    }
}

fn not_evaluated_report(first: Check, names: &[&str], why: &str) -> Report {
    let mut checks = vec![first];
    for n in names {
        checks.push(Check {
            name: (*n).into(),
            ok: false,
            detail: format!("{NOT_EVAL}{why}"),
        });
    }
    Report::new(checks, false)
}

const ALL_AFTER_FORMAT: [&str; 13] = [
    "trust_anchors",
    "cose_signature",
    "statement_schema",
    "receipt_inclusion",
    "checkpoint_operator",
    "checkpoint_witness_quorum",
    "checkpoint_agreement",
    "chain_rules",
    "partial_coverage",
    "completeness_ceiling",
    "policy_min_completeness",
    "policy_min_assurance",
    "open_intents",
];

/// Report for input that could not even be parsed into a pack.
pub fn malformed_report(msg: &str) -> Report {
    not_evaluated_report(
        Check {
            name: "pack_format".into(),
            ok: false,
            detail: echo(msg),
        },
        &ALL_AFTER_FORMAT,
        "pack is malformed",
    )
}

/// Verify one receipt against its statement bytes and trust anchors.
/// Checks: `trust_anchors`, `cose_signature`, `receipt_inclusion`, `checkpoint_operator`, `checkpoint_witness_quorum`.
pub fn verify_receipt(entry_cose: &[u8], receipt: &Receipt, trust: &Trust) -> Report {
    if entry_cose.len() > MAX_STATEMENT_BYTES {
        return malformed_report("statement larger than the statement limit");
    }
    if let Err(e) = receipt.validate_shape() {
        return malformed_report(&e);
    }
    let ct = compile(trust);
    let names = [
        "cose_signature",
        "receipt_inclusion",
        "checkpoint_operator",
        "checkpoint_witness_quorum",
    ];
    let c = match &ct {
        Ok(c) => c,
        Err(_) => return not_evaluated_report(trust_check(&ct), &names, "trust anchors invalid"),
    };
    let e = eval_entry(entry_cose, receipt, c);
    let cp = eval_checkpoint(&receipt.checkpoint, c);
    Report::new(
        vec![
            trust_check(&ct),
            check_of("cose_signature", e.cose),
            check_of("receipt_inclusion", e.inclusion),
            check_of("checkpoint_operator", cp.operator),
            check_of("checkpoint_witness_quorum", cp.quorum),
        ],
        false,
    )
}

/// Verifier-side pins. A pack carries its own trust anchors and policy, which proves nothing about who is
/// trusted; a relying party pins what it actually trusts and requires.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pins {
    /// If set, the pack's trust anchors must equal this exactly.
    pub trust: Option<Trust>,
    /// If set, the pack's policy must be at least this strict (minimums not lower, open-intent allowance not higher);
    /// the stricter of the two is enforced either way.
    pub policy: Option<Policy>,
    /// If true, every agent's chain in the pack must END with a valid epoch seal (a `lifecycle` statement whose
    /// `ext.seal.count` equals its own `seq`), and every seal anywhere in a chain must be truthful. This closes the
    /// tail-truncation hole: without it, deleting an agent's newest statements is invisible from the pack alone.
    pub require_seals: bool,
}

/// Approval ordering (separation of duties), judged by LOG ORDER, never by claimed time.
/// For every intent with `ext.requires_approval == true` that has an outcome: the latest approval statement
/// for that intent before the outcome must say "approve" and must come from a different agent than the intent's.
/// Input: (leaf_index, statement_id, value) for every statement in the pack.
fn check_approvals(stmts: &[(u64, String, Value)], classes: &[String]) -> Option<Check> {
    let mut sorted: Vec<&(u64, String, Value)> = stmts.iter().collect();
    sorted.sort_by_key(|s| s.0);
    let needs: HashMap<&str, &str> = sorted
        .iter()
        .filter(|s| {
            s.2.get("kind").and_then(Value::as_str) == Some("intent")
                && (s.2.pointer("/ext/requires_approval") == Some(&Value::Bool(true))
                    || s.2
                        .pointer("/ext/action_class")
                        .and_then(Value::as_str)
                        .is_some_and(|c| classes.iter().any(|x| x == c)))
        })
        .filter_map(|s| Some((s.1.as_str(), s.2.pointer("/agent/id")?.as_str()?)))
        .collect();
    if needs.is_empty() {
        return None;
    }
    let fail = |d: String| Check {
        name: "approval_order".into(),
        ok: false,
        detail: d,
    };
    // latest approval decision seen so far per intent: (decision_is_approve, approver agent)
    let mut latest: HashMap<&str, (bool, &str)> = HashMap::new();
    let mut checked = 0usize;
    for (_, _, v) in sorted.iter().map(|s| (&s.0, &s.1, &s.2)) {
        match v.get("kind").and_then(Value::as_str) {
            Some("approval") => {
                if let (Some(target), Some(dec), Some(who)) = (
                    v.pointer("/ext/approval/action_id").and_then(Value::as_str),
                    v.pointer("/ext/approval/decision").and_then(Value::as_str),
                    v.pointer("/agent/id").and_then(Value::as_str),
                ) {
                    if needs.contains_key(target) {
                        latest.insert(target, (dec == "approve", who));
                    }
                }
            }
            Some("outcome") => {
                let Some(iid) = v.pointer("/pair/intent_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(owner) = needs.get(iid) else {
                    continue;
                };
                checked += 1;
                match latest.get(iid) {
                    None => {
                        return Some(fail(format!(
                            "an outcome for intent {} appears in the log with no earlier approval",
                            echo(iid)
                        )))
                    }
                    Some((false, _)) => {
                        return Some(fail(format!(
                            "an outcome for intent {} follows a denial (or non-approval)",
                            echo(iid)
                        )))
                    }
                    Some((true, who)) if who == owner => {
                        return Some(fail(format!(
                            "intent {} was approved by the same agent that issued it",
                            echo(iid)
                        )))
                    }
                    Some(_) => {}
                }
            }
            _ => {}
        }
    }
    Some(Check {
        name: "approval_order".into(),
        ok: true,
        detail: format!(
            "{checked} outcome(s) of approval-gated intents each follow an approval by a different agent (log order)"
        ),
    })
}

/// Check epoch seals. A seal at seq N claims the agent emitted exactly N statements before it.
fn check_seals(stmts: &[(String, u64, Value)], require: bool) -> Check {
    let mut last: HashMap<&str, (u64, &Value)> = HashMap::new();
    for (agent, seq, v) in stmts {
        let is_seal = v.get("kind").and_then(Value::as_str) == Some("lifecycle")
            && v.pointer("/ext/seal").is_some();
        if is_seal {
            let ok = v.pointer("/ext/seal/count").and_then(Value::as_u64) == Some(*seq)
                && v.pointer("/ext/seal/epoch")
                    .and_then(Value::as_str)
                    .is_some_and(|e| !e.is_empty() && e.len() <= 64);
            if !ok {
                return Check {
                    name: "epoch_seal".into(),
                    ok: false,
                    detail: format!(
                        "agent {}: seal at seq {} is malformed or its count does not equal its seq",
                        echo(agent),
                        seq
                    ),
                };
            }
        }
        let e = last.entry(agent.as_str()).or_insert((*seq, v));
        if *seq >= e.0 {
            *e = (*seq, v);
        }
    }
    if require {
        for (agent, (seq, v)) in &last {
            let sealed = v.get("kind").and_then(Value::as_str) == Some("lifecycle")
                && v.pointer("/ext/seal").is_some();
            if !sealed {
                return Check {
                    name: "epoch_seal".into(),
                    ok: false,
                    detail: format!(
                        "agent {}: chain ends at seq {} without an epoch seal (tail may be truncated)",
                        echo(agent),
                        seq
                    ),
                };
            }
        }
    }
    Check {
        name: "epoch_seal".into(),
        ok: true,
        detail: format!(
            "{} agent chain(s) end in a valid seal{}",
            last.len(),
            if require {
                ""
            } else {
                " or seals are not required"
            }
        ),
    }
}

/// Verify an evidence pack (C5) against the trust anchors it carries (self-consistency only; see [`Pins`]).
pub fn verify_pack(pack: &EvidencePack) -> Report {
    verify_pack_inner(pack, None)
}

/// Verify a pack and additionally require that its embedded trust anchors equal `pinned` exactly.
pub fn verify_pack_pinned(pack: &EvidencePack, pinned: &Trust) -> Report {
    verify_pack_inner(
        pack,
        Some(&Pins {
            trust: Some(pinned.clone()),
            policy: None,
            require_seals: false,
        }),
    )
}

/// Verify a pack against verifier-side pins.
pub fn verify_pack_with(pack: &EvidencePack, pins: &Pins) -> Report {
    verify_pack_inner(pack, Some(pins))
}

fn verify_pack_inner(pack: &EvidencePack, pins: Option<&Pins>) -> Report {
    if let Err(e) = pack.validate_shape() {
        return malformed_report(&e);
    }
    let mut checks = vec![Check {
        name: "pack_format".into(),
        ok: true,
        detail: format!("v1 pack with {} entries", pack.entries.len()),
    }];
    let mut policy = pack.policy.clone();
    if let Some(pp) = pins.and_then(|p| p.policy.as_ref()) {
        let weaker = pack.policy.min_completeness < pp.min_completeness
            || pack.policy.min_assurance < pp.min_assurance
            || pack.policy.max_open_intents > pp.max_open_intents
            || pp
                .gated_classes
                .iter()
                .any(|c| !pack.policy.gated_classes.contains(c));
        for c in &pp.gated_classes {
            if !policy.gated_classes.contains(c) {
                policy.gated_classes.push(c.clone());
            }
        }
        policy.min_completeness = policy.min_completeness.max(pp.min_completeness);
        policy.min_assurance = policy.min_assurance.max(pp.min_assurance);
        policy.max_open_intents = policy.max_open_intents.min(pp.max_open_intents);
        checks.push(Check {
            name: "policy_pinned".into(),
            ok: !weaker,
            detail: if weaker {
                "pack policy is weaker than the pinned policy (the pinned policy is enforced below)"
                    .into()
            } else {
                "pack policy is at least as strict as the pinned policy".into()
            },
        });
    }
    if let Some(p) = pins.and_then(|p| p.trust.as_ref()) {
        let same = *p == pack.trust;
        checks.push(Check {
            name: "trust_pinned".into(),
            ok: same,
            detail: if same {
                "pack trust anchors equal the pinned trust".into()
            } else {
                "pack trust anchors differ from the pinned trust".into()
            },
        });
    }
    let ct = compile(&pack.trust);
    checks.push(trust_check(&ct));
    let Ok(ct) = ct else {
        let rest = &ALL_AFTER_FORMAT[1..];
        checks.extend(rest.iter().map(|n| Check {
            name: (*n).into(),
            ok: false,
            detail: format!("{NOT_EVAL}trust anchors invalid"),
        }));
        return Report::new(checks, false);
    };

    let evals: Vec<EntryEval> = pack
        .entries
        .iter()
        .map(|e| eval_entry(&e.statement_cose_hex.0, &e.receipt, &ct))
        .collect();
    let mut cps: HashMap<&str, CpEval> = HashMap::new();
    for e in &pack.entries {
        cps.entry(e.receipt.checkpoint.as_str())
            .or_insert_with(|| eval_checkpoint(&e.receipt.checkpoint, &ct));
    }

    let col = |f: &dyn Fn(&EntryEval) -> Res| evals.iter().map(f).collect::<Vec<Res>>();
    checks.push(aggregate("cose_signature", &col(&|e| e.cose.clone())));
    checks.push(aggregate("statement_schema", &col(&|e| e.schema.clone())));
    checks.push(aggregate(
        "receipt_inclusion",
        &col(&|e| e.inclusion.clone()),
    ));
    checks.push(aggregate(
        "checkpoint_operator",
        &col(&|e| cps[e.cp_key.as_str()].operator.clone()),
    ));
    checks.push(aggregate(
        "checkpoint_witness_quorum",
        &col(&|e| cps[e.cp_key.as_str()].quorum.clone()),
    ));
    checks.push(checkpoint_agreement(&evals));

    let unauth = |e: &EntryEval| -> Option<Res> {
        e.stmt.is_none().then(|| {
            Err(format!(
                "{NOT_EVAL}statement not authenticated or schema-invalid"
            ))
        })
    };

    // Chain rules (needs every statement authenticated).
    let mut partial = false;
    let mut open: Option<Vec<String>> = None;
    let chain_check = if let Some(e) = evals.iter().find_map(unauth) {
        check_of("chain_rules", e)
    } else {
        let inputs: Vec<ChainInput<'_>> = evals
            .iter()
            .filter_map(|e| {
                e.stmt.as_ref().map(|s| ChainInput {
                    leaf_index: e.leaf_index,
                    value: &s.value,
                    id: &s.id,
                })
            })
            .collect();
        let out = check_chain(&inputs);
        partial = !out.partial_agents.is_empty();
        if out.result.is_ok() {
            open = Some(out.open_intents);
        }
        check_of("chain_rules", out.result)
    };
    checks.push(chain_check);
    if pins.is_some_and(|p| p.require_seals) {
        checks.push(if evals.iter().any(|e| e.stmt.is_none()) {
            Check {
                name: "epoch_seal".into(),
                ok: false,
                detail: format!("{NOT_EVAL}statement not authenticated or schema-invalid"),
            }
        } else {
            let all: Vec<(String, u64, Value)> = evals
                .iter()
                .filter_map(|e| e.stmt.as_ref())
                .filter_map(|s| {
                    Some((
                        s.value.pointer("/agent/id")?.as_str()?.to_string(),
                        s.value.get("seq")?.as_u64()?,
                        s.value.clone(),
                    ))
                })
                .collect();
            check_seals(&all, true)
        });
    }
    if evals.iter().all(|e| e.stmt.is_some()) {
        let all: Vec<(u64, String, Value)> = evals
            .iter()
            .filter_map(|e| {
                e.stmt
                    .as_ref()
                    .map(|s| (e.leaf_index, s.id.clone(), s.value.clone()))
            })
            .collect();
        if let Some(c) = check_approvals(&all, &policy.gated_classes) {
            checks.push(c);
        }
    }
    checks.push(Check {
        name: "partial_coverage".into(),
        ok: true, // informational by design: never fails, never silent (see detail and Report::partial_coverage)
        detail: if partial {
            "informational: at least one agent's run does not start at seq 0; earlier statements are not covered by this pack".into()
        } else {
            "informational: every agent's run starts at seq 0, or chain_rules did not pass".into()
        },
    });

    let per = |f: &dyn Fn(&Stmt) -> Res| -> Vec<Res> {
        evals
            .iter()
            .map(|e| {
                e.stmt
                    .as_ref()
                    .map_or_else(|| unauth(e).unwrap_or(Err("unreachable".into())), f)
            })
            .collect()
    };
    checks.push(aggregate(
        "completeness_ceiling",
        &per(&|s| {
            if s.completeness <= s.issuer_max {
                Ok(format!(
                    "{:?} within issuer ceiling {:?}",
                    s.completeness, s.issuer_max
                ))
            } else {
                Err(format!(
                    "statement claims {:?} above issuer ceiling {:?}",
                    s.completeness, s.issuer_max
                ))
            }
        }),
    ));
    let (minc, mina) = (policy.min_completeness, policy.min_assurance);
    checks.push(aggregate(
        "policy_min_completeness",
        &per(&|s| {
            if s.completeness >= minc {
                Ok(format!("{:?} >= {:?}", s.completeness, minc))
            } else {
                Err(format!(
                    "completeness {:?} below policy minimum {:?}",
                    s.completeness, minc
                ))
            }
        }),
    ));
    checks.push(aggregate(
        "policy_min_assurance",
        &per(&|s| {
            if s.assurance >= mina {
                Ok(format!("{:?} >= {:?}", s.assurance, mina))
            } else {
                Err(format!(
                    "assurance {:?} below policy minimum {:?}",
                    s.assurance, mina
                ))
            }
        }),
    ));
    checks.push(match open {
        None => Check { name: "open_intents".into(), ok: false, detail: format!("{NOT_EVAL}chain_rules failed") },
        Some(o) => {
            let max = policy.max_open_intents as usize;
            let mut d = format!("{} open intent(s), policy allows {}", o.len(), max);
            if partial {
                d.push_str(" (partial coverage: outcomes outside the pack are not visible, so this count can only be an overcount)");
            }
            Check { name: "open_intents".into(), ok: o.len() <= max, detail: d }
        }
    });
    Report::new(checks, partial)
}

/// Same checkpoint size with different roots among the receipts of one pack is a split view (T-L0-01).
fn checkpoint_agreement(evals: &[EntryEval]) -> Check {
    let mut by_size: HashMap<u64, [u8; 32]> = HashMap::new();
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for e in evals {
        if seen.insert(e.cp_key.as_str(), ()).is_some() {
            continue;
        }
        let Ok(cp) = parse_note(&e.cp_key) else {
            continue;
        };
        if let Some(prev) = by_size.insert(cp.size, cp.root) {
            if prev != cp.root {
                return Check {
                    name: "checkpoint_agreement".into(),
                    ok: false,
                    detail: format!("split view: two different roots for tree size {}", cp.size),
                };
            }
        }
    }
    Check {
        name: "checkpoint_agreement".into(),
        ok: true,
        detail: format!(
            "{} distinct checkpoint size(s), no conflicting roots",
            by_size.len()
        ),
    }
}

/// Parse and verify a pack from bytes (size cap and UTF-8 enforced). Never panics.
pub fn verify_pack_bytes(bytes: &[u8], pins: Option<&Pins>) -> Report {
    if bytes.len() > MAX_PACK_BYTES {
        return malformed_report(&PackError::TooLarge(bytes.len()).to_string());
    }
    let Ok(s) = std::str::from_utf8(bytes) else {
        return malformed_report(&PackError::NotUtf8.to_string());
    };
    match EvidencePack::from_json(s) {
        Ok(p) => verify_pack_inner(&p, pins),
        Err(e) => malformed_report(&e.to_string()),
    }
}

/// Parse and verify a receipt bundle from bytes.
pub fn verify_receipt_bytes(bytes: &[u8]) -> Report {
    if bytes.len() > MAX_PACK_BYTES {
        return malformed_report(&PackError::TooLarge(bytes.len()).to_string());
    }
    let Ok(s) = std::str::from_utf8(bytes) else {
        return malformed_report(&PackError::NotUtf8.to_string());
    };
    match ReceiptBundle::from_json(s) {
        Ok(b) => verify_receipt(&b.statement_cose_hex.0, &b.receipt, &b.trust),
        Err(e) => malformed_report(&e.to_string()),
    }
}
