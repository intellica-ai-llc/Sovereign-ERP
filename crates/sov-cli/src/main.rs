//! The `sov` binary: a thin shell around [`sov_cli::run`]. All behaviour lives in the library so it can
//! be tested; this file only wires standard input, output and the exit code.
#![allow(clippy::print_stdout, clippy::print_stderr)] // writing to the terminal is this binary's job

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = sov_cli::run(&args);
    print!("{}", outcome.stdout);
    if !outcome.stderr.is_empty() {
        eprint!("{}", outcome.stderr);
    }
    std::process::exit(outcome.code);
}
