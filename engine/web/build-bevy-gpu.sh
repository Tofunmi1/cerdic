#!/bin/sh
# Rebuilds web/dist-bevy-gpu (the Bevy WebGPU bundle) from the current sources: runtime wasm, evaluator wasm, their glue, and the page bridge.
# The glue's wasi imports are pointed at the local shim with a bun script (no sed), the same change `cargo xtask web` makes.
set -e
cd "$(dirname "$0")/.."
SDK="${WASI_SDK:-$HOME/.local/share/toolchains/wasi-sdk-34.0-arm64-macos}"
export CC_wasm32_unknown_unknown="$SDK/bin/clang" AR_wasm32_unknown_unknown="$SDK/bin/llvm-ar"
DIST=web/dist-bevy-gpu
cargo build -q -p ve-runtime --target wasm32-unknown-unknown --features bevy-webgpu --profile web-dev
cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --profile web-dev
target/tools/bin/wasm-bindgen --target web --out-dir $DIST --no-typescript target/wasm32-unknown-unknown/web-dev/ve_runtime.wasm
target/tools/bin/wasm-bindgen --target web --out-dir $DIST --no-typescript target/wasm32-unknown-unknown/web-dev/ve_evaluator.wasm
bun -e "for (const f of ['ve_runtime.js','ve_evaluator.js']) { const p='$DIST/'+f; await Bun.write(p, (await Bun.file(p).text()).replaceAll('from \"wasi_snapshot_preview1\"','from \"./wasi_snapshot_preview1.js\"')); }"
(cd .. && bun build src/engine/viewer-bridge.ts --outfile engine/$DIST/viewer-bridge.js --target browser --format esm)
cp web/eval-worker.js $DIST/eval-worker.js
mkdir -p $DIST/worlds
cp web/demos/modern_room.txt $DIST/worlds/modern_room.txt
echo built $DIST
