//! Strict serde types for Receipt v0 (C4) and Evidence pack v0 (C5).
//!
//! Closed contracts: every struct denies unknown fields; duplicate keys are rejected by serde; hex is strict
//! lowercase `[0-9a-f]` with explicit length bounds; levels are closed enums. Shape bounds that serde cannot
//! express (counts, string lengths) are enforced by `validate_shape`, which `verify_pack` always re-runs so a
//! pack built in code gets the same treatment as one parsed from JSON.
use crate::limits::*;
use crate::strict::hex_strict;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;

/// Lowercase hex string decoding to at most `MAX` bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hex<const MAX: usize>(pub Vec<u8>);

impl<const MAX: usize> Serialize for Hex<MAX> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(&self.0))
    }
}
impl<'de, const MAX: usize> Deserialize<'de> for Hex<MAX> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        hex_strict(&s, MAX).map(Hex).map_err(D::Error::custom)
    }
}

/// Lowercase hex string decoding to exactly 32 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hex32(pub [u8; 32]);

impl Serialize for Hex32 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(self.0))
    }
}
impl<'de> Deserialize<'de> for Hex32 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        crate::strict::hex32(&s)
            .map(Hex32)
            .map_err(D::Error::custom)
    }
}

/// Completeness level `C0 < C1 < C2 < C3`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Completeness {
    C0,
    C1,
    C2,
    C3,
}
impl Completeness {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "C0" => Self::C0,
            "C1" => Self::C1,
            "C2" => Self::C2,
            "C3" => Self::C3,
            _ => return None,
        })
    }
}

/// Assurance level `A0 < A1 < A2 < A3`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Assurance {
    A0,
    A1,
    A2,
    A3,
}
impl Assurance {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "A0" => Self::A0,
            "A1" => Self::A1,
            "A2" => Self::A2,
            "A3" => Self::A3,
            _ => return None,
        })
    }
}

/// A named Ed25519 public key (operator or witness).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedKey {
    pub name: String,
    pub pubkey_hex: Hex32,
}

/// A trusted statement issuer.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issuer {
    pub kid_hex: Hex<MAX_KID_BYTES>,
    pub pubkey_hex: Hex32,
    pub max_completeness: Completeness,
}

/// Trust anchors of a pack (C5 `trust`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trust {
    pub origin: String,
    pub operator: NamedKey,
    pub witnesses: Vec<NamedKey>,
    pub min_witnesses: u32,
    pub issuers: Vec<Issuer>,
}

/// Receipt v0 (C4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub v: u32,
    pub origin: String,
    pub leaf_index: u64,
    pub tree_size: u64,
    pub leaf_hash: Hex32,
    pub inclusion_proof: Vec<Hex32>,
    pub checkpoint: String,
}

/// One statement plus its receipt.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub statement_cose_hex: Hex<MAX_STATEMENT_BYTES>,
    pub receipt: Receipt,
}

/// Verification policy (C5 `policy`). All fields are required: a missing minimum must never default to "off".
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub min_completeness: Completeness,
    pub min_assurance: Assurance,
    pub max_open_intents: u32,
    /// Intent action classes (`ext.action_class`) that always need approval, whatever the issuer marked.
    /// Absent means none; a pinned list can only add to the pack's list, never remove from it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gated_classes: Vec<String>,
}

/// Evidence pack v0 (C5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePack {
    pub v: u32,
    pub trust: Trust,
    pub entries: Vec<Entry>,
    pub policy: Policy,
}

/// Single-receipt bundle for `vcv verify-receipt`: a receipt, its statement and the trust anchors.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptBundle {
    pub v: u32,
    pub trust: Trust,
    pub statement_cose_hex: Hex<MAX_STATEMENT_BYTES>,
    pub receipt: Receipt,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PackError {
    #[error("input is {0} bytes, over the {MAX_PACK_BYTES}-byte limit")]
    TooLarge(usize),
    #[error("input is not valid UTF-8")]
    NotUtf8,
    #[error("malformed JSON or contract violation: {0}")]
    Json(String),
    #[error("{0}")]
    Shape(String),
}

/// Reject what serde_json silently tolerates but another implementation would read differently: duplicate object
/// keys (serde keeps the last), lone surrogates, non-I-JSON numbers, trailing data (T-L0-04 parser differentials).
/// Uses the same strict parser as statement canonicalisation (vc-core).
fn strict_json(s: &str) -> Result<(), PackError> {
    vc_core::canonical::canonicalize_str(s)
        .map(|_| ())
        .map_err(|e| PackError::Json(format!("not strict I-JSON: {e}")))
}

