//! Repository rules that no off-the-shelf tool checks.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

type Failure = Box<dyn std::error::Error>;

/// Longest allowed `.rs` file under `crates/`. Small files keep LLM edits and reviews tractable.
const MAX_FILE_LINES: usize = 400;

/// Crates that must stay independent of the rendering engine and platform layers.
const ENGINE_AGNOSTIC: &[&str] = &[
    "ve-core",
    "ve-geometry",
    "ve-graph",
    "ve-interact",
    "ve-kernel",
    "ve-kernel-sys",
    "ve-modeling",
    "ve-provider",
    "ve-xr-math",
];
/// Dependency name prefixes the engine-agnostic crates may not use.
const FORBIDDEN_PREFIXES: &[&str] = &["bevy", "wgpu", "winit", "web-sys", "js-sys", "wasm-bindgen"];

pub(crate) fn run() -> Result<(), Failure> {
    println!("-- rules: file size <= {MAX_FILE_LINES} lines, dependency boundaries");
    let mut problems = Vec::new();
    file_sizes(Path::new("crates"), &mut problems)?;
    dependency_boundaries(&mut problems)?;
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n").into())
    }
}

fn file_sizes(dir: &Path, problems: &mut Vec<String>) -> Result<(), Failure> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            file_sizes(&path, problems)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let lines = fs::read_to_string(&path)?.lines().count();
            if lines > MAX_FILE_LINES {
                problems.push(format!(
                    "{} has {lines} lines (max {MAX_FILE_LINES})",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn dependency_boundaries(problems: &mut Vec<String>) -> Result<(), Failure> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()?;
    if !out.status.success() {
        return Err("cargo metadata failed".into());
    }
    let meta: Value = serde_json::from_slice(&out.stdout)?;
    let packages = meta["packages"]
        .as_array()
        .ok_or("metadata has no packages")?;
    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or_default();
        if !ENGINE_AGNOSTIC.contains(&name) {
            continue;
        }
        for dep in pkg["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            if FORBIDDEN_PREFIXES.iter().any(|p| dep_name.starts_with(p)) {
                problems.push(format!(
                    "{name} must not depend on {dep_name} (engine-agnostic crate)"
                ));
            }
        }
    }
    Ok(())
}
