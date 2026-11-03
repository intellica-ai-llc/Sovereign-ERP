//! `mas-data` command line. The only code here that touches the network or files.
//!   mas-data discover --out events.json [--limit 100] [--gamma URL]
//!   mas-data record   --events events.json --out rec.jsonl [--rounds 20] [--interval 30] [--clob URL]
//!   mas-data trades   --events events.json --out trades.jsonl [--rounds 20] [--interval 60] [--data URL]
//!   mas-data paper    --books books.jsonl [--trades trades.jsonl]   (replay through the paper engine, print the claim card)
//!   mas-data verify   rec.jsonl                       (checks the hash chain, prints the head to commit)
//!   mas-data scan     rec.jsonl events.json [--fee-rate 0.04] [--haircut 0.01] [--min-edge 0.02]
//! Data is only read from public endpoints; nothing here can place an order.
use mas_data::runner::{CLOB, DATA_API, GAMMA};
use mas_data::*;
use serde_json::json;
use std::io::Write;
use std::process::ExitCode;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("discover") => {
            let out = flag(args, "--out").ok_or("--out required")?;
            let limit: u32 = flag(args, "--limit")
                .unwrap_or("100")
                .parse()
                .map_err(|_| "bad --limit")?;
            let base = flag(args, "--gamma").unwrap_or(GAMMA);
            let body = http_get(&format!("{base}/events?active=true&closed=false&limit={limit}&order=volume24hr&ascending=false"))?;
            let ev = parse_events(&body).map_err(|e| e.to_string())?;
            std::fs::write(
                out,
                serde_json::to_string_pretty(&events_to_json(&ev)).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let neg = ev
                .iter()
                .filter(|e| e.neg_risk && e.markets.len() >= 2)
                .count();
            println!(
                "{} events saved ({} negRisk with 2+ markets) -> {out}",
                ev.len(),
                neg
            );
            Ok(())
        }
        Some("record") => {
            let ev = events_from_json(
                &std::fs::read_to_string(flag(args, "--events").ok_or("--events required")?)
                    .map_err(|e| e.to_string())?,
            )?;
            let out = flag(args, "--out").ok_or("--out required")?;
            let rounds: u64 = flag(args, "--rounds")
                .unwrap_or("20")
                .parse()
                .map_err(|_| "bad --rounds")?;
            let interval: u64 = flag(args, "--interval")
                .unwrap_or("30")
                .parse()
                .map_err(|_| "bad --interval")?;
            let base = flag(args, "--clob").unwrap_or(CLOB);
            let tokens: Vec<&str> = ev
                .iter()
                .filter(|e| e.neg_risk)
                .flat_map(|e| e.markets.iter().map(|m| m.yes_token.as_str()))
                .collect();
            if tokens.is_empty() {
                return Err("no negRisk tokens in the events file".into());
            }
            let mut f = std::fs::OpenOptions::new()
                .create_new(true)
                .append(true)
                .open(out)
                .map_err(|e| format!("{out}: {e} (will not overwrite)"))?;
            let (mut prev, mut seq, mut bad) = (GENESIS.to_string(), 0u64, 0u32);
            for round in 0..rounds {
                for t in &tokens {
                    match http_get(&format!("{base}/book?token_id={t}")) {
                        Ok(raw) => {
                            let rec = make_rec(seq, round, now_ms(), &raw, &prev);
                            writeln!(f, "{}", rec_to_line(&rec)).map_err(|e| e.to_string())?;
                            f.sync_data().map_err(|e| e.to_string())?;
                            prev = rec.h;
                            seq += 1;
                        }
                        Err(e) => {
                            bad += 1;
                            eprintln!("round {round} token {t}: {e}");
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
                eprintln!("round {} of {rounds} done ({seq} books so far)", round + 1);
                if round + 1 < rounds {
                    std::thread::sleep(std::time::Duration::from_secs(interval));
                }
            }
            println!("recorded {seq} books ({bad} failed requests). head {prev}");
            Ok(())
        }
        Some("trades") => {
            let ev = events_from_json(
                &std::fs::read_to_string(flag(args, "--events").ok_or("--events required")?)
                    .map_err(|e| e.to_string())?,
            )?;
            let out = flag(args, "--out").ok_or("--out required")?;
            let rounds: u64 = flag(args, "--rounds")
                .unwrap_or("20")
                .parse()
                .map_err(|_| "bad --rounds")?;
            let interval: u64 = flag(args, "--interval")
                .unwrap_or("60")
                .parse()
                .map_err(|_| "bad --interval")?;
            let base = flag(args, "--data").unwrap_or(DATA_API);
            let conds: Vec<&str> = ev
                .iter()
                .flat_map(|e| e.markets.iter().map(|m| m.condition_id.as_str()))
                .filter(|c| !c.is_empty())
                .collect();
            if conds.is_empty() {
                return Err("no condition ids in the events file".into());
            }
            let mut f = std::fs::OpenOptions::new()
                .create_new(true)
                .append(true)
                .open(out)
                .map_err(|e| format!("{out}: {e} (will not overwrite)"))?;
            let (mut prev, mut seq, mut bad) = (GENESIS.to_string(), 0u64, 0u32);
            for round in 0..rounds {
                for c in &conds {
                    match http_get(&format!("{base}/trades?market={c}&limit=500"))
                        .and_then(|raw| parse_trades(&raw).map(|_| raw).map_err(|e| e.to_string()))
                    {
                        Ok(raw) => {
                            let rec = make_rec(seq, round, now_ms(), &raw, &prev);
                            writeln!(f, "{}", rec_to_line(&rec)).map_err(|e| e.to_string())?;
                            f.sync_data().map_err(|e| e.to_string())?;
                            prev = rec.h;
                            seq += 1;
                        }
                        Err(e) => {
                            bad += 1;
                            eprintln!("round {round} market {c}: {e}");
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                eprintln!("round {} of {rounds} done ({seq} polls so far)", round + 1);
                if round + 1 < rounds {
                    std::thread::sleep(std::time::Duration::from_secs(interval));
                }
            }
            println!("recorded {seq} trade polls ({bad} failed). head {prev}");
            Ok(())
        }
        Some("paper") => {
            let rd = |p: &str| -> Result<Vec<Rec>, String> {
                read_recording(&std::fs::read_to_string(p).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("{p}: INVALID recording: {e}"))
            };
            let books = rd(flag(args, "--books").ok_or("--books required")?)?;
            let trades = match flag(args, "--trades") {
                Some(p) => rd(p)?,
                None => vec![],
            };
            let mut p = replay::Params::default();
            let num = |n: &str, d: f64| {
                flag(args, n).map_or(Ok(d), |v| v.parse::<f64>().map_err(|_| format!("bad {n}")))
            };
            p.capital = num("--capital", p.capital)?;
            p.cap = num("--cap", p.cap)?;
            p.size = num("--size", p.size)?;
            p.queue_factor = num("--queue", p.queue_factor)?;
            p.period_secs = num("--period-secs", p.period_secs as f64)? as u64;
            let r = replay::run(&books, &trades, &p)?;
            let (bh, _) = head(&books);
            let card = replay::make_card(&r, &p, &bh);
            println!("{}", mas_research::card::public_statement(&card));
            println!("card_id {}", mas_research::card::card_id(&card));
            println!(
                "periods {} books {} trades_applied {}",
                r.returns.len(),
                r.books_used,
                r.trades_used
            );
            Ok(())
        }
        Some("verify") => {
            let recs = read_recording(
                &std::fs::read_to_string(args.get(1).ok_or("file required")?)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("INVALID: {e}"))?;
            let (h, n) = head(&recs);
            println!("OK {n} records, head {h}");
            Ok(())
        }
        Some("scan") => {
            let recs = read_recording(
                &std::fs::read_to_string(args.get(1).ok_or("recording required")?)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("INVALID recording: {e}"))?;
            let ev = events_from_json(
                &std::fs::read_to_string(args.get(2).ok_or("events file required")?)
                    .map_err(|e| e.to_string())?,
            )?;
            let num = |n: &str, d: f64| {
                flag(args, n).map_or(Ok(d), |v| v.parse::<f64>().map_err(|_| format!("bad {n}")))
            };
            let s = scan_rebalance(
                &recs,
                &ev,
                num("--fee-rate", 0.04)?,
                num("--haircut", 0.01)?,
                num("--min-edge", 0.02)?,
            );
            let best = s.opportunities.iter().map(|o| o.edge).fold(0.0, f64::max);
            println!(
                "{}",
                json!({"rounds":s.rounds,"books_ok":s.books_ok,"books_bad":s.books_bad,"events_complete":s.events_complete,
                "opportunities":s.opportunities.len(),"best_edge":best,
                "first":s.opportunities.iter().take(10).map(|o| json!({"round":o.round,"event":o.event,"buy_all":o.side_buy_all,"shares":o.shares,"edge":o.edge})).collect::<Vec<_>>()})
            );
            Ok(())
        }
        _ => Err(
            "usage: mas-data discover|record|trades|paper|verify|scan (see source header)".into(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
