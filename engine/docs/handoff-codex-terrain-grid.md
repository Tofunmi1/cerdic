# Handoff to Codex: `ve-terrain`, heightfield grids, generators and masks

> **Crate layout (owner decision 2026-10-21: too many crates).** The crate was merged: this is now the module `terrain` of `ve-world` (`crates/ve-world/src/terrain/`, tests `crates/ve-world/tests/terrain.rs`). Read `ve-terrain` as `ve_world::terrain` and `-p ve-terrain` as `-p ve-world`. First pass is done (29 tests pass); the open work is the follow-up section at the end of this file.

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no `Co-Authored-By` or tool attribution, file changes through the edit tools only). **Never run `cargo fmt`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use `CARGO_TARGET_DIR=target-codex-terrain` for your cargo runs. Edit only the new crate `crates/ve-terrain` (and add it to the workspace `Cargo.toml` members). Plan: `docs/plans/world-generators.md`.

## State of the crate (read first)
The crate **already exists** (`crates/ve-terrain`, in the workspace, ADR `docs/adr/0056-world-tiles-channels-and-height-grids.md`). `grid.rs` (the `HeightGrid` implementation), `error.rs`, `lib.rs` and the tests `tests/terrain.rs` are written and must not be edited. The bodies in `src/generate.rs`, `src/mask.rs` and `src/mesh.rs` are stubs that return `Err(TerrainError::NotImplemented { .. })` or do nothing: **replace them** (split files if one passes 400 lines; add new modules to `lib.rs` only for new files). When the last stub is gone, remove the `NotImplemented` variant from `error.rs` and say so in the Status. 5 grid tests pass now; 24 fail until you finish. `Aspect` is defined by the test: the downhill facing direction with north (+z) 0.0, east (+x) 0.25, south 0.5, west 0.75, flat cells 0.
Run: `CARGO_TARGET_DIR=target-codex-terrain cargo test -p ve-terrain`.

## Why
Terrain must stop being a solid slab made of one noise function. The engine needs a heightfield **layer**: a grid of heights plus named mask layers (slope, curvature, height, flow, wetness) that scatter, vegetation and materials can read. This crate is pure arithmetic with no graph or Bevy dependency; the owner-side session wires nodes to it afterwards.

## Rules for the crate
- Core crate: no Bevy, wgpu, winit or web dependencies. Allowed dependencies: `thiserror`, `serde` (derive) only.
- Deterministic and platform-identical: use `+ - * /`, `sqrt`, `floor`, integer hashing; no `sin`/`cos`/`exp`/`powf` from std where a result must match WASM bit for bit. If you need a smooth function use polynomials (smoothstep). Randomness from the SplitMix64 stream in `crates/ve-modeling/src/instances/scatter.rs` (copy the algorithm, do not depend on `ve-modeling`).
- Every long loop checks a cancel callback: `&dyn Fn() -> bool`; on cancel return `Err(TerrainError::Cancelled)`.
- Sizes are bounded: a TILE is at most 2048 cells per side (`MAX_SIDE`); the world is many tiles (the owner-side session handles the tile graph); larger requests return `TerrainError::TooLarge { side, max }` with a message that names the fix.

