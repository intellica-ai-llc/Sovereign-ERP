//! Hosted VeriChain log server (production slice 1): tenants with hashed API keys, one durable sequencer
//! per tenant, statement ingest, signed checkpoints, inclusion proofs and a dashboard summary API.
//! Every tenant log is independent (own origin, own tile store); one tenant's key can never touch another's log.
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vc_log::checkpoint::{note, sign_line, verify_note, Checkpoint, KeyInfo};
use vc_seq::{Issuer, Registry, Sequencer};
use vc_store::FsStore;

pub mod webauthn;

pub const DASHBOARD: &str = include_str!("../assets/app.html");
const APPROVE: &str = include_str!("../assets/approve.html");
const MANIFEST: &str = include_str!("../assets/manifest.webmanifest");
const MAX_BODY: usize = 70 * 1024;

#[derive(Serialize, Deserialize, Clone)]
struct AgentRec {
    id: String,
    public_key: String,
    max_completeness: u8,
    max_assurance: u8,
}

#[derive(Serialize, Deserialize, Clone)]
struct WitnessRec {
    name: String,
    public_key: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ApproverRec {
    name: String,
    credential_id: String,
    public_key: String,
    sign_count: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct TenantRec {
    #[serde(default)]
    approvers: Vec<ApproverRec>,
    name: String,
    key_hash: String,
    agents: Vec<AgentRec>,
    #[serde(default)]
    witnesses: Vec<WitnessRec>,
    #[serde(default)]
    min_witnesses: u32,
}

pub mod anchor;
mod audit;
mod trading;

type Seq = Sequencer<FsStore>;

struct Tenant {
    rec: TenantRec,
    seq: Arc<Mutex<Seq>>,
}

pub struct App {
    dir: PathBuf,
    host: String,
    admin_hash: [u8; 32],
    operator: SigningKey,
    tenants: Mutex<BTreeMap<String, Tenant>>,
    rp_id: String,
    origin: String,
    challenges: Mutex<HashMap<String, Chal>>,
    trading: trading::Trading,
    audit: audit::AuditLog,
    tsa_fetch: Mutex<anchor::TsaFetch>,
    collect_expected: std::sync::atomic::AtomicBool,
    /// Recent failed-auth timestamps per client key (see `throttle_mw`).
    fails: Mutex<HashMap<String, Vec<u64>>>,
    /// Header that carries the real client address, set by OUR reverse proxy (never a header clients can choose).
    client_hdr: Option<String>,
}

enum ChalKind {
    Register,
    Approve {
        action_id: String,
        decision: String,
        nonce: String,
    },
}

struct Chal {
    tid: String,
    kind: ChalKind,
    expires: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn iso(t: u64) -> String {
    let (days, rem) = (t / 86400, t % 86400);
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn h256(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}

fn issuer_of(a: &AgentRec) -> Option<Issuer> {
    let pk: [u8; 32] = hex::decode(&a.public_key).ok()?.try_into().ok()?;
    let key = VerifyingKey::from_bytes(&pk).ok()?;
    (!key.is_weak()).then(|| Issuer {
        key,
        agent_id: a.id.clone(),
        max_completeness: a.max_completeness.min(3),
        max_assurance: a.max_assurance.min(3),
    })
}

fn atomic_write(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    std::fs::File::open(path.parent().unwrap_or(std::path::Path::new(".")))?.sync_all()
}

impl App {
    /// Open (or initialise) a data directory. `admin_token` guards tenant creation; it is stored only as a hash.
    pub fn open(dir: PathBuf, host: &str, admin_token: &str) -> Result<Arc<App>, String> {
        if admin_token.len() < 24 {
            return Err("admin token must be at least 24 characters".into());
        }
        std::fs::create_dir_all(dir.join("logs")).map_err(|e| e.to_string())?;
        let kp = dir.join("operator.key");
        let operator = match std::fs::read_to_string(&kp) {
            Ok(s) => {
                let b: [u8; 32] = hex::decode(s.trim())
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|_| "bad operator key")?;
                SigningKey::from_bytes(&b)
            }
            Err(_) => {
                let mut b = [0u8; 32];
                getrandom(&mut b)?;
                atomic_write(&kp, hex::encode(b).as_bytes()).map_err(|e| e.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&kp, std::fs::Permissions::from_mode(0o600));
                }
                SigningKey::from_bytes(&b)
            }
        };
        let recs: BTreeMap<String, TenantRec> = match std::fs::read(dir.join("tenants.json")) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("tenants.json: {e}"))?,
            Err(_) => BTreeMap::new(),
        };
        let trading_dir = dir.join("trading");
        let dir_for_audit = dir.clone();
        let app = Arc::new(App {
            dir,
            host: host.to_string(),
            admin_hash: h256(admin_token),
            operator,
            tenants: Mutex::new(BTreeMap::new()),
            rp_id: host.split(':').next().unwrap_or(host).to_string(),
            origin: std::env::var("VC_ORIGIN").unwrap_or_else(|_| format!("https://{host}")),
            challenges: Mutex::new(HashMap::new()),
            trading: trading::Trading::new(trading_dir),
            audit: audit::AuditLog::open(&dir_for_audit),
            tsa_fetch: Mutex::new(anchor::default_tsa_fetch()),
            collect_expected: std::sync::atomic::AtomicBool::new(false),
            fails: Mutex::new(HashMap::new()),
            client_hdr: std::env::var("VC_CLIENT_IP_HEADER")
                .ok()
                .map(|h| h.to_ascii_lowercase())
                .filter(|h| !h.is_empty())
                .or_else(|| {
                    std::env::var("VC_TRUST_CF")
                        .is_ok_and(|v| v == "1")
                        .then(|| "cf-connecting-ip".to_string())
                }),
        });
        for (id, rec) in recs {
            let t = app.open_tenant(&id, rec)?;
            app.tenants.lock().unwrap().insert(id, t);
        }
        Ok(app)
    }

