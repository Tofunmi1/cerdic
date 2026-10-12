//! Native versus wasm parity: the kernel corpus must print identical output in both worlds.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::fmt::Write as _;
use std::process::Command;

type Failure = Box<dyn std::error::Error>;

const WASM_PATH: &str = "target/wasm32-wasip1/debug/examples/corpus.wasm";

pub(crate) fn run() -> Result<(), Failure> {
    println!("-- parity: ve-kernel corpus, native vs wasm32-wasip1 under Node WASI");
    if !Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        println!("-- parity: skipped (node not found)");
        return Ok(());
    }
    compare("ve-kernel")?;
    compare("ve-modeling")
}

fn compare(crate_name: &str) -> Result<(), Failure> {
    let native = capture(
        "cargo",
        &["run", "-q", "-p", crate_name, "--example", "corpus"],
    )?;
    run_quiet(
        "cargo",
        &[
            "build",
            "-q",
            "-p",
            crate_name,
            "--example",
            "corpus",
            "--target",
            "wasm32-wasip1",
        ],
    )?;
    let wasm = capture("node", &["tools/run-wasi.mjs", WASM_PATH])?;
    if native == wasm {
        println!(
            "   {crate_name}: identical ({} lines)",
            native.lines().count()
        );
        return Ok(());
    }
    let mut report = String::from("native and wasm corpus output differ:\n");
    for (index, pair) in native.lines().zip(wasm.lines()).enumerate() {
        if pair.0 != pair.1 {
            writeln!(
                report,
                "  line {}:\n    native: {}\n    wasm:   {}",
                index + 1,
                pair.0,
                pair.1
            )?;
        }
    }
    if native.lines().count() != wasm.lines().count() {
        report.push_str("  the outputs have different line counts\n");
    }
    Err(report.into())
}

fn capture(program: &str, args: &[&str]) -> Result<String, Failure> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn run_quiet(program: &str, args: &[&str]) -> Result<(), Failure> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed").into())
    }
}
