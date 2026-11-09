//! The `sov` command-line tool (evaluation build).
//!
//! Commands: `profile` (learn relationships from table exports and reconcile them with a Maximo
//! dictionary), `audit` (re-derive stored balances from transactions), `demo` (a self-contained tour on
//! a synthetic database), `digest` (SHA-256 of a file, so downloads can be verified without other
//! tools), `version`, `help`.
//!
//! The tool reads local files and writes to standard output. It opens **no network connections**,
//! sends **no telemetry**, and never modifies the files it reads. Output is deterministic: the same
//! input gives byte-identical output and the same report digest.

mod demo;
mod load;
mod report;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

/// What a command produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Text for standard output.
    pub stdout: String,
    /// Text for standard error.
    pub stderr: String,
    /// Process exit code: 0 success, 1 failure, 2 usage error.
    pub code: i32,
}

impl Outcome {
    fn ok(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            code: 0,
        }
    }

    fn fail(message: &str) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("error: {message}\n"),
            code: 1,
        }
    }

    fn usage(message: &str) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("error: {message}\n\n{HELP}"),
            code: 2,
        }
    }
}

/// The tool's version line.
pub const VERSION: &str = concat!("sov ", env!("CARGO_PKG_VERSION"), " (evaluation build)");

const HELP: &str = "sov: Sovereign ERP source profiler (evaluation build)

