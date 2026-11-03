//! Signed checkpoints in the C2SP tlog-checkpoint / signed-note text format.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use vc_core::Hash;

const SIG_TYPE_ED25519: u8 = 0x01;
const SIG_TYPE_COSIG: u8 = 0x04;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointError {
    #[error("malformed checkpoint")]
    Malformed,
    #[error("quorum not met: {got} of {need} required cosigners valid")]
    Quorum { got: usize, need: usize },
    #[error("operator signature missing or invalid")]
    NoOperator,
    #[error("checkpoint origin does not match the expected log")]
    WrongOrigin,
    #[error("invalid key name (empty, whitespace, or '+')")]
    BadName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub origin: String,
    pub size: u64,
    pub root: Hash,
}

impl Checkpoint {
    /// Note text: origin, decimal size, base64 root; each line ends in "\n".
    pub fn text(&self) -> String {
        format!(
            "{}\n{}\n{}\n",
            self.origin,
            self.size,
            B64.encode(self.root)
        )
    }

    pub fn parse(text: &str) -> Result<Self, CheckpointError> {
        let mut l = text
            .strip_suffix('\n')
            .ok_or(CheckpointError::Malformed)?
            .split('\n');
        let origin = l
            .next()
            .filter(|s| !s.is_empty())
            .ok_or(CheckpointError::Malformed)?
            .to_string();
        let size_s = l.next().ok_or(CheckpointError::Malformed)?;
        let size = size_s
            .parse::<u64>()
            .ok()
            .filter(|n| n.to_string() == size_s)
            .ok_or(CheckpointError::Malformed)?;
        let root: Hash = B64
            .decode(l.next().ok_or(CheckpointError::Malformed)?)
            .map_err(|_| CheckpointError::Malformed)?
            .try_into()
            .map_err(|_| CheckpointError::Malformed)?;
        Ok(Checkpoint { origin, size, root })
    }
}

/// A trusted verifier key: name + Ed25519 public key.
#[derive(Clone)]
pub struct KeyInfo {
    pub name: String,
    pub key: VerifyingKey,
}

