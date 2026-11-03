//! RFC 3161 anchoring, checked against the real `openssl ts` implementation (tests skip if openssl is absent).
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sha2::Digest;
use std::process::Command;
use std::sync::Arc;
use tower::ServiceExt;

const ADMIN: &str = "admin-token-admin-token-1234";

fn have_openssl() -> bool {
    Command::new("openssl")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
}
fn sh(dir: &std::path::Path, args: &[&str]) -> bool {
    Command::new("openssl")
        .args(args)
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.success())
}
fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vcs-anchor-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn our_request_bytes_equal_opensl_and_the_nonce_round_trips() {
    if !have_openssl() {
        return;
    }
    let d = tmp("q");
    let digest = [0xabu8; 32];
    let hex = hex::encode(digest);
    assert!(sh(
        &d,
        &[
            "ts",
            "-query",
            "-digest",
            &hex,
            "-sha256",
            "-cert",
            "-no_nonce",
            "-out",
            "ref.tsq"
        ]
    ));
    assert_eq!(
        std::fs::read(d.join("ref.tsq")).unwrap(),
        vc_server::anchor::tsq(&digest, None)
    );
    for nonce in [1u64, 127, 128, 255, 256, 0x7fff_ffff_ffff_ffff] {
        std::fs::write(
            d.join("mine.tsq"),
            vc_server::anchor::tsq(&digest, Some(nonce)),
        )
        .unwrap();
        let out = Command::new("openssl")
            .args(["ts", "-query", "-in", "mine.tsq", "-text"])
            .current_dir(&d)
            .output()
            .unwrap();
        let t = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "{t}");
        let shown = t
            .lines()
            .find_map(|l| l.strip_prefix("Nonce: 0x"))
            .and_then(|h| u64::from_str_radix(h.trim(), 16).ok());
        assert_eq!(shown, Some(nonce), "nonce {nonce}: {t}");
    }
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn status_parsing_rejects_garbage_and_refusals() {
    use vc_server::anchor::tsr_status;
    assert_eq!(
        tsr_status(&[0x30, 0x05, 0x30, 0x03, 0x02, 0x01, 0x00]),
        Some(0)
    );
    assert_eq!(
        tsr_status(&[0x30, 0x05, 0x30, 0x03, 0x02, 0x01, 0x02]),
        Some(2)
    );
    for bad in [
        &[][..],
        b"hello",
        &[0x30],
        &[0x30, 0x03, 0x02, 0x01, 0x00],
        &[0x30, 0x80],
    ] {
        assert_eq!(tsr_status(bad), None);
    }
}

