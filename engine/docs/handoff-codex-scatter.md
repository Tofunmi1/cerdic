# Handoff to Codex: `ve-scatter`, attribute-carrying point sets and Poisson-disk placement

> **Crate layout (owner decision 2026-10-21: too many crates).** There is no `ve-scatter` crate. This work is the module `scatter` of the single crate `ve-world` (`crates/ve-world/src/scatter/mod.rs` re-exporting the API, tests in `crates/ve-world/tests/scatter.rs`, `pub mod scatter;` added to `crates/ve-world/src/lib.rs`). Wherever this file says "the crate", `ve-scatter` or `-p ve-scatter`, read "module `scatter`", `ve_world::scatter` and `-p ve-world`. Files inside the module import siblings with `super::`; other modules are `crate::terrain`, `crate::erosion`. Do not add a Cargo dependency for them.

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no `Co-Authored-By` or tool attribution, file changes through the edit tools only). **Never run `cargo fmt`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use `CARGO_TARGET_DIR=target-codex-scatter` for your cargo runs. Edit only the new crate `crates/ve-scatter` (add it to the workspace members). Plan: `docs/plans/world-generators.md`.

## State of the module (read first)
The module **already exists** (`crates/ve-world/src/scatter/`, in the workspace). `points.rs` (the `PointSet` implementation), `field.rs` (`Field2::sample`), `error.rs`, `mod.rs` and the tests `crates/ve-world/tests/scatter.rs` are written and must not be edited (the 2 tests of those pass now; 15 fail until you finish). The bodies in `place.rs` (`poisson`, `vary`, `align_to_surface`, `filter`, `relax`, `cluster`) and `species.rs` (`place_species`) are stubs: **replace them** and remove the `NotImplemented` variant when the last stub is gone. The signatures there are the contract (they differ slightly from the sketch below: `place_species` takes `channels: &[(&str, Field2<'_>)]`, masks and heights are `Option<&Field2<'_>>`). Reuse `crate::num` helpers instead of raw casts, write normal readable one-statement-per-line Rust, run `rustfmt --edition 2024 --config-path rustfmt.toml` on every file you touch, and keep `cargo clippy -p ve-world --all-targets -- -D warnings` clean for the `scatter` module (no `#[allow]`). The 200,000-point timing test is `#[ignore]`: run it with `--release -- --ignored` and report the time. Use `CARGO_TARGET_DIR=target-codex-scatter` and `cargo test -p ve-world --test scatter`.
Design hints: lattice cell = `radius / sqrt(2)` (at most one point per cell), each cell's candidate offsets come from `splitmix64(hash(seed, cell_x, cell_z, try))`, a candidate is accepted if no point in the 5 by 5 neighbouring cells is closer than the local spacing; to make the result independent of tile cuts, decide cells in a fixed global priority (a hash of the cell) and generate the neighbourhood (cells within 2 of the rectangle) regardless of the rectangle, keeping only the points inside it.

## Why
`inst.scatter_on` places at most 5,000 points, filters only by normal direction, checks spacing against every placed point (quadratic) and keeps no per-point data. Forests, grass and rocks need blue-noise placement across big areas driven by masks, and each point must carry attributes (scale, rotation, species, age, wind weight) for later nodes. This crate is pure arithmetic and has no graph or Bevy dependency.

## Rules
- Core crate: allowed dependencies are `thiserror` only. Deterministic and platform-identical (`+ - * /`, `sqrt`, `floor`, integer hashing; no `sin`/`cos`/`exp`; for rotation angles use a polynomial or table-free rotation by building a unit vector from two uniform draws and normalising). Randomness: SplitMix64 as in `crates/ve-modeling/src/instances/scatter.rs`, one stream per cell derived from the seed and the cell coordinates, so the result does not depend on the order cells are processed.
- Cancellation: `cancel: &dyn Fn() -> bool` checked at least once per grid cell row; `ScatterError::Cancelled`.
- Limits: at most 1,000,000 points per call (`ScatterError::TooMany { requested, max }` naming the fix: raise the spacing or shrink the area).