    fn open_tenant(&self, id: &str, rec: TenantRec) -> Result<Tenant, String> {
        let mut reg = Registry::new();
        for a in &rec.agents {
            let iss = issuer_of(a).ok_or("stored agent key invalid")?;
            reg.register(a.id.as_bytes(), iss);
        }
        let store = FsStore::new(self.dir.join("logs").join(id)).map_err(|e| e.to_string())?;
        let seq = Sequencer::open(
            store,
            reg,
            self.operator.clone(),
            "operator",
            &format!("{}/{}", self.host, id),
        )
        .map_err(|e| e.to_string())?;
        Ok(Tenant {
            rec,
            seq: Arc::new(Mutex::new(seq)),
        })
    }

    fn save(&self, t: &BTreeMap<String, Tenant>) -> Result<(), String> {
        let recs: BTreeMap<&String, &TenantRec> = t.iter().map(|(k, v)| (k, &v.rec)).collect();
        let b = serde_json::to_vec_pretty(&recs).map_err(|e| e.to_string())?;
        atomic_write(&self.dir.join("tenants.json"), &b).map_err(|e| e.to_string())
    }

    /// Operator public key (hex): what customers pin in their trust file.
    pub fn operator_public_key(&self) -> String {
        hex::encode(self.operator.verifying_key().to_bytes())
    }

    #[allow(clippy::result_large_err)]
    fn auth(&self, headers: &HeaderMap) -> Result<(String, Arc<Mutex<Seq>>), Response> {
        let tok =
            bearer(headers).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "missing bearer token"))?;
        let hh = hex::encode(h256(tok));
        let ts = self.tenants.lock().unwrap();
        // compare against every tenant hash without early exit on partial matches
        let mut found = None;
        for (id, t) in ts.iter() {
            if t.rec.key_hash == hh {
                found = Some((id.clone(), t.seq.clone()));
            }
        }
        found.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "invalid API key"))
    }
}

fn getrandom(b: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(b))
        .map_err(|e| format!("no system randomness: {e}"))
}

fn bearer(h: &HeaderMap) -> Option<&str> {
    h.get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error": msg}))).into_response()
}

#[derive(Deserialize)]
struct NewTenant {
    id: String,
    name: String,
}

async fn create_tenant(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    match bearer(&headers) {
        Some(t) if h256(t) == app.admin_hash => {}
        _ => return err(StatusCode::UNAUTHORIZED, "admin token required"),
    }
    let Ok(b) = serde_json::from_slice::<NewTenant>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    let ok_id = !b.id.is_empty()
        && b.id.len() <= 48
        && b.id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok_id || b.name.is_empty() || b.name.len() > 120 || b.name.chars().any(char::is_control) {
        return err(
            StatusCode::BAD_REQUEST,
            "id must be lowercase letters, digits or '-'; name 1-120 printable chars",
        );
    }
    let mut raw = [0u8; 32];
    if getrandom(&mut raw).is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "randomness unavailable");
    }
    let key = format!("vck_{}", hex::encode(raw));
    let rec = TenantRec {
        name: b.name,
        key_hash: hex::encode(h256(&key)),
        agents: vec![],
        witnesses: vec![],
        min_witnesses: 0,
        approvers: vec![],
    };
    let mut ts = app.tenants.lock().unwrap();
    if ts.contains_key(&b.id) {
        return err(StatusCode::CONFLICT, "tenant exists");
    }
    let t = match app.open_tenant(&b.id, rec) {
        Ok(t) => t,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    ts.insert(b.id.clone(), t);
    if let Err(e) = app.save(&ts) {
        ts.remove(&b.id);
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    (
        StatusCode::CREATED,
        Json(
            json!({"tenant": b.id, "origin": format!("{}/{}", app.host, b.id),
                    "api_key": key, "operator_public_key": app.operator_public_key(),
                    "note": "the API key is shown once"}),
        ),
    )
        .into_response()
}

#[derive(Deserialize)]
struct NewAgent {
    id: String,
    public_key: String,
    #[serde(default = "d2")]
    max_completeness: u8,
    #[serde(default = "d1")]
    max_assurance: u8,
}
fn d2() -> u8 {
    2
}
fn d1() -> u8 {
    1
}

async fn add_agent(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<NewAgent>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    let rec = AgentRec {
        id: b.id,
        public_key: b.public_key,
        max_completeness: b.max_completeness,
        max_assurance: b.max_assurance,
    };
    let ok_id =
        !rec.id.is_empty() && rec.id.len() <= 64 && rec.id.chars().all(|c| c.is_ascii_graphic());
    let Some(iss) = issuer_of(&rec).filter(|_| ok_id) else {
        return err(StatusCode::BAD_REQUEST, "invalid agent id or public key");
    };
    let mut ts = app.tenants.lock().unwrap();
    let t = ts.get_mut(&tid).expect("authenticated tenant");
    if t.rec.agents.len() >= vc_license::FREE_LIMITS.max_agents as usize {
        return err(
            StatusCode::PAYMENT_REQUIRED,
            "agent limit of the current plan reached",
        );
    }
    if t.rec
        .agents
        .iter()
        .any(|a| a.id == rec.id || a.public_key == rec.public_key)
    {
        return err(StatusCode::CONFLICT, "agent id or key already registered");
    }
    t.rec.agents.push(rec.clone());
    if let Err(e) = app.save(&ts) {
        ts.get_mut(&tid).unwrap().rec.agents.pop();
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    seq.lock().unwrap().register_issuer(rec.id.as_bytes(), iss);
    (StatusCode::CREATED, Json(json!({"agent": rec.id}))).into_response()
}

async fn ingest(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (_, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = tokio::task::spawn_blocking(move || seq.lock().unwrap().submit(&body))
        .await
        .expect("join");
    match r {
        Ok(reg) => Json(
            json!({"index": reg.index, "leaf_hash": hex::encode(reg.leaf_hash),
                               "statement_id": reg.statement_id, "duplicate": reg.duplicate}),
        )
        .into_response(),
        Err(vc_seq::SeqError::Store(_)) | Err(vc_seq::SeqError::Poisoned(_)) => err(
            StatusCode::SERVICE_UNAVAILABLE,
            "log temporarily unavailable; retry the same bytes",
        ),
        Err(e) => err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()),
    }
}

async fn checkpoint(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (_, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = tokio::task::spawn_blocking(move || seq.lock().unwrap().checkpoint().map(|c| c.1))
        .await
        .expect("join");
    match r {
        Ok(n) => ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], n).into_response(),
        Err(e) => err(StatusCode::SERVICE_UNAVAILABLE, &e.to_string()),
    }
}

async fn proof(State(app): State<Arc<App>>, headers: HeaderMap, Path(i): Path<u64>) -> Response {
    let (_, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let size = s.size();
        let entry = s.log().entry(i).map_err(|e| e.to_string())?;
        let p = s
            .log()
            .inclusion_proof(i, size)
            .map_err(|e| e.to_string())?;
        let (_, note) = s.checkpoint().map_err(|e| e.to_string())?;
        Ok::<_, String>(json!({"index": i, "tree_size": size,
            "statement_cose": hex::encode(entry),
            "inclusion_proof": p.iter().map(hex::encode).collect::<Vec<_>>(),
            "checkpoint": note}))
    })
    .await
    .expect("join");
    match r {
        Ok(v) => Json(v).into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "no such entry"),
    }
}

