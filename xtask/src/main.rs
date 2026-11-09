//! Repository automation, run as `cargo xtask <task>`.
//!
//! Tasks:
//! - `doctor` check every prerequisite and print the exact fix for anything missing.
//! - `dist`   build the release binary, package it with its checksum under `dist/`.
//! - `sbom`   write a `CycloneDX` software bill of materials from `Cargo.lock` to `dist/`.
//! - `ci`    format check, clippy (deny warnings), tests, docs (deny warnings), toolchain consistency.
//! - `tools` download the pinned TLC (`tla2tools.jar`) into `.tools/` and verify its SHA-256.
//! - `tlc`   model-check every spec in `formal/tla` and compare state counts with the recorded run.
//! - `lean`  compile every proof in `formal/lean` (needs `lean` on `PATH`).
//! - `bench-transfer` leave-one-corpus-out evaluation across `OFBiz`, Chinook, Northwind and Sakila (slow, manual).
//! - `bench-fk` FK-stripping benchmark on Apache `OFBiz` at a pinned commit; fails below the quality gate
//!   (needs `git` and `python3`).
//!
//! Automation is Rust, not shell, so it is typed, testable and works the same everywhere.
//! This binary is the one place that talks to the terminal, hence the print allowance below.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// TLC 2.19 (August 2024). An immutable release asset, unlike the rolling `v1.8.0` tag.
const TLA2TOOLS_URL: &str =
    "https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar";
const TLA2TOOLS_SHA256: &str = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88";

/// Apache `OFBiz` (Apache-2.0), pinned so the benchmark is reproducible, and the parts of it we read.
const OFBIZ_URL: &str = "https://github.com/apache/ofbiz-framework.git";
const OFBIZ_COMMIT: &str = "1234bc367afd381424a11ba8a66ec3ee721ea179";
const OFBIZ_PATHS: &[&str] = &[
    "applications/datamodel",
    "applications/accounting/data",
    "applications/order/data",
    "applications/product/data",
    "applications/party/data",
    "applications/humanres/data",
    "applications/workeffort/data",
    "applications/manufacturing/data",
    "applications/marketing/data",
    "applications/securityext/data",
    "framework/entity/entitydef",
    "framework/common/entitydef",
    "framework/common/data",
    "framework/security/entitydef",
    "framework/service/entitydef",
    "framework/webtools/entitydef",
];
/// Held-out quality gate (conservative ground truth). Measured at the pinned commit: precision 0.842, recall 0.855.
const GATE_MIN_PRECISION: &str = "0.80";
const GATE_MIN_RECALL: &str = "0.80";

/// A spec and the state counts its recorded TLC run produced. A spec change that moves these
/// numbers must update them deliberately, in the same commit, with the arc42 appendix.
struct Spec {
    module: &'static str,
    generated: u64,
    distinct: u64,
}

const SPECS: &[Spec] = &[
    Spec {
        module: "PeriodClose",
        generated: 18_833,
        distinct: 4_160,
    },
    Spec {
        module: "Ledger",
        generated: 713_185,
        distinct: 21_433,
    },
    Spec {
        module: "SyncMerge",
        generated: 33_895,
        distinct: 9_018,
    },
];

fn main() -> ExitCode {
    let task = env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "ci" => ci(),
        "tools" => tools(),
        "tlc" => tlc(),
        "lean" => lean(),
        "doctor" => doctor(),
        "dist" => dist(),
        "sbom" => sbom(),
        "bench-fk" => bench_fk(),
        "bench-transfer" => bench_transfer(),
        _ => {
            eprintln!(
                "usage: cargo xtask <doctor|ci|tools|tlc|lean|dist|sbom|bench-fk|bench-transfer>"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask {task}: FAILED: {message}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    // xtask lives at <root>/xtask, so its manifest dir's parent is the repository root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn run(cmd: &mut Command) -> Result<(), String> {
    println!("+ {cmd:?}");
    let status = cmd
        .status()
        .map_err(|e| format!("could not start {cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} exited with {status}"))
    }
}

fn cargo() -> Command {
    let mut c = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    c.current_dir(root());
    c
}

fn ci() -> Result<(), String> {
    toolchain_consistency()?;
    run(cargo().args(["fmt", "--all", "--", "--check"]))?;
    run(cargo().args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ]))?;
    run(cargo().args(["test", "--workspace", "--locked"]))?;
    run(cargo()
        .args(["doc", "--workspace", "--no-deps", "--locked"])
        .env("RUSTDOCFLAGS", "-D warnings"))?;
    println!("ci: all gates passed");
    Ok(())
}

