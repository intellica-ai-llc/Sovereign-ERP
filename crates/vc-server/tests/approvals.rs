//! Passkey approvals end to end with a software authenticator (P-256): enrol, request, assert, log, verify.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B, Engine};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use p256::ecdsa::{signature::Signer, Signature, SigningKey as P256Key};
use p256::pkcs8::EncodePublicKey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const ADMIN: &str = "admin-token-admin-token-1234";
const ORIGIN: &str = "https://vc.test";
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "vcs-ap-{}-{}",
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
    m: &str,
    uri: &str,
    tok: &str,
    body: Value,
) -> (StatusCode, Value) {
    let r = Request::builder()
        .method(m)
        .uri(uri)
        .header("authorization", format!("Bearer {tok}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(r).await.unwrap();
    let s = resp.status();
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}
async fn raw(app: &axum::Router, uri: &str, tok: &str, body: Vec<u8>) -> (StatusCode, Value) {
    let r = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {tok}"))
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(r).await.unwrap();
    let s = resp.status();
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}
fn stmt(
    sk: &SigningKey,
    agent: &str,
    seq: u64,
    prev: &str,
    kind: &str,
    ext: Value,
) -> (Vec<u8>, String) {
    let v = json!({"v":2,"kind":kind,"agent":{"id":agent},"seq":seq,"prev":prev,"ts":"2026-11-01T00:00:00Z","completeness":"C1","assurance":["A0"],"ext":ext});
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

struct Authenticator {
    key: P256Key,
    count: u32,
}
impl Authenticator {
    fn assertion(&mut self, challenge: &str, uv: bool, origin: &str) -> Value {
        self.count += 1;
        let mut ad = Sha256::digest(b"vc.test").to_vec();
        ad.push(if uv { 0x05 } else { 0x01 });
        ad.extend_from_slice(&self.count.to_be_bytes());
        let cd = serde_json::to_vec(
            &json!({"type":"webauthn.get","challenge":challenge,"origin":origin}),
        )
        .unwrap();
        let mut msg = ad.clone();
        msg.extend_from_slice(&Sha256::digest(&cd));
        let sig: Signature = self.key.sign(&msg);
        json!({"authenticator_data":B.encode(&ad),"client_data_json":B.encode(&cd),"signature":B.encode(sig.to_der().as_bytes())})
    }
}

#[tokio::test]
async fn passkey_approval_flow_and_attacks() {
    let app = vc_server::router(vc_server::App::open(tmp(), "vc.test", ADMIN).unwrap());
    let (_, t) = call(
        &app,
        "POST",
        "/admin/v1/tenants",
        ADMIN,
        json!({"id":"acme","name":"Acme"}),
    )
    .await;
    let key = t["api_key"].as_str().unwrap().to_string();
    let agent = SigningKey::from_bytes(&[7u8; 32]);
    call(
        &app,
        "POST",
        "/v1/agents",
        &key,
        json!({"id":"payer","public_key":hex::encode(agent.verifying_key().to_bytes())}),
    )
    .await;

    // enrol a passkey
    let pk = P256Key::from_bytes(&[5u8; 32].into()).unwrap();
    let mut au = Authenticator {
        key: pk.clone(),
        count: 0,
    };
    let (_, ch) = call(&app, "POST", "/v1/approvers/challenge", &key, json!({})).await;
    let chal = ch["challenge"].as_str().unwrap();
    let cd =
        serde_json::to_vec(&json!({"type":"webauthn.create","challenge":chal,"origin":ORIGIN}))
            .unwrap();
    let spki = pk.verifying_key().to_public_key_der().unwrap();
    let enrol = json!({"name":"Dana","credential_id":B.encode(b"cred-1"),"public_key":B.encode(spki.as_bytes()),"client_data_json":B.encode(&cd),"challenge":chal});
    assert_eq!(
        call(&app, "POST", "/v1/approvers", &key, enrol.clone())
            .await
            .0,
        StatusCode::CREATED
    );
    // the challenge is single use
    assert_eq!(
        call(&app, "POST", "/v1/approvers", &key, enrol).await.0,
        StatusCode::BAD_REQUEST
    );

    // an agent files a gated intent; it shows up as pending
    let (c0, intent_id) = stmt(
        &agent,
        "payer",
        0,
        ZERO,
        "intent",
        json!({"requires_approval":true,"summary":"Pay 100 USD to X","action_class":"payment"}),
    );
    assert_eq!(
        raw(&app, "/v1/statements", &key, c0).await.0,
        StatusCode::OK
    );
    let (_, p) = call(&app, "GET", "/v1/approvals/pending", &key, json!({})).await;
    assert_eq!(p["pending"][0]["action_id"], intent_id);

    // approval challenge for an unknown action is refused
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/approvals/challenge",
            &key,
            json!({"action_id":"ab".repeat(32),"decision":"approve"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (s, ac) = call(
        &app,
        "POST",
        "/v1/approvals/challenge",
        &key,
        json!({"action_id":intent_id,"decision":"approve"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let chal = ac["challenge"].as_str().unwrap().to_string();
    let cred = B.encode(b"cred-1");

    // attacks: user verification missing, wrong origin, unknown credential, tampered signature
    let mk = |a: &Value, c: &str, cr: &str| json!({"challenge":c,"credential_id":cr,"authenticator_data":a["authenticator_data"],"client_data_json":a["client_data_json"],"signature":a["signature"]});
    let (_, ac2) = call(
        &app,
        "POST",
        "/v1/approvals/challenge",
        &key,
        json!({"action_id":intent_id,"decision":"approve"}),
    )
    .await;
    let c2 = ac2["challenge"].as_str().unwrap().to_string();
    let no_uv = au.assertion(&c2, false, ORIGIN);
    assert_eq!(
        call(&app, "POST", "/v1/approvals", &key, mk(&no_uv, &c2, &cred))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, ac3) = call(
        &app,
        "POST",
        "/v1/approvals/challenge",
        &key,
        json!({"action_id":intent_id,"decision":"approve"}),
    )
    .await;
    let c3 = ac3["challenge"].as_str().unwrap().to_string();
    let evil = au.assertion(&c3, true, "https://evil.example");
    assert_eq!(
        call(&app, "POST", "/v1/approvals", &key, mk(&evil, &c3, &cred))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // the real approval
    let good = au.assertion(&chal, true, ORIGIN);
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/approvals",
            &key,
            mk(&good, &chal, &B.encode(b"nope"))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    ); // challenge consumed by wrong-credential attempt
    let (_, ac4) = call(
        &app,
        "POST",
        "/v1/approvals/challenge",
        &key,
        json!({"action_id":intent_id,"decision":"approve"}),
    )
    .await;
    let c4 = ac4["challenge"].as_str().unwrap().to_string();
    let good = au.assertion(&c4, true, ORIGIN);
    let mut bad = mk(&good, &c4, &cred);
    let mut sig = B.decode(good["signature"].as_str().unwrap()).unwrap();
    let n = sig.len() - 2;
    sig[n] ^= 1;
    bad["signature"] = json!(B.encode(sig));
    assert_eq!(
        call(&app, "POST", "/v1/approvals", &key, bad).await.0,
        StatusCode::UNAUTHORIZED
    );
    // fresh challenge, honest assertion succeeds
    let (_, ac5) = call(
        &app,
        "POST",
        "/v1/approvals/challenge",
        &key,
        json!({"action_id":intent_id,"decision":"approve"}),
    )
    .await;
    let c5 = ac5["challenge"].as_str().unwrap().to_string();
    let good = au.assertion(&c5, true, ORIGIN);
    let (s, r) = call(&app, "POST", "/v1/approvals", &key, mk(&good, &c5, &cred)).await;
    assert_eq!(s, StatusCode::OK, "{r}");
    // replay of the same assertion bytes fails (challenge consumed and counter did not increase)
    assert_ne!(
        call(&app, "POST", "/v1/approvals", &key, mk(&good, &c5, &cred))
            .await
            .0,
        StatusCode::OK
    );
    // approved action is no longer pending
    let (_, p) = call(&app, "GET", "/v1/approvals/pending", &key, json!({})).await;
    assert_eq!(p["pending"].as_array().unwrap().len(), 0);

    // the agent now completes the action; the pack passes approval_order with the pinned class gated
    let (c1, _) = stmt(&agent, "payer", 1, &intent_id, "outcome", json!({}));
    let mut v: Value = serde_json::from_slice(&vc_cose::peek_payload(&c1).unwrap().1).unwrap();
    v["pair"] = json!({"intent_id": intent_id});
    let id1 = vc_core::chain::statement_id(&v).unwrap();
    let _ = id1;
    let c1 = vc_cose::sign1(
        &agent,
        b"payer",
        "application/vc-statement+json",
        &vc_core::canonical::canonicalize(&v).unwrap(),
    );
    assert_eq!(
        raw(&app, "/v1/statements", &key, c1).await.0,
        StatusCode::OK
    );
    let r = Request::builder()
        .uri("/v1/pack")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(r).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let pb = resp.into_body().collect().await.unwrap().to_bytes();
    let r = Request::builder()
        .uri("/v1/trust")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::empty())
        .unwrap();
    let tb = app
        .clone()
        .oneshot(r)
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let pins = vc_verify::Pins {
        trust: Some(serde_json::from_slice(&tb).unwrap()),
        policy: Some(serde_json::from_value(json!({"min_completeness":"C0","min_assurance":"A0","max_open_intents":4294967295u32,"gated_classes":["payment"]})).unwrap()),
        require_seals: false,
    };
    let rep = vc_verify::verify_pack_bytes(&pb, Some(&pins));
    let failed: Vec<_> = rep
        .checks
        .iter()
        .filter(|c| !c.ok)
        .map(|c| c.name.as_str())
        .collect();
    // no witnesses in this test, so only the quorum (and the pack's own weaker policy) may fail
    assert!(
        failed
            .iter()
            .all(|n| ["checkpoint_witness_quorum", "policy_pinned"].contains(n)),
        "{failed:?}"
    );
    assert!(
        rep.checks
            .iter()
            .any(|c| c.name == "approval_order" && c.ok),
        "{:#?}",
        rep.checks
    );
}