async fn dashboard_api(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let (name, agents) = {
        let ts = app.tenants.lock().unwrap();
        let t = &ts[&tid];
        (t.rec.name.clone(), t.rec.agents.clone())
    };
    let host = app.host.clone();
    let tid2 = tid.clone();
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let size = s.size();
        let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
        let mut per_agent: HashMap<String, (u64, String)> = HashMap::new();
        let mut recent = vec![];
        s.log()
            .scan(0, |i, e| {
                if let Ok((_, payload)) = vc_cose::peek_payload(e) {
                    if let Ok(v) = serde_json::from_slice::<Value>(&payload) {
                        let kind = v["kind"].as_str().unwrap_or("?").to_string();
                        *kinds.entry(kind.clone()).or_default() += 1;
                        let ag = v["agent"]["id"].as_str().unwrap_or("?").to_string();
                        let ts = v["ts"].as_str().unwrap_or("").to_string();
                        let e = per_agent.entry(ag.clone()).or_default();
                        e.0 += 1;
                        e.1 = ts.clone();
                        recent.push(json!({"index": i, "kind": kind, "agent": ag, "ts": ts}));
                        if recent.len() > 25 {
                            recent.remove(0);
                        }
                    }
                }
                Ok::<(), vc_store::StoreError>(())
            })
            .map_err(|e| e.to_string())?;
        let (_, note) = s.checkpoint().map_err(|e| e.to_string())?;
        Ok::<_, String>(json!({
            "origin": format!("{}/{}", host, tid2), "size": size, "kinds": kinds,
            "open_intents": s.open_intents(), "checkpoint": note,
            "recent": recent,
            "per_agent": per_agent.iter().map(|(k, v)| json!({"agent": k, "statements": v.0, "last_ts": v.1})).collect::<Vec<_>>(),
        }))
    })
    .await
    .expect("join");
    match r {
        Ok(mut v) => {
            v["tenant"] = json!(tid);
            v["name"] = json!(name);
            v["registered_agents"] = json!(agents.iter().map(|a| json!({"id": a.id, "max_completeness": a.max_completeness, "max_assurance": a.max_assurance})).collect::<Vec<_>>());
            Json(v).into_response()
        }
        Err(e) => err(StatusCode::SERVICE_UNAVAILABLE, &e),
    }
}

fn trust_json(app: &App, tid: &str, rec: &TenantRec) -> Value {
    let agents = &rec.agents;
    json!({"origin": format!("{}/{}", app.host, tid),
        "operator": {"name": "operator", "pubkey_hex": app.operator_public_key()},
        "witnesses": rec.witnesses.iter().map(|w| json!({"name": w.name, "pubkey_hex": w.public_key})).collect::<Vec<_>>(),
        "min_witnesses": rec.min_witnesses,
        "issuers": agents.iter().map(|a| json!({"kid_hex": hex::encode(a.id.as_bytes()),
            "pubkey_hex": a.public_key, "max_completeness": format!("C{}", a.max_completeness.min(3))})).collect::<Vec<_>>()})
}