/// `rust-toolchain.toml` and `[workspace.package] rust-version` must name the same minor version.
fn toolchain_consistency() -> Result<(), String> {
    let read =
        |name: &str| fs::read_to_string(root().join(name)).map_err(|e| format!("{name}: {e}"));
    let value = |text: &str, key: &str| -> Option<String> {
        text.lines()
            .find(|l| l.trim_start().starts_with(key))
            .and_then(|l| l.split('"').nth(1))
            .map(str::to_owned)
    };
    let channel = value(&read("rust-toolchain.toml")?, "channel")
        .ok_or("no channel in rust-toolchain.toml")?;
    let msrv =
        value(&read("Cargo.toml")?, "rust-version").ok_or("no rust-version in Cargo.toml")?;
    let minor = |v: &str| v.split('.').take(2).collect::<Vec<_>>().join(".");
    if minor(&channel) == minor(&msrv) {
        Ok(())
    } else {
        Err(format!(
            "rust-toolchain channel {channel} and rust-version {msrv} disagree"
        ))
    }
}

fn jar() -> PathBuf {
    env::var_os("TLA2TOOLS").map_or_else(|| root().join(".tools/tla2tools.jar"), PathBuf::from)
}

fn sha256_of(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(sov_evidence::sha256(&data).to_hex())
}

fn tools() -> Result<(), String> {
    let jar = jar();
    if let Some(dir) = jar.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    run(Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&jar)
        .arg(TLA2TOOLS_URL))?;
    let got = sha256_of(&jar)?;
    if got == TLA2TOOLS_SHA256 {
        println!("tools: {} verified", jar.display());
        Ok(())
    } else {
        let _ = fs::remove_file(&jar);
        Err(format!(
            "SHA-256 mismatch for {TLA2TOOLS_URL}: expected {TLA2TOOLS_SHA256}, got {got}"
        ))
    }
}