fn ok_text(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_NAME_BYTES && !s.chars().any(char::is_control)
}

impl Trust {
    pub fn validate_shape(&self) -> Result<(), String> {
        if !ok_text(&self.origin) {
            return Err("trust.origin: must be 1..=256 bytes without control characters".into());
        }
        if self.witnesses.len() > MAX_WITNESSES {
            return Err(format!("trust.witnesses: more than {MAX_WITNESSES}"));
        }
        if self.issuers.len() > MAX_ISSUERS {
            return Err(format!("trust.issuers: more than {MAX_ISSUERS}"));
        }
        if self.min_witnesses as usize > MAX_WITNESSES {
            return Err(format!("trust.min_witnesses: more than {MAX_WITNESSES}"));
        }
        for k in std::iter::once(&self.operator).chain(self.witnesses.iter()) {
            if !vc_log::checkpoint::valid_name(&k.name) || k.name.len() > MAX_NAME_BYTES {
                return Err("trust: key name must be 1..=256 bytes, no whitespace, '+' or control characters".into());
            }
        }
        for i in &self.issuers {
            if i.kid_hex.0.is_empty() {
                return Err("trust.issuers: kid_hex must not be empty".into());
            }
        }
        Ok(())
    }
}

impl Receipt {
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.v != 1 {
            return Err(format!("receipt.v: must be 1, got {}", self.v));
        }
        if !ok_text(&self.origin) {
            return Err("receipt.origin: must be 1..=256 bytes without control characters".into());
        }
        if self.inclusion_proof.len() > MAX_PROOF_HASHES {
            return Err(format!(
                "receipt.inclusion_proof: more than {MAX_PROOF_HASHES} hashes"
            ));
        }
        if self.checkpoint.len() > MAX_CHECKPOINT_BYTES {
            return Err(format!(
                "receipt.checkpoint: more than {MAX_CHECKPOINT_BYTES} bytes"
            ));
        }
        Ok(())
    }
}

impl EvidencePack {
    /// Parse a pack from JSON text (size-capped, strict) and validate its shape.
    pub fn from_json(s: &str) -> Result<Self, PackError> {
        if s.len() > MAX_PACK_BYTES {
            return Err(PackError::TooLarge(s.len()));
        }
        strict_json(s)?;
        let p: Self = serde_json::from_str(s).map_err(|e| PackError::Json(e.to_string()))?;
        p.validate_shape().map_err(PackError::Shape)?;
        Ok(p)
    }

    pub fn validate_shape(&self) -> Result<(), String> {
        if self.v != 1 {
            return Err(format!("pack.v: must be 1, got {}", self.v));
        }
        self.trust.validate_shape()?;
        if self.entries.is_empty() {
            return Err("pack.entries: empty pack (nothing to verify)".into());
        }
        if self.entries.len() > MAX_ENTRIES {
            return Err(format!("pack.entries: more than {MAX_ENTRIES}"));
        }
        let mut notes: HashSet<&str> = HashSet::new();
        for (i, e) in self.entries.iter().enumerate() {
            if e.statement_cose_hex.0.is_empty() {
                return Err(format!("entries/{i}: empty statement"));
            }
            e.receipt
                .validate_shape()
                .map_err(|m| format!("entries/{i}: {m}"))?;
            notes.insert(e.receipt.checkpoint.as_str());
        }
        if notes.len() > MAX_DISTINCT_CHECKPOINTS {
            return Err(format!(
                "pack: more than {MAX_DISTINCT_CHECKPOINTS} distinct checkpoints"
            ));
        }
        Ok(())
    }
}

impl ReceiptBundle {
    pub fn from_json(s: &str) -> Result<Self, PackError> {
        if s.len() > MAX_PACK_BYTES {
            return Err(PackError::TooLarge(s.len()));
        }
        strict_json(s)?;
        let b: Self = serde_json::from_str(s).map_err(|e| PackError::Json(e.to_string()))?;
        if b.v != 1 {
            return Err(PackError::Shape(format!(
                "bundle.v: must be 1, got {}",
                b.v
            )));
        }
        b.trust.validate_shape().map_err(PackError::Shape)?;
        b.receipt.validate_shape().map_err(PackError::Shape)?;
        Ok(b)
    }
}