fn rec_of(app: &App, tid: &str) -> TenantRec {
    app.tenants.lock().unwrap()[tid].rec.clone()
}

type Cosigs = BTreeMap<u64, Vec<String>>;

fn cosig_path(app: &App, tid: &str) -> PathBuf {
    app.dir.join("logs").join(tid).join("cosigs.json")
}

fn load_cosigs(app: &App, tid: &str) -> Cosigs {
    std::fs::read(cosig_path(app, tid))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn witness_keys(rec: &TenantRec) -> Vec<KeyInfo> {
    rec.witnesses
        .iter()
        .filter_map(|w| {
            let pk: [u8; 32] = hex::decode(&w.public_key).ok()?.try_into().ok()?;
            Some(KeyInfo {
                name: w.name.clone(),
                key: VerifyingKey::from_bytes(&pk).ok()?,
            })
        })
        .collect()
}

/// Operator-signed checkpoint note at `size` (root derived from the log), plus any stored witness lines
/// that verify for that exact text.
fn note_at(
    app: &App,
    s: &Seq,
    rec: &TenantRec,
    tid: &str,
    size: u64,
) -> Result<(Checkpoint, String, usize), String> {
    let cp = Checkpoint {
        origin: format!("{}/{}", app.host, tid),
        size,
        root: s.log().root_at(size).map_err(|e| e.to_string())?,
    };
    let text = cp.text();
    let mut lines = vec![sign_line(&text, "operator", &app.operator)];
    let keys = witness_keys(rec);
    let op = KeyInfo {
        name: "operator".into(),
        key: app.operator.verifying_key(),
    };
    let mut n = 0;
    for l in load_cosigs(app, tid)
        .get(&size)
        .cloned()
        .unwrap_or_default()
    {
        let candidate = note(&text, &[lines[0].clone(), l.clone()]);
        // count each stored line only if it verifies for exactly one registered witness
        if keys
            .iter()
            .any(|k| verify_note(&candidate, &op, std::slice::from_ref(k), 1).is_ok())
        {
            lines.push(l);
            n += 1;
        }
    }
    Ok((cp, note(&text, &lines), n))
}

/// Trust anchors for this tenant's log, to be pinned by the relying party out of band.
async fn trust(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (tid, _) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    Json(trust_json(&app, &tid, &rec_of(&app, &tid))).into_response()
}

#[derive(Deserialize)]
struct PackQuery {
    from: Option<u64>,
    limit: Option<u64>,
}

/// Evidence pack (C5) for entries [from, from+limit) against the current signed checkpoint. It verifies offline
/// with `vcv`; the relying party should pin trust from `/v1/trust` (or its own records), not from the pack.
async fn pack(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<PackQuery>,
) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let rec = rec_of(&app, &tid);
    let trust = trust_json(&app, &tid, &rec);
    let app2 = app.clone();
    let tid2 = tid.clone();
    let (from, limit) = (q.from.unwrap_or(0), q.limit.unwrap_or(1000).clamp(1, 2000));
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let cur = s.size();
        // the pack is anchored at the newest checkpoint that carries enough witness cosignatures
        let need = rec.min_witnesses as usize;
        let size = if need == 0 {
            cur
        } else {
            let mut best = 0;
            for (&sz, _) in load_cosigs(&app2, &tid2).iter().rev() {
                if sz <= cur && sz >= 1 && note_at(&app2, &s, &rec, &tid2, sz)?.2 >= need {
                    best = sz;
                    break;
                }
            }
            best
        };
        if size == 0 {
            return Err("no checkpoint has enough witness cosignatures yet".to_string());
        }
        if from >= size {
            return Err("no entries in range".to_string());
        }
        let (cp, note, _) = note_at(&app2, &s, &rec, &tid2, size)?;
        let end = (from + limit).min(size);
        let mut entries = vec![];
        for i in from..end {
            let cose = s.log().entry(i).map_err(|e| e.to_string())?;
            let leaf = s.log().leaf_hash_at(i).map_err(|e| e.to_string())?;
            let pr = s
                .log()
                .inclusion_proof(i, size)
                .map_err(|e| e.to_string())?;
            entries.push(json!({"statement_cose_hex": hex::encode(&cose),
                "receipt": {"v":1,"origin":cp.origin,"leaf_index":i,"tree_size":size,
                    "leaf_hash":hex::encode(leaf),
                    "inclusion_proof":pr.iter().map(hex::encode).collect::<Vec<_>>(),
                    "checkpoint":note}}));
        }
        Ok(json!({"v":1,"trust":trust,
            "entries":entries,
            "policy":{"min_completeness":"C0","min_assurance":"A0","max_open_intents":u32::MAX}}))
    })
    .await
    .expect("join");
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(StatusCode::NOT_FOUND, &e),
    }
}

#[derive(Deserialize)]
struct NewWitness {
    name: String,
    public_key: String,
    min_witnesses: Option<u32>,
}

