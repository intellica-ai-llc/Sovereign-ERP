//! Witness (spec C6): cosigns checkpoints only if they extend what it has already seen.
//! Rules: persist state BEFORE releasing a cosignature; never cosign a smaller size; a same-size
//! different-root pair freezes that log and keeps both signed notes as equivocation evidence.
//! Cosignatures follow C2SP tlog-cosignature/v1 (timestamped, signature type 0x04).
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use std::collections::HashMap;
use vc_core::Hash;
use vc_log::checkpoint::{
    cosig_key_id, cosig_message, valid_name, verify_note, Checkpoint, KeyInfo,
};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WitnessError {
    #[error("log origin is not configured on this witness")]
    UnknownLog,
    #[error("checkpoint is malformed or not signed by the log operator")]
    BadCheckpoint,
    #[error("checkpoint is smaller than one already seen (have {have})")]
    Rollback { have: u64 },
    #[error("old size does not match witness state (witness has {have})")]
    OldSizeMismatch { have: u64 },
    #[error("consistency proof invalid")]
    BadProof,
    #[error("log equivocated; it is frozen on this witness")]
    Equivocation,
    #[error("state could not be persisted; no cosignature issued")]
    Persist,
    #[error("invalid witness name")]
    BadName,
}

/// Storage failure (the witness then issues no cosignature).
#[derive(Debug, PartialEq, Eq)]
pub struct StoreError;

/// Durable witness state per log origin. `store` must be atomic and durable when it returns Ok.
pub trait StateStore {
    fn load(&self, origin: &str) -> Result<Option<Vec<u8>>, StoreError>;
    fn store(&mut self, origin: &str, data: &[u8]) -> Result<(), StoreError>;
}

/// Directory-backed state: one file per origin, written to a temp file, fsynced, renamed, directory fsynced.
pub struct FileState {
    dir: std::path::PathBuf,
}
impl FileState {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
    fn path(&self, origin: &str) -> std::path::PathBuf {
        // the origin is hex-encoded so no origin can escape the directory or collide
        self.dir
            .join(format!("{}.state", hex::encode(origin.as_bytes())))
    }
}
impl StateStore for FileState {
    fn load(&self, o: &str) -> Result<Option<Vec<u8>>, StoreError> {
        match std::fs::read(self.path(o)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StoreError),
        }
    }
    fn store(&mut self, o: &str, d: &[u8]) -> Result<(), StoreError> {
        use std::io::Write;
        let p = self.path(o);
        let tmp = p.with_extension("tmp");
        let w = || -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(d)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &p)?;
            std::fs::File::open(&self.dir)?.sync_all()
        };
        w().map_err(|_| StoreError)
    }
}

#[derive(Default)]
pub struct MemState {
    pub map: HashMap<String, Vec<u8>>,
    pub fail_store: bool,
}
impl StateStore for MemState {
    fn load(&self, o: &str) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.map.get(o).cloned())
    }
    fn store(&mut self, o: &str, d: &[u8]) -> Result<(), StoreError> {
        if self.fail_store {
            return Err(StoreError);
        }
        self.map.insert(o.to_string(), d.to_vec());
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    size: u64,
    root: Hash,
    ts: u64,
    /// Two signed notes proving a split view, once detected.
    evidence: Option<(String, String)>,
    last_note: String,
}

impl Seen {
    fn encode(&self) -> Vec<u8> {
        let (a, b) = self.evidence.clone().unwrap_or_default();
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            self.size,
            B64.encode(self.root),
            self.ts,
            B64.encode(&self.last_note),
            u8::from(self.evidence.is_some()),
            B64.encode(a),
            B64.encode(b)
        )
        .into_bytes()
    }
    fn decode(d: &[u8]) -> Option<Seen> {
        let s = std::str::from_utf8(d).ok()?;
        let l: Vec<&str> = s.strip_suffix('\n')?.split('\n').collect();
        if l.len() != 7 {
            return None;
        }
        let dec = |x: &str| -> Option<String> { String::from_utf8(B64.decode(x).ok()?).ok() };
        Some(Seen {
            size: l[0].parse().ok()?,
            root: B64.decode(l[1]).ok()?.try_into().ok()?,
            ts: l[2].parse().ok()?,
            last_note: dec(l[3])?,
            evidence: if l[4] == "1" {
                Some((dec(l[5])?, dec(l[6])?))
            } else {
                None
            },
        })
    }
}

pub mod client;

pub struct Witness<S: StateStore> {
    name: String,
    sk: SigningKey,
    logs: HashMap<String, KeyInfo>,
    store: S,
}

