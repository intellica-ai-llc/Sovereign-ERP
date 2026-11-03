use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const ADMIN: &str = "admin-token-admin-token-1234";

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "vcs-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}
async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    tok: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let mut r = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = tok {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let resp = app
        .clone()
        .oneshot(r.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let s = resp.status();
    (
        s,
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}
async fn tenant(app: &axum::Router, id: &str) -> String {
    let (s, b) = call(
        app,
        "POST",
        "/admin/v1/tenants",
        Some(ADMIN),
        serde_json::to_vec(&json!({"id": id, "name": "Acme"})).unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    serde_json::from_slice::<Value>(&b).unwrap()["api_key"]
        .as_str()
        .unwrap()
        .to_string()
}
fn stmt(sk: &SigningKey, agent: &str, seq: u64, prev: &str) -> (Vec<u8>, String) {
    let v = json!({"v":2,"kind":"lifecycle","agent":{"id":agent},"seq":seq,"prev":prev,"ts":"2026-11-01T00:00:00Z","completeness":"C1","assurance":["A0"]});
    let id = vc_core::chain::statement_id(&v).unwrap();
    (
        vc_cose::sign1(
            sk,
            agent.as_bytes(),
            "application/vc-statement+json",
            &vc_core::canonical::canonicalize(&v).unwrap(),
        ),
        id,
    )
}
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[tokio::test]
async fn end_to_end_and_isolation() {
    let dir = tmp("e2e");
    let app = vc_server::router(vc_server::App::open(dir.clone(), "vc.test", ADMIN).unwrap());
    let ka = tenant(&app, "acme").await;
    let kb = tenant(&app, "beta").await;
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let reg = json!({"id":"agent-1","public_key":hex::encode(sk.verifying_key().to_bytes())});
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/agents",
            Some(&ka),
            serde_json::to_vec(&reg).unwrap()
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (c0, id0) = stmt(&sk, "agent-1", 0, ZERO);
    let (s, b) = call(&app, "POST", "/v1/statements", Some(&ka), c0.clone()).await;
    assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
    // idempotent retry
    let (_, b2) = call(&app, "POST", "/v1/statements", Some(&ka), c0.clone()).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&b2).unwrap()["duplicate"],
        true
    );
    let (c1, _) = stmt(&sk, "agent-1", 1, &id0);
    assert_eq!(
        call(&app, "POST", "/v1/statements", Some(&ka), c1).await.0,
        StatusCode::OK
    );
    // tenant B cannot use tenant A's agent
    assert_eq!(
        call(&app, "POST", "/v1/statements", Some(&kb), c0).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // dashboard
    let (s, b) = call(&app, "GET", "/v1/dashboard", Some(&ka), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let d: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(d["size"], 2);
    assert_eq!(d["kinds"]["lifecycle"], 2);
    let (_, b) = call(&app, "GET", "/v1/dashboard", Some(&kb), vec![]).await;
    assert_eq!(serde_json::from_slice::<Value>(&b).unwrap()["size"], 0);
    // checkpoint verifies under the operator key and carries the tenant origin
    let (_, cp) = call(&app, "GET", "/v1/checkpoint", Some(&ka), vec![]).await;
    let cp = String::from_utf8(cp).unwrap();
    assert!(cp.starts_with("vc.test/acme\n2\n"));
    // inclusion proof verifies against the checkpoint root
    let (s, b) = call(&app, "GET", "/v1/entries/1/proof", Some(&ka), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let p: Value = serde_json::from_slice(&b).unwrap();
    let cose = hex::decode(p["statement_cose"].as_str().unwrap()).unwrap();
    let proof: Vec<[u8; 32]> = p["inclusion_proof"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            hex::decode(x.as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap()
        })
        .collect();
    let root = vc_log::checkpoint::Checkpoint::parse(
        &p["checkpoint"].as_str().unwrap()
            [..p["checkpoint"].as_str().unwrap().find("\n\n").unwrap() + 1],
    )
    .unwrap()
    .root;
    assert!(
        vc_log::verify_inclusion(&vc_core::hash::leaf_hash(&cose), 1, 2, &proof, &root).is_ok()
    );
    // evidence pack export verifies offline with the library verifier, pinned to the served trust
    let (s, tb) = call(&app, "GET", "/v1/trust", Some(&ka), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let (s, pb) = call(&app, "GET", "/v1/pack", Some(&ka), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let pins = vc_verify::Pins {
        trust: Some(serde_json::from_slice(&tb).unwrap()),
        policy: None,
        require_seals: false,
    };
    let rep = vc_verify::verify_pack_bytes(&pb, Some(&pins));
    // Unwitnessed: the verifier must fail closed on exactly the witness-quorum check.
    let failed: Vec<_> = rep
        .checks
        .iter()
        .filter(|c| !c.ok)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(failed, vec!["checkpoint_witness_quorum"], "{failed:?}");
    // a pack with one flipped byte must fail
    let mut bad: Value = serde_json::from_slice(&pb).unwrap();
    let h = bad["entries"][0]["statement_cose_hex"]
        .as_str()
        .unwrap()
        .to_string();
    let mut raw = hex::decode(h).unwrap();
    let n = raw.len() - 3;
    raw[n] ^= 1;
    bad["entries"][0]["statement_cose_hex"] = json!(hex::encode(raw));
    assert!(!vc_verify::verify_pack_bytes(&serde_json::to_vec(&bad).unwrap(), Some(&pins)).ok);
    assert_eq!(
        call(&app, "GET", "/v1/pack?from=99", Some(&kb), vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // --- independent witnesses make the pack fully verifiable ---
    let wsk = [
        SigningKey::from_bytes(&[21u8; 32]),
        SigningKey::from_bytes(&[22u8; 32]),
    ];
    for (i, k) in wsk.iter().enumerate() {
        let b = json!({"name": format!("w{}", i + 1), "public_key": hex::encode(k.verifying_key().to_bytes())});
        assert_eq!(
            call(
                &app,
                "POST",
                "/v1/witnesses",
                Some(&ka),
                serde_json::to_vec(&b).unwrap()
            )
            .await
            .0,
            StatusCode::CREATED
        );
    }
    // not yet cosigned: no pack can claim witnessed status
    assert_eq!(
        call(&app, "GET", "/v1/pack", Some(&ka), vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, cb) = call(
        &app,
        "GET",
        "/v1/checkpoint/consistency?from=0",
        Some(&ka),
        vec![],
    )
    .await;
    let cons: Value = serde_json::from_slice(&cb).unwrap();
    let note = cons["checkpoint"].as_str().unwrap().to_string();
    let op_key = SigningKey::from_bytes(&[0u8; 32]); // placeholder to keep types obvious; operator key is server-held
    let _ = op_key;
    let mut lines = vec![];
    for (i, k) in wsk.iter().enumerate() {
        let mut w = vc_witness::Witness::new(
            &format!("w{}", i + 1),
            k.clone(),
            vc_witness::MemState::default(),
        )
        .unwrap();
        let (_, tb2) = call(&app, "GET", "/v1/trust", Some(&ka), vec![]).await;
        let t: Value = serde_json::from_slice(&tb2).unwrap();
        let opk: [u8; 32] = hex::decode(t["operator"]["pubkey_hex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        w.add_log(
            "vc.test/acme",
            vc_log::checkpoint::KeyInfo {
                name: "operator".into(),
                key: ed25519_dalek::VerifyingKey::from_bytes(&opk).unwrap(),
            },
        );
        lines.push(
            w.add_checkpoint(&note, 0, &[], 1_800_000_000 + i as u64)
                .unwrap(),
        );
    }
    // a line from an unregistered key is refused
    let mut rogue = vc_witness::Witness::new(
        "w1",
        SigningKey::from_bytes(&[99u8; 32]),
        vc_witness::MemState::default(),
    )
    .unwrap();
    let (_, tb3) = call(&app, "GET", "/v1/trust", Some(&ka), vec![]).await;
    let opk: [u8; 32] = hex::decode(
        serde_json::from_slice::<Value>(&tb3).unwrap()["operator"]["pubkey_hex"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    rogue.add_log(
        "vc.test/acme",
        vc_log::checkpoint::KeyInfo {
            name: "operator".into(),
            key: ed25519_dalek::VerifyingKey::from_bytes(&opk).unwrap(),
        },
    );
    let rl = rogue.add_checkpoint(&note, 0, &[], 1).unwrap();
    let post = |l: String| serde_json::to_vec(&json!({"size": 2, "line": l})).unwrap();
    assert_eq!(
        call(&app, "POST", "/v1/cosignatures", Some(&ka), post(rl))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // one cosignature is below the quorum of two
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/cosignatures",
            Some(&ka),
            post(lines[0].clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/v1/pack", Some(&ka), vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/cosignatures",
            Some(&ka),
            post(lines[1].clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, tb) = call(&app, "GET", "/v1/trust", Some(&ka), vec![]).await;
    let (s, pb) = call(&app, "GET", "/v1/pack", Some(&ka), vec![]).await;
    assert_eq!(s, StatusCode::OK);
    let pins = vc_verify::Pins {
        trust: Some(serde_json::from_slice(&tb).unwrap()),
        policy: None,
        require_seals: false,
    };
    let rep = vc_verify::verify_pack_bytes(&pb, Some(&pins));
    assert!(
        rep.ok,
        "{:#?}",
        rep.checks.iter().filter(|c| !c.ok).collect::<Vec<_>>()
    );
    // persistence across restart
    let app2 = vc_server::router(vc_server::App::open(dir, "vc.test", ADMIN).unwrap());
    let (_, b) = call(&app2, "GET", "/v1/dashboard", Some(&ka), vec![]).await;
    assert_eq!(serde_json::from_slice::<Value>(&b).unwrap()["size"], 2);
}

#[tokio::test]
async fn auth_and_validation() {
    let app = vc_server::router(vc_server::App::open(tmp("auth"), "vc.test", ADMIN).unwrap());
    assert_eq!(
        call(&app, "GET", "/v1/dashboard", None, vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/v1/dashboard", Some("vck_nope"), vec![])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/admin/v1/tenants",
            Some("wrong"),
            b"{}".to_vec()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let bad = serde_json::to_vec(&json!({"id":"Bad Id!","name":"x"})).unwrap();
    assert_eq!(
        call(&app, "POST", "/admin/v1/tenants", Some(ADMIN), bad)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let k = tenant(&app, "t1").await;
    let dup = serde_json::to_vec(&json!({"id":"t1","name":"x"})).unwrap();
    assert_eq!(
        call(&app, "POST", "/admin/v1/tenants", Some(ADMIN), dup)
            .await
            .0,
        StatusCode::CONFLICT
    );
    // weak (all-zero) key refused; garbage statement refused; oversized body refused
    let weak = serde_json::to_vec(&json!({"id":"a","public_key":hex::encode([0u8;32])})).unwrap();
    assert_eq!(
        call(&app, "POST", "/v1/agents", Some(&k), weak).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&app, "POST", "/v1/statements", Some(&k), b"junk".to_vec())
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(&app, "POST", "/v1/statements", Some(&k), vec![0u8; 80_000])
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        call(&app, "GET", "/v1/entries/9/proof", Some(&k), vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(call(&app, "GET", "/", None, vec![]).await.0, StatusCode::OK);
}

#[test]
fn short_admin_token_refused() {
    assert!(vc_server::App::open(tmp("tok"), "h", "short").is_err());
}
