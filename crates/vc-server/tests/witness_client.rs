//! Real HTTP: two independent witness clients (own keys, own file state) cosign over the network, including a
//! second round that needs a consistency proof; the resulting pack verifies offline.
use axum::{body::Body, http::Request};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::future::IntoFuture;
use tower::ServiceExt;
use vc_log::checkpoint::KeyInfo;
use vc_witness::{client, FileState, Witness};

const ADMIN: &str = "admin-token-admin-token-1234";
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    tok: &str,
    body: Vec<u8>,
) -> (u16, Vec<u8>) {
    let r = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {tok}"))
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(r).await.unwrap();
    (
        resp.status().as_u16(),
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}
fn stmt(sk: &SigningKey, seq: u64, prev: &str) -> (Vec<u8>, String) {
    let v = json!({"v":2,"kind":"lifecycle","agent":{"id":"a1"},"seq":seq,"prev":prev,"ts":"2026-11-01T00:00:00Z","completeness":"C1","assurance":["A0"]});
    let id = vc_core::chain::statement_id(&v).unwrap();
    (
        vc_cose::sign1(
            sk,
            b"a1",
            "application/vc-statement+json",
            &vc_core::canonical::canonicalize(&v).unwrap(),
        ),
        id,
    )
}
fn tmpdir(t: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "vcw-{t}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[tokio::test(flavor = "multi_thread")]
async fn witnesses_cosign_over_http_and_pack_verifies() {
    let app = vc_server::App::open(tmpdir("srv"), "vc.test", ADMIN).unwrap();
    let router = vc_server::router(app.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(axum::serve(l, router.clone()).into_future());

    let (_, b) = call(
        &router,
        "POST",
        "/admin/v1/tenants",
        ADMIN,
        serde_json::to_vec(&json!({"id":"acme","name":"Acme"})).unwrap(),
    )
    .await;
    let key = serde_json::from_slice::<Value>(&b).unwrap()["api_key"]
        .as_str()
        .unwrap()
        .to_string();
    let ask = SigningKey::from_bytes(&[7u8; 32]);
    call(
        &router,
        "POST",
        "/v1/agents",
        &key,
        serde_json::to_vec(
            &json!({"id":"a1","public_key":hex::encode(ask.verifying_key().to_bytes())}),
        )
        .unwrap(),
    )
    .await;
    let wk = [
        SigningKey::from_bytes(&[31u8; 32]),
        SigningKey::from_bytes(&[32u8; 32]),
    ];
    for (i, k) in wk.iter().enumerate() {
        let (s, _) = call(&router, "POST", "/v1/witnesses", &key, serde_json::to_vec(&json!({"name":format!("w{}",i+1),"public_key":hex::encode(k.verifying_key().to_bytes())})).unwrap()).await;
        assert_eq!(s, 201);
    }
    let mut prev = ZERO.to_string();
    for seq in 0..2 {
        let (c, id) = stmt(&ask, seq, &prev);
        prev = id;
        assert_eq!(
            call(&router, "POST", "/v1/statements", &key, c).await.0,
            200
        );
    }
    let opk: [u8; 32] = hex::decode(app.operator_public_key())
        .unwrap()
        .try_into()
        .unwrap();
    let mut witnesses: Vec<Witness<FileState>> = wk
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let mut w = Witness::new(
                &format!("w{}", i + 1),
                k.clone(),
                FileState::new(tmpdir(&format!("w{i}"))).unwrap(),
            )
            .unwrap();
            w.add_log(
                "vc.test/acme",
                KeyInfo {
                    name: "operator".into(),
                    key: ed25519_dalek::VerifyingKey::from_bytes(&opk).unwrap(),
                },
            );
            w
        })
        .collect();
    let round = |mut ws: Vec<Witness<FileState>>, url: String, key: String| {
        tokio::task::spawn_blocking(move || {
            let sizes: Vec<u64> = ws
                .iter_mut()
                .map(|w| client::run_once(w, &url, "vc.test/acme", &key, 1_800_000_000).unwrap())
                .collect();
            (ws, sizes)
        })
    };
    let (ws, sizes) = round(std::mem::take(&mut witnesses), url.clone(), key.clone())
        .await
        .unwrap();
    assert_eq!(sizes, vec![2, 2]);
    witnesses = ws;
    let (s, tb) = call(&router, "GET", "/v1/trust", &key, vec![]).await;
    assert_eq!(s, 200);
    let pins = vc_verify::Pins {
        trust: Some(serde_json::from_slice(&tb).unwrap()),
        policy: None,
        require_seals: false,
    };
    let (s, pb) = call(&router, "GET", "/v1/pack", &key, vec![]).await;
    assert_eq!(s, 200);
    let rep = vc_verify::verify_pack_bytes(&pb, Some(&pins));
    assert!(
        rep.ok,
        "{:#?}",
        rep.checks.iter().filter(|c| !c.ok).collect::<Vec<_>>()
    );

    // log grows; witnesses need a consistency proof from size 2 to 3
    let (c, _) = stmt(&ask, 2, &prev);
    assert_eq!(
        call(&router, "POST", "/v1/statements", &key, c).await.0,
        200
    );
    // until witnesses catch up, the pack stays at the last fully witnessed checkpoint (size 2)
    let (_, pb2) = call(&router, "GET", "/v1/pack", &key, vec![]).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&pb2).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let (_, sizes) = round(witnesses, url, key.clone()).await.unwrap();
    assert_eq!(sizes, vec![3, 3]);
    let (_, pb3) = call(&router, "GET", "/v1/pack", &key, vec![]).await;
    assert_eq!(
        serde_json::from_slice::<Value>(&pb3).unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(vc_verify::verify_pack_bytes(&pb3, Some(&pins)).ok);
}
