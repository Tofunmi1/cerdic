//! `cargo xtask check`: every gate that must pass before work counts as done.
//!
//! Gates: formatting, clippy (warnings are errors), tests, browser (wasm) build, licence policy,
//! file-size cap and the crate dependency rules. One command, same locally and in CI.
#![expect(
    clippy::print_stdout,
    reason = "this is a CLI; stdout is its interface"
)]
#![expect(
    clippy::print_stderr,
    reason = "this is a CLI; stderr is its interface"
)]

mod cpp;
mod parity;
mod rules;
mod web;

use std::process::{Command, ExitCode};

type Failure = Box<dyn std::error::Error>;

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let result = match arg.as_str() {
        "check" => check(),
        "advisories" => advisories(),
        "rules" => rules::run(),
        "cpp" => cpp::run(),
        "parity" => parity::run(),
        "web" => web::build(web::Profile::from_arg(std::env::args().nth(2).as_deref())),
        "web-test" => web::test(web::Profile::from_arg(std::env::args().nth(2).as_deref())),
        _ => {
            eprintln!(
                "usage: cargo xtask <check [--strict]|advisories|rules|cpp|parity|web [dev]|web-test [dev]>"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nFAILED: {e}");
            ExitCode::FAILURE
        }
    }
}

fn check() -> Result<(), Failure> {
    run("fmt", "cargo", &["fmt", "--all", "--check"])?;
    run(
        "clippy",
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    if has("cargo-nextest") {
        run(
            "tests",
            "cargo",
            &["nextest", "run", "--workspace", "--no-tests=pass"],
        )?;
    } else {
        run("tests", "cargo", &["test", "--workspace"])?;
    }
    run(
        "wasm-clippy",
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--exclude",
            "xtask",
            "--target",
            "wasm32-unknown-unknown",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    let mut skipped = Vec::new();
    if has("cargo-deny") {
        run(
            "deny",
            "cargo",
            &["deny", "check", "licenses", "bans", "sources"],
        )?;
    } else {
        println!("-- deny: skipped (cargo-deny not installed)");
        skipped.push("deny (cargo-deny not installed)");
    }
    cpp::run()?;
    parity::run()?;
    if has("node") {
        web::test(web::Profile::Dev)?;
    } else {
        println!("-- web-test: skipped (node not found)");
        skipped.push("web-test (node not found)");
    }
    rules::run()?;
    finish(&skipped)
}

/// Reports the outcome without overstating it: a run that skipped a gate has not passed that gate,
/// and with `--strict` (or `VE_STRICT=1`, for release evidence and CI) a skip is a failure.
fn finish(skipped: &[&str]) -> Result<(), Failure> {
    if skipped.is_empty() {
        println!("\nall gates passed");
        return Ok(());
    }
    let strict = std::env::args().any(|a| a == "--strict")
        || std::env::var("VE_STRICT").is_ok_and(|v| v == "1");
    if strict {
        return Err(format!("strict mode: skipped gates: {}", skipped.join(", ")).into());
    }
    println!(
        "\nchecks that ran passed, but {} gate(s) were SKIPPED: {}\n(use `cargo xtask check --strict` to make a skip a failure)",
        skipped.len(),
        skipped.join(", ")
    );
    Ok(())
}

/// Checks the dependency tree against the `RustSec` advisory database. It needs network access to
/// fetch the database, so it is its own command rather than part of `check`.
fn advisories() -> Result<(), Failure> {
    if !has("cargo-deny") {
        return Err("cargo-deny is not installed".into());
    }
    run("advisories", "cargo", &["deny", "check", "advisories"])
}

fn has(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run(label: &str, program: &str, args: &[&str]) -> Result<(), Failure> {
    println!("-- {label}: {program} {}", args.join(" "));
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed").into())
    }
}
