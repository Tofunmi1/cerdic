//! C++ gates: formatting, sanitized native tests, and clang-tidy against the wasm sysroot.
#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::path::PathBuf;
use std::process::Command;

type Failure = Box<dyn std::error::Error>;

const SOURCES: &[&str] = &[
    "cpp/src/kernel.cpp",
    "cpp/src/profiles.cpp",
    "cpp/src/profile_validation.hpp",
    "cpp/src/solid_handle.hpp",
    "cpp/src/extrusion.hpp",
    "cpp/tests/kernel_test.cpp",
    "cpp/include/ve_kernel.h",
];
const SAN_DIR: &str = "target/cpp-san";
const WASM_DIR: &str = "target/cpp-wasm";

/// The wasi-sdk root: `$WASI_SDK`, else the conventional install location.
pub(crate) fn wasi_sdk() -> Option<PathBuf> {
    let from_env = std::env::var_os("WASI_SDK").map(PathBuf::from);
    let default = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/share/toolchains/wasi-sdk-34.0-arm64-macos"));
    from_env
        .or(default)
        .filter(|p| p.join("bin/clang-tidy").exists())
}

pub(crate) fn run() -> Result<(), Failure> {
    println!("-- cpp: format, sanitized native tests, clang-tidy (wasm sysroot)");
    format_check()?;
    sanitized_tests()?;
    match wasi_sdk() {
        Some(sdk) => tidy(&sdk)?,
        None => println!("-- cpp: clang-tidy skipped (set WASI_SDK to a wasi-sdk install)"),
    }
    Ok(())
}

fn format_check() -> Result<(), Failure> {
    let mut args = vec!["--dry-run", "-Werror", "--style=file:cpp/.clang-format"];
    args.extend(SOURCES);
    run_cmd("clang-format", &args)
}

fn sanitized_tests() -> Result<(), Failure> {
    run_cmd(
        "cmake",
        &[
            "-S",
            "cpp",
            "-B",
            SAN_DIR,
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
            "-DVE_BUILD_TESTS=ON",
            "-DVE_SANITIZE=ON",
        ],
    )?;
    run_cmd("cmake", &["--build", SAN_DIR])?;
    run_cmd(&format!("{SAN_DIR}/kernel_test"), &[])
}

fn tidy(sdk: &std::path::Path) -> Result<(), Failure> {
    let toolchain = sdk.join("share/cmake/wasi-sdk-p1.cmake");
    let toolchain_arg = format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain.display());
    let sdk_arg = format!("-DWASI_SDK_PREFIX={}", sdk.display());
    run_cmd(
        "cmake",
        &[
            "-S",
            "cpp",
            "-B",
            WASM_DIR,
            "-G",
            "Ninja",
            &toolchain_arg,
            &sdk_arg,
            "-DCMAKE_BUILD_TYPE=MinSizeRel",
            "-DVE_BUILD_TESTS=ON",
        ],
    )?;
    let tidy = sdk.join("bin/clang-tidy");
    let tidy = tidy.to_string_lossy();
    run_cmd(
        &tidy,
        &[
            "--config-file=cpp/.clang-tidy",
            "-p",
            WASM_DIR,
            "cpp/src/kernel.cpp",
            "cpp/src/profiles.cpp",
            "cpp/tests/kernel_test.cpp",
        ],
    )
}

fn run_cmd(program: &str, args: &[&str]) -> Result<(), Failure> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed").into())
    }
}
