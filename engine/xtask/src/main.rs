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

use std::process::{Command, ExitCode};

type Failure = Box<dyn std::error::Error>;

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let result = match arg.as_str() {
        "check" => check(),
        "rules" => rules::run(),
        "cpp" => cpp::run(),
        "parity" => parity::run(),
        _ => {
            eprintln!("usage: cargo xtask <check|rules|cpp|parity>");
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
        "wasm",
        "cargo",
        &[
            "check",
            "--workspace",
            "--exclude",
            "xtask",
            "--target",
            "wasm32-unknown-unknown",
        ],
    )?;
    if has("cargo-deny") {
        run(
            "deny",
            "cargo",
            &["deny", "check", "licenses", "bans", "sources"],
        )?;
    } else {
        println!("-- deny: skipped (cargo-deny not installed)");
    }
    cpp::run()?;
    parity::run()?;
    rules::run()?;
    println!("\nall gates passed");
    Ok(())
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