async fn call(r: &axum::Router, m: &str, u: &str, tok: &str, body: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(m)
        .uri(u)
        .header("authorization", format!("Bearer {tok}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = r.clone().oneshot(req).await.unwrap();
    let s = resp.status();
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

#[tokio::test]
async fn an_anchor_is_stored_and_verifies_with_openssl_against_a_local_tsa() {
    if !have_openssl() {
        return;
    }
    // a throwaway CA and time-stamp authority built with openssl itself
    let ca = tmp("ca");
    let ok = sh(
        &ca,
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "ca.key",
            "-out",
            "ca.crt",
            "-subj",
            "/CN=TestCA",
            "-days",
            "2",
        ],
    ) && sh(
        &ca,
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "tsa.key",
            "-out",
            "tsa.csr",
            "-subj",
            "/CN=TestTSA",
        ],
    );
    assert!(ok);
    std::fs::write(
        ca.join("ext.cnf"),
        "[v3]\nextendedKeyUsage=critical,timeStamping\n",
    )
    .unwrap();
    assert!(sh(
        &ca,
        &[
            "x509",
            "-req",
            "-in",
            "tsa.csr",
            "-CA",
            "ca.crt",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "tsa.crt",
            "-days",
            "2",
            "-extfile",
            "ext.cnf",
            "-extensions",
            "v3"
        ]
    ));
    std::fs::write(ca.join("tsa.cnf"), "[tsa]\ndefault_tsa=t\n[t]\nserial=./serial\ncrypto_device=builtin\nsigner_cert=./tsa.crt\ncerts=./ca.crt\nsigner_key=./tsa.key\nsigner_digest=sha256\ndefault_policy=1.2.3.4\nother_policies=1.2.3.4\ndigests=sha256\naccuracy=secs:1\nordering=yes\ntsa_name=yes\ness_cert_id_chain=no\ness_cert_id_alg=sha256\n").unwrap();
    std::fs::write(ca.join("serial"), "01\n").unwrap();

    let dir = tmp("srv");
    let app = vc_server::App::open(dir.join("data"), "localhost", ADMIN).unwrap();
    let cadir = ca.clone();
    app.set_tsa_fetch(Arc::new(move |_url: &str, body: &[u8]| {
        std::fs::write(cadir.join("req.tsq"), body).map_err(|e| e.to_string())?;
        let o = Command::new("openssl")
            .args([
                "ts",
                "-reply",
                "-queryfile",
                "req.tsq",
                "-config",
                "tsa.cnf",
                "-out",
                "resp.tsr",
            ])
            .current_dir(&cadir)
            .output()
            .map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(String::from_utf8_lossy(&o.stderr).to_string());
        }
        std::fs::read(cadir.join("resp.tsr")).map_err(|e| e.to_string())
    }));
    let r = vc_server::router(app);
    let (s, t) = call(
        &r,
        "POST",
        "/admin/v1/tenants",
        ADMIN,
        r#"{"id":"acme","name":"Acme"}"#,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{t}");
    let key = t["api_key"].as_str().unwrap().to_string();
    assert_eq!(
        call(&r, "POST", "/v1/anchor", "bad-key", "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (s, a) = call(&r, "POST", "/v1/anchor", &key, "").await;
    assert_eq!(s, StatusCode::CREATED, "{a}");
    // verify the STORED reply with openssl, against the digest of the stored checkpoint note
    use base64::Engine;
    let reply = base64::engine::general_purpose::STANDARD
        .decode(a["reply_b64"].as_str().unwrap())
        .unwrap();
    std::fs::write(ca.join("stored.tsr"), reply).unwrap();
    let digest = hex::encode(sha2::Sha256::digest(
        a["checkpoint"].as_str().unwrap().as_bytes(),
    ));
    assert_eq!(digest, a["digest_sha256"].as_str().unwrap());
    assert!(
        sh(
            &ca,
            &[
                "ts",
                "-verify",
                "-in",
                "stored.tsr",
                "-digest",
                &digest,
                "-CAfile",
                "ca.crt"
            ]
        ),
        "openssl must accept the stored anchor"
    );
    assert!(
        !sh(
            &ca,
            &[
                "ts",
                "-verify",
                "-in",
                "stored.tsr",
                "-digest",
                &"00".repeat(32),
                "-CAfile",
                "ca.crt"
            ]
        ),
        "a different digest must fail"
    );
    let (_, l) = call(&r, "GET", "/v1/anchors", &key, "").await;
    assert_eq!(l["anchors"].as_array().unwrap().len(), 1);

    // an unreachable or refusing authority is an error, never a stored anchor
    let a2 = vc_server::App::open(dir.join("data2"), "localhost", ADMIN).unwrap();
    a2.set_tsa_fetch(Arc::new(|_: &str, _: &[u8]| {
        Ok(vec![0x30, 0x05, 0x30, 0x03, 0x02, 0x01, 0x02])
    }));
    let r2 = vc_server::router(a2);
    let (_, t2) = call(
        &r2,
        "POST",
        "/admin/v1/tenants",
        ADMIN,
        r#"{"id":"b","name":"B"}"#,
    )
    .await;
    let k2 = t2["api_key"].as_str().unwrap().to_string();
    assert_eq!(
        call(&r2, "POST", "/v1/anchor", &k2, "").await.0,
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(
        call(&r2, "GET", "/v1/anchors", &k2, "").await.1["anchors"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    std::fs::remove_dir_all(ca).ok();
    std::fs::remove_dir_all(dir).ok();
}