fn tlc() -> Result<(), String> {
    let jar = jar();
    if !jar.exists() {
        return Err(format!(
            "{} not found; run `cargo xtask tools` first",
            jar.display()
        ));
    }
    let dir = root().join("formal/tla");
    for spec in SPECS {
        println!("== TLC: {}", spec.module);
        let out = Command::new("java")
            .current_dir(&dir)
            .args(["-XX:+UseParallelGC", "-cp"])
            .arg(&jar)
            .args(["tlc2.TLC", "-workers", "auto", "-metadir"])
            .arg(root().join(".tools/tlc-states"))
            .arg("-config")
            .arg(format!("{}.cfg", spec.module))
            .arg(format!("{}.tla", spec.module))
            .output()
            .map_err(|e| format!("could not start java: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || !text.contains("No error has been found") {
            println!("{text}");
            return Err(format!("TLC reported a problem for {}", spec.module));
        }
        let (generated, distinct) = parse_counts(&text).ok_or("could not read TLC state counts")?;
        println!("   {generated} generated, {distinct} distinct");
        if (generated, distinct) != (spec.generated, spec.distinct) {
            return Err(format!(
                "{}: state counts changed (recorded {} / {}, now {generated} / {distinct}); \
                 update SPECS in xtask and the arc42 appendix together if this is intended",
                spec.module, spec.generated, spec.distinct
            ));
        }
    }
    println!("tlc: all specs pass with the recorded state counts");
    Ok(())
}

/// Parses the final "N states generated, M distinct states found" line.
fn parse_counts(text: &str) -> Option<(u64, u64)> {
    let line = text
        .lines()
        .rev()
        .find(|l| l.contains("states generated") && l.contains("distinct states found"))?;
    let mut numbers = line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(str::parse::<u64>);
    let generated = numbers.next()?.ok()?;
    let distinct = numbers.next()?.ok()?;
    Some((generated, distinct))
}

/// Clones the pinned `OFBiz` commit (sparse) and converts it into the benchmark dataset.
fn prepare_ofbiz() -> Result<PathBuf, String> {
    let checkout = root().join(".tools/ofbiz");
    if !checkout.join(".git").exists() {
        fs::create_dir_all(&checkout).map_err(|e| format!("{}: {e}", checkout.display()))?;
        let git = |args: &[&str]| run(Command::new("git").current_dir(&checkout).args(args));
        git(&["init", "-q"])?;
        git(&["remote", "add", "origin", OFBIZ_URL])?;
        let mut sparse = vec!["sparse-checkout", "set"];
        sparse.extend(OFBIZ_PATHS);
        git(&sparse)?;
        git(&[
            "fetch",
            "--depth",
            "1",
            "--filter=blob:none",
            "origin",
            OFBIZ_COMMIT,
        ])?;
        git(&["checkout", "-q", "FETCH_HEAD"])?;
    }
    let out = root().join(".tools/ofbiz-bench");
    run(Command::new("python3")
        .current_dir(root())
        .arg("bench/ofbiz/convert.py")
        .arg(&checkout)
        .arg(&out))?;
    Ok(out)
}

fn bench_fk() -> Result<(), String> {
    let out = prepare_ofbiz()?;
    run(cargo()
        .args([
            "run",
            "--release",
            "--locked",
            "-p",
            "sov-profiler",
            "--example",
            "fkbench",
            "--",
        ])
        .arg(&out)
        .args([
            "--forward-only",
            "--min-precision",
            GATE_MIN_PRECISION,
            "--min-recall",
            GATE_MIN_RECALL,
        ]))?;
    Ok(())
}

fn lean() -> Result<(), String> {
    let dir = root().join("formal/lean");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "lean"))
        .collect();
    files.sort();
    for file in &files {
        run(Command::new("lean").arg(file))?;
    }
    println!("lean: {} file(s) compile", files.len());
    Ok(())
}

/// Public sample databases with declared foreign keys, pinned by SHA-256 (the URLs follow branch heads,
/// so a changed file fails loudly instead of silently changing the benchmark).
const TRANSFER_DBS: &[(&str, &str, &str)] = &[
    (
        "chinook",
        "https://raw.githubusercontent.com/lerocha/chinook-database/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite",
        "7651ba378ac2fcd0dfc3c66fb101f7a7eed3ba39a612ec642b96e20702061f15",
    ),
    (
        "northwind",
        "https://raw.githubusercontent.com/jpwhite3/northwind-SQLite3/main/dist/northwind.db",
        "2f4f5c68dfcd33ba27373eae48c7a4869800c68095ee0f9f0da494f83382a877",
    ),
    (
        "sakila",
        "https://raw.githubusercontent.com/bradleygrant/sakila-sqlite3/main/sakila_master.db",
        "88c91a4a1a6b61f9d3f35904c0a173c887b25e73f20c3c2fdb073818c06f4268",
    ),
];
/// Mean held-out F1 across corpora that `bench-transfer` must not fall below (0.86 when set).
const TRANSFER_MIN_MEAN_F1: &str = "0.80";

