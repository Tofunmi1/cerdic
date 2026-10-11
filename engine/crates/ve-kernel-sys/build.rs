//! Builds the C++ kernel (`cpp/`, with vendored Manifold and `meshoptimizer`) via `CMake` and links it.
//!
//! One `CMake` project is the source of truth for flags on every target. `Wasm` targets use the
//! wasi-sdk toolchain: the C++ is compiled for `wasm32-wasip1` and linked into the Rust module, which
//! leaves only a handful of wasi imports for the host to answer.

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let engine = manifest.join("../..").canonicalize()?;
    let out = PathBuf::from(env::var("OUT_DIR")?);
    let target = env::var("TARGET")?;
    let is_wasm = target.starts_with("wasm32");
    let build_dir = out.join("cmake");

    let source = engine.join("cpp");
    let mut configure = vec![
        "-S".to_owned(),
        source.display().to_string(),
        "-B".to_owned(),
        build_dir.display().to_string(),
        "-G".to_owned(),
        "Ninja".to_owned(),
        format!(
            "-DCMAKE_BUILD_TYPE={}",
            if is_wasm { "MinSizeRel" } else { "Release" }
        ),
    ];
    let wasi = if is_wasm { Some(wasi_sdk()?) } else { None };
    if let Some(sdk) = &wasi {
        configure.push(format!(
            "-DCMAKE_TOOLCHAIN_FILE={}",
            sdk.join("share/cmake/wasi-sdk-p1.cmake").display()
        ));
        configure.push(format!("-DWASI_SDK_PREFIX={}", sdk.display()));
    }
    run("cmake", &configure)?;
    run(
        "cmake",
        &[
            "--build".to_owned(),
            build_dir.display().to_string(),
            "--target".to_owned(),
            "ve_kernel".to_owned(),
        ],
    )?;

    println!(
        "cargo:rustc-link-search=native={}",
        build_dir.join("lib").display()
    );
    for lib in ["ve_kernel", "manifold", "Clipper2", "ve_meshopt"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    match &wasi {
        Some(sdk) => {
            let sysroot = sdk.join("share/wasi-sysroot/lib/wasm32-wasip1");
            println!(
                "cargo:rustc-link-search=native={}",
                sysroot.join("noeh").display()
            );
            println!("cargo:rustc-link-lib=static=c++");
            println!("cargo:rustc-link-lib=static=c++abi");
            if target == "wasm32-unknown-unknown" {
                // No bundled libc on this target: borrow the wasi one the C++ was compiled against.
                println!("cargo:rustc-link-search=native={}", sysroot.display());
                println!("cargo:rustc-link-lib=static=c");
            }
        }
        None if target.contains("apple") => println!("cargo:rustc-link-lib=dylib=c++"),
        None => println!("cargo:rustc-link-lib=dylib=stdc++"),
    }

    for path in [
        "cpp",
        "third_party/manifold/src",
        "third_party/manifold/include",
        "third_party/meshoptimizer/src",
    ] {
        println!("cargo:rerun-if-changed={}", engine.join(path).display());
    }
    println!("cargo:rerun-if-env-changed=WASI_SDK");
    Ok(())
}

/// `$WASI_SDK`, else the conventional install location.
fn wasi_sdk() -> Result<PathBuf, Box<dyn Error>> {
    let from_env = env::var_os("WASI_SDK").map(PathBuf::from);
    let default = env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/share/toolchains/wasi-sdk-34.0-arm64-macos"));
    let sdk = from_env
        .or(default)
        .ok_or("set WASI_SDK to a wasi-sdk install")?;
    if sdk.join("share/cmake/wasi-sdk-p1.cmake").exists() {
        Ok(sdk)
    } else {
        Err(format!(
            "{} is not a wasi-sdk install; set WASI_SDK",
            Path::new(&sdk).display()
        )
        .into())
    }
}

fn run(program: &str, args: &[String]) -> Result<(), Box<dyn Error>> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed ({status})").into())
    }
}