/// Verify one cosignature line over `cp_text`; returns the signed timestamp.
pub fn verify_cosignature(
    line: &str,
    cp_text: &str,
    name: &str,
    key: &VerifyingKey,
) -> Option<u64> {
    let rest = line.trim_end_matches('\n').strip_prefix("\u{2014} ")?;
    let (n, b64) = rest.rsplit_once(' ')?;
    if n != name || key.is_weak() {
        return None;
    }
    let raw = B64.decode(b64).ok()?;
    if raw.len() != 4 + 8 + 64 || raw[..4] != cosig_key_id(name, key) {
        return None;
    }
    let ts = u64::from_be_bytes(raw[4..12].try_into().ok()?);
    let sig = Signature::from_slice(&raw[12..]).ok()?;
    key.verify_strict(&cosig_message(ts, cp_text), &sig).ok()?;
    Some(ts)
}

impl<S: StateStore> Witness<S> {
    pub fn new(name: &str, sk: SigningKey, store: S) -> Result<Self, WitnessError> {
        if !valid_name(name) {
            return Err(WitnessError::BadName);
        }
        Ok(Self {
            name: name.to_string(),
            sk,
            logs: HashMap::new(),
            store,
        })
    }

    pub fn public_key(&self) -> VerifyingKey {
        self.sk.verifying_key()
    }