## API to build (signatures are fixed; tests will import these)
```rust
pub struct PointSet { /* private */ }
impl PointSet {
    pub fn len(&self) -> usize; pub fn is_empty(&self) -> bool;
    pub fn positions(&self) -> &[[f32; 3]];
    pub fn channel(&self, name: &str) -> Option<&[f32]>;          // one f32 per point
    pub fn vector_channel(&self, name: &str) -> Option<&[[f32; 3]]>;
    pub fn set_channel(&mut self, name: &str, values: Vec<f32>) -> Result<(), ScatterError>;       // length must equal len()
    pub fn set_vector_channel(&mut self, name: &str, values: Vec<[f32; 3]>) -> Result<(), ScatterError>;
    pub fn channel_names(&self) -> Vec<String>;                   // sorted, both kinds
    pub fn retain(&mut self, keep: &[bool]);                      // drops points and the same entries of every channel
    pub fn merge(&mut self, other: &PointSet) -> Result<(), ScatterError>; // same channel names required
}

/// A scalar field the sampler reads: heights, density masks. Row-major, centred on the origin like the terrain grid.
pub struct Field2<'a> { pub values: &'a [f32], pub width: usize, pub depth: usize, pub cell: f32, pub origin: [f32; 2] /* world position of cell (0,0) */ }
impl Field2<'_> { pub fn sample(&self, x: f32, z: f32) -> f32 } // bilinear, clamps at the edge

/// LOCALLY STABLE world placement (the Horizon Zero Dawn property): the candidate for each cell of a world-aligned lattice (cell = radius) is a pure function of (world seed, species seed, cell x, cell z). The result for a rectangle is therefore the same however it is cut into tiles, and editing a mask changes only the points whose cell or neighbours' masks changed. `area` is in WORLD metres; a point belongs to the tile whose half-open rectangle contains it, so adjacent tiles never duplicate or drop a point.
pub struct Poisson { pub seed: u32, pub radius: f32, pub density: f32, pub area: [f32; 4] /* min_x, min_z, max_x, max_z */, pub tries: u32 }
/// Blue-noise points over the rectangle. `radius` is the minimum spacing at density 1; a point is kept with probability `density * mask(x,z)` and the local spacing grows as the mask falls (radius / sqrt(mask), mask floored at 0.05). `heights` places y on the surface (bilinear), otherwise y = 0.
pub fn poisson(params: &Poisson, mask: Option<&Field2>, heights: Option<&Field2>, cancel: &dyn Fn() -> bool) -> Result<PointSet, ScatterError>;

/// Fills standard channels on a point set: `scale` (f32), `yaw` (f32, radians in 0..TAU), `age` (f32, 0..1), `id` (f32, species index)
pub struct Variation { pub seed: u32, pub scale: [f32; 2], pub species: u32, pub age: [f32; 2] }
pub fn vary(points: &mut PointSet, params: &Variation);

pub fn align_to_surface(points: &mut PointSet, heights: &Field2, amount: f32); // adds the vector channel `normal` from the height field and a float channel `tilt` = amount * angle of the normal from +y (0..1 of 90 degrees)
pub fn filter(points: &mut PointSet, channel: &str, lo: f32, hi: f32) -> Result<(), ScatterError>; // keeps points whose channel value is within lo..=hi
pub fn relax(points: &mut PointSet, radius: f32, iterations: u32, cancel: &dyn Fn() -> bool) -> Result<(), ScatterError>; // Lloyd-style: push points apart inside `radius` using a grid; positions stay in the original bounding box
pub fn cluster(params_seed: u32, parents: &PointSet, children_per_parent: [u32; 2], spread: f32, cancel: &dyn Fn() -> bool) -> Result<PointSet, ScatterError>; // children around each parent, channel `parent` = parent index
```
`ScatterError`: `Cancelled`, `TooMany { requested, max }`, `BadChannel { name }`, `LengthMismatch { name, expected, got }`, `BadParameter { name, why }` (radius <= 0, empty area, `lo > hi`).

