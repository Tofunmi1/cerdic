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

mod blender_audit;
mod command;
mod cpp;
mod parity;
mod quick;
mod rules;
mod stamp;
mod texture_assets;
mod web;
mod web_pool;

use std::process::ExitCode;

pub(crate) use command::{has, run, timed};

type Failure = Box<dyn std::error::Error>;

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let result = match arg.as_str() {
        "check" => check(),
        "quick" => quick::run_quick(&std::env::args().skip(2).collect::<Vec<_>>()),
        "advisories" => command::advisories(),
        "rules" => rules::run(),
        "cpp" => cpp::run(),
        "parity" => parity::run(),
        "web" => web::build(web::Profile::from_arg(std::env::args().nth(2).as_deref())),
        "web-test" => web::test_filtered(
            web::Profile::from_arg(std::env::args().nth(2).as_deref()),
            &std::env::args().skip(3).collect::<Vec<_>>(),
        ),
        _ => {
            eprintln!(
                "usage: cargo xtask <check [--strict] [--no-cache]|quick [crate...]|advisories|rules|cpp|parity|web [dev]|web-test [dev] [name...] >"
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

macro_rules! gate {
    ($report
       : ident, $no_cache
       : ident, $name
       : literal, $paths
       : expr, $rerun
       : literal, $action
       : expr) => {
        stage(&mut *$report, $no_cache, $name, $paths, $rerun, $action)?
    };
}

fn check() -> Result<(), Failure> {
    let no_cache = std::env::args().any(|arg| arg == "--no-cache")
        || std::env::var("VE_NO_CACHE").is_ok_and(|value| value == "1");
    let mut report = stamp::Report::default();
    let mut skipped = Vec::new();
    let result = run_gates(&mut report, no_cache, &mut skipped);
    report.print();
    result?;
    finish(&skipped)
}

fn run_gates(
    report: &mut stamp::Report,
    no_cache: bool,
    skipped: &mut Vec<&'static str>,
) -> Result<(), Failure> {
    early_gates(report, no_cache)?;
    optional_gate(report, no_cache, skipped)?;
    system_gates(report, no_cache)?;
    browser_gate(report, no_cache, skipped)?;
    Ok(())
}

fn early_gates(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    schema_gate(report, no_cache)?;
    format_gate(report, no_cache)?;
    rules_gate(report, no_cache)?;
    lint_gates(report, no_cache)
}

fn schema_gate(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    gate!(
        report,
        no_cache,
        "bridge-schema",
        // The check compares the Rust output with the checked-in contract, so that file is an input.
        &[
            "crates/ve-graph",
            "../src/engine/generated.ts",
            "Cargo.toml",
            "Cargo.lock"
        ],
        "cargo run -p ve-graph --features typescript --example typescript -- --check",
        || run(
            "bridge-schema",
            "cargo",
            &[
                "run",
                "-p",
                "ve-graph",
                "--features",
                "typescript",
                "--example",
                "typescript",
                "--",
                "--check"
            ]
        )
    );
    Ok(())
}

fn format_gate(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    gate!(
        report,
        no_cache,
        "fmt",
        &["crates", "xtask", "Cargo.toml", "rustfmt.toml"],
        "cargo fmt --all --check",
        || run("fmt", "cargo", &["fmt", "--all", "--check"])
    );
    Ok(())
}

fn lint_gates(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    gate!(
        report,
        no_cache,
        "clippy",
        &["crates", "xtask", "Cargo.toml", "Cargo.lock"],
        "cargo clippy --workspace --all-targets -- -D warnings",
        || run(
            "clippy",
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings"
            ]
        )
    );
    gate!(
        report,
        no_cache,
        "tests",
        &[
            "crates",
            "cpp",
            "lib",
            "xtask",
            "Cargo.toml",
            "Cargo.lock"
        ],
        "cargo nextest run --workspace --no-tests=pass",
        || {
            if has("cargo-nextest") {
                run(
                    "tests",
                    "cargo",
                    &["nextest", "run", "--workspace", "--no-tests=pass"],
                )
            } else {
                run("tests", "cargo", &["test", "--workspace"])
            }
        }
    );
    gate!(
        report,
        no_cache,
        "wasm-clippy",
        &["crates", "cpp", "lib", "Cargo.toml", "Cargo.lock"],
        "cargo clippy --workspace --exclude xtask --target wasm32-unknown-unknown -- -D warnings",
        || run(
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
                "warnings"
            ]
        )
    );
    Ok(())
}

fn optional_gate(
    report: &mut stamp::Report,
    no_cache: bool,
    skipped: &mut Vec<&'static str>,
) -> Result<(), Failure> {
    if has("cargo-deny") {
        gate!(
            report,
            no_cache,
            "deny",
            &["crates", "Cargo.toml", "Cargo.lock", "deny.toml"],
            "cargo deny check licenses bans sources",
            || run(
                "deny",
                "cargo",
                &["deny", "check", "licenses", "bans", "sources"]
            )
        );
    } else {
        println!("-- deny: skipped (cargo-deny not installed)");
        skipped.push("deny (cargo-deny not installed)");
    }
    Ok(())
}

fn system_gates(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    gate!(
        report,
        no_cache,
        "cpp",
        &["cpp", "lib", "crates/ve-kernel-sys"],
        "cargo xtask cpp",
        || timed("cpp", cpp::run)
    );
    gate!(
        report,
        no_cache,
        "parity",
        &[
            "crates",
            "cpp",
            "lib",
            "tools/run-wasi.mjs",
            "Cargo.toml",
            "Cargo.lock"
        ],
        "cargo xtask parity",
        || timed("parity", parity::run)
    );
    Ok(())
}

fn browser_gate(
    report: &mut stamp::Report,
    no_cache: bool,
    skipped: &mut Vec<&'static str>,
) -> Result<(), Failure> {
    if has("bun") {
        gate!(
            report,
            no_cache,
            "web-test",
            &[
                "web",
                "../src",
                "../scripts",
                "../package.json",
                "../bun.lock",
                "../tsconfig.json",
                "../tsconfig.app.json",
                "../tsconfig.node.json",
                "../tsconfig.server.json",
                "crates",
                "cpp",
                "lib",
                "Cargo.lock",
                "tools/mcp"
            ],
            "cargo xtask web-test dev <name>",
            || timed("web-test", || web::test(web::Profile::Dev))
        );
    } else {
        println!("-- web-test: skipped (bun not found)");
        skipped.push("web-test (bun not found)");
    }
    Ok(())
}

fn rules_gate(report: &mut stamp::Report, no_cache: bool) -> Result<(), Failure> {
    gate!(
        report,
        no_cache,
        "rules",
        &["crates", "Cargo.toml"],
        "cargo xtask rules",
        rules::run
    );
    Ok(())
}

fn stage(
    report: &mut stamp::Report,
    no_cache: bool,
    name: &str,
    paths: &[&str],
    rerun: &str,
    run_stage: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure> {
    let key = stamp::key(paths)?;
    if !no_cache && stamp::matches(name, &key) {
        println!(
            "-- {name}: cached pass (inputs unchanged since {})",
            stamp::cached_at(name)
        );
        report.record(name, "cached", std::time::Duration::ZERO);
        return Ok(());
    }
    let started = std::time::Instant::now();
    match stamp::run_fresh(name, &key, run_stage) {
        Ok(()) => {
            report.record(name, "fresh", started.elapsed());
            Ok(())
        }
        Err(error) => {
            report.record(name, "FAILED", started.elapsed());
            Err(format!("{error}; rerun with `{rerun}`").into())
        }
    }
}

/// Reports the outcome without overstating it: a run that skipped a gate has not passed that
/// gate, and with `--strict` (or `VE_STRICT=1`, for release evidence and CI) a skip is a
/// failure.
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