fn bench_transfer() -> Result<(), String> {
    let ofbiz = prepare_ofbiz()?;
    let dir = root().join(".tools/corpora");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut datasets = vec![ofbiz];
    for (name, url, sha) in TRANSFER_DBS {
        let db = dir.join(format!("{name}.db"));
        if !db.exists() {
            run(Command::new("curl").args(["-fsSL", "-o"]).arg(&db).arg(url))?;
        }
        let got = sha256_of(&db)?;
        if got != *sha {
            let _ = fs::remove_file(&db);
            return Err(format!(
                "{name}: SHA-256 mismatch (expected {sha}, got {got}); the upstream file changed"
            ));
        }
        let converted = dir.join(format!("out-{name}"));
        run(Command::new("python3")
            .current_dir(root())
            .arg("bench/sqlite/convert.py")
            .arg(&db)
            .arg(&converted)
            .arg(name))?;
        datasets.push(converted);
    }
    run(cargo()
        .args([
            "run",
            "--release",
            "--locked",
            "-p",
            "sov-profiler",
            "--example",
            "fkloco",
            "--",
        ])
        .args(&datasets)
        .args(["--min-mean-f1", TRANSFER_MIN_MEAN_F1]))
}

fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR").map_or_else(|| root().join("target"), PathBuf::from)
}

/// One prerequisite: what it is for, whether the basics need it, and the exact fix when missing.
struct Tool {
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    needed_for: &'static str,
    required: bool,
    fix: &'static str,
}

const TOOLS: &[Tool] = &[
    Tool { name: "rustc", command: "rustc", args: &["--version"], needed_for: "build and test", required: true, fix: "run `bash scripts/bootstrap.sh` (installs rustup and the toolchain pinned in rust-toolchain.toml)" },
    Tool { name: "cargo", command: "cargo", args: &["--version"], needed_for: "build and test", required: true, fix: "run `bash scripts/bootstrap.sh`" },
    Tool { name: "rustfmt", command: "cargo", args: &["fmt", "--version"], needed_for: "ci (format check)", required: true, fix: "`rustup component add rustfmt` (bootstrap.sh does this)" },
    Tool { name: "clippy", command: "cargo", args: &["clippy", "--version"], needed_for: "ci (lint check)", required: true, fix: "`rustup component add clippy` (bootstrap.sh does this)" },
    Tool { name: "git", command: "git", args: &["--version"], needed_for: "bench-fk, bench-transfer", required: false, fix: "`sudo apt-get install -y git`" },
    Tool { name: "curl", command: "curl", args: &["--version"], needed_for: "tools, bench-fk, bench-transfer", required: false, fix: "`sudo apt-get install -y curl`" },
    Tool { name: "java", command: "java", args: &["-version"], needed_for: "tlc (model checking)", required: false, fix: "`sudo apt-get install -y default-jre-headless`" },
    Tool { name: "python3", command: "python3", args: &["--version"], needed_for: "bench-fk, bench-transfer", required: false, fix: "`sudo apt-get install -y python3`" },
    Tool { name: "tar", command: "tar", args: &["--version"], needed_for: "dist", required: false, fix: "`sudo apt-get install -y tar`" },
    Tool { name: "lean", command: "lean", args: &["--version"], needed_for: "lean (proof checking, optional)", required: false, fix: "`bash scripts/bootstrap.sh --with-lean` (installs elan and the pinned Lean)" },
];

