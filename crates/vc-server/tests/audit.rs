use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const ADMIN: &str = "admin-token-admin-token-1234";

async fn call(
    r: &axum::Router,
    m: &str,
    u: &str,
    tok: Option<&str>,
    body: &str,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(m)
        .uri(u)
        .header("content-type", "application/json");
    if let Some(t) = tok {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let resp = r
        .clone()
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let s = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn operator_actions_are_logged_tamper_evident_and_never_contain_secrets() {
    let dir = std::env::temp_dir().join(format!("vcs-audit-{}", std::process::id()));
    let app = vc_server::App::open(dir.clone(), "localhost", ADMIN).unwrap();
    let r = vc_server::router(app.clone());
    assert_eq!(
        call(
            &r,
            "POST",
            "/admin/v1/tenants",
            Some("wrong-wrong-wrong-wrong-wrong"),
            "{}"
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &r,
            "POST",
            "/admin/v1/tenants",
            Some(ADMIN),
            r#"{"id":"acme","name":"Acme"}"#
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(&r, "GET", "/admin/v1/audit", None, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, a) = call(&r, "GET", "/admin/v1/audit", Some(ADMIN), "").await;
    assert_eq!(a["valid"], true, "{a}");
    assert_eq!(a["count"], 2);
    let first = a["latest_first"][1]["action"].as_str().unwrap();
    assert_eq!(first, "admin_auth_failed");
    assert_eq!(a["latest_first"][0]["action"], "admin_action");
    let dump = a.to_string();
    assert!(
        !dump.contains(ADMIN) && !dump.contains("wrong-wrong"),
        "secrets must never be logged"
    );
    // survives a restart with the chain intact and keeps appending
    drop(r);
    let r2 = vc_server::router(vc_server::App::open(dir.clone(), "localhost", ADMIN).unwrap());
    call(&r2, "POST", "/admin/v1/trading/stop", Some(ADMIN), "").await;
    let (_, a) = call(&r2, "GET", "/admin/v1/audit", Some(ADMIN), "").await;
    assert_eq!(
        (a["valid"].clone(), a["count"].clone()),
        (Value::Bool(true), Value::from(3))
    );
    // editing a line is detected
    let p = dir.join("admin-audit.jsonl");
    let t = std::fs::read_to_string(&p)
        .unwrap()
        .replacen("admin_auth_failed", "admin_action", 1);
    std::fs::write(&p, t).unwrap();
    let (_, a) = call(&r2, "GET", "/admin/v1/audit", Some(ADMIN), "").await;
    assert_eq!(a["valid"], false);
    assert_eq!(a["bad_line"], 0);
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn readyz_reflects_storage_and_collector_expectations() {
    let dir = std::env::temp_dir().join(format!("vcs-ready-{}", std::process::id()));
    let app = vc_server::App::open(dir.clone(), "localhost", ADMIN).unwrap();
    let r = vc_server::router(app.clone());
    let get = |r: &axum::Router| {
        let r = r.clone();
        async move {
            r.oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }
    };
    assert_eq!(get(&r).await, StatusCode::OK);
    app.expect_collector(true);
    assert_eq!(get(&r).await, StatusCode::SERVICE_UNAVAILABLE);
    app.expect_collector(false);
    assert_eq!(get(&r).await, StatusCode::OK);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(get(&r).await, StatusCode::SERVICE_UNAVAILABLE); // data directory gone
}
