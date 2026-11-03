use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use tower::ServiceExt;
use vc_log::checkpoint::{note, sign_line, verify_note, Checkpoint, KeyInfo};

const ORIGIN: &str = "vc.test/log";
fn leaves(n: usize) -> Vec<vc_core::Hash> {
    let e: Vec<Vec<u8>> = (0..n).map(|i| vec![i as u8]).collect();
    let r: Vec<&[u8]> = e.iter().map(|v| v.as_slice()).collect();
    vc_log::leaves_from_entries(&r)
}
fn op() -> SigningKey {
    SigningKey::from_bytes(&[1u8; 32])
}
fn signed(n: usize, bad_root: bool) -> String {
    let mut root = vc_log::root(&leaves(n));
    if bad_root {
        root[0] ^= 1;
    }
    let t = Checkpoint {
        origin: ORIGIN.into(),
        size: n as u64,
        root,
    }
    .text();
    note(&t, &[sign_line(&t, "op", &op())])
}
fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "vcw-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}
async fn post(app: &axum::Router, body: String) -> (StatusCode, String) {
    let r = Request::builder()
        .method("POST")
        .uri("/add-checkpoint")
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(r).await.unwrap();
    let s = resp.status();
    (
        s,
        String::from_utf8_lossy(&resp.into_body().collect().await.unwrap().to_bytes()).to_string(),
    )
}
fn req(old: u64, proof: &[vc_core::Hash], n: &str) -> String {
    let mut s = format!("old {old}\n");
    for p in proof {
        s.push_str(&B64.encode(p));
        s.push('\n');
    }
    format!("{s}\n{n}")
}

#[tokio::test]
async fn protocol_and_persistence() {
    let dir = tmp();
    std::fs::write(
        dir.join("logs.txt"),
        format!(
            "{ORIGIN} op {}\n",
            hex::encode(op().verifying_key().to_bytes())
        ),
    )
    .unwrap();
    let wsk = SigningKey::from_bytes(&[2u8; 32]);
    let app = vc_witnessd::router(vc_witnessd::open(dir.clone(), "w1", wsk.clone()).unwrap());
    let (s, line) = post(&app, req(0, &[], &signed(3, false))).await;
    assert_eq!(s, StatusCode::OK, "{line}");
    // the cosignature verifies in the log verifier alongside the operator signature
    let t = Checkpoint {
        origin: ORIGIN.into(),
        size: 3,
        root: vc_log::root(&leaves(3)),
    }
    .text();
    let n = note(&t, &[sign_line(&t, "op", &op()), line]);
    assert!(verify_note(
        &n,
        &KeyInfo {
            name: "op".into(),
            key: op().verifying_key()
        },
        &[KeyInfo {
            name: "w1".into(),
            key: wsk.verifying_key()
        }],
        1
    )
    .is_ok());
    // wrong old size -> 409 with the witness's size
    let p = vc_log::consistency_proof(&leaves(7), 3).unwrap();
    assert_eq!(
        post(&app, req(2, &p, &signed(7, false))).await,
        (StatusCode::CONFLICT, "3".to_string())
    );
    // fork -> 422; rollback -> 422; garbage -> 400; unknown log -> 403
    assert_eq!(
        post(&app, req(3, &p, &signed(7, true))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(&app, "nonsense".into()).await.0,
        StatusCode::BAD_REQUEST
    );
    let other = {
        let t = Checkpoint {
            origin: "x/y".into(),
            size: 1,
            root: [0; 32],
        }
        .text();
        note(&t, &[sign_line(&t, "op", &op())])
    };
    assert_eq!(
        post(&app, req(0, &[], &other)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&app, req(3, &p, &signed(7, false))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(&app, req(0, &[], &signed(3, false))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // restart: state survives, a fork at the same size is still caught
    let app2 = vc_witnessd::router(vc_witnessd::open(dir, "w1", wsk).unwrap());
    assert_eq!(
        post(&app2, req(7, &[], &signed(7, true))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
