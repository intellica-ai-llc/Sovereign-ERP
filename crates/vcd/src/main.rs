//! `vcd`: install-and-try binary.
//!
//! ```text
//! vcd demo  <dir>            create a demo evidence pack (two trading agents) + pinned trust/policy files
//! vcd serve <dir> [--port N] local dashboard on 127.0.0.1 (default 8787): live verification, tamper tests
//! vcd verify <dir>           verify <dir>/pack.json against <dir>/trust.json + policy.json (exit 0/1)
//! vcd version
//! ```
//! The dashboard only ever shows what `vc-verify` re-derives from the pack bytes; it never trusts display data.
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use vc_pack::PackBuilder;
use vc_verify::{verify_pack_bytes, Pins, Policy, Trust};

const DASHBOARD: &str = include_str!("../assets/dashboard.html");

fn rand_key() -> Result<SigningKey, String> {
    let mut b = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| format!("no system randomness available: {e}"))?;
    Ok(SigningKey::from_bytes(&b))
}

fn iso(t: u64) -> String {
    let (days, rem) = (t / 86400, t % 86400);
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn demo(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut b = PackBuilder::new("vc.local/demo-log", ("operator", rand_key()?))
        .witness("witness-1", rand_key()?)
        .witness("witness-2", rand_key()?);
    b.agent("lp-1", rand_key()?);
    b.agent("fc-1", rand_key()?);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let mut t = now - 3600;
    let mut x: u64 = now | 1;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let markets = [
        "will-fed-cut-in-dec",
        "btc-above-100k-dec",
        "rain-in-port-of-spain",
        "team-a-wins-final",
    ];
    for i in 0..24u64 {
        let m = markets[(rnd() % 4) as usize];
        let px = 20 + (rnd() % 60);
        t += 60 + rnd() % 90;
        let id = b
            .statement(
                "lp-1",
                "intent",
                &iso(t),
                json!({"action":{"type":"place_order","target":m},
                       "ext":{"side": if i % 2 == 0 {"buy"} else {"sell"}, "price_cents":px, "size":50}}),
            )
            .map_err(|e| e.to_string())?;
        t += 2;
        let filled = rnd() % 3 != 0;
        let markout = (rnd() % 7) as i64 - 3;
        b.statement(
            "lp-1",
            "outcome",
            &iso(t),
            json!({"pair":{"intent_id":id},"ext":{"filled":filled,"markout_cents": if filled {markout} else {0}}}),
        )
        .map_err(|e| e.to_string())?;
        if i % 4 == 0 {
            let p = 25 + rnd() % 50;
            let q = (p as i64 + (rnd() % 9) as i64 - 4).clamp(1, 99);
            t += 5;
            let fid = b
                .statement(
                    "fc-1",
                    "intent",
                    &iso(t),
                    json!({"action":{"type":"forecast","target":m},"ext":{"p_pct":p,"market_pct":q}}),
                )
                .map_err(|e| e.to_string())?;
            t += 1;
            b.statement(
                "fc-1",
                "outcome",
                &iso(t),
                json!({"pair":{"intent_id":fid},"ext":{"recorded":true}}),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    for a in ["lp-1", "fc-1"] {
        t += 1;
        b.seal(a, "demo-epoch-1", &iso(t))
            .map_err(|e| e.to_string())?;
    }
    let policy = json!({"min_completeness":"C1","min_assurance":"A0","max_open_intents":0});
    let w = |f: &str, v: &Value| {
        std::fs::write(dir.join(f), serde_json::to_vec_pretty(v).unwrap())
            .map_err(|e| format!("{f}: {e}"))
    };
    w(
        "pack.json",
        &b.build(policy.clone()).map_err(|e| e.to_string())?,
    )?;
    w("trust.json", &b.trust())?;
    w("policy.json", &policy)?;
    println!(
        "Created demo pack with {} statements in {}\nNext: vcd serve {}",
        b.len(),
        dir.display(),
        dir.display()
    );
    Ok(())
}

struct Loaded {
    pack: Value,
    pins: Pins,
}

fn load(dir: &Path) -> Result<Loaded, String> {
    let rd =
        |f: &str| std::fs::read(dir.join(f)).map_err(|e| format!("{}: {e}", dir.join(f).display()));
    let pack: Value = serde_json::from_slice(&rd("pack.json")?).map_err(|e| e.to_string())?;
    let trust: Trust =
        serde_json::from_slice(&rd("trust.json")?).map_err(|e| format!("trust.json: {e}"))?;
    let policy: Policy =
        serde_json::from_slice(&rd("policy.json")?).map_err(|e| format!("policy.json: {e}"))?;
    Ok(Loaded {
        pack,
        pins: Pins {
            trust: Some(trust),
            policy: Some(policy),
            require_seals: true,
        },
    })
}

fn report(pack: &Value, pins: &Pins) -> Value {
    let r = verify_pack_bytes(&serde_json::to_vec(pack).unwrap_or_default(), Some(pins));
    serde_json::from_str(&r.to_json()).unwrap_or(json!({"ok":false,"checks":[]}))
}

/// Statements as the dashboard shows them: decoded from the COSE bytes themselves.
fn statements(pack: &Value) -> Vec<Value> {
    let mut out = vec![];
    for (i, e) in pack["entries"].as_array().into_iter().flatten().enumerate() {
        let Some(h) = e["statement_cose_hex"].as_str() else {
            continue;
        };
        let Ok(bytes) = hex::decode(h) else { continue };
        let Ok((_, payload)) = vc_cose::peek_payload(&bytes) else {
            continue;
        };
        let Ok(v) = serde_json::from_slice::<Value>(&payload) else {
            continue;
        };
        out.push(
            json!({"i":i,"agent":v["agent"]["id"],"seq":v["seq"],"kind":v["kind"],"ts":v["ts"],
            "action":v["action"],"ext":v["ext"],"pair":v["pair"]}),
        );
    }
    out
}

fn state(l: &Loaded) -> Value {
    let rep = report(&l.pack, &l.pins);
    json!({"report":rep,"statements":statements(&l.pack),
           "trust":l.pack["trust"],"policy":l.pack["policy"],
           "checkpoint":l.pack["entries"][0]["receipt"]["checkpoint"]})
}

fn tamper(l: &Loaded, mode: &str) -> Value {
    let mut p = l.pack.clone();
    let what = match mode {
        "edit" => {
            let s = p["entries"][3]["statement_cose_hex"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let mut b = hex::decode(&s).unwrap_or_default();
            let n = b.len();
            if n > 0 {
                b[n / 2] ^= 1;
            }
            p["entries"][3]["statement_cose_hex"] = json!(hex::encode(b));
            "Flipped one bit inside statement #3 (as if someone edited a past decision)."
        }
        _ => {
            if let Some(a) = p["entries"].as_array_mut() {
                let keep = a.len().saturating_sub(4);
                a.truncate(keep);
            }
            "Removed the last 4 statements (as if someone hid the most recent activity)."
        }
    };
    json!({"what":what,"report":report(&p,&l.pins)})
}

fn respond(s: &mut TcpStream, code: &str, ctype: &str, body: &[u8]) {
    let h = format!(
        "HTTP/1.1 {code}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(h.as_bytes());
    let _ = s.write_all(body);
}

fn handle(mut s: TcpStream, dir: &Path) {
    let mut buf = [0u8; 4096];
    let n = s.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("");
    let mut it = line.split(' ');
    let (method, target) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
    if method != "GET" {
        return respond(&mut s, "405 Method Not Allowed", "text/plain", b"GET only");
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let json_ct = "application/json";
    match path {
        "/" => respond(
            &mut s,
            "200 OK",
            "text/html; charset=utf-8",
            DASHBOARD.as_bytes(),
        ),
        "/api/state" | "/api/tamper" => match load(dir) {
            Ok(l) => {
                let v = if path == "/api/state" {
                    state(&l)
                } else {
                    tamper(
                        &l,
                        if query.contains("mode=drop") {
                            "drop"
                        } else {
                            "edit"
                        },
                    )
                };
                respond(&mut s, "200 OK", json_ct, v.to_string().as_bytes())
            }
            Err(e) => respond(
                &mut s,
                "500 Internal Server Error",
                json_ct,
                json!({"error":e}).to_string().as_bytes(),
            ),
        },
        "/pack.json" => match std::fs::read(dir.join("pack.json")) {
            Ok(b) => respond(&mut s, "200 OK", json_ct, &b),
            Err(_) => respond(&mut s, "404 Not Found", "text/plain", b"missing"),
        },
        _ => respond(&mut s, "404 Not Found", "text/plain", b"not found"),
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let dir = |i: usize| -> Result<PathBuf, String> {
        args.get(i)
            .map(PathBuf::from)
            .ok_or_else(|| "missing <dir>".to_string())
    };
    match args.first().map(String::as_str) {
        Some("version") => {
            println!("vcd {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Some("demo") => demo(&dir(1)?).map(|_| ExitCode::SUCCESS),
        Some("verify") => {
            let l = load(&dir(1)?)?;
            let r = report(&l.pack, &l.pins);
            println!("{r}");
            Ok(if r["ok"] == json!(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Some("serve") => {
            let d = dir(1)?;
            let mut port = 8787u16;
            if args.get(2).map(String::as_str) == Some("--port") {
                port = args
                    .get(3)
                    .and_then(|p| p.parse().ok())
                    .ok_or("bad --port")?;
            }
            load(&d)?;
            let l = TcpListener::bind(("127.0.0.1", port))
                .map_err(|e| format!("cannot listen on port {port}: {e}"))?;
            println!("VeriChain dashboard: http://127.0.0.1:{port}  (Ctrl+C to stop)");
            for c in l.incoming().flatten() {
                handle(c, &d);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(String::new()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(c) => c,
        Err(m) => {
            if !m.is_empty() {
                eprintln!("vcd: {m}");
            }
            eprintln!("usage: vcd demo <dir> | vcd serve <dir> [--port N] | vcd verify <dir> | vcd version");
            ExitCode::from(2)
        }
    }
}