    /// Trust `operator` as the signer of checkpoints for `origin`.
    pub fn add_log(&mut self, origin: &str, operator: KeyInfo) {
        self.logs.insert(origin.to_string(), operator);
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    fn seen(&self, origin: &str) -> Result<Option<Seen>, WitnessError> {
        match self.store.load(origin) {
            Ok(None) => Ok(None),
            Ok(Some(d)) => Seen::decode(&d).map(Some).ok_or(WitnessError::Persist),
            Err(StoreError) => Err(WitnessError::Persist),
        }
    }

    /// Size of the last checkpoint this witness has cosigned for `origin` (0 if none).
    pub fn last_size(&self, origin: &str) -> Result<u64, WitnessError> {
        Ok(self.seen(origin)?.map_or(0, |s| s.size))
    }

    /// Equivocation evidence (two operator-signed notes with the same size, different roots).
    pub fn evidence(&self, origin: &str) -> Option<(String, String)> {
        self.seen(origin).ok().flatten().and_then(|s| s.evidence)
    }

    /// Handle an add-checkpoint request: `note` is the operator-signed checkpoint, `old_size` the
    /// size the log believes this witness has, `proof` a consistency proof from old_size to note.
    /// Returns one cosignature line (with trailing newline). `now` is unix seconds.
    pub fn add_checkpoint(
        &mut self,
        note: &str,
        old_size: u64,
        proof: &[Hash],
        now: u64,
    ) -> Result<String, WitnessError> {
        let text_end = note.find("\n\n").ok_or(WitnessError::BadCheckpoint)? + 1;
        let cp = Checkpoint::parse(&note[..text_end]).map_err(|_| WitnessError::BadCheckpoint)?;
        let op = self
            .logs
            .get(&cp.origin)
            .ok_or(WitnessError::UnknownLog)?
            .clone();
        let cp = verify_note(note, &op, &[], 0).map_err(|_| WitnessError::BadCheckpoint)?;
        // Cosign only the exact bytes the operator signed (no dropped extension lines).
        if cp.text() != note[..text_end] {
            return Err(WitnessError::BadCheckpoint);
        }
        let prev = self.seen(&cp.origin)?;
        let mut ts = now;
        if let Some(p) = &prev {
            if p.evidence.is_some() {
                return Err(WitnessError::Equivocation);
            }
            ts = ts.max(p.ts);
            if cp.size < p.size {
                return Err(WitnessError::Rollback { have: p.size });
            }
            if cp.size == p.size {
                if cp.root != p.root {
                    let mut f = p.clone();
                    f.evidence = Some((p.last_note.clone(), note.to_string()));
                    self.store
                        .store(&cp.origin, &f.encode())
                        .map_err(|_| WitnessError::Persist)?;
                    return Err(WitnessError::Equivocation);
                }
            } else {
                if old_size != p.size {
                    return Err(WitnessError::OldSizeMismatch { have: p.size });
                }
                if p.size > 0 {
                    vc_log::verify_consistency(
                        p.size as usize,
                        cp.size as usize,
                        &p.root,
                        &cp.root,
                        proof,
                    )
                    .map_err(|_| WitnessError::BadProof)?;
                }
            }
        } else if old_size != 0 {
            return Err(WitnessError::OldSizeMismatch { have: 0 });
        }
        // Persist BEFORE signing: a crash after this point can only cause a retry, never a
        // cosignature for a state the witness has forgotten.
        let state = Seen {
            size: cp.size,
            root: cp.root,
            ts,
            evidence: None,
            last_note: note.to_string(),
        };
        self.store
            .store(&cp.origin, &state.encode())
            .map_err(|_| WitnessError::Persist)?;
        let text = cp.text();
        let sig = self.sk.sign(&cosig_message(ts, &text));
        let mut raw = cosig_key_id(&self.name, &self.sk.verifying_key()).to_vec();
        raw.extend_from_slice(&ts.to_be_bytes());
        raw.extend_from_slice(&sig.to_bytes());
        Ok(format!("\u{2014} {} {}\n", self.name, B64.encode(raw)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vc_log::checkpoint::{note, sign_line};

    const ORIGIN: &str = "vc.example/log1";

    fn leaves(n: usize) -> Vec<Hash> {
        let e: Vec<Vec<u8>> = (0..n).map(|i| vec![i as u8, 7]).collect();
        let r: Vec<&[u8]> = e.iter().map(|v| v.as_slice()).collect();
        vc_log::leaves_from_entries(&r)
    }
    fn op() -> SigningKey {
        SigningKey::from_bytes(&[1u8; 32])
    }
    fn signed(n: usize, tamper: bool) -> String {
        let mut root = vc_log::root(&leaves(n));
        if tamper {
            root[0] ^= 1;
        }
        let cp = Checkpoint {
            origin: ORIGIN.into(),
            size: n as u64,
            root,
        };
        let t = cp.text();
        note(&t, &[sign_line(&t, "op", &op())])
    }
    fn w() -> Witness<MemState> {
        let mut w = Witness::new(
            "w1",
            SigningKey::from_bytes(&[2u8; 32]),
            MemState::default(),
        )
        .unwrap();
        w.add_log(
            ORIGIN,
            KeyInfo {
                name: "op".into(),
                key: op().verifying_key(),
            },
        );
        w
    }
    fn cp_text(n: usize) -> String {
        Checkpoint {
            origin: ORIGIN.into(),
            size: n as u64,
            root: vc_log::root(&leaves(n)),
        }
        .text()
    }

    #[test]
    fn first_then_extend_with_proof() {
        let mut w = w();
        let c1 = w.add_checkpoint(&signed(3, false), 0, &[], 100).unwrap();
        assert_eq!(
            verify_cosignature(&c1, &cp_text(3), "w1", &w.public_key()),
            Some(100)
        );
        let p = vc_log::consistency_proof(&leaves(7), 3).unwrap();
        let c2 = w.add_checkpoint(&signed(7, false), 3, &p, 90).unwrap();
        // timestamps never go backwards
        assert_eq!(
            verify_cosignature(&c2, &cp_text(7), "w1", &w.public_key()),
            Some(100)
        );
    }

    #[test]
    fn cosignature_binds_text_name_and_key() {
        let mut w = w();
        let c = w.add_checkpoint(&signed(3, false), 0, &[], 5).unwrap();
        assert!(verify_cosignature(&c, &cp_text(4), "w1", &w.public_key()).is_none());
        assert!(verify_cosignature(&c, &cp_text(3), "w2", &w.public_key()).is_none());
        let other = SigningKey::from_bytes(&[9u8; 32]).verifying_key();
        assert!(verify_cosignature(&c, &cp_text(3), "w1", &other).is_none());
    }

    #[test]
    fn rollback_refused() {
        let mut w = w();
        let p = vc_log::consistency_proof(&leaves(7), 3).unwrap();
        w.add_checkpoint(&signed(3, false), 0, &[], 1).unwrap();
        w.add_checkpoint(&signed(7, false), 3, &p, 2).unwrap();
        assert_eq!(
            w.add_checkpoint(&signed(3, false), 0, &[], 3),
            Err(WitnessError::Rollback { have: 7 })
        );
    }

    #[test]
    fn equivocation_freezes_and_keeps_evidence() {
        let mut w = w();
        w.add_checkpoint(&signed(5, false), 0, &[], 1).unwrap();
        assert_eq!(
            w.add_checkpoint(&signed(5, true), 5, &[], 2),
            Err(WitnessError::Equivocation)
        );
        let (a, b) = w.evidence(ORIGIN).unwrap();
        assert_ne!(a, b);
        // frozen: even a valid extension is refused
        let p = vc_log::consistency_proof(&leaves(8), 5).unwrap();
        assert_eq!(
            w.add_checkpoint(&signed(8, false), 5, &p, 3),
            Err(WitnessError::Equivocation)
        );
    }

    #[test]
    fn bad_proof_and_wrong_old_size() {
        let mut w = w();
        w.add_checkpoint(&signed(3, false), 0, &[], 1).unwrap();
        let p = vc_log::consistency_proof(&leaves(7), 3).unwrap();
        assert_eq!(
            w.add_checkpoint(&signed(7, false), 2, &p, 2),
            Err(WitnessError::OldSizeMismatch { have: 3 })
        );
        assert_eq!(
            w.add_checkpoint(&signed(7, false), 3, &[], 2),
            Err(WitnessError::BadProof)
        );
        // a fork (different history up to 3) cannot be proven consistent
        let mut other: Vec<Hash> = leaves(7);
        other[1][0] ^= 1;
        let pf = vc_log::consistency_proof(&other, 3).unwrap();
        let root = vc_log::root(&other);
        let cp = Checkpoint {
            origin: ORIGIN.into(),
            size: 7,
            root,
        };
        let t = cp.text();
        let n = note(&t, &[sign_line(&t, "op", &op())]);
        assert_eq!(w.add_checkpoint(&n, 3, &pf, 2), Err(WitnessError::BadProof));
    }

    #[test]
    fn unknown_log_bad_signature_and_first_nonzero_old_size() {
        let mut w = w();
        let cp = Checkpoint {
            origin: "other/log".into(),
            size: 1,
            root: [0; 32],
        };
        let t = cp.text();
        let n = note(&t, &[sign_line(&t, "op", &op())]);
        assert_eq!(
            w.add_checkpoint(&n, 0, &[], 1),
            Err(WitnessError::UnknownLog)
        );
        let t = cp_text(3);
        let bad = note(
            &t,
            &[sign_line(&t, "op", &SigningKey::from_bytes(&[5u8; 32]))],
        );
        assert_eq!(
            w.add_checkpoint(&bad, 0, &[], 1),
            Err(WitnessError::BadCheckpoint)
        );
        assert_eq!(
            w.add_checkpoint(&signed(3, false), 2, &[], 1),
            Err(WitnessError::OldSizeMismatch { have: 0 })
        );
        assert_eq!(
            w.add_checkpoint("garbage", 0, &[], 1),
            Err(WitnessError::BadCheckpoint)
        );
    }

    #[test]
    fn persist_failure_yields_no_cosignature_and_no_state() {
        let mut w = w();
        w.store.fail_store = true;
        assert_eq!(
            w.add_checkpoint(&signed(3, false), 0, &[], 1),
            Err(WitnessError::Persist)
        );
        w.store.fail_store = false;
        assert!(w.store().map.is_empty());
        assert!(w.add_checkpoint(&signed(3, false), 0, &[], 1).is_ok());
    }

    #[test]
    fn restart_keeps_state_and_same_size_resign_is_idempotent() {
        let mut w1 = w();
        w1.add_checkpoint(&signed(4, false), 0, &[], 10).unwrap();
        let mut w2 = Witness::new(
            "w1",
            SigningKey::from_bytes(&[2u8; 32]),
            MemState {
                map: w1.store.map.clone(),
                fail_store: false,
            },
        )
        .unwrap();
        w2.add_log(
            ORIGIN,
            KeyInfo {
                name: "op".into(),
                key: op().verifying_key(),
            },
        );
        assert!(w2.add_checkpoint(&signed(4, false), 4, &[], 11).is_ok());
        assert_eq!(
            w2.add_checkpoint(&signed(2, false), 0, &[], 12),
            Err(WitnessError::Rollback { have: 4 })
        );
    }

    #[test]
    fn corrupt_state_fails_closed() {
        let mut w = w();
        w.store.map.insert(ORIGIN.into(), b"junk".to_vec());
        assert_eq!(
            w.add_checkpoint(&signed(3, false), 0, &[], 1),
            Err(WitnessError::Persist)
        );
    }

    #[test]
    fn bad_witness_name() {
        assert!(Witness::new(
            "a b",
            SigningKey::from_bytes(&[2u8; 32]),
            MemState::default()
        )
        .is_err());
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use vc_log::checkpoint::{note, sign_line};

    #[test]
    fn extra_checkpoint_lines_are_refused_not_silently_dropped() {
        let sk = SigningKey::from_bytes(&[1u8; 32]);
        let mut w = Witness::new(
            "w1",
            SigningKey::from_bytes(&[2u8; 32]),
            MemState::default(),
        )
        .unwrap();
        w.add_log(
            "o",
            KeyInfo {
                name: "op".into(),
                key: sk.verifying_key(),
            },
        );
        let t = format!("o\n1\n{}\nextension line\n", B64.encode([0u8; 32]));
        let n = note(&t, &[sign_line(&t, "op", &sk)]);
        assert_eq!(
            w.add_checkpoint(&n, 0, &[], 1),
            Err(WitnessError::BadCheckpoint)
        );
    }
}
