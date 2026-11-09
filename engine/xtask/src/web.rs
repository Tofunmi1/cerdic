//! Web build and browser tests: wasm release build, wasm-bindgen glue, page, Playwright run.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::web_pool as pool;

/// How many parallel shards the gallery test is split into (each uses its own dev-server port).
const GALLERY_SHARDS: usize = 3;

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
    fs::create_dir_all(format!("{DIST}/models"))?;
    fs::copy(
        "crates/ve-modeling/tests/gallery/viaduct.txt",
        format!("{DIST}/models/viaduct.txt"),
    )?;
    for demo in [
        "physics",
        "physics_big",
        "physics_joints",
        "physics_walk",
        "animation",
        "scenery",
        "village",
        "village_hd",
        "beach_arch",
        "mannequin",
        "toon_gallery",
        "weathered_stone",
        "style_gallery",
        "outdoor_sky",
        "materials_grid",
        "lit_room",
        "modern_room",
        "pegasus_studio",
        "angel_studio",
        "building_asset",
        "architecture_gallery",
        "church_sanctuary",
        "main_street",
        "model_y",
        "anime_girl",
        "eiffel_tower",
        "cinder_votary",
        "riftback_brute",
        "iron_regent",
    ] {
        fs::copy(
            format!("web/demos/{demo}.txt"),
            format!("{DIST}/models/{demo}.txt"),
        )?;
    }
    fs::create_dir_all(format!("{DIST}/outfits"))?;
    // Human bodies and outfit textures are runtime assets for `body.human` and imported hair.
    fs::copy("assets/human/human.pack", format!("{DIST}/human.pack"))?;
    fs::copy(
        "assets/outfits/long01_diffuse.png",
        format!("{DIST}/outfits/long01_diffuse.png"),
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
    crate::texture_assets::copy_assets()?;
    // The editing panels are a folder of small modules; every file in it ships.
    fs::create_dir_all(format!("{DIST}/panels"))?;
    for entry in fs::read_dir("web/panels")? {
        let path = entry?.path();
        if let Some(name) = path.file_name() {
            fs::copy(&path, Path::new(DIST).join("panels").join(name))?;
        }
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
    run_in(
        "..",
        "bun",
        &[
            "build",
            "src/engine/texture/preview.ts",
            "--outfile",
            &format!("engine/{DIST}/texture-preview.js"),
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
    test_filtered(profile, &[])
}

/// Builds and runs all scripts, or only scripts matching requested names.
pub(crate) fn test_filtered(profile: Profile, names: &[String]) -> Result<(), Failure> {
    prepare_browser(profile)?;
    let (jobs, mut timed) = browser_jobs()?;
    let jobs = filter_jobs(jobs, names);
    if !names.is_empty() {
        timed.retain(|script| names.iter().any(|name| script.contains(name)));
    }
    if !names.is_empty() && jobs.is_empty() && timed.is_empty() {
        return Err(format!("no browser scripts matched: {}", names.join(", ")).into());
    }
    if !names.is_empty() {
        println!("-- web-test: filtered browser run; other scripts skipped");
    }
    pool::run_all(jobs, pool::workers()).map_err(|error| rerun_error(error, names))?;
    pool::run_all(
        timed
            .into_iter()
            .map(|script| pool::Job::new(&script, "web"))
            .collect(),
        1,
    )
    .map_err(|error| rerun_error(error, names))
}

fn prepare_browser(profile: Profile) -> Result<(), Failure> {
    build(profile)?;
    if !Path::new("web/node_modules/playwright").exists() {
        run_in("web", "bun", &["install", "--frozen-lockfile"])?;
    }
    run_in("..", "bun", &["run", "typecheck"])?;
    run_in("..", "bun", &["scripts/build-engine.ts", "--copy-only"])?;
    Ok(())
}

fn browser_jobs() -> Result<(Vec<pool::Job>, Vec<String>), Failure> {
    println!("-- web-test: playwright (headless Chromium)");
    let (timed, independent): (Vec<_>, Vec<_>) =
        pool::chain_scripts()?.into_iter().partition(|script| {
            pool::TIMING_SENSITIVE
                .iter()
                .any(|name| script.contains(name))
        });
    // Longest first, so the slow gallery shards start at once and the pool stays busy.
    let mut jobs = Vec::new();
    for shard in 0..GALLERY_SHARDS {
        let mut job = pool::Job::new("web/tests/gallery.mjs", ".");
        job.env = vec![
            ("GALLERY_SHARD", format!("{shard}/{GALLERY_SHARDS}")),
            ("GALLERY_PORT", (5189 + shard).to_string()),
        ];
        jobs.push(job);
    }
    for script in [
        "instances",
        "panels",
        "evaluation",
        "g9-study-room",
        "g9-material-gallery",
        "g9-environment",
        "g9-grading",
        "g10-render-aa",
        "g9-chamfer",
        "g9-kits",
        "g10-bevel",
        "g9-aqueduct-depth",
        "g9-review",
        "mcp",
        "panels-groups",
        "reflection",
        "editor",
        "sockets",
        "panels-graph",
        "v3-transform-commands",
        "v3-gizmo",
        "v3-parameter-handles",
        "v3-profile-points",
        "v3-curve-points",
        "v3-radius-handle",
        "v3-drag-guards",
        "v3-scope",
        "v3-frames",
    ] {
        jobs.push(pool::Job::new(&format!("web/tests/{script}.mjs"), "."));
    }
    // The package's own chain (minus the frame-pacing tests) runs in `web/`, as `bun run test`
    // does.
    jobs.extend(
        independent
            .iter()
            .map(|script| pool::Job::new(script, "web")),
    );
    Ok((jobs, timed))
}

fn filter_jobs(mut jobs: Vec<pool::Job>, names: &[String]) -> Vec<pool::Job> {
    if names.is_empty() {
        return jobs;
    }
    jobs.retain(|job| names.iter().any(|name| job.script.contains(name)));
    jobs
}

fn rerun_error(error: Box<dyn std::error::Error>, names: &[String]) -> Box<dyn std::error::Error> {
    if names.is_empty() {
        error
    } else {
        format!(
            "{error}; rerun with `cargo xtask web-test dev {}`",
            names.join(" ")
        )
        .into()
    }
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
    let started = std::time::Instant::now();
    let status = Command::new(program).args(args).status()?;
    println!(
        "   timing: {program} {} {:.0}s",
        args.join(" "),
        started.elapsed().as_secs_f64()
    );
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