fn probe(tool: &Tool) -> Option<String> {
    let out = Command::new(tool.command).args(tool.args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = if out.stdout.is_empty() {
        out.stderr
    } else {
        out.stdout
    };
    Some(
        String::from_utf8_lossy(&text)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    )
}

fn pinned_channel() -> Result<String, String> {
    let text = fs::read_to_string(root().join("rust-toolchain.toml"))
        .map_err(|e| format!("rust-toolchain.toml: {e}"))?;
    text.lines()
        .find(|l| l.trim_start().starts_with("channel"))
        .and_then(|l| l.split('"').nth(1))
        .map(str::to_owned)
        .ok_or_else(|| "no channel in rust-toolchain.toml".to_owned())
}

fn doctor() -> Result<(), String> {
    let pinned = pinned_channel()?;
    println!("doctor: checking prerequisites (pinned Rust toolchain: {pinned})");
    let mut missing_required = 0;
    for tool in TOOLS {
        match probe(tool) {
            Some(version) => {
                let note = if tool.name == "rustc" && !version.contains(&pinned) {
                    format!("  (WARNING: pinned toolchain is {pinned}; rustup will fetch it on the first cargo command)")
                } else {
                    String::new()
                };
                println!("  ok       {:<8} {version}{note}", tool.name);
            }
            None if tool.required => {
                missing_required += 1;
                println!(
                    "  MISSING  {:<8} needed for {}: {}",
                    tool.name, tool.needed_for, tool.fix
                );
            }
            None => println!(
                "  missing  {:<8} needed for {}: {}",
                tool.name, tool.needed_for, tool.fix
            ),
        }
    }
    if missing_required == 0 {
        println!("doctor: everything required is present; run `cargo xtask ci`");
        Ok(())
    } else {
        Err(format!(
            "{missing_required} required tool(s) missing; see the fixes above"
        ))
    }
}

/// Parses `Cargo.lock` into (name, version, source, checksum) tuples, sorted by name and version.
fn lock_packages(lock: &str) -> Vec<(String, String, Option<String>, Option<String>)> {
    let mut out = Vec::new();
    let (mut name, mut version, mut source, mut checksum): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = (None, None, None, None);
    let mut flush = |name: &mut Option<String>,
                     version: &mut Option<String>,
                     source: &mut Option<String>,
                     checksum: &mut Option<String>| {
        if let (Some(n), Some(v)) = (name.take(), version.take()) {
            out.push((n, v, source.take(), checksum.take()));
        }
        *source = None;
        *checksum = None;
    };
    for line in lock.lines() {
        let value = |key: &str| {
            line.strip_prefix(key)
                .and_then(|r| r.split('"').nth(1))
                .map(str::to_owned)
        };
        if line.starts_with("[[package]]") {
            flush(&mut name, &mut version, &mut source, &mut checksum);
        } else if let Some(v) = value("name = ") {
            name = Some(v);
        } else if let Some(v) = value("version = ") {
            version = Some(v);
        } else if let Some(v) = value("source = ") {
            source = Some(v);
        } else if let Some(v) = value("checksum = ") {
            checksum = Some(v);
        }
    }
    flush(&mut name, &mut version, &mut source, &mut checksum);
    out.sort();
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A reproducible `CycloneDX` 1.5 document (no timestamp, no random serial number).
fn sbom_json(lock: &str, product_version: &str) -> String {
    let components: Vec<String> = lock_packages(lock)
        .into_iter()
        .map(|(name, version, source, checksum)| {
            let origin = source.as_deref().map_or("workspace", |_| "registry");
            let hashes = checksum.map_or_else(String::new, |h| format!(",\"hashes\":[{{\"alg\":\"SHA-256\",\"content\":\"{}\"}}]", json_escape(&h)));
            format!(
                "{{\"type\":\"library\",\"name\":\"{n}\",\"version\":\"{v}\",\"purl\":\"pkg:cargo/{n}@{v}\",\"properties\":[{{\"name\":\"origin\",\"value\":\"{origin}\"}}]{hashes}}}",
                n = json_escape(&name),
                v = json_escape(&version)
            )
        })
        .collect();
    format!(
        "{{\"bomFormat\":\"CycloneDX\",\"specVersion\":\"1.5\",\"version\":1,\"metadata\":{{\"component\":{{\"type\":\"application\",\"name\":\"sov\",\"version\":\"{}\"}}}},\"components\":[{}]}}\n",
        json_escape(product_version),
        components.join(",")
    )
}

fn workspace_version() -> Result<String, String> {
    let text =
        fs::read_to_string(root().join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    text.lines()
        .find(|l| l.starts_with("version = "))
        .and_then(|l| l.split('"').nth(1))
        .map(str::to_owned)
        .ok_or_else(|| "no workspace version in Cargo.toml".to_owned())
}

fn sbom() -> Result<(), String> {
    let lock =
        fs::read_to_string(root().join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    let dist = root().join("dist");
    fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;
    let path = dist.join("sbom.cdx.json");
    fs::write(&path, sbom_json(&lock, &workspace_version()?))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let third_party = lock_packages(&lock)
        .iter()
        .filter(|p| p.2.is_some())
        .count();
    println!(
        "sbom: wrote {} ({third_party} third-party packages)",
        path.display()
    );
    Ok(())
}

fn dist() -> Result<(), String> {
    run(cargo().args(["build", "--release", "--locked", "-p", "sov-cli"]))?;
    sbom()?;
    let version = workspace_version()?;
    let name = format!("sov-{version}-{}-{}", env::consts::OS, env::consts::ARCH);
    let dist = root().join("dist");
    let staging = dist.join(&name);
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    let binary = target_dir().join("release/sov");
    fs::copy(&binary, staging.join("sov")).map_err(|e| format!("{}: {e}", binary.display()))?;
    for doc in [
        "EVALUATION-NOTICE.md",
        "docs/EVALUATION-GUIDE.md",
        "docs/SECURITY-AND-PRIVACY.md",
    ] {
        let from = root().join(doc);
        let to = staging.join(Path::new(doc).file_name().unwrap_or_default());
        fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    fs::copy(dist.join("sbom.cdx.json"), staging.join("sbom.cdx.json"))
        .map_err(|e| format!("sbom: {e}"))?;
    // Reproducible archive: sorted names, zero timestamps and owners (GNU tar).
    let archive = dist.join(format!("{name}.tar.gz"));
    run(Command::new("tar")
        .args([
            "--sort=name",
            "--mtime=@0",
            "--owner=0",
            "--group=0",
            "--numeric-owner",
            "-czf",
        ])
        .arg(&archive)
        .arg("-C")
        .arg(&dist)
        .arg(&name))?;
    // Smoke test the packaged binary before publishing anything about it.
    let shown = Command::new(staging.join("sov"))
        .arg("version")
        .output()
        .map_err(|e| format!("packaged binary: {e}"))?;
    if !shown.status.success() {
        return Err("the packaged binary failed to run `version`".to_owned());
    }
    println!(
        "dist: packaged binary reports: {}",
        String::from_utf8_lossy(&shown.stdout).trim()
    );
    let mut sums = String::new();
    for file in [archive.clone(), dist.join("sbom.cdx.json")] {
        let file_name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let _ = writeln!(sums, "{}  {file_name}", sha256_of(&file)?);
    }
    fs::write(dist.join("SHA256SUMS"), &sums).map_err(|e| format!("SHA256SUMS: {e}"))?;
    println!("dist: {}\n{sums}", archive.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{lock_packages, parse_counts, sbom_json};

    #[test]
    fn reads_tlc_summary_line() {
        let out =
            "x\n18833 states generated, 4160 distinct states found, 0 states left on queue.\ny";
        assert_eq!(parse_counts(out), Some((18_833, 4_160)));
    }

    #[test]
    fn ignores_progress_lines_and_uses_the_last_summary() {
        let out = "Progress(6) at t: 76,558 states generated (1 s/min), 8,992 distinct states found (2 ds/min)\n\
                   713185 states generated, 21433 distinct states found, 0 states left on queue.";
        assert_eq!(parse_counts(out), Some((713_185, 21_433)));
    }

    #[test]
    fn lock_parsing_and_sbom_are_deterministic_and_well_formed() {
        let lock = "version = 4\n\n[[package]]\nname = \"zeta\"\nversion = \"1.0.0\"\nsource = \"registry+https://x\"\nchecksum = \"abc123\"\n\n[[package]]\nname = \"alpha\"\nversion = \"0.1.0\"\n";
        let pk = lock_packages(lock);
        assert_eq!(pk.len(), 2);
        assert_eq!(pk[0].0, "alpha");
        let a = sbom_json(lock, "9.9.9");
        assert_eq!(a, sbom_json(lock, "9.9.9"));
        assert!(
            a.contains("\"purl\":\"pkg:cargo/zeta@1.0.0\"")
                && a.contains("\"content\":\"abc123\"")
                && a.contains("\"value\":\"workspace\"")
        );
        assert!(a.starts_with("{\"bomFormat\":\"CycloneDX\"") && a.trim_end().ends_with('}'));
    }

    #[test]
    fn missing_summary_is_none() {
        assert_eq!(parse_counts("no counts here"), None);
    }
}