/// Register an independent witness for this tenant's log (its key is then part of the pinned trust).
async fn add_witness(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (tid, _) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<NewWitness>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    let pk: Option<[u8; 32]> = hex::decode(&b.public_key)
        .ok()
        .and_then(|v| v.try_into().ok());
    let key = pk
        .and_then(|p| VerifyingKey::from_bytes(&p).ok())
        .filter(|k| !k.is_weak());
    if !vc_log::checkpoint::valid_name(&b.name) || b.name == "operator" || key.is_none() {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid witness name or public key",
        );
    }
    let mut ts = app.tenants.lock().unwrap();
    let t = ts.get_mut(&tid).expect("authenticated tenant");
    if t.rec.witnesses.len() >= 16 {
        return err(StatusCode::CONFLICT, "witness limit reached");
    }
    if t.rec
        .witnesses
        .iter()
        .any(|w| w.name == b.name || w.public_key == b.public_key)
        || b.public_key == app.operator_public_key()
    {
        return err(
            StatusCode::CONFLICT,
            "witness name or key already used, or equals the operator key",
        );
    }
    let old = t.rec.clone();
    t.rec.witnesses.push(WitnessRec {
        name: b.name.clone(),
        public_key: b.public_key,
    });
    let n = t.rec.witnesses.len() as u32;
    t.rec.min_witnesses = b.min_witnesses.unwrap_or(n).clamp(1, n);
    if let Err(e) = app.save(&ts) {
        ts.get_mut(&tid).unwrap().rec = old;
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    (
        StatusCode::CREATED,
        Json(
            json!({"witness": b.name, "min_witnesses": n.min(b.min_witnesses.unwrap_or(n).max(1))}),
        ),
    )
        .into_response()
}

#[derive(Deserialize)]
struct ConsQuery {
    from: Option<u64>,
}

/// What a witness needs to call add-checkpoint: the current signed checkpoint and a consistency proof from `from`.
async fn consistency(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<ConsQuery>,
) -> Response {
    let (_, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let from = q.from.unwrap_or(0);
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let size = s.size();
        let (_, note) = s.checkpoint().map_err(|e| e.to_string())?;
        let proof = if from == 0 || from >= size {
            vec![]
        } else {
            s.log()
                .consistency_proof(from, size)
                .map_err(|e| e.to_string())?
        };
        Ok::<_, String>(json!({"tree_size": size, "checkpoint": note, "from": from,
            "proof": proof.iter().map(hex::encode).collect::<Vec<_>>(),
            "proof_b64": proof.iter().map(|h| base64::engine::general_purpose::STANDARD.encode(h)).collect::<Vec<_>>()}))
    })
    .await
    .expect("join");
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(StatusCode::SERVICE_UNAVAILABLE, &e),
    }
}

#[derive(Deserialize)]
struct NewCosig {
    size: u64,
    line: String,
}

/// Accept a witness cosignature line for the checkpoint at `size`; stored only if it verifies for a registered witness.
async fn add_cosig(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<NewCosig>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    if b.line.len() > 400 || b.size == 0 || !b.line.ends_with('\n') {
        return err(StatusCode::BAD_REQUEST, "invalid cosignature line");
    }
    let rec = rec_of(&app, &tid);
    let app2 = app.clone();
    let tid2 = tid.clone();
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        if b.size > s.size() {
            return Err((
                StatusCode::BAD_REQUEST,
                "checkpoint size is beyond the log".to_string(),
            ));
        }
        let (cp, _, _) = note_at(&app2, &s, &rec, &tid2, b.size)
            .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;
        let text = cp.text();
        let op = KeyInfo {
            name: "operator".into(),
            key: app2.operator.verifying_key(),
        };
        let cand = note(
            &text,
            &[sign_line(&text, "operator", &app2.operator), b.line.clone()],
        );
        let Some(k) = witness_keys(&rec)
            .into_iter()
            .find(|k| verify_note(&cand, &op, std::slice::from_ref(k), 1).is_ok())
        else {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "not a valid cosignature from a registered witness for this checkpoint".to_string(),
            ));
        };
        let mut all = load_cosigs(&app2, &tid2);
        let v = all.entry(b.size).or_default();
        // one line per witness name per size: replace the previous one
        let prefix = format!("\u{2014} {} ", k.name);
        v.retain(|l| !l.starts_with(&prefix));
        v.push(b.line);
        let data = serde_json::to_vec(&all)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        atomic_write(&cosig_path(&app2, &tid2), &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok(k.name)
    })
    .await
    .expect("join");
    match r {
        Ok(n) => Json(json!({"accepted": n})).into_response(),
        Err((c, m)) => err(c, &m),
    }
}

const APPROVER_ID: &str = "approval-service";
const CHAL_TTL: u64 = 300;

#[allow(clippy::result_large_err)]
fn new_challenge(app: &App, tid: &str, kind: ChalKind, raw: [u8; 32]) -> Result<String, Response> {
    let ch = webauthn::b64u(&raw);
    let mut m = app.challenges.lock().unwrap();
    let now = now_secs();
    m.retain(|_, c| c.expires > now);
    if m.len() >= 10_000 {
        return Err(err(
            StatusCode::TOO_MANY_REQUESTS,
            "too many pending challenges",
        ));
    }
    m.insert(
        ch.clone(),
        Chal {
            tid: tid.to_string(),
            kind,
            expires: now + CHAL_TTL,
        },
    );
    Ok(ch)
}

/// Single-use: removes and returns the challenge if it belongs to `tid` and has not expired.
fn take_challenge(app: &App, tid: &str, ch: &str) -> Option<ChalKind> {
    let c = app.challenges.lock().unwrap().remove(ch)?;
    (c.tid == tid && c.expires > now_secs()).then_some(c.kind)
}

