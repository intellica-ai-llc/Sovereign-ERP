//! Witness client loop: fetch the log's current checkpoint and a consistency proof from the size this witness
//! last cosigned, run the witness rules locally (the key never leaves this process), post the cosignature back.
use crate::{StateStore, Witness, WitnessError};
use vc_core::Hash;

#[derive(Debug)]
pub enum ClientError {
    Http(String),
    Bad(String),
    Witness(WitnessError),
}
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Http(m) => write!(f, "http: {m}"),
            ClientError::Bad(m) => write!(f, "bad response: {m}"),
            ClientError::Witness(e) => write!(f, "witness refused: {e}"),
        }
    }
}

/// One cosigning round. Returns the tree size cosigned. `now` is unix seconds.
pub fn run_once<S: StateStore>(
    w: &mut Witness<S>,
    base_url: &str,
    origin: &str,
    token: &str,
    now: u64,
) -> Result<u64, ClientError> {
    let base = base_url.trim_end_matches('/');
    let auth = format!("Bearer {token}");
    let from = w.last_size(origin).map_err(ClientError::Witness)?;
    let r: serde_json::Value = ureq::get(&format!("{base}/v1/checkpoint/consistency?from={from}"))
        .set("Authorization", &auth)
        .call()
        .map_err(|e| ClientError::Http(e.to_string()))?
        .into_json()
        .map_err(|e| ClientError::Bad(e.to_string()))?;
    let note = r["checkpoint"]
        .as_str()
        .ok_or_else(|| ClientError::Bad("no checkpoint".into()))?;
    let size = r["tree_size"]
        .as_u64()
        .ok_or_else(|| ClientError::Bad("no tree_size".into()))?;
    let proof: Vec<Hash> = r["proof"]
        .as_array()
        .ok_or_else(|| ClientError::Bad("no proof".into()))?
        .iter()
        .map(|h| {
            h.as_str()
                .and_then(|s| hex::decode(s).ok())
                .and_then(|v| v.try_into().ok())
                .ok_or_else(|| ClientError::Bad("bad proof element".into()))
        })
        .collect::<Result<_, _>>()?;
    // a log with nothing new needs no new cosignature
    let line = w
        .add_checkpoint(note, from, &proof, now)
        .map_err(ClientError::Witness)?;
    ureq::post(&format!("{base}/v1/cosignatures"))
        .set("Authorization", &auth)
        .send_json(serde_json::json!({"size": size, "line": line}))
        .map_err(|e| ClientError::Http(e.to_string()))?;
    Ok(size)
}
