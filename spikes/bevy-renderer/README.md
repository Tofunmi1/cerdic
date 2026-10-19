# Spike: Bevy 0.20's full renderer in our browser harness

Question: which of Bevy v0.20.0's renderer features (`bevy_pbr`, `bevy_core_pipeline`, `bevy_post_process`, `bevy_anti_alias`, `bevy_light`) run on WebGL2 and
on WebGPU in a browser, so we can decide how far to adopt it (ADR 0001 trimmed it down to the render core). Run 2026-10-19. Throwaway: do not import from engine code.

## Rebuild
1. A scratch directory with a sparse checkout of the Bevy tag (crates only, about 40 MB):
   `git init . && git remote add origin https://github.com/bevyengine/bevy.git && git fetch --depth 1 --filter=blob:none origin tag v0.20.0`, then
   `git sparse-checkout set --no-cone '/Cargo.toml' '/crates/**' '/src/**'` and `git checkout FETCH_HEAD`.
2. Replace the root `Cargo.toml` with `workspace-Cargo.toml` (the original lists examples that are not checked out), copy `rust-toolchain.toml` there, and put `spike/` beside `crates/`.
3. `cargo build -p spike --target wasm32-unknown-unknown` (add `--no-default-features --features webgpu` for the WebGPU build), then
   `wasm-bindgen --target web --no-typescript --out-dir spike/web-<backend>/pkg target/wasm32-unknown-unknown/debug/spike.wasm` with `index.html` copied to `spike/web-<backend>/`.
4. `bun spike/run.mjs <webgl|webgpu> [fx-set ...]` drives every effect in a fresh page; `REAL_GPU=1` uses the full Chromium build on the machine's GPU (the default headless shell is
   SwiftShader, whose WebGPU adapter reports no compute support); `DEBUG=1` prints the console, where Bevy logs which plugins it left out. `bun spike/diff.mjs <backend>` measures how much each
   effect changed the picture against the baseline. Judge an effect by that, not by `verdict` alone.

`results/*.jsonl` hold the two matrices.
