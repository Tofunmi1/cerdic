# Handoff: make the custom materials cross-fade with `VisibilityRange` (ve-bevy materials only)

Owner: Claude manages; Codex implements. **Files you may edit: `engine/crates/ve-bevy/src/{toon.rs,terrain.rs,water.rs}` and `engine/crates/ve-bevy/src/shaders/*` only.** Two other helpers work in `ve-bevy/src/stats.rs` (new), `ve-bevy/src/lib.rs`,
`ve-runtime` and `engine/web/viewer.js` at the same time: do not touch those. If the crate fails to build because of THEIR half-written files, wait a minute and retry; do not fix their files. Rules: never run `git checkout`, `git clean`, `git stash`
or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; own `CARGO_TARGET_DIR` (for example `/tmp/codex-target-matfade`); do not commit or stage; edit with your edit tools, not scripts.
The sandbox cannot run the browser; Claude does that.

## Why

Sets of placed copies can be drawn only inside a camera-distance band with a cross-fade (`scatter.place` show_from / show_to / show_fade, which `ve-bevy/src/sets.rs` turns into a Bevy `VisibilityRange` with margins). Bevy's own `StandardMaterial` shader (`pbr.wesl`) calls
`visibility_range_dither` so the margin becomes a dithered cross-fade. Our four custom materials, `ToonMaterial`, `DetailMaterial` (both in `toon.rs`), `TerrainMaterial` (`terrain.rs`) and `WaterMaterial` (`water.rs`), have their own fragment shaders that do not call it, so they pop at the middle of the margin.
The forest's trees use these (check which material a `flora.tree` set gets in `map.rs`/`toon.rs`; report it in the Status).

## What to build

For each custom material's fragment shader (WGSL/WESL under `ve-bevy/src/shaders` or inline in the `.rs`): include the same dither call Bevy's `pbr.wesl` makes. Find it with
`grep -rn "visibility_range_dither" engine/third_party/bevy/crates` (the function lives in `bevy_pbr`'s mesh-view or visibility-range shader module and needs the mesh's `lod_index`/`visibility_range` data that the standard mesh vertex output already carries:
read `pbr.wesl` and `bevy_pbr/src/render/mesh.wgsl` or `.wesl` to see how `lod_index` flows from the vertex output to the fragment). Do it for the forward pass and, where our material has its own prepass/depth/shadow fragment shader, the same call there so the dither is consistent (a dithered
hole in the colour pass but a solid shadow is acceptable only if the prepass shader cannot discard; say so in the Status).

Keep every material's look unchanged when no visibility range is set (the dither function must be a no-op then; confirm from the Bevy source and say so).

## Tests

Shader code cannot be unit-tested without a GPU here, so the contract is: `cargo test -p ve-bevy` passes (all existing tests, none ignored), `cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` builds, and if the crate has a shader
validation test (look for naga validation in tests or `build.rs`), it passes with your changes. Add a naga validation test of each changed shader if the crate already depends on naga (do not add a dependency otherwise; say so).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked, and what exactly blocked you: the requested `CARGO_TARGET_DIR=/tmp/codex-target-` stayed locked by another Cargo process for more than two minutes. The WASM build was retried and remained blocked on that lock; it was stopped without changing the target directory.
- Which material the forest trees use: foliage uses `StandardMaterial`; bark uses `DetailMaterial` on the close LOD with procedural relief and `StandardMaterial` on simpler LODs.
- Which shaders now dither (forward, prepass, shadow) and which could not: `toon.wesl`, `detail.wesl`, `terrain.wesl`, and `water.wesl` call Bevy's dither helper in the forward fragment pass. They define no custom prepass, depth, or shadow fragment shader: the `ExtendedMaterial` fallback uses `StandardMaterial`'s prepass shader, which calls the helper; Bevy has no corresponding visibility dither call in its shadow shader, so shadow maps remain solid through the fade. The helper returns immediately when `dither == 0`, preserving the appearance outside a fade range.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target- cargo test -p ve-bevy` passed: 24 passed, 0 failed, 0 ignored; doc tests 0 passed, 0 failed. Its first attempt encountered compile errors in concurrent `stats.rs`; after waiting per instructions, the retry passed. `CARGO_TARGET_DIR=/tmp/codex-target- cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` remained blocked waiting for the target-directory lock, including on retry. No crate-local naga shader validation test or `build.rs` was present.
- Files changed: `engine/crates/ve-bevy/src/shaders/{toon,detail,terrain,water}.wesl`; this Status section in `engine/docs/handoff-codex-material-fade.md`.