/// Key ID per signed-note: SHA-256(name || 0x0A || type || pubkey)[:4].
pub fn key_id(name: &str, key: &VerifyingKey) -> [u8; 4] {
    let mut h = Sha256::new();
    h.update(name.as_bytes());
    h.update([0x0a, SIG_TYPE_ED25519]);
    h.update(key.as_bytes());
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

/// Cosignature key ID (C2SP tlog-cosignature/v1): SHA-256(name || 0x0A || 0x04 || pubkey)[:4].
pub fn cosig_key_id(name: &str, key: &VerifyingKey) -> [u8; 4] {
    let mut h = Sha256::new();
    h.update(name.as_bytes());
    h.update([0x0a, SIG_TYPE_COSIG]);
    h.update(key.as_bytes());
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

/// The exact bytes a witness signs for a timestamped cosignature over checkpoint body `cp_text`.
pub fn cosig_message(ts: u64, cp_text: &str) -> Vec<u8> {
    format!("cosignature/v1\ntime {ts}\n{cp_text}").into_bytes()
}

/// signed-note key names: non-empty, no whitespace, no '+'.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c == '+' || c.is_control())
}

/// Strict variant: refuses invalid key names.
pub fn try_sign_line(text: &str, name: &str, sk: &SigningKey) -> Result<String, CheckpointError> {
    if !valid_name(name) {
        return Err(CheckpointError::BadName);
    }
    Ok(sign_line_unchecked(text, name, sk))
}

/// Produce one signature line ("— name base64(keyid||sig)\n") over `text`. Invalid characters in
/// `name` that are control characters (including newlines) are replaced with '_' so a hostile name can never inject extra lines (the resulting
/// line will not verify under the intended key name); use `try_sign_line` to reject names that
/// violate signed-note rules (empty, whitespace, '+') instead. Verification stays lenient about spaces.
pub fn sign_line(text: &str, name: &str, sk: &SigningKey) -> String {
    let safe: String = name
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let safe = if safe.is_empty() {
        "_".to_string()
    } else {
        safe
    };
    sign_line_unchecked(text, &safe, sk)
}

fn sign_line_unchecked(text: &str, name: &str, sk: &SigningKey) -> String {
    let sig = sk.sign(text.as_bytes());
    let mut raw = key_id(name, &sk.verifying_key()).to_vec();
    raw.extend_from_slice(&sig.to_bytes());
    format!("\u{2014} {} {}\n", name, B64.encode(raw))
}

/// Full signed note: text, blank line, signature lines.
pub fn note(text: &str, sig_lines: &[String]) -> String {
    format!("{}\n{}", text, sig_lines.concat())
}

/// Split a note into (text, signature lines).
fn split_note(n: &str) -> Result<(&str, Vec<&str>), CheckpointError> {
    let i = n.find("\n\n").ok_or(CheckpointError::Malformed)?;
    let (text, rest) = (&n[..=i], &n[i + 2..]);
    Ok((text, rest.lines().collect()))
}

/// (name, public key) pairs among `keys` whose signature on `text` verifies.
/// Weak (small-order) keys never count; verification is strict.
fn valid_signers(
    text: &str,
    lines: &[&str],
    keys: &[KeyInfo],
    allow_cosig: bool,
) -> Vec<(String, [u8; 32])> {
    let mut ok: Vec<(String, [u8; 32])> = Vec::new();
    for line in lines {
        let Some(rest) = line.strip_prefix("\u{2014} ") else {
            continue;
        };
        let Some((name, b64)) = rest.rsplit_once(' ') else {
            continue;
        };
        let Ok(raw) = B64.decode(b64) else { continue };
        let cosig = raw.len() == 4 + 8 + 64;
        if raw.len() != 4 + 64 && !(allow_cosig && cosig) {
            continue;
        }
        for k in keys.iter().filter(|k| k.name == name && !k.key.is_weak()) {
            if cosig {
                // timestamped witness cosignature (tlog-cosignature/v1): its own key-id type and message
                let Ok(ts) = <[u8; 8]>::try_from(&raw[4..12]) else {
                    continue;
                };
                let Ok(sig) = Signature::from_slice(&raw[12..]) else {
                    continue;
                };
                if raw[..4] == cosig_key_id(&k.name, &k.key)
                    && k.key
                        .verify_strict(&cosig_message(u64::from_be_bytes(ts), text), &sig)
                        .is_ok()
                {
                    let e = (k.name.clone(), k.key.to_bytes());
                    if !ok.contains(&e) {
                        ok.push(e);
                    }
                }
                continue;
            }
            if raw[..4] != key_id(&k.name, &k.key) {
                continue;
            }
            let Ok(sig) = Signature::from_slice(&raw[4..]) else {
                continue;
            };
            if k.key.verify_strict(text.as_bytes(), &sig).is_ok() {
                let e = (k.name.clone(), k.key.to_bytes());
                if !ok.contains(&e) {
                    ok.push(e);
                }
            }
        }
    }
    ok
}

/// Verify a checkpoint note: the operator must have signed, and at least `min_witnesses`
/// **distinct public keys, none of them the operator's key**, must have cosigned this exact text.
/// (Counting is by key, not by name, so aliases cannot inflate the quorum.)
pub fn verify_note(
    n: &str,
    operator: &KeyInfo,
    witnesses: &[KeyInfo],
    min_witnesses: usize,
) -> Result<Checkpoint, CheckpointError> {
    let (text, lines) = split_note(n)?;
    let cp = Checkpoint::parse(text)?;
    let op = valid_signers(text, &lines, std::slice::from_ref(operator), false);
    if !op.iter().any(|(_, k)| *k == operator.key.to_bytes()) {
        return Err(CheckpointError::NoOperator);
    }
    let independent: Vec<KeyInfo> = witnesses
        .iter()
        .filter(|w| w.key != operator.key && w.name != operator.name)
        .cloned()
        .collect();
    // A witness counts once per name AND once per public key: neither aliases (one key, many names)
    // nor ambiguous names (one name, many keys) can inflate the quorum.
    let (mut names, mut pks) = (HashSet::new(), HashSet::new());
    let mut got = 0usize;
    for (name, pk) in valid_signers(text, &lines, &independent, true) {
        if !names.contains(&name) && !pks.contains(&pk) {
            got += 1;
        }
        names.insert(name);
        pks.insert(pk);
    }
    if got < min_witnesses {
        return Err(CheckpointError::Quorum {
            got,
            need: min_witnesses,
        });
    }
    Ok(cp)
}

/// Like `verify_note` but also requires the checkpoint origin to equal `expected_origin`, so a
/// validly signed checkpoint of a different log cannot be substituted (threat T-L0-09).
pub fn verify_note_for_origin(
    n: &str,
    expected_origin: &str,
    operator: &KeyInfo,
    witnesses: &[KeyInfo],
    min_witnesses: usize,
) -> Result<Checkpoint, CheckpointError> {
    let cp = verify_note(n, operator, witnesses, min_witnesses)?;
    if cp.origin != expected_origin {
        return Err(CheckpointError::WrongOrigin);
    }
    Ok(cp)
}

/// Split-view detection: same origin and size but different roots.
pub fn is_split_view(a: &Checkpoint, b: &Checkpoint) -> bool {
    a.origin == b.origin && a.size == b.size && a.root != b.root
}