async fn approver_challenge(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (tid, _) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let mut raw = [0u8; 32];
    if getrandom(&mut raw).is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "randomness unavailable");
    }
    match new_challenge(&app, &tid, ChalKind::Register, raw) {
        Ok(c) => {
            Json(json!({"challenge": c, "rp_id": app.rp_id, "origin": app.origin})).into_response()
        }
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct NewApprover {
    name: String,
    credential_id: String,
    public_key: String,
    client_data_json: String,
    challenge: String,
}

/// Register a passkey as an approver. The browser supplies the credential id and SPKI public key
/// (`getPublicKey()`); attestation is not evaluated: the tenant API key is what authorises enrolment.
async fn add_approver(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (tid, _) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<NewApprover>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    if !matches!(
        take_challenge(&app, &tid, &b.challenge),
        Some(ChalKind::Register)
    ) {
        return err(StatusCode::BAD_REQUEST, "unknown or expired challenge");
    }
    let (Some(cd), Some(spki)) = (
        webauthn::unb64u(&b.client_data_json),
        webauthn::unb64u(&b.public_key),
    ) else {
        return err(StatusCode::BAD_REQUEST, "invalid base64url");
    };
    if let Err(e) = webauthn::verify_registration(&cd, &b.challenge, &app.origin) {
        return err(StatusCode::BAD_REQUEST, e);
    }
    if webauthn::parse_spki(&spki).is_err()
        || b.name.is_empty()
        || b.name.len() > 64
        || b.name.chars().any(char::is_control)
        || webauthn::unb64u(&b.credential_id).is_none_or(|c| c.is_empty() || c.len() > 1024)
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid approver name, credential id or public key",
        );
    }
    let mut ts = app.tenants.lock().unwrap();
    let t = ts.get_mut(&tid).expect("authenticated tenant");
    if t.rec.approvers.len() >= 32
        || t.rec
            .approvers
            .iter()
            .any(|a| a.credential_id == b.credential_id)
    {
        return err(
            StatusCode::CONFLICT,
            "approver limit reached or credential already enrolled",
        );
    }
    t.rec.approvers.push(ApproverRec {
        name: b.name.clone(),
        credential_id: b.credential_id,
        public_key: b.public_key,
        sign_count: 0,
    });
    if let Err(e) = app.save(&ts) {
        ts.get_mut(&tid).unwrap().rec.approvers.pop();
        return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
    }
    (StatusCode::CREATED, Json(json!({"approver": b.name}))).into_response()
}

