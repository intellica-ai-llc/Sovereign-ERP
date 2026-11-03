//! Seeds a data dir for the browser test (run with: VC_SEED_DIR=/path cargo test -p vc-server --test seed_browser -- --ignored).
use axum::{body::Body, http::Request};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
#[ignore]
async fn seed() {
    let dir = std::path::PathBuf::from(std::env::var("VC_SEED_DIR").unwrap());
    let app = vc_server::router(
        vc_server::App::open(
            dir.clone(),
            &std::env::var("VC_SEED_HOST").unwrap_or("localhost".into()),
            "admin-token-admin-token-1234",
        )
        .unwrap(),
    );
    let go = |m: &str, u: &str, tok: &str, b: Vec<u8>| {
        let r = Request::builder()
            .method(m)
            .uri(u)
            .header("authorization", format!("Bearer {tok}"))
            .header("content-type", "application/json")
            .body(Body::from(b))
            .unwrap();
        let a = app.clone();
        async move {
            a.oneshot(r)
                .await
                .unwrap()
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
        }
    };
    let t: Value = serde_json::from_slice(
        &go(
            "POST",
            "/admin/v1/tenants",
            "admin-token-admin-token-1234",
            serde_json::to_vec(&json!({"id":"demo","name":"Demo"})).unwrap(),
        )
        .await,
    )
    .unwrap();
    let key = t["api_key"].as_str().unwrap().to_string();
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    go(
        "POST",
        "/v1/agents",
        &key,
        serde_json::to_vec(
            &json!({"id":"payer","public_key":hex::encode(sk.verifying_key().to_bytes())}),
        )
        .unwrap(),
    )
    .await;
    let v = json!({"v":2,"kind":"intent","agent":{"id":"payer"},"seq":0,"prev":"0".repeat(64),"ts":"2026-11-01T00:00:00Z","completeness":"C1","assurance":["A0"],
        "ext":{"requires_approval":true,"summary":"Pay 100 USD to Supplier X","action_class":"payment"}});
    let cose = vc_cose::sign1(
        &sk,
        b"payer",
        "application/vc-statement+json",
        &vc_core::canonical::canonicalize(&v).unwrap(),
    );
    let r = Request::builder()
        .method("POST")
        .uri("/v1/statements")
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(cose))
        .unwrap();
    app.clone().oneshot(r).await.unwrap();
    let id0 = vc_core::chain::statement_id(&v).unwrap();
    let v1 = json!({"v":2,"kind":"lifecycle","agent":{"id":"payer"},"seq":1,"prev":id0,"ts":"2026-11-01T00:00:01Z","completeness":"C1","assurance":["A0"]});
    std::fs::write(
        dir.join("stmt1.cose"),
        vc_cose::sign1(
            &sk,
            b"payer",
            "application/vc-statement+json",
            &vc_core::canonical::canonicalize(&v1).unwrap(),
        ),
    )
    .unwrap();
    std::fs::write(dir.join("SEED_KEY"), key).unwrap();
}