## API to build (signatures are fixed; tests will import these)
```rust
pub const MAX_SIDE: usize = 2048;

/// A tile of the world. Cell (0,0) sits at world position `origin`; cell (i,j) at `origin + (i,j) * cell`. Everything that
/// depends on position uses WORLD coordinates, so two tiles that share an edge agree on it and a tile built alone equals the same
/// area cut from a larger grid (the "tile invariance" tests).
pub struct HeightGrid { /* private */ }
impl HeightGrid {
    pub fn new(width: usize, depth: usize, cell: f32, origin: [f32; 2]) -> Result<Self, TerrainError>; // heights 0.0
    pub fn width(&self) -> usize; pub fn depth(&self) -> usize; pub fn cell(&self) -> f32; pub fn origin(&self) -> [f32; 2];
    pub fn heights(&self) -> &[f32];                    // row-major, index = z * width + x
    pub fn heights_mut(&mut self) -> &mut [f32];
    pub fn sample(&self, x: f32, z: f32) -> f32;        // bilinear at WORLD x,z in metres, clamps at the edge
    pub fn normal(&self, x: f32, z: f32) -> [f32; 3];   // unit, from central differences at WORLD x,z
    pub fn sub_grid(&self, x0: usize, z0: usize, width: usize, depth: usize) -> Result<HeightGrid, TerrainError>; // copy of a window with its layers; origin moves accordingly
    pub fn paste(&mut self, other: &HeightGrid) -> Result<(), TerrainError>; // copies the overlap of `other` (matched by world position, same cell size) into self, layers by name
    pub fn layer(&self, name: &str) -> Option<&[f32]>;
    pub fn set_layer(&mut self, name: &str, values: Vec<f32>) -> Result<(), TerrainError>; // length must equal width*depth
    pub fn layer_names(&self) -> Vec<String>;           // sorted
}

pub struct Noise { pub seed: u32, pub scale: f32, pub octaves: u32, pub roughness: f32, pub lacunarity: f32, pub ridged: bool, pub warp: f32 }
pub fn generate(grid: &mut HeightGrid, noise: &Noise, height: f32, cancel: &dyn Fn() -> bool) -> Result<(), TerrainError>;
pub fn terrace(grid: &mut HeightGrid, steps: u32, sharpness: f32);   // snaps heights to steps, 0 = off
pub fn smooth(grid: &mut HeightGrid, passes: u32);                   // 3x3 box blur, edges clamp
pub fn edge_falloff(grid: &mut HeightGrid, centre: [f32; 2], radius: f32, reach: f32); // island mask in WORLD coordinates: full height inside radius*(1-reach), smoothstep to 0 at radius (not tied to the tile border)
pub fn carve_path(grid: &mut HeightGrid, points: &[[f32; 2]], width: f32, depth: f32, flatten: bool); // WORLD-coordinate polyline; roads and trenches; only cells inside the grid change, a path crossing tiles matches across them
pub fn stamp(grid: &mut HeightGrid, centre: [f32; 2], radius: f32, height: f32, falloff: StampShape); // add a hill, crater or mesa; StampShape = Smooth, Cone, Plateau, Crater
pub fn domain_warp(grid: &mut HeightGrid, seed: u32, scale: f32, strength: f32); // resample heights through a noise-displaced lookup in WORLD coordinates (needs a one-cell-wide margin policy: document it)

pub enum Mask { Slope, Curvature, Height, Aspect, Occlusion }
pub fn compute_mask(grid: &HeightGrid, mask: Mask, cancel: &dyn Fn() -> bool) -> Result<Vec<f32>, TerrainError>; // each in 0..=1, see below
pub fn remap(values: &mut [f32], lo: f32, hi: f32, smooth: bool);    // (v-lo)/(hi-lo) clamped to 0..=1, optional smoothstep
pub fn combine(a: &[f32], b: &[f32], op: Combine) -> Vec<f32>;       // Multiply, Add, Max, Min, Subtract (clamped 0..=1)

pub struct MeshOut { pub positions: Vec<[f32; 3]>, pub normals: Vec<[f32; 3]>, pub uvs: Vec<[f32; 2]>, pub indices: Vec<u32> }
pub fn to_mesh(grid: &HeightGrid, stride: usize) -> MeshOut;         // stride 1 = every cell, 2 = every other, ... counter-clockwise seen from +y (the CPU/WebGL2 fallback path; the WebGPU path uses clipmaps, owned by the renderer side)
pub fn to_height_texture(grid: &HeightGrid, min: f32, max: f32) -> Vec<u16>;   // heights quantised to 16 bits over min..=max, for the clipmap vertex texture
pub fn pack_splat(layers: &[&[f32]]) -> Vec<u8>;                    // up to 4 mask layers into RGBA8, weights normalised so they sum to 255 per cell
```
Mask definitions: `Slope` = angle of the normal from vertical divided by 90 degrees (flat 0, vertical 1); `Curvature` = Laplacian of heights scaled by cell size, mapped so flat is 0.5, ridges above, hollows below; `Height` = (h - min) / (max - min) (0 if flat); `Aspect` = direction the slope faces, 0..1 around the compass with north +z; `Occlusion` = fraction of 8 directions (up to 32 cells out) that rise above the horizon of 20 degrees. Use only `+ - * /`, `sqrt` and a polynomial `atan` approximation (document its maximum error in a comment and test it).

## Tile invariance (the property AAA worlds need)
`generate` evaluates noise at world coordinates only (cell position = origin + index * cell), with the seed and no per-grid state. Required properties, each with a test: (1) a 129-cell tile at origin (0,0) and a 129-cell tile at origin (128*cell, 0) agree exactly on the shared column; (2) generating a 257-cell grid and cutting a window with `sub_grid` equals generating that window directly, bit for bit; (3) `compute_mask` over a tile that carries a one-cell halo equals the same cells of a mask over the whole grid, bit for bit (document that masks need a 1-cell halo, 32 for `Occlusion`); (4) `carve_path` run on two adjacent tiles then pasted equals one run on the joined grid.

## Tests
Written by the owner-side session before this job is launched, in `crates/ve-terrain/tests/`. Do not weaken them. If a test premise is wrong, say so in the Status and explain; do not change it quietly. Add unit tests of your own for anything the written tests do not cover (mask definitions, cancel, too-large).