/// Gated intents that have no approval statement yet (derived from the log, never from side state).
async fn pending_approvals(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let (_, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let r = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let mut gated: Vec<(String, Value)> = vec![];
        let mut decided: std::collections::HashSet<String> = Default::default();
        s.log()
            .scan(0, |_, e| {
                if let Ok((_, p)) = vc_cose::peek_payload(e) {
                    if let Ok(v) = serde_json::from_slice::<Value>(&p) {
                        match v["kind"].as_str() {
                            Some("intent")
                                if v.pointer("/ext/requires_approval")
                                    == Some(&Value::Bool(true)) =>
                            {
                                if let Ok(id) = vc_core::chain::statement_id(&v) {
                                    gated.push((id, v));
                                }
                            }
                            Some("approval") => {
                                if let Some(a) =
                                    v.pointer("/ext/approval/action_id").and_then(Value::as_str)
                                {
                                    decided.insert(a.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok::<(), vc_store::StoreError>(())
            })
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(
            gated
                .into_iter()
                .filter(|(id, _)| !decided.contains(id))
                .map(|(id, v)| {
                    json!({
            "action_id": id, "agent": v["agent"]["id"], "ts": v["ts"],
            "summary": v.pointer("/ext/summary").cloned().unwrap_or(Value::Null),
            "action_class": v.pointer("/ext/action_class").cloned().unwrap_or(Value::Null)})
                })
                .collect::<Vec<_>>(),
        )
    })
    .await
    .expect("join");
    match r {
        Ok(v) => Json(json!({"pending": v})).into_response(),
        Err(e) => err(StatusCode::SERVICE_UNAVAILABLE, &e),
    }
}

#[derive(Deserialize)]
struct ApprovalChallengeReq {
    action_id: String,
    decision: String,
}

fn approval_challenge_bytes(
    origin: &str,
    action_id: &str,
    decision: &str,
    nonce: &str,
) -> [u8; 32] {
    Sha256::digest(format!("vc-approval-v1\n{origin}\n{action_id}\n{decision}\n{nonce}").as_bytes())
        .into()
}

async fn approval_challenge(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<ApprovalChallengeReq>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    if !matches!(b.decision.as_str(), "approve" | "deny") || b.action_id.len() != 64 {
        return err(
            StatusCode::BAD_REQUEST,
            "decision must be approve or deny; action_id a 64-hex statement id",
        );
    }
    let rec = rec_of(&app, &tid);
    if rec.approvers.is_empty() {
        return err(StatusCode::CONFLICT, "no approver passkey enrolled");
    }
    let aid = b.action_id.clone();
    let exists = tokio::task::spawn_blocking(move || {
        let s = seq.lock().unwrap();
        let mut found = false;
        let _ = s.log().scan(0, |_, e| {
            if let Ok((_, p)) = vc_cose::peek_payload(e) {
                if let Ok(v) = serde_json::from_slice::<Value>(&p) {
                    if v["kind"] == "intent"
                        && vc_core::chain::statement_id(&v).is_ok_and(|id| id == aid)
                    {
                        found = true;
                    }
                }
            }
            Ok::<(), vc_store::StoreError>(())
        });
        found
    })
    .await
    .expect("join");
    if !exists {
        return err(StatusCode::NOT_FOUND, "no such intent in this log");
    }
    let mut nraw = [0u8; 16];
    if getrandom(&mut nraw).is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "randomness unavailable");
    }
    let nonce = hex::encode(nraw);
    let raw = approval_challenge_bytes(&app.origin, &b.action_id, &b.decision, &nonce);
    match new_challenge(&app, &tid, ChalKind::Approve { action_id: b.action_id, decision: b.decision, nonce: nonce.clone() }, raw) {
        Ok(c) => Json(json!({"challenge": c, "nonce": nonce, "rp_id": app.rp_id,
            "credential_ids": rec.approvers.iter().map(|a| a.credential_id.clone()).collect::<Vec<_>>()})).into_response(),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
struct ApprovalSubmit {
    challenge: String,
    credential_id: String,
    authenticator_data: String,
    client_data_json: String,
    signature: String,
}

fn approver_key(app: &App, tid: &str) -> Result<SigningKey, String> {
    let p = app.dir.join("logs").join(tid).join("approver.key");
    if let Ok(s) = std::fs::read_to_string(&p) {
        let b: [u8; 32] = hex::decode(s.trim())
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "bad approver key")?;
        return Ok(SigningKey::from_bytes(&b));
    }
    let mut b = [0u8; 32];
    getrandom(&mut b)?;
    atomic_write(&p, hex::encode(b).as_bytes()).map_err(|e| e.to_string())?;
    Ok(SigningKey::from_bytes(&b))
}

/// Verify the passkey assertion for a pending approval challenge and, only then, append the
/// `approval` statement (signed by the tenant's approval-service key) to the log.
async fn submit_approval(State(app): State<Arc<App>>, headers: HeaderMap, body: Bytes) -> Response {
    let (tid, seq) = match app.auth(&headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let Ok(b) = serde_json::from_slice::<ApprovalSubmit>(&body) else {
        return err(StatusCode::BAD_REQUEST, "invalid JSON body");
    };
    let Some(ChalKind::Approve {
        action_id,
        decision,
        nonce,
    }) = take_challenge(&app, &tid, &b.challenge)
    else {
        return err(
            StatusCode::BAD_REQUEST,
            "unknown, expired or already used challenge",
        );
    };
    let (Some(ad), Some(cd), Some(sig)) = (
        webauthn::unb64u(&b.authenticator_data),
        webauthn::unb64u(&b.client_data_json),
        webauthn::unb64u(&b.signature),
    ) else {
        return err(StatusCode::BAD_REQUEST, "invalid base64url");
    };
    let rec = rec_of(&app, &tid);
    let Some(ap) = rec
        .approvers
        .iter()
        .find(|a| a.credential_id == b.credential_id)
    else {
        return err(
            StatusCode::UNAUTHORIZED,
            "credential is not an enrolled approver",
        );
    };
    let key = match webauthn::unb64u(&ap.public_key)
        .ok_or("bad key")
        .and_then(|k| webauthn::parse_spki(&k).map_err(|_| "bad key"))
    {
        Ok(k) => k,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let count = match webauthn::verify_assertion(
        &app.rp_id,
        &app.origin,
        &b.challenge,
        &key,
        ap.sign_count,
        &ad,
        &cd,
        &sig,
    ) {
        Ok(c) => c,
        Err(e) => return err(StatusCode::UNAUTHORIZED, e),
    };
    // the challenge must be exactly the one derived for this action and decision
    let expect = webauthn::b64u(&approval_challenge_bytes(
        &app.origin,
        &action_id,
        &decision,
        &nonce,
    ));
    if expect != b.challenge {
        return err(
            StatusCode::BAD_REQUEST,
            "challenge does not match the action",
        );
    }
    let approver_name = ap.name.clone();
    // persist the counter first: a replayed assertion can never be accepted twice
    {
        let mut ts = app.tenants.lock().unwrap();
        let t = ts.get_mut(&tid).expect("tenant");
        if let Some(a) = t
            .rec
            .approvers
            .iter_mut()
            .find(|a| a.credential_id == b.credential_id)
        {
            a.sign_count = count;
        }
        if let Err(e) = app.save(&ts) {
            return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
        }
    }
    let sk = match approver_key(&app, &tid) {
        Ok(k) => k,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e),
    };
    // make sure the approval-service issuer is known to the log and to the pinned trust
    {
        let pk = hex::encode(sk.verifying_key().to_bytes());
        let mut ts = app.tenants.lock().unwrap();
        let t = ts.get_mut(&tid).expect("tenant");
        if !t.rec.agents.iter().any(|a| a.id == APPROVER_ID) {
            t.rec.agents.push(AgentRec {
                id: APPROVER_ID.into(),
                public_key: pk.clone(),
                max_completeness: 2,
                max_assurance: 1,
            });
            if let Err(e) = app.save(&ts) {
                return err(StatusCode::INTERNAL_SERVER_ERROR, &e);
            }
        }
    }
    let credential_id = b.credential_id.clone();
    let r = tokio::task::spawn_blocking(move || {
        let mut s = seq.lock().unwrap();
        s.register_issuer(
            APPROVER_ID.as_bytes(),
            Issuer { key: sk.verifying_key(), agent_id: APPROVER_ID.into(), max_completeness: 2, max_assurance: 1 },
        );
        let (n, prev) = s.next_for(APPROVER_ID);
        let v = json!({"v":2,"kind":"approval","agent":{"id":APPROVER_ID},"seq":n,"prev":prev,
            "ts": iso(now_secs()),"completeness":"C1","assurance":["A0"],
            "ext":{"approval":{"action_id":action_id,"decision":decision,"approver":approver_name,
                "credential_id":credential_id,"nonce":nonce,
                "webauthn":{"authenticator_data":webauthn::b64u(&ad),"client_data_json":webauthn::b64u(&cd),"signature":webauthn::b64u(&sig)}}}});
        let payload = vc_core::canonical::canonicalize(&v).map_err(|e| e.to_string())?;
        let cose = vc_cose::sign1(&sk, APPROVER_ID.as_bytes(), "application/vc-statement+json", &payload);
        s.submit(&cose).map_err(|e| e.to_string())
    })
    .await
    .expect("join");
    match r {
        Ok(reg) => Json(json!({"index": reg.index, "statement_id": reg.statement_id, "ok": true}))
            .into_response(),
        Err(e) => err(StatusCode::UNPROCESSABLE_ENTITY, &e),
    }
}

async fn page() -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"),
      (header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'"),
      (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], DASHBOARD).into_response()
}

async fn approve_page() -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"),
      (header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; manifest-src 'self'; base-uri 'none'; frame-ancestors 'none'"),
      (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], APPROVE).into_response()
}

async fn manifest() -> Response {
    (
        [(header::CONTENT_TYPE, "application/manifest+json")],
        MANIFEST,
    )
        .into_response()
}

/// Readiness for monitors: the data directory is writable and, when a collector is expected, it is running.
async fn ready(State(app): State<Arc<App>>) -> Response {
    let probe = app.dir.join(".readyz");
    let writable = std::fs::write(&probe, b"ok")
        .and_then(|()| std::fs::remove_file(&probe))
        .is_ok();
    let collector_ok = !app
        .collect_expected
        .load(std::sync::atomic::Ordering::SeqCst)
        || app.trading.is_running();
    if writable && collector_ok {
        (StatusCode::OK, "ready").into_response()
    } else {
        let why = if writable {
            "collector not running"
        } else {
            "data directory not writable"
        };
        (StatusCode::SERVICE_UNAVAILABLE, why).into_response()
    }
}

async fn health() -> &'static str {
    "ok"
}

const THROTTLE_WINDOW_S: u64 = 60;
const THROTTLE_MAX_FAILS: usize = 20;

/// Slows down token guessing: more than 20 rejected (401) requests from one client in 60 s get 429 until the
/// window clears. Behind a proxy set VC_CLIENT_IP_HEADER (Caddy: x-real-ip, set by the proxy) or VC_TRUST_CF=1
/// (Cloudflare) so each client has its own budget; otherwise all clients share one budget (safe default: under attack legitimate users are slowed too, but nothing is guessable).
async fn throttle_mw(
    State(app): State<Arc<App>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let key = match &app.client_hdr {
        Some(h) => req
            .headers()
            .get(h.as_str())
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= 64)
            .unwrap_or("unknown")
            .to_string(),
        None => "all".to_string(),
    };
    let now = now_secs();
    {
        let mut m = app.fails.lock().unwrap();
        if m.len() > 10_000 {
            m.clear();
        }
        let v = m.entry(key.clone()).or_default();
        v.retain(|t| now.saturating_sub(*t) < THROTTLE_WINDOW_S);
        if v.len() >= THROTTLE_MAX_FAILS {
            let mut r = err(
                StatusCode::TOO_MANY_REQUESTS,
                "too many failed attempts; slow down",
            );
            r.headers_mut()
                .insert(header::RETRY_AFTER, header::HeaderValue::from_static("60"));
            return r;
        }
    }
    let (method, path) = (req.method().clone(), req.uri().path().to_string());
    let resp = next.run(req).await;
    let status = resp.status();
    if status == StatusCode::UNAUTHORIZED {
        app.fails
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_default()
            .push(now);
    }
    if path.starts_with("/admin/v1/") && method != axum::http::Method::GET {
        if status.is_success() {
            app.audit.append(
                "admin_action",
                &format!("{method} {path} {}", status.as_u16()),
            );
        } else if status == StatusCode::UNAUTHORIZED {
            app.audit.append(
                "admin_auth_failed",
                &format!("{method} {path} client={key}"),
            );
        }
    }
    resp
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(page))
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/admin/v1/audit", get(audit::list))
        .route("/approve", get(approve_page))
        .route("/manifest.webmanifest", get(manifest))
        .route("/admin/v1/tenants", post(create_tenant))
        .route("/admin/v1/trading/start", post(trading::start))
        .route("/admin/v1/trading/stop", post(trading::stop))
        .route("/admin/v1/trading/status", get(trading::status))
        .route("/v1/agents", post(add_agent))
        .route("/v1/statements", post(ingest))
        .route("/v1/checkpoint", get(checkpoint))
        .route("/v1/anchor", post(anchor::create))
        .route("/v1/anchors", get(anchor::list))
        .route("/v1/entries/{i}/proof", get(proof))
        .route("/v1/dashboard", get(dashboard_api))
        .route("/v1/trust", get(trust))
        .route("/v1/witnesses", post(add_witness))
        .route("/v1/checkpoint/consistency", get(consistency))
        .route("/v1/cosignatures", post(add_cosig))
        .route("/v1/approvers/challenge", post(approver_challenge))
        .route("/v1/approvers", post(add_approver))
        .route("/v1/approvals/pending", get(pending_approvals))
        .route("/v1/approvals/challenge", post(approval_challenge))
        .route("/v1/approvals", post(submit_approval))
        .route("/v1/pack", get(pack))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            throttle_mw,
        ))
        .with_state(app)
}