## Species and biome rules (added in v2)
```rust
pub struct Species { pub id: u32, pub seed: u32, pub radius: f32, pub density: f32, pub rules: Vec<Rule> }
pub struct Rule { pub channel: String, pub lo: f32, pub hi: f32, pub soft: f32, pub weight: f32 } // soft = smoothstep width on both ends; the species weight at a point is the product of rule memberships times weight
pub fn place_species(species: &[Species], area: [f32; 4], channels: &dyn Fn(&str) -> Option<Field2<'static>>, heights: Option<&Field2>, cancel: &dyn Fn() -> bool) -> Result<PointSet, ScatterError>; // one PointSet, channel `id` = species id; where species overlap, the cell goes to the species with the highest weight (ties by id), so forests and meadows get clean ecotones
```
`place_species` must keep the locally stable property across species: adding a species changes only the cells where its weight wins.

## Tests
Written by the owner-side session before launch, in `crates/ve-scatter/tests/`: minimum spacing honoured (no pair closer than `radius`), maximal-ish coverage (the area is filled to at least 60 percent of the theoretical packing), density mask thins points where the mask is low, determinism and tile invariance (the union of the points of four tiles equals the points of the joined rectangle, bit for bit, with no duplicates; shifting the rectangle by whole cells keeps shared points identical; changing the mask in one region changes no point farther than one radius outside it), channels stay aligned after `retain` and `filter`, `relax` does not reduce the minimum spacing, 200,000 points in under 2 s in release (report measured), cancellation, errors. Do not weaken them; report wrong premises in the Status.

## Acceptance
`cargo test -p ve-scatter`, `cargo clippy -p ve-scatter --all-targets -- -D warnings`, `cargo check -p ve-scatter --target wasm32-unknown-unknown` clean; no file over 400 lines; an FNV hash test of `poisson` output for native versus WASI comparison.

## Status (Codex: fill in when you finish, then stop)
- State: scatter implementation complete; the focused test target passes all active tests.
- What was built: deterministic world-aligned Poisson placement, post-placement density and mask thinning, variation channels, surface normals and tilt, channel filtering, relaxation, clustering, and weighted species placement. Removed `ScatterError::NotImplemented`. Added shared deterministic hash helpers under `scatter/place/random.rs`.
- Commands run and results:
  - `CARGO_TARGET_DIR=target-codex-scatter cargo test -p ve-world --test scatter` — 16 passed, 0 failed, 1 ignored.
  - `CARGO_TARGET_DIR=target-codex-scatter cargo test -p ve-world --test scatter --release -- --ignored` — timing test passed: 205,952 points in 0.28 s.
  - `CARGO_TARGET_DIR=target-codex-scatter cargo clippy -p ve-world --all-targets -- -D warnings` — blocked by the existing `clippy::mod_module_files` lint on the protected `scatter/mod.rs`; clippy reported no scatter implementation lints before stopping.
  - `CARGO_TARGET_DIR=target-codex-scatter cargo xtask rules` — scatter files satisfy the 400-line limit; the repository rule still fails on the pre-existing 403-line `ve-human/src/eval.rs` and 409-line `ve-graph/src/builtin/fields.rs`.
  - `CARGO_TARGET_DIR=target-codex-scatter cargo xtask quick ve-world` — stopped at its repository-wide `cargo fmt --all --check` stage because many unrelated files have formatting diffs. It made no formatting changes.
- Could not verify / open questions: the implementation applies mask and density thinning after stable base placement, so low mask density reduces population without explicitly increasing the minimum separation radius. The crate-wide clippy gate remains blocked by the protected module-layout lint. The strict gate is pending.
