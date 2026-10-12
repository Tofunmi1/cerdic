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
    for page_file in [
        "index.html",
        "bench.js",
        "viewer.js",
        "document-api.js",
        "docpanel.js",
    ] {
        fs::copy(format!("web/{page_file}"), format!("{DIST}/{page_file}"))?;
    }
    let size = fs::metadata(format!("{DIST}/ve_runtime_bg.wasm"))?.len();
    println!("   wasm: {size} bytes");
    build_evaluator(profile, &bindgen)?;
    // The viewer page is plain JavaScript; it gets the evaluation pipeline and the test MCP bridge
    // from the same TypeScript the editor uses, bundled into one module.
    run_in(
        "..",
        "bun",
        &[
            "build",
            "src/engine/viewer-bridge.ts",
            "--outfile",
            &format!("engine/{DIST}/viewer-bridge.js"),
            "--target",
            "browser",
            "--format",
            "esm",
        ],
    )?;
    Ok(())
}

fn build_evaluator(profile: Profile, bindgen: &str) -> Result<(), Failure> {
    run(
        "bun",
        &[
            "-e",
            "new Bun.Transpiler({loader:'js'}).transformSync(await Bun.file('web/eval-worker.js').text());",
        ],
    )?;
    let name = profile.cargo_name();
    run(
        "cargo",
        &[
            "build",
            "-p",
            "ve-evaluator",
            "--target",
            "wasm32-unknown-unknown",
            "--profile",
            name,
        ],
    )?;
    run(
        bindgen,
        &[
            "--target",
            "web",
            "--out-dir",
            DIST,
            "--no-typescript",
            &format!("target/wasm32-unknown-unknown/{name}/ve_evaluator.wasm"),
        ],
    )?;
    let glue = format!("{DIST}/ve_evaluator.js");
    let patched = fs::read_to_string(&glue)?.replace(
        "from \"wasi_snapshot_preview1\"",
        "from \"./wasi_snapshot_preview1.js\"",
    );
    fs::write(glue, patched)?;
    fs::copy("web/eval-worker.js", format!("{DIST}/eval-worker.js"))?;
    Ok(())
}

/// Builds, then runs the Playwright acceptance test against the build.
pub(crate) fn test(profile: Profile) -> Result<(), Failure> {
    build(profile)?;
    if !Path::new("web/node_modules/playwright").exists() {
        run_in("web", "bun", &["install", "--frozen-lockfile"])?;
    }
    println!("-- web-test: playwright (headless Chromium)");
    run_in("web", "bun", &["run", "test"])?;
    run_in("..", "bun", &["run", "typecheck"])?;
    run_in("..", "bun", &["scripts/build-engine.ts", "--copy-only"])?;
    run("bun", &["web/tests/editor.mjs"])?;
    run("bun", &["web/tests/evaluation.mjs"])?;
    run("bun", &["web/tests/gallery.mjs"])?;
    run("bun", &["web/tests/mcp.mjs"])?;
    run("bun", &["web/tests/reflection.mjs"])
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
