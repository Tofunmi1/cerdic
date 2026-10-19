//! Shared command runners for xtask subcommands.

use std::process::Command;

use crate::Failure;

pub(crate) fn advisories() -> Result<(), Failure> {
    if !has("cargo-deny") {
        return Err("cargo-deny is not installed".into());
    }
    run("advisories", "cargo", &["deny", "check", "advisories"])
}

pub(crate) fn has(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub(crate) fn run(label: &str, program: &str, args: &[&str]) -> Result<(), Failure> {
    println!("-- {label}: {program} {}", args.join(" "));
    timed(label, || {
        let status = Command::new(program).args(args).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{label} failed").into())
        }
    })
}

pub(crate) fn timed(
    label: &str,
    stage: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure> {
    let started = std::time::Instant::now();
    let result = stage();
    println!(
        "-- {label}: {} in {:.0}s",
        if result.is_ok() { "done" } else { "FAILED" },
        started.elapsed().as_secs_f64()
    );
    result
}
