#!/usr/bin/env bash
# Builds Manifold (CMake + wasi-sdk), then the Rust crate that links it.
set -euo pipefail
cd "$(dirname "$0")"
: "${WASI_SDK:=$HOME/.local/share/toolchains/wasi-sdk-34.0-arm64-macos}"
export WASI_SDK
if [ ! -f build/manifold/lib/libmanifold.a ]; then
  cmake -S third_party/manifold -B build/manifold-cmake -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$WASI_SDK/share/cmake/wasi-sdk-p1.cmake" -DWASI_SDK_PREFIX="$WASI_SDK" \
    -DCMAKE_BUILD_TYPE=MinSizeRel -DBUILD_SHARED_LIBS=OFF -DMANIFOLD_TEST=OFF -DMANIFOLD_PAR=OFF \
    -DMANIFOLD_CBIND=ON -DMANIFOLD_CROSS_SECTION=ON -DMANIFOLD_USE_BUILTIN_CLIPPER2=ON \
    -DMANIFOLD_NO_IOSTREAM=ON -DCMAKE_CXX_FLAGS="-fno-exceptions -include iterator -include cstdlib" -DCMAKE_INSTALL_PREFIX="$PWD/build/manifold"
  cmake --build build/manifold-cmake
  cmake --install build/manifold-cmake
fi
(cd engine && cargo build --release --target wasm32-wasip1)
mkdir -p web
cp engine/target/wasm32-wasip1/release/hybrid_engine.wasm web/engine.wasm
"$WASI_SDK/bin/wasm-opt" -Oz web/engine.wasm -o web/engine.opt.wasm 2>/dev/null || wasm-opt -Oz web/engine.wasm -o web/engine.opt.wasm
ls -l web/*.wasm
