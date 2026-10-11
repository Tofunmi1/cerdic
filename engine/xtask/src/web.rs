//! Web build and browser tests: wasm release build, wasm-bindgen glue, page, Playwright run.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::fs;
use std::path::Path;
use std::process::Command;

type Failure = Box<dyn std::error::Error>;

const TOOLS: &str = "target/tools";
const DIST: &str = "web/dist";

/// How the wasm is built: `dev` for fast iteration, `release` for real measurements.
#[derive(Clone, Copy)]
pub(crate) enum Profile {
    Dev,
    Release,
}

impl Profile {
    /// `dev` selects the fast profile; anything else (or nothing) is a release build.
    pub(crate) fn from_arg(arg: Option<&str>) -> Self {
        if arg == Some("dev") {
            Self::Dev
        } else {
            Self::Release
        }
    }

    fn cargo_name(self) -> &'static str {
        match self {
            Self::Dev => "web-dev",
            Self::Release => "release",
        }
    }
}

/// Builds `web/dist`: the wasm, its JS glue and the page.
pub(crate) fn build(profile: Profile) -> Result<(), Failure> {
    let name = profile.cargo_name();
    println!("-- web: wasm {name} build + wasm-bindgen into {DIST}");
    let bindgen = ensure_wasm_bindgen()?;
    run(
        "cargo",
        &[
            "build",
            "-p",
            "ve-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--profile",
            name,
        ],
    )?;
    fs::create_dir_all(DIST)?;
    let wasm = format!("target/wasm32-unknown-unknown/{name}/ve_runtime.wasm");
    run(
        &bindgen,
        &[
            "--target",
            "web",
            "--out-dir",
            DIST,
            "--no-typescript",
            &wasm,
        ],
    )?;
    // The linked C++ imports a few wasi functions; point them at the local shim.
    let glue = format!("{DIST}/ve_runtime.js");
    // Only the module specifiers change; the import-object keys keep the module name the wasm uses.
    let patched = fs::read_to_string(&glue)?.replace(
        "from \"wasi_snapshot_preview1\"",
        "from \"./wasi_snapshot_preview1.js\"",
    );
    fs::write(&glue, patched)?;
    fs::copy(
        "web/wasi_snapshot_preview1.js",
        format!("{DIST}/wasi_snapshot_preview1.js"),
    )?;
    fs::copy("web/index.html", format!("{DIST}/index.html"))?;
    let size = fs::metadata(format!("{DIST}/ve_runtime_bg.wasm"))?.len();
    println!("   wasm: {size} bytes");
    Ok(())
}

/// Builds, then runs the Playwright acceptance test against the build.
pub(crate) fn test(profile: Profile) -> Result<(), Failure> {
    build(profile)?;
    if !Path::new("web/node_modules/playwright").exists() {
        run_in("web", "npm", &["install", "--no-audit", "--no-fund"])?;
    }
    println!("-- web-test: playwright (headless Chromium)");
    run_in("web", "npm", &["test", "--silent"])
}

/// The wasm-bindgen CLI must match the library version in `Cargo.lock` exactly.
fn ensure_wasm_bindgen() -> Result<String, Failure> {
    let version = locked_version("wasm-bindgen")?;
    let binary = format!("{TOOLS}/bin/wasm-bindgen");
    let installed = Command::new(&binary)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    if installed.as_deref() == Some(&format!("wasm-bindgen {version}")) {
        return Ok(binary);
    }
    println!("   installing wasm-bindgen-cli {version} into {TOOLS}");
    run(
        "cargo",
        &[
            "install",
            "wasm-bindgen-cli",
            "--version",
            &version,
            "--locked",
            "--root",
            TOOLS,
        ],
    )?;
    Ok(binary)
}

fn locked_version(package: &str) -> Result<String, Failure> {
    let lock = fs::read_to_string("Cargo.lock")?;
    let header = format!("name = \"{package}\"\nversion = \"");
    let start = lock
        .find(&header)
        .ok_or_else(|| format!("{package} not in Cargo.lock"))?
        + header.len();
    let end = lock[start..].find('"').ok_or("malformed Cargo.lock")? + start;
    Ok(lock[start..end].to_owned())
}

fn run(program: &str, args: &[&str]) -> Result<(), Failure> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed").into())
    }
}

fn run_in(dir: &str, program: &str, args: &[&str]) -> Result<(), Failure> {
    let status = Command::new(program).args(args).current_dir(dir).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed in {dir}").into())
    }
}
