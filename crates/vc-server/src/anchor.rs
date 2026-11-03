//! Independent time anchoring (RFC 3161). The latest signed checkpoint of a tenant is hashed and sent to a public
//! Time-Stamp Authority (default FreeTSA); the signed reply is stored next to the log. The TSA's clock and signature
//! are NOT ours, so this shows the checkpoint existed no later than the TSA's timestamp, whatever the operator says.
//! We store the reply verbatim and do not parse trust out of it: verify it with standard tooling (see docs):
//!   openssl ts -verify -in reply.tsr -digest <sha256 of the checkpoint note> -CAfile cacert.pem -untrusted tsa.crt
//! Only the PKI status (granted or not) is read here, so a refusal is never stored as an anchor.
use super::{err, now_secs, App};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

pub type TsaFetch = Arc<dyn Fn(&str, &[u8]) -> Result<Vec<u8>, String> + Send + Sync>;
pub const DEFAULT_TSA: &str = "https://freetsa.org/tsr";

fn der(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![tag];
    match body.len() {
        n if n < 128 => v.push(n as u8),
        n if n < 256 => v.extend([0x81, n as u8]),
        n => v.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    v.extend_from_slice(body);
    v
}

/// DER TimeStampReq for a SHA-256 digest: version 1, optional nonce, certReq true.
pub fn tsq(digest: &[u8; 32], nonce: Option<u64>) -> Vec<u8> {
    let alg = der(
        0x30,
        &[
            der(
                0x06,
                &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01],
            ),
            vec![0x05, 0x00],
        ]
        .concat(),
    );
    let imprint = der(0x30, &[alg, der(0x04, digest)].concat());
    let mut body = [der(0x02, &[1]), imprint].concat();
    if let Some(n) = nonce {
        let mut b = n.to_be_bytes().to_vec();
        while b.len() > 1 && b[0] == 0 {
            b.remove(0);
        }
        if b[0] & 0x80 != 0 {
            b.insert(0, 0);
        }
        body.extend(der(0x02, &b));
    }
    body.extend(der(0x01, &[0xff]));
    der(0x30, &body)
}

fn read_len(b: &[u8]) -> Option<(usize, usize)> {
    let f = *b.first()?;
    if f < 128 {
        Some((f as usize, 1))
    } else {
        let n = (f & 0x7f) as usize;
        if n == 0 || n > 2 || b.len() < 1 + n {
            return None;
        }
        Some((
            b[1..=n].iter().fold(0usize, |a, x| (a << 8) | *x as usize),
            1 + n,
        ))
    }
}

/// PKIStatus of a TimeStampResp: 0 granted, 1 granted with modifications, 2+ refused. None if it is not a TimeStampResp.
pub fn tsr_status(b: &[u8]) -> Option<u8> {
    if *b.first()? != 0x30 {
        return None;
    }
    let (_, l1) = read_len(&b[1..])?;
    let rest = &b[1 + l1..];
    if *rest.first()? != 0x30 {
        return None;
    }
    let (_, l2) = read_len(&rest[1..])?;
    let st = &rest[1 + l2..];
    if *st.first()? != 0x02 || *st.get(1)? != 1 {
        return None;
    }
    st.get(2).copied()
}

fn anchors_dir(app: &App, tid: &str) -> PathBuf {
    app.dir.join("logs").join(tid).join("anchors")
}

impl App {
    /// Replace the function that talks to the TSA (tests, or a proxy-aware client).
    pub fn set_tsa_fetch(&self, f: TsaFetch) {
        *self.tsa_fetch.lock().unwrap() = f;
    }
}

pub fn default_tsa_fetch() -> TsaFetch {
    Arc::new(|url: &str, body: &[u8]| {
        use std::io::Read;
        let r = ureq::post(url)
            .set("Content-Type", "application/timestamp-query")
            .set("User-Agent", "vc-server/anchor")
            .timeout(std::time::Duration::from_secs(20))
            .send_bytes(body)
            .map_err(|e| e.to_string())?;
        let mut out = vec![];
        r.into_reader()
            .take(256 * 1024)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out)
    })
}

/// POST /v1/anchor: timestamp this tenant's latest checkpoint with the TSA.
pub(crate) async fn create(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let fetch = app.tsa_fetch.lock().unwrap().clone();
    let url = std::env::var("VC_TSA_URL").unwrap_or_else(|_| DEFAULT_TSA.to_string());
    let dir = anchors_dir(&app, &tid);
    let r = tokio::task::spawn_blocking(move || -> Result<Value, (StatusCode, String)> {
        let note = seq.lock().unwrap().checkpoint().map(|c| c.1).map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
        let digest: [u8; 32] = Sha256::digest(note.as_bytes()).into();
        let mut nb = [0u8; 8];
        super::getrandom(&mut nb).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        let nonce = u64::from_be_bytes(nb) >> 1;
        let q = tsq(&digest, Some(nonce));
        let reply = fetch(&url, &q).map_err(|e| (StatusCode::BAD_GATEWAY, format!("time-stamp authority unreachable: {e}")))?;
        match tsr_status(&reply) {
            Some(0 | 1) => {}
            Some(s) => return Err((StatusCode::BAD_GATEWAY, format!("time-stamp authority refused (status {s})"))),
            None => return Err((StatusCode::BAD_GATEWAY, "reply is not a TimeStampResp".into())),
        }
        let b64 = |x: &[u8]| base64::engine::general_purpose::STANDARD.encode(x);
        let t = now_secs();
        let rec = json!({"t": t, "tsa": url, "digest_sha256": hex::encode(digest), "checkpoint": note,
            "request_b64": b64(&q), "reply_b64": b64(&reply)});
        std::fs::create_dir_all(&dir).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let name = format!("{t}-{}.json", &hex::encode(digest)[..12]);
        std::fs::write(dir.join(&name), serde_json::to_vec_pretty(&rec).unwrap_or_default())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok(rec)
    })
    .await
    .expect("join");
    match r {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err((c, m)) => err(c, &m),
    }
}

/// GET /v1/anchors: every stored anchor for this tenant, newest first.
pub(crate) async fn list(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (tid, _) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let dir = anchors_dir(&app, &tid);
    let v = tokio::task::spawn_blocking(move || {
        let mut names: Vec<String> = std::fs::read_dir(dir.clone())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        names.reverse();
        names.truncate(100);
        names
            .iter()
            .filter_map(|n| serde_json::from_slice::<Value>(&std::fs::read(dir.join(n)).ok()?).ok())
            .collect::<Vec<_>>()
    })
    .await
    .expect("join");
    Json(json!({"anchors": v})).into_response()
}
