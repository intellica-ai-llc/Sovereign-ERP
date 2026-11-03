//! The collector driven through the HTTP API with a fake market (no network).
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const ADMIN: &str = "admin-token-admin-token-1234";

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    tok: Option<&str>,
    body: &str,
) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = tok {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let resp = app
        .clone()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let s = resp.status();
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn fake() -> mas_data::runner::Fetch {
    Arc::new(|u: &str| {
        if u.contains("/events?") {
            Ok(r#"[{"slug":"e1","negRisk":true,"markets":[
              {"question":"a","conditionId":"c1","active":true,"closed":false,"enableOrderBook":true,"clobTokenIds":"[\"y1\",\"n1\"]","outcomes":"[\"Yes\",\"No\"]"},
              {"question":"b","conditionId":"c2","active":true,"closed":false,"enableOrderBook":true,"clobTokenIds":"[\"y2\",\"n2\"]","outcomes":"[\"Yes\",\"No\"]"}]}]"#.into())
        } else if let Some(t) = u.split("token_id=").nth(1) {
            Ok(format!(
                r#"{{"market":"m","asset_id":"{t}","timestamp":"1762000000000","hash":"h","bids":[{{"price":"0.28","size":"100"}}],"asks":[{{"price":"0.30","size":"100"}}],"min_order_size":"5","tick_size":"0.01","neg_risk":true}}"#
            ))
        } else if u.contains("/trades?") {
            Ok(r#"[{"side":"SELL","asset":"y1","size":5,"price":0.29,"timestamp":1700000000,"transactionHash":"0x1"}]"#.into())
        } else {
            Err("unexpected url".into())
        }
    })
}

#[tokio::test]
async fn collector_is_admin_only_runs_from_the_api_and_the_card_is_honest() {
    let dir = std::env::temp_dir().join(format!("vcs-trading-{}", std::process::id()));
    let app = vc_server::App::open(dir.clone(), "localhost", ADMIN).unwrap();
    app.set_market_fetch(fake());
    let r = vc_server::router(app);

    // admin only, on every route
    for (m, u) in [
        ("POST", "/admin/v1/trading/start"),
        ("POST", "/admin/v1/trading/stop"),
        ("GET", "/admin/v1/trading/status"),
    ] {
        assert_eq!(
            call(&r, m, u, None, "").await.0,
            StatusCode::UNAUTHORIZED,
            "{u}"
        );
        assert_eq!(
            call(&r, m, u, Some("not-the-admin-token-not-the-admin"), "")
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{u}"
        );
    }
    // before anything runs: no collector, an honest card, nothing proven
    let (s, st) = call(&r, "GET", "/admin/v1/trading/status", Some(ADMIN), "").await;
    assert_eq!(s, StatusCode::OK);
    assert!(st["collector"].is_null());
    assert_eq!(st["card"]["standing"], "unverified");
    assert!(st["card"]["claims"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["status"] == "not_yet"));
    assert!(st["card"]["statement"]
        .as_str()
        .unwrap()
        .contains("NOT YET ESTABLISHED"));

    // bad parameters are refused
    for b in [
        r#"{"limit":0}"#,
        r#"{"interval_secs":5}"#,
        r#"{"limit":1000}"#,
        "not json",
    ] {
        assert_eq!(
            call(&r, "POST", "/admin/v1/trading/start", Some(ADMIN), b)
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{b}"
        );
    }
    // one round, then it finishes by itself
    assert_eq!(
        call(
            &r,
            "POST",
            "/admin/v1/trading/start",
            Some(ADMIN),
            r#"{"rounds":1,"interval_secs":30}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut last = Value::Null;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        last = call(
            &r,
            "GET",
            "/admin/v1/trading/status?scan=1",
            Some(ADMIN),
            "",
        )
        .await
        .1;
        if last["collector"]["running"] == false {
            break;
        }
    }
    let c = &last["collector"];
    assert_eq!(c["phase"], "finished", "{last}");
    assert_eq!(
        (c["books_ok"].as_u64(), c["trade_polls_ok"].as_u64()),
        (Some(2), Some(2))
    );
    assert!(c["books_head"].as_str().unwrap().len() == 64);
    assert!(last["files"].as_array().unwrap().len() == 3);
    assert_eq!(last["scan"]["records"], 2, "{}", last["scan"]);
    // paper replay of the recording: a card exists, carries a card id, and cannot be Verified
    let (_, pp) = call(
        &r,
        "GET",
        "/admin/v1/trading/status?paper=1",
        Some(ADMIN),
        "",
    )
    .await;
    assert!(pp["paper"]["error"].is_null(), "{pp}");
    assert_eq!(pp["paper"]["card_id"].as_str().unwrap().len(), 64);
    assert_ne!(pp["paper"]["standing"], "verified");
    assert!(pp["paper"]["source"]["books"]
        .as_str()
        .unwrap()
        .starts_with("books-"));
    // a finished session can be followed by another; stopping when idle is harmless
    assert_eq!(
        call(&r, "POST", "/admin/v1/trading/stop", Some(ADMIN), "")
            .await
            .1,
        json!({"stopping": false})
    );
    assert_eq!(
        call(
            &r,
            "POST",
            "/admin/v1/trading/start",
            Some(ADMIN),
            r#"{"rounds":1}"#
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &r,
            "POST",
            "/admin/v1/trading/start",
            Some(ADMIN),
            r#"{"rounds":1}"#
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    std::fs::remove_dir_all(dir).ok();
}
