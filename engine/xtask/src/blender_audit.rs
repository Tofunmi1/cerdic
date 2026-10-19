//! Audit of the Blender functions the wasm build deliberately leaves unresolved.
//!
//! The linker turns every unresolved function into a trap (`.cargo/config.toml`), which would also hide
//! a function we forgot to link. So the set of unresolved Blender symbols is pinned in
//! `cpp/blender/unresolved.txt` (native links trap stubs generated from the same file) and this gate
//! fails when the wasm libraries leave a symbol out of that list or the list names one that is no
//! longer unresolved.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

type Failure = Box<dyn std::error::Error>;

const LIBRARIES: &[&str] = &[
    "libve_blender_core.a",
    "libve_blender_bmesh.a",
    "libve_blender_geometry.a",
    "libve_blender_ops.a",
];

/// Names compared across platforms: the wasi sysroot's libc++ spells its inline namespace `__2`, the
/// macOS one `__1`, and `unresolved.txt` is written without the Itanium leading underscore.
fn normalise(name: &str) -> String {
    name.trim_start_matches('_')
        .replace("NSt3__2", "NSt3__1")
        .replace("St3__2", "St3__1")
}

fn symbols(llvm_nm: &Path, library: &Path, flag: &str) -> Result<BTreeSet<String>, Failure> {
    let output = Command::new(llvm_nm).arg(flag).arg(library).output()?;
    if !output.status.success() {
        return Err(format!("llvm-nm failed on {}", library.display()).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter(|name| name.contains("7blender"))
        .map(normalise)
        .collect())
}

/// Fails when the unresolved Blender symbols of the wasm libraries differ from `unresolved.txt`.
pub(crate) fn run(sdk: &Path, build_dir: &str) -> Result<(), Failure> {
    println!("-- cpp: audit of unresolved Blender symbols (wasm)");
    let status = Command::new("cmake")
        .args(["--build", build_dir, "--target", "ve_blender_ops"])
        .status()?;
    if !status.success() {
        return Err("building the wasm Blender libraries failed".into());
    }
    let llvm_nm = sdk.join("bin/llvm-nm");
    let (mut undefined, mut defined) = (BTreeSet::new(), BTreeSet::new());
    for library in LIBRARIES {
        let path = Path::new(build_dir).join("lib").join(library);
        undefined.extend(symbols(&llvm_nm, &path, "--undefined-only")?);
        defined.extend(symbols(&llvm_nm, &path, "--defined-only")?);
    }
    let unresolved: BTreeSet<String> = undefined.difference(&defined).cloned().collect();
    let listed: BTreeSet<String> = std::fs::read_to_string("cpp/blender/unresolved.txt")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(normalise)
        .collect();
    let missing: Vec<&String> = unresolved.difference(&listed).collect();
    let stale: Vec<&String> = listed.difference(&unresolved).collect();
    for name in &missing {
        println!("unresolved but not in cpp/blender/unresolved.txt: {name}");
    }
    for name in &stale {
        println!("in cpp/blender/unresolved.txt but resolved or unused: {name}");
    }
    if missing.is_empty() && stale.is_empty() {
        println!(
            "   {} unresolved Blender symbols, all listed",
            unresolved.len()
        );
        Ok(())
    } else {
        Err("cpp/blender/unresolved.txt is out of date".into())
    }
}
