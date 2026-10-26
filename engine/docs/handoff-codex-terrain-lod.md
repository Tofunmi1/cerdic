# Handoff: terrain tile selection (quadtree LOD) and tile streaming diff (ADR 0070 port 6, part 1)

Reference (read first, read-only, MIT): `engine/third_party/redgpu/src/landscape/core/spatial/LandscapeTileStreamer.ts` (40 KB), `LandscapeSpatialGrid.ts`, `LandscapeGPUCuller.ts` and `core/shader/landscapeCull.wgsl`. Also our terrain code (`crates/ve-world/src/terrain.rs`, `terrain/`) to see how a `terrain.tile` and its height grid are stored. The idea (CDLOD): a quadtree over the world, a tile is split into four while the camera is within a range of it, so tiles near the camera are small and far tiles big; a tile near the end of its range reports a morph factor so the shader can blend its heights toward the coarser parent and avoid popping. Write our own version; keep the upstream notice in a comment where a formula is adapted.

## Test (already written, do not weaken)
`crates/ve-world/tests/terrain_lod.rs` (10 tests). If one looks logically wrong, say so in Status with the numbers; do not change it.

## Build: `crates/ve-world/src/terrain_lod.rs` (a module of `ve-world`, exported as `ve_world::terrain_lod`; split into `terrain_lod/` files under 400 lines)
```rust
pub struct LodOptions { pub tile_size: f64 /*edge of a level-0 tile, m*/, pub levels: usize, pub base_range: f64 /*level 0 range, m*/, pub range_ratio: f64 /*range of level l = base_range * range_ratio^l*/, pub morph: f64 /*the last fraction of a range in which a tile morphs, 0..1*/ }
pub struct SelectedTile { pub level: usize, pub x: i64, pub z: i64, pub min: [f64; 2], pub size: f64, pub morph: f64 }  // size = tile_size * 2^level, min = (x, z) * size
pub struct TileDiff { pub load: Vec<SelectedTile>, pub unload: Vec<SelectedTile> }
pub fn select_tiles(world_min: [f64;2], world_max: [f64;2], camera: [f64;3], options: &LodOptions, height_range: [f64;2]) -> Vec<SelectedTile>
pub fn diff_tiles(previous: &[SelectedTile], next: &[SelectedTile]) -> TileDiff   // keyed by (level, x, z)
```
- Roots: the world rectangle is covered by tiles of the top level (`levels - 1`) laid out on the same grid (x, z integer multiples of the top size); only roots that overlap the world are used. A tile is split into its four children while the camera's distance to the tile's 3D box (the tile square times `height_range`) is less than the tile's level range, down to level 0. The selected tiles cover the world exactly once, in a deterministic order (by level, then z, then x).
- **Balancing**: neighbouring selected tiles may differ by at most one level (the test checks this). Plain range-splitting does not guarantee it with every ratio; add an explicit pass (split a tile whose neighbour is two or more levels finer) and keep coverage exact.
- `morph` of a tile is 0 when the camera distance is below `(1 - morph) * range`, rises linearly to 1 at `range`, clamped to 0..1; the tile that contains the camera has 0.
- Bad input (empty world, NaN camera, tile_size <= 0, levels == 0) returns an empty list. No unwrap/expect/panic, documented, deterministic.

## Part 2 (after part 1 passes): a plan for the renderer
Write `docs/plans/terrain-lod-streaming.md` (short): how `select_tiles` would drive per-tile meshes of a `terrain.tile` (tile mesh grid resolution, one shared index buffer, height from the tile's height grid, morph in the vertex shader like `water_waves.wgsl`/`wind.wgsl` are included into the forward shader), the streaming diff driving load and unload of baked height/normal tiles, the WebGPU-only compute culling of tiles (RedGPU `landscapeCull.wgsl`) versus the CPU selection that WebGL2 keeps, and the open questions. No renderer code in this job.

## Rules
- AGENTS.md rules. No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-world --test terrain_lod` and the rest of `cargo test -p ve-world --lib --tests` except tests that already failed to compile before (`tests/rooms_layout.rs`: skip it and say so); `cargo check -p ve-runtime --target wasm32-unknown-unknown`.

## Status
Completed part 1, then part 2. Exported `ve_world::terrain_lod` with documented
options, selected tiles, deterministic residency diffs and CPU quadtree selection.
Selection uses camera-to-3D-box distance, overlapping grid-aligned roots, explicit
edge-neighbour balancing, and clamped morph factors. Invalid/unrepresentable inputs
return an empty selection. Boundary tiles retain their full grid square; their
intersections cover an unaligned world rectangle exactly once. The distance
adaptation retains RedGPU's MIT attribution. All new Rust files are under 400 lines.

The supplied `tests/terrain_lod.rs` is unchanged; all 10 tests pass and none appeared
logically wrong. Added five unit tests covering balancing with small range ratios,
negative coordinates, partial boundaries, morph endpoints and narrow bands,
invalid/extreme inputs, coordinate underflow, and residency-key semantics.

Wrote `docs/plans/terrain-lod-streaming.md` after the supplied tests passed: shared
32-quad patch topology, height-grid sampling, parent morph and edge continuity,
height/normal page residency, WebGPU compute visibility, the WebGL2 CPU path, and
open questions. No renderer code was changed.

Checks (all Cargo invocations used `CARGO_TARGET_DIR=/tmp/codex-target-octa`):

- `cargo test -p ve-world --test terrain_lod`: 10 passed.
- `cargo test -p ve-world --lib`: 11 passed, including all five new unit tests.
- All 19 buildable integration targets, selected with explicit `--test` flags:
  194 passed, five existing timing tests ignored.
- `cargo check -p ve-runtime --target wasm32-unknown-unknown`: passed, with existing
  warnings outside this change.
- `cargo clippy -p ve-world --lib`: passed, with existing warnings outside the new
  module. Direct `rustfmt --check` passed for the changed Rust files.

The complete integration build cannot pass because of unrelated existing targets:
`tests/rooms_layout.rs` imports missing `ve_world::rooms` (confirmed before adding
the module, then skipped as instructed). The initial run of all remaining targets
also found `tests/survey.rs` cannot compile: missing `SurveyMode` at line 8 and
`SurveyOptions::mode` at lines 209, 211 and 220. Reproduced those four errors in an
isolated temporary copy without the terrain LOD module export, then ran every
other target successfully. Neither test was edited or weakened.

No `cargo xtask` or `cargo fmt` was run, per this handoff; the strict gate remains
pending. Nothing was staged or committed.
