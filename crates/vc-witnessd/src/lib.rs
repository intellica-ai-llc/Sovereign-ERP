//! Witness daemon. HTTP API (C2SP tlog-witness add-checkpoint):
//!   POST /add-checkpoint   body: "old <size>\n" + zero or more base64 proof lines + "\n" + signed checkpoint note
//!   200 -> cosignature line(s), text/plain;  409 -> witness's current size as decimal (old size mismatch);
//!   403 unknown log or bad signature;  422 bad proof, rollback or equivocation;  500 could not persist.
//! Logs are configured in `<dir>/logs.txt`: one line per log: `<origin> <operator-key-name> <pubkey-hex>`.
//! State is stored one file per log origin (hex-named), written atomically and fsynced before any cosignature.
use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vc_core::Hash;
use vc_log::checkpoint::KeyInfo;
use vc_witness::{StateStore, StoreError, Witness, WitnessError};

pub struct FileState(pub PathBuf);

impl FileState {
    fn path(&self, origin: &str) -> PathBuf {
        self.0
            .join(format!("{}.state", hex::encode(origin.as_bytes())))
    }
}

impl StateStore for FileState {
    fn load(&self, origin: &str) -> Result<Option<Vec<u8>>, StoreError> {
        match std::fs::read(self.path(origin)) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StoreError),
        }
    }
    fn store(&mut self, origin: &str, data: &[u8]) -> Result<(), StoreError> {
        use std::io::Write;
        let p = self.path(origin);
        let tmp = p.with_extension("tmp");
        let mut f = std::fs::File::create(&tmp).map_err(|_| StoreError)?;
        f.write_all(data).map_err(|_| StoreError)?;
        f.sync_all().map_err(|_| StoreError)?;
        std::fs::rename(&tmp, &p).map_err(|_| StoreError)?;
        std::fs::File::open(&self.0)
            .and_then(|d| d.sync_all())
            .map_err(|_| StoreError)
    }
}

pub type Shared = Arc<Mutex<Witness<FileState>>>;

pub fn open(dir: PathBuf, name: &str, sk: SigningKey) -> Result<Shared, String> {
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut w = Witness::new(name, sk, FileState(dir.clone())).map_err(|e| e.to_string())?;
    let cfg = std::fs::read_to_string(dir.join("logs.txt")).unwrap_or_default();
    for (n, line) in cfg.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let p: Vec<&str> = line.split(' ').collect();
        if p.len() != 3 {
            return Err(format!(
                "logs.txt line {}: expected '<origin> <key-name> <pubkey-hex>'",
                n + 1
            ));
        }
        let pk: [u8; 32] = hex::decode(p[2])
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "pubkey must be 32 bytes")?;
        let key = VerifyingKey::from_bytes(&pk).map_err(|e| e.to_string())?;
        w.add_log(
            p[0],
            KeyInfo {
                name: p[1].into(),
                key,
            },
        );
    }
    Ok(Arc::new(Mutex::new(w)))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse(body: &str) -> Option<(u64, Vec<Hash>, &str)> {
    let (first, rest) = body.split_once('\n')?;
    let old: u64 = first
        .strip_prefix("old ")?
        .parse()
        .ok()
        .filter(|n: &u64| n.to_string() == first[4..])?;
    let mut proof = vec![];
    let mut rest = rest;
    loop {
        let (l, r) = rest.split_once('\n')?;
        rest = r;
        if l.is_empty() {
            break;
        }
        if proof.len() >= 64 {
            return None;
        }
        proof.push(B64.decode(l).ok()?.try_into().ok()?);
    }
    Some((old, proof, rest))
}

async fn add_checkpoint(State(w): State<Shared>, body: Bytes) -> Response {
    let Ok(text) = std::str::from_utf8(&body) else {
        return (StatusCode::BAD_REQUEST, "not utf-8").into_response();
    };
    let Some((old, proof, note)) = parse(text) else {
        return (StatusCode::BAD_REQUEST, "malformed request").into_response();
    };
    let (note, w2) = (note.to_string(), w.clone());
    let r = tokio::task::spawn_blocking(move || {
        w2.lock().unwrap().add_checkpoint(&note, old, &proof, now())
    })
    .await
    .expect("join");
    match r {
        Ok(line) => ([("content-type", "text/plain; charset=utf-8")], line).into_response(),
        Err(WitnessError::OldSizeMismatch { have }) => {
            (StatusCode::CONFLICT, have.to_string()).into_response()
        }
        Err(WitnessError::UnknownLog) | Err(WitnessError::BadCheckpoint) => {
            (StatusCode::FORBIDDEN, "unknown log or bad checkpoint").into_response()
        }
        Err(WitnessError::BadProof)
        | Err(WitnessError::Rollback { .. })
        | Err(WitnessError::Equivocation) => {
            (StatusCode::UNPROCESSABLE_ENTITY, "checkpoint refused").into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "state not persisted; retry",
        )
            .into_response(),
    }
}

pub fn router(w: Shared) -> Router {
    Router::new()
        .route("/add-checkpoint", post(add_checkpoint))
        .route("/healthz", get(|| async { "ok" }))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .with_state(w)
}
