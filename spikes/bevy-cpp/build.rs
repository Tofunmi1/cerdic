// C++ (meshoptimizer + our code + Manifold prebuilt) compiled for wasip1 and linked into a
// wasm32-unknown-unknown Rust crate. Only 4 wasi imports remain (answered by a JS shim).
use std::{env, path::PathBuf};
fn main() {
    let spike = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../hybrid-wasm");
    let sdk = PathBuf::from(env::var("WASI_SDK").expect("set WASI_SDK"));
    let meshopt = spike.join("third_party/meshoptimizer/src");
    let mut build = cc::Build::new();
    build.cpp(true).compiler(sdk.join("bin/clang++")).archiver(sdk.join("bin/llvm-ar"))
        .target("wasm32-wasip1").flag("-fno-exceptions").flag("-std=c++17").opt_level_str("s")
        .file(spike.join("cpp/geomproc.cpp"));
    for e in std::fs::read_dir(&meshopt).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "cpp") { build.file(p); }
    }
    build.compile("cxx_geom");
    let manifold = spike.join("build/manifold");
    println!("cargo:rustc-link-search=native={}", manifold.join("lib").display());
    for lib in ["manifoldc", "manifold", "Clipper2"] { println!("cargo:rustc-link-lib=static={lib}"); }
    let wasi = sdk.join("share/wasi-sysroot/lib/wasm32-wasip1");
    println!("cargo:rustc-link-search=native={}", wasi.join("noeh").display());
    println!("cargo:rustc-link-lib=static=c++");
    println!("cargo:rustc-link-lib=static=c++abi");
    println!("cargo:rustc-link-search=native={}", wasi.display());
    println!("cargo:rustc-link-lib=static=c");
    println!("cargo:rerun-if-changed=../hybrid-wasm/cpp");
}
