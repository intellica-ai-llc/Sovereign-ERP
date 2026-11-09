//! Tests of the `sov` tool: behaviour through the library entry point, plus one test of the real binary.
//! Test code: panics are how failures surface.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use sov_cli::run;
use std::path::{Path, PathBuf};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sov-cli-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample(name: &str) -> PathBuf {
    let dir = scratch(name);
    let out = run(&args(&["demo", "--export", dir.to_str().unwrap()]));
    assert_eq!(out.code, 0, "{}", out.stderr);
    dir
}

fn digest_all(dir: &Path) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let hex = sov_evidence::sha256(&std::fs::read(&p).unwrap()).to_hex();
            (p.file_name().unwrap().to_string_lossy().into_owned(), hex)
        })
        .collect();
    v.sort();
    v
}

#[test]
fn version_and_help_state_what_the_tool_is() {
    let v = run(&args(&["version"]));
    assert_eq!(
        (v.code, v.stdout.as_str()),
        (0, "sov 0.1.0 (evaluation build)\n")
    );
    let h = run(&args(&["help"]));
    assert_eq!(h.code, 0);
    for needle in [
        "evaluation build",
        "no telemetry",
        "never modifies its input",
        "sov profile",
        "sov audit",
        "sov demo",
        "sov digest",
    ] {
        assert!(h.stdout.contains(needle), "help is missing {needle:?}");
    }
}

#[test]
fn usage_mistakes_exit_2_and_show_help() {
    for bad in [
        &[][..],
        &["frobnicate"],
        &["profile"],
        &["profile", "a", "b"],
        &["demo", "--bogus"],
        &["profile", "x", "--max-rows"],
        &["profile", "x", "--max-rows", "many"],
        &["digest"],
    ] {
        let out = run(&args(bad));
        assert_eq!(out.code, 2, "{bad:?} -> {}", out.stderr);
        assert!(
            out.stderr.contains("error:") && out.stderr.contains("USAGE"),
            "{bad:?}"
        );
        assert_eq!(out.stdout, "");
    }
}

#[test]
fn runtime_failures_exit_1_with_a_clear_message() {
    let missing = run(&args(&["profile", "/definitely/not/here"]));
    assert_eq!(missing.code, 1);
    assert!(missing.stderr.contains("/definitely/not/here"));

    let empty = scratch("empty");
    let out = run(&args(&["profile", empty.to_str().unwrap()]));
    assert_eq!((out.code, out.stderr.contains("no .tsv files")), (1, true));

    let ragged = scratch("ragged");
    std::fs::write(ragged.join("T.tsv"), "a\tb\n1\n").unwrap();
    let out = run(&args(&["profile", ragged.to_str().unwrap()]));
    assert_eq!(out.code, 1);
    assert!(
        out.stderr.contains("T.tsv") && out.stderr.contains("line 2"),
        "{}",
        out.stderr
    );

    let out = run(&args(&["digest", "/definitely/not/here"]));
    assert_eq!(out.code, 1);
}

#[test]
fn demo_is_deterministic_and_complete() {
    let (a, b) = (run(&args(&["demo"])), run(&args(&["demo"])));
    assert_eq!(a.code, 0);
    assert_eq!(a, b, "same command, same bytes");
    for needle in [
        "SYNTHETIC",
        "Relationships: 40",
        "orphans 3/952",
        "Drift: 3 stored balances",
        "LateEntry",
        "EarlierPeriodNotClosed",
        "Demo digest (SHA-256",
    ] {
        assert!(
            a.stdout.contains(needle),
            "demo output is missing {needle:?}"
        );
    }
}

#[test]
fn profile_on_an_export_matches_the_demo_and_never_touches_its_input() {
    let dir = sample("profile");
    let before = digest_all(&dir);
    let text = run(&args(&["profile", dir.to_str().unwrap()]));
    assert_eq!(text.code, 0, "{}", text.stderr);
    assert!(text
        .stdout
        .contains("WORKORDER(ASSETNUM,SITEID) -> ASSET(ASSETNUM,SITEID)"));
    assert!(text.stdout.contains("orphans 3/952"));
    assert_eq!(
        text,
        run(&args(&["profile", dir.to_str().unwrap()])),
        "profiling is deterministic"
    );
    assert_eq!(
        before,
        digest_all(&dir),
        "the input files are byte-for-byte unchanged"
    );

    let json = run(&args(&["profile", dir.to_str().unwrap(), "--json"]));
    assert_eq!(json.code, 0);
    let s = json.stdout.trim();
    assert!(s.starts_with('{') && s.ends_with('}') && s.contains("\"digest_sha256\""));
    let count = |c: char| s.chars().filter(|x| *x == c).count();
    assert_eq!(
        (count('{'), count('[')),
        (count('}'), count(']')),
        "braces and brackets balance"
    );

    // The digest in the text report is the SHA-256 of the text above it.
    let (body, last) = text.stdout.trim_end().rsplit_once('\n').unwrap();
    let claimed = last.rsplit(' ').next().unwrap();
    assert_eq!(
        sov_evidence::sha256(format!("{body}\n").as_bytes()).to_hex(),
        claimed
    );
}

#[test]
fn row_limit_is_applied_and_disclosed() {
    let dir = sample("rowlimit");
    let out = run(&args(&[
        "profile",
        dir.to_str().unwrap(),
        "--max-rows",
        "50",
    ]));
    assert_eq!(out.code, 0);
    assert!(
        out.stdout.contains("was cut to the row limit"),
        "truncation must be disclosed"
    );
}

#[test]
fn audit_finds_the_drift_in_the_export() {
    let dir = sample("audit");
    let out = run(&args(&[
        "audit",
        dir.to_str().unwrap(),
        "--tx",
        "INVTRANS",
        "--tx-keys",
        "ITEMNUM,STORELOC,SITEID",
        "--tx-amount",
        "QUANTITY",
        "--bal",
        "INVBALANCES",
        "--bal-keys",
        "ITEMNUM,LOCATION,SITEID",
        "--bal-amount",
        "CURBAL",
    ]));
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout.contains("Balances checked: 400")
            && out.stdout.contains("Drift: 3 stored balances"),
        "{}",
        out.stdout
    );
    let missing = run(&args(&[
        "audit",
        dir.to_str().unwrap(),
        "--tx",
        "NOPE",
        "--tx-keys",
        "A",
        "--tx-amount",
        "B",
        "--bal",
        "INVBALANCES",
        "--bal-keys",
        "A",
        "--bal-amount",
        "B",
    ]));
    assert_eq!(missing.code, 1);
}

#[test]
fn digest_matches_the_fips_example_and_the_real_binary_wires_up_io() {
    let dir = scratch("digest");
    let f = dir.join("abc.txt");
    std::fs::write(&f, "abc").unwrap();
    let out = run(&args(&["digest", f.to_str().unwrap()]));
    assert!(
        out.stdout
            .starts_with("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  "),
        "{}",
        out.stdout
    );

    // The actual executable: stdout, stderr and exit codes.
    let exe = env!("CARGO_BIN_EXE_sov");
    let ok = std::process::Command::new(exe)
        .arg("version")
        .output()
        .unwrap();
    assert!(ok.status.success());
    assert_eq!(
        String::from_utf8_lossy(&ok.stdout),
        "sov 0.1.0 (evaluation build)\n"
    );
    let bad = std::process::Command::new(exe)
        .arg("nonsense")
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown command"));
    assert!(bad.stdout.is_empty());
}
