use std::{env, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let sdk = PathBuf::from(env::var("WASI_SDK").expect("set WASI_SDK"));
    let meshopt = root.join("third_party/meshoptimizer/src");

    // meshoptimizer + our own C++ compiled with wasi-sdk clang++ into this crate.
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .compiler(sdk.join("bin/clang++"))
        .archiver(sdk.join("bin/llvm-ar"))
        .flag("-fno-exceptions")
        .flag("-std=c++17")
        .opt_level_str("s")
        .file(root.join("cpp/geomproc.cpp"));
    for entry in std::fs::read_dir(&meshopt).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "cpp") {
            build.file(path);
        }
    }
    build.compile("cxx_geom");

    // Manifold is built separately with CMake (see build.sh).
    let manifold = root.join("build/manifold");
    println!("cargo:rustc-link-search=native={}", manifold.join("lib").display());
    for lib in ["manifoldc", "manifold", "Clipper2"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    let sysroot = sdk.join("share/wasi-sysroot/lib/wasm32-wasip1/noeh");
    println!("cargo:rustc-link-search=native={}", sysroot.display());
    println!("cargo:rustc-link-lib=static=c++");
    println!("cargo:rustc-link-lib=static=c++abi");
    println!("cargo:rerun-if-changed=../cpp");
}
