//! End to end against a local fake of the two Polymarket endpoints (no real network).
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

fn serve(events: String, books: Vec<(String, String)>) -> (String, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        l.set_nonblocking(false).unwrap();
        let mut idle = 0;
        loop {
            l.set_ttl(64).ok();
            let (mut s, _) = match l.accept() {
                Ok(x) => x,
                Err(_) => break,
            };
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let target = req.split(' ').nth(1).unwrap_or("").to_string();
            let body = if target.starts_with("/events") {
                events.clone()
            } else if target.starts_with("/__stop") {
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                break;
            } else if let Some(t) = target.strip_prefix("/book?token_id=") {
                books
                    .iter()
                    .find(|(k, _)| k == t)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let code = if body.is_empty() {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let _ = s.write_all(format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            idle += 1;
            if idle > 1000 {
                break;
            }
        }
    });
    (addr, h)
}

fn book(token: &str) -> String {
    format!(
        r#"{{"market":"m","asset_id":"{token}","timestamp":"1762000000000","hash":"h","bids":[{{"price":"0.28","size":"100"}}],"asks":[{{"price":"0.30","size":"100"}}],"min_order_size":"5","tick_size":"0.01","neg_risk":true,"last_trade_price":"0.29"}}"#
    )
}

#[test]
fn discover_record_verify_scan_against_a_fake_server() {
    let events = r#"[{"slug":"ev","negRisk":true,"markets":[
      {"question":"A","conditionId":"1","clobTokenIds":"[\"ay\",\"an\"]","outcomes":"[\"Yes\",\"No\"]","active":true,"enableOrderBook":true},
      {"question":"B","conditionId":"2","clobTokenIds":"[\"by\",\"bn\"]","outcomes":"[\"Yes\",\"No\"]","active":true,"enableOrderBook":true},
      {"question":"C","conditionId":"3","clobTokenIds":"[\"cy\",\"cn\"]","outcomes":"[\"Yes\",\"No\"]","active":true,"enableOrderBook":true}]}]"#.to_string();
    let (addr, h) = serve(
        events,
        vec![
            ("ay".into(), book("ay")),
            ("by".into(), book("by")),
            ("cy".into(), book("cy")),
        ],
    );
    let dir = std::env::temp_dir().join(format!(
        "masdata-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = env!("CARGO_BIN_EXE_mas-data");
    let run = |a: &[&str]| Command::new(bin).args(a).output().unwrap();
    let (ev, rec) = (dir.join("ev.json"), dir.join("rec.jsonl"));
    let o = run(&["discover", "--out", ev.to_str().unwrap(), "--gamma", &addr]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = run(&[
        "record",
        "--events",
        ev.to_str().unwrap(),
        "--out",
        rec.to_str().unwrap(),
        "--rounds",
        "2",
        "--interval",
        "0",
        "--clob",
        &addr,
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    // refuses to overwrite an existing recording
    assert!(!run(&[
        "record",
        "--events",
        ev.to_str().unwrap(),
        "--out",
        rec.to_str().unwrap(),
        "--rounds",
        "1",
        "--clob",
        &addr
    ])
    .status
    .success());
    let o = run(&["verify", rec.to_str().unwrap()]);
    assert!(
        String::from_utf8_lossy(&o.stdout).starts_with("OK 6 records"),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    // asks 0.30 x3 = 0.90: with zero fees and haircut the scan sees both rounds
    let o = run(&[
        "scan",
        rec.to_str().unwrap(),
        ev.to_str().unwrap(),
        "--fee-rate",
        "0",
        "--haircut",
        "0",
        "--min-edge",
        "0.05",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["opportunities"], 2);
    assert_eq!(v["events_complete"], 2);
    // a tampered recording is rejected by both verify and scan
    let text = std::fs::read_to_string(&rec)
        .unwrap()
        .replacen("0.28", "0.20", 1);
    let bad = dir.join("bad.jsonl");
    std::fs::write(&bad, text).unwrap();
    assert!(!run(&["verify", bad.to_str().unwrap()]).status.success());
    assert!(!run(&["scan", bad.to_str().unwrap(), ev.to_str().unwrap()])
        .status
        .success());
    // stop the fake server
    let _ = ureq_stop(&addr);
    let _ = h.join();
}

fn ureq_stop(addr: &str) -> std::io::Result<()> {
    let mut s = std::net::TcpStream::connect(addr.trim_start_matches("http://"))?;
    s.write_all(b"GET /__stop HTTP/1.1\r\nHost: x\r\n\r\n")
}