## Acceptance
`cargo test -p ve-terrain`, `cargo clippy -p ve-terrain --all-targets -- -D warnings`, `cargo check -p ve-terrain --target wasm32-unknown-unknown` all clean; no file over 400 lines; native and wasm give the same hash of `generate` output for three seeds (add a test that prints the FNV hash and assert the hard-coded value you measured natively; the owner will compare it under WASI).

## Follow-up: review of the first pass (do this now)
The first pass passes all 29 tests but clippy fails and nothing was formatted-for-reading or hash-checked. Another agent edits the sibling module `erosion` in the same crate at the same time: touch only `crates/ve-world/src/terrain/` (not `grid.rs` or `error.rs` unless a lint forces it, and then say so); if the crate fails to compile because of the sibling, wait a minute and retry.
1. **Readable, formatted code.** One statement per line, small helpers, doc comments on their own lines. Run `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on every file you touch. `cargo clippy -p ve-world --all-targets -- -D warnings` clean for the `terrain` module, no `#[allow]`, well-reasoned `#[expect]` only, restructure to avoid casts where you can (index helpers).
2. **Noise quality.** If `generate` uses value noise, change it to gradient (Perlin-style) noise with a hashed gradient set and a quintic fade `6t^5 - 15t^4 + 10t^3`: value noise shows axis-aligned grid artefacts that read as fake terrain. Keep the output in `0..=height`, keep world-coordinate evaluation and every tile-invariance property; the ridged variant and domain warp stay. No std trig: the gradients come from integer hashing and a fixed table of unit vectors.
3. **Determinism evidence.** Add a test `generate_hash_is_stable` that computes the FNV-1a hash (64-bit, over the little-endian bytes of the heights) of `generate` for three seeds on a 129 by 129 grid and asserts the hard-coded values you measured natively (write them into the test, and into the Status, so the owner can compare against the WASI run). Add the same for `compute_mask` (slope and occlusion) on one seed. Put them in a new file `crates/ve-world/tests/terrain_hashes.rs`, not in the existing tests.
4. **Performance.** Report release timings in the Status for: `generate` of a 1025 by 1025 grid, all five masks on 1025 by 1025, `to_mesh` stride 1 of 1025 by 1025. Occlusion must not exceed 2 s; if it does, restructure (for example walk each direction incrementally) rather than lower the quality.
5. **Honesty in the Status:** list the tests run with results and anything you could not meet. Do not edit the existing tests.

## Status (Codex: fill in when you finish, then stop)
- State: First-pass terrain implementation reviewed; generation now uses deterministic hashed-gradient noise with quintic fade. The terrain-specific integration tests pass.
- What changed: Replaced value noise with a fixed table of eight unit gradients and quintic interpolation, retaining ridged noise, world-coordinate evaluation, and domain warp. Added native FNV-1a stability tests in `src/terrain/review_tests.rs` for three generation seeds and slope/occlusion masks. Kept the tests under `src/terrain/` per the requested file scope; they are not in the follow-up's suggested `tests/terrain_hashes.rs` path. Existing mask halo policy and domain-warp edge-clamp policy remain documented. No existing tests were edited.
- Hashes measured natively: generation seeds `[1, 42, 3735928559]` => `[10564738071198853707, 3173558287248542844, 10110277755925135599]`; slope => `1501502308323042419`; occlusion => `7176336250419348776` (seed 42 for both masks). WASI comparison was not run.
- Release timings (1025 by 1025, this machine): generate `65.689041 ms`; all five masks `377.816833 ms` (occlusion `361.367417 ms`); stride-1 mesh `25.620208 ms`. Occlusion is under the 2 s target.
- Commands run and results: `CARGO_TARGET_DIR=target-codex-terrain cargo test -p ve-world` first failed because the sibling `erosion/stream.rs` was missing. After waiting one minute, it compiled and ran, but two unrelated erosion integration tests failed (`the_river_mask_ramps_with_area`, `refined_tiles_agree_on_their_shared_edge_and_with_the_whole_world`). `CARGO_TARGET_DIR=target-codex-terrain cargo test -p ve-world --test terrain` passed (29 tests). `CARGO_TARGET_DIR=target-codex-terrain cargo test -p ve-world --lib terrain::review_tests` passed (3 tests). Release measurements were collected with `cargo test -p ve-world --lib terrain::review_tests --release -- --nocapture`. Rustfmt was run on the changed terrain Rust files. `CARGO_TARGET_DIR=target-codex-terrain cargo clippy -p ve-world --all-targets -- -D warnings` failed: the workspace crate has extensive terrain cast/readability lints plus sibling erosion lints; no lint suppressions were added. The old `-p ve-terrain` commands and obsolete-variant note do not apply to the merged crate.
- Could not verify / open questions: The follow-up clippy requirement remains unmet, including terrain lints and the `mod.rs` layout lint. `cargo xtask quick` and `cargo xtask rules` were not run. Hashes have not been compared under WASI. The full crate test remains red on the two erosion tests above; strict gate pending.