USAGE
  sov profile <DIR> [--dictionary FILE] [--max-rows N] [--json]
      Read every <DIR>/*.tsv (header line, then rows; \\N means NULL) as a table, find relationships,
      and reconcile them with a Maximo MAXRELATIONSHIP dictionary (FILE, or a table named
      MAXRELATIONSHIP in <DIR>). Reports provenance, key direction, cardinality and orphan rows.
  sov audit <DIR> --tx TABLE --tx-keys A,B --tx-amount COL --bal TABLE --bal-keys A,B --bal-amount COL
      Re-derive each stored balance from its transactions and list the differences.
  sov demo [--export DIR]
      Run a self-contained tour on a synthetic Maximo-shaped database. --export writes that database
      as TSV files you can feed to `sov profile`.
  sov digest <FILE>
      Print the SHA-256 of a file.
  sov version | sov help

This is an evaluation build, not licensed for production use. It reads local files only, opens no
network connections, sends no telemetry and never modifies its input.
";

/// Parsed command line: positional words and `--flag [value]` options.
struct Args {
    positional: Vec<String>,
    flags: HashMap<String, Option<String>>,
}

const VALUE_FLAGS: [&str; 10] = [
    "dictionary",
    "max-rows",
    "export",
    "tx",
    "tx-keys",
    "tx-amount",
    "bal",
    "bal-keys",
    "bal-amount",
    "format",
];

fn parse(args: &[String]) -> Result<Args, String> {
    let (mut positional, mut flags) = (Vec::new(), HashMap::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            if VALUE_FLAGS.contains(&name) {
                let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
                flags.insert(name.to_owned(), Some(value.clone()));
            } else if name == "json" {
                flags.insert(name.to_owned(), None);
            } else {
                return Err(format!("unknown option --{name}"));
            }
        } else {
            positional.push(a.clone());
        }
    }
    Ok(Args { positional, flags })
}

impl Args {
    fn value(&self, name: &str) -> Option<&str> {
        self.flags.get(name).and_then(|v| v.as_deref())
    }

    fn need(&self, name: &str) -> Result<&str, String> {
        self.value(name).ok_or_else(|| format!("missing --{name}"))
    }

    fn list(&self, name: &str) -> Result<Vec<String>, String> {
        Ok(self
            .need(name)?
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect())
    }
}

/// Runs the tool with the given arguments (everything after the program name).
#[must_use]
pub fn run(args: &[String]) -> Outcome {
    let Some(command) = args.first() else {
        return Outcome::usage("no command given");
    };
    let rest = &args[1..];
    match command.as_str() {
        "version" | "--version" | "-V" => Outcome::ok(format!("{VERSION}\n")),
        "help" | "--help" | "-h" => Outcome::ok(HELP.to_owned()),
        "profile" | "audit" | "demo" | "digest" => match parse(rest) {
            Ok(parsed) => dispatch(command, &parsed),
            Err(message) => Outcome::usage(&message),
        },
        other => Outcome::usage(&format!("unknown command {other:?}")),
    }
}

fn dispatch(command: &str, args: &Args) -> Outcome {
    let result = match command {
        "profile" => profile(args),
        "audit" => audit(args),
        "demo" => demo::run(args.value("export").map(Path::new)),
        _ => digest(args),
    };
    match result {
        Ok(text) => Outcome::ok(text),
        Err(CliError::Usage(m)) => Outcome::usage(&m),
        Err(CliError::Failed(m)) => Outcome::fail(&m),
    }
}

/// A command failure: a usage mistake (exit 2) or a runtime failure (exit 1).
enum CliError {
    Usage(String),
    Failed(String),
}

impl From<String> for CliError {
    fn from(m: String) -> Self {
        Self::Failed(m)
    }
}

fn one_dir(args: &Args) -> Result<&str, CliError> {
    match args.positional.as_slice() {
        [dir] => Ok(dir),
        _ => Err(CliError::Usage("expected exactly one directory".to_owned())),
    }
}

fn profile(args: &Args) -> Result<String, CliError> {
    let dir = one_dir(args)?;
    let max_rows = match args.value("max-rows") {
        Some(v) => v.parse::<usize>().map_err(|_| {
            CliError::Usage(format!("--max-rows must be a whole number, got {v:?}"))
        })?,
        None => load::DEFAULT_MAX_ROWS,
    };
    let loaded = load::tables(Path::new(dir), max_rows)?;
    let dictionary = match args.value("dictionary") {
        Some(file) => load::dictionary_file(Path::new(file))?,
        None => loaded.dictionary.clone(),
    };
    let report = report::profile(&loaded, &dictionary);
    Ok(if args.flags.contains_key("json") {
        report::render_json(&report)
    } else {
        report::render_text(&report)
    })
}

fn audit(args: &Args) -> Result<String, CliError> {
    let dir = one_dir(args)?;
    let loaded = load::tables(Path::new(dir), load::DEFAULT_MAX_ROWS)?;
    let find = |name: &str| {
        loaded
            .all
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| CliError::Failed(format!("no table named {name:?} in {dir}")))
    };
    let (tx, bal) = (
        find(args.need("tx").map_err(CliError::Usage)?)?,
        find(args.need("bal").map_err(CliError::Usage)?)?,
    );
    let tx_keys = args.list("tx-keys").map_err(CliError::Usage)?;
    let bal_keys = args.list("bal-keys").map_err(CliError::Usage)?;
    let tx_key_refs: Vec<&str> = tx_keys.iter().map(String::as_str).collect();
    let bal_key_refs: Vec<&str> = bal_keys.iter().map(String::as_str).collect();
    let spec = sov_profiler::audit::BalanceSpec {
        transactions: tx,
        tx_keys: &tx_key_refs,
        tx_amount: args.need("tx-amount").map_err(CliError::Usage)?,
        balances: bal,
        bal_keys: &bal_key_refs,
        bal_amount: args.need("bal-amount").map_err(CliError::Usage)?,
    };
    let result =
        sov_profiler::audit::balance_audit(&spec).map_err(|e| CliError::Failed(e.to_string()))?;
    Ok(report::render_audit(&result))
}

fn digest(args: &Args) -> Result<String, CliError> {
    let [file] = args.positional.as_slice() else {
        return Err(CliError::Usage("expected exactly one file".to_owned()));
    };
    let mut hasher = sov_evidence::Sha256::new();
    let data = std::fs::read(file).map_err(|e| CliError::Failed(format!("{file}: {e}")))?;
    hasher.update(&data);
    let mut out = String::new();
    let _ = writeln!(out, "{}  {file}", hasher.finalize());
    Ok(out)
}
