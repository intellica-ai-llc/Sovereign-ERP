//! Failed-auth throttling. Own test binary: it sets VC_TRUST_CF, which is process-wide.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

async fn hit(r: &axum::Router, uri: &str, tok: &str, ip: &str) -> StatusCode {
    let req = Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {tok}"))
        .header("cf-connecting-ip", ip)
        .body(Body::empty())
        .unwrap();
    r.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn guessing_tokens_is_throttled_per_client_and_health_is_exempt_from_counting() {
    std::env::set_var("VC_TRUST_CF", "1");
    let dir = std::env::temp_dir().join(format!("vcs-throttle-{}", std::process::id()));
    let r = vc_server::router(
        vc_server::App::open(dir.clone(), "localhost", "admin-token-admin-token-1234").unwrap(),
    );
    for i in 0..20 {
        assert_eq!(
            hit(&r, "/v1/dashboard", &format!("guess-{i}"), "203.0.113.7").await,
            StatusCode::UNAUTHORIZED
        );
    }
    // 21st attempt from the same client is refused before any token check, even with the admin token
    assert_eq!(
        hit(
            &r,
            "/admin/v1/trading/status",
            "admin-token-admin-token-1234",
            "203.0.113.7"
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // another client is unaffected, and the right credential works for it
    assert_eq!(
        hit(
            &r,
            "/admin/v1/trading/status",
            "admin-token-admin-token-1234",
            "198.51.100.9"
        )
        .await,
        StatusCode::OK
    );
    // a monitor on another address still gets health checks (401s only are counted, never /healthz)
    assert_eq!(
        hit(&r, "/healthz", "x", "198.51.100.9").await,
        StatusCode::OK
    );
    std::fs::remove_dir_all(dir).ok();
}
