//! `cargo xtask quick [crate...]`: the fast loop while working. It checks formatting, lints and
//! tests only the crates with uncommitted changes (or the ones named) and the crates that depend on
//! them, then the file-size and boundary rules. It does not replace `cargo xtask check` before a
//! commit: it skips the other crates, the wasm lint, the C++ gates, parity and the browser suites.

use std::collections::BTreeSet;
use std::process::Command;

use crate::{Failure, has, rules, run};

/// The crates under `crates/` that `git status` reports as changed or new.
fn changed_crates() -> Result<BTreeSet<String>, Failure> {
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "crates",
        ])
        .output()?;
    if !output.status.success() {
        return Err("git status failed".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines()
         // A rename prints "old -> new"; the new path is the one that matters.
         .filter_map(| line | line.rsplit(" -> ").next())
         .filter_map(| path |
                     {
                       let rest = path.split_once("crates/") ? .1;
            Some(rest.split('/').next()?.trim_matches('"').to_owned())
                     })
         .collect())
}

/// Runs the fast checks. `named` overrides the changed-crate detection.
pub(crate) fn run_quick(named: &[String]) -> Result<(), Failure> {
    let crates = if named.is_empty() {
        changed_crates()?
    } else {
        named.iter().cloned().collect()
    };
    run("fmt", "cargo", &["fmt", "--all", "--check"])?;
    if crates.is_empty() {
        println!("-- quick: no changed crates, so no clippy or tests");
    } else {
        println!(
            "-- quick: crates {}",
            crates.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        let mut clippy = vec!["clippy", "--all-targets"];
        for name in &crates {
            clippy.extend(["-p", name]);
        }
        clippy.extend(["--", "-D", "warnings"]);
        run("clippy", "cargo", &clippy)?;
        test(&crates)?;
    }
    rules::run()?;
    println!("\nquick checks passed (run `cargo xtask check` before a commit)");
    Ok(())
}

/// Tests the crates and everything that depends on them.
fn test(crates: &BTreeSet<String>) -> Result<(), Failure> {
    if has("cargo-nextest") {
        let filter = crates
            .iter()
            .map(|name| format!("rdeps({name})"))
            .collect::<Vec<_>>()
            .join(" | ");
        run(
            "tests",
            "cargo",
            &[
                "nextest",
                "run",
                "--workspace",
                "--no-tests=pass",
                "-E",
                &filter,
            ],
        )
    } else {
        let mut args = vec!["test"];
        for name in crates {
            args.extend(["-p", name]);
        }
        run("tests", "cargo", &args)
    }
}
