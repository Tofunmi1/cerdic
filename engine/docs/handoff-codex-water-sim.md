# Handoff: a real water simulation (ripples that react), ADR 0070 follow-up

The procedural wave field (`ve_render::water_waves`) gives the resting look. What it cannot do is react: ripples from a hand or a thrown crate, waves that slow in shallow water and bounce off the shore, rain, wakes. RedGPU's lake has an interaction simulator for that (`engine/third_party/redgpu/src/water/lakeCore/interaction/LakeWaveSimulator.ts`, `lakeWaveSimulation.wgsl`, `LakeCapturePass.ts`: read them for the idea; it runs as a WebGPU compute shader, we need a WebGL2-safe path, so the simulation runs on the CPU and uploads a texture). Write our own version.

## Tests (already written, do not weaken)
`crates/ve-world/tests/water_sim.rs` (14 tests): the physics. Ripple speed sqrt(g * depth), half speed over a quarter of the depth, stability for any time step, volume conservation, damping, open edges, symmetry, land stays dry and reflects, normals, interpolation, bad input, determinism. If one looks wrong, say so in Status with numbers.

## Part 1: `ve_world::water_sim` (module of `ve-world`; `water_sim.rs` plus `water_sim/` files, each under 400 lines)
```rust
pub struct SimOptions { pub gravity: f32, pub damping: f32 /*per second, >= 0*/, pub absorb_edges: bool }
pub enum SimError { BadShape { .. }, BadParameter { .. } }   // thiserror, Debug
pub struct WaterSim { .. }
impl WaterSim {
  pub fn new(depths: &[f32], width: usize, depth: usize, cell: f32, options: SimOptions) -> Result<Self, SimError>   // depth in metres per cell, 0 = land
  pub fn step(&mut self, dt: f32)                         // any dt; sub-steps inside so it never goes unstable; dt <= 0 or non-finite does nothing
  pub fn disturb(&mut self, x: f32, z: f32, radius: f32, amount: f32)   // world metres; a smooth bump of `amount` metres of displacement, only on water cells; bad input does nothing
  pub fn height(&self, x: f32, z: f32) -> f32             // displacement from rest, bilinear, 0 outside the grid or for non-finite input
  pub fn normal(&self, x: f32, z: f32) -> [f32; 3]        // unit surface normal from the height gradient (up on still water)
  pub fn heights(&self) -> &[f32]                         // row-major, for texture upload
  pub fn total_volume(&self) -> f64                       // sum of displacement * cell^2
  pub fn time(&self) -> f64
}
```
Numerics: the linear shallow-water equations on a staggered (C) grid with variable depth, `eta_t = -div(h u)`, `u_t = -g grad(eta) - damping u`, forward-backward (symplectic) time stepping, no flux through land (depth 0) faces, so the water volume is conserved without damping; sub-step so `dt_sub <= 0.45 * cell / sqrt(2 * g * h_max)`. `absorb_edges`: a sponge of about 8 cells at the grid border whose damping rises smoothly to the border so waves leave instead of reflecting. Deterministic: same inputs, bit-identical outputs. No unwrap/expect/panic, documented.

## Part 2: reach it from the language and the page
- **Node parameters** on `water.fill` and `water.surface` (see `crates/ve-modeling/src/water.rs`, `water_look.rs`, `world_river.rs`; if `water.fill` from the job in `docs/handoff-codex-water-fill.md` is not there yet, wait for it by checking the file `crates/ve-modeling/tests/water_fill.rs` exists and passes; if not, do `water.surface` only): `sim` (bool, default false), `sim_resolution` (Int 32..256, cells along the longer side), `sim_damping` (Num 0..5, default 0.6), `sim_depth` (Num 0.05..50, default 1: the depth that sets the wave speed; where the node knows the ground, use level minus ground per cell instead and treat `sim_depth` as the fallback), `rain` (Num 0..1, default 0: random drops per second per 100 m2 of water, deterministic from `seed`). Carry them in the part's water material (`SurfaceWater`, reflection and serde defaults so old documents load), with node tests following the existing ones.
- **Runtime** (`crates/ve-runtime/src`, find where water parts become entities; a new file `water_sim.rs`): one `WaterSim` per water part with `sim` on, built from the part's bounding box and (when available) its cells; stepped each frame from the shared animation clock (fixed 1/60 sub-step budget, at most 4 steps a frame); the height field uploaded as a single-channel float or half-float texture (WebGL2-safe, no compute; reuse the existing texture upload code, `textures_gpu.rs` or similar) at most once per frame. `ve-render`: bind that texture in the water branch (a small separate bind group, only for sim water): the vertex stage adds the sampled displacement to the sheet height (vertex texture fetch with `textureLoad`), the fragment stage adds the finite-difference slope of the texture to the wave normal; bounding box origin and size in the water uniform. Water without `sim` is untouched. Add the shader changes to the validation tests (`shader_tests.rs`: parse, validate, GLSL ES 3.00 translation).
- **World API** (`crates/ve-runtime/src/world_api_methods.rs`; the methods are described in the `world_api` guide topic, add them there): `water.disturb {part?, x, z, radius, amount}` (default radius 0.5, amount 0.04; `part` optional name, default every sim water part the point is over) and `water.height {x, z}` (the simulated displacement plus the procedural wave height at that point, for buoyancy and scripts). A splash: when `amount` exceeds 0.03 also call the `fx.burst` splash at the point.
- **Rain** (`rain` > 0): deterministic drops from a seeded generator, as `disturb` calls in the runtime step.
- Tests for part 2 in Rust (parameter parsing and defaults, texture packing round trip within tolerance, uniform layout, rain determinism). You cannot run a browser; Claude looks at it live.
- Do NOT rebuild wasm. Disturbance from moving bodies (hands, thrown objects), buoyancy and river flow are later jobs.

## Rules
- AGENTS.md rules (no unwrap/expect/panic in library code, `#[expect]` not `#[allow]`, docs on public items, files under 400 lines, tests for behaviour). No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-world --test water_sim`, `cargo test -p ve-render`, the `ve-modeling` tests you touch (by `--test` selector), `cargo check -p ve-runtime --target wasm32-unknown-unknown`.

## Status
Completed parts 1 and 2 (2026-10-26). Reviewed and completed the implementation already present in the working tree, preserving unrelated work.

- `ve_world::water_sim` implements the staggered shallow-water solver, variable wet-cell depths, CFL substeps, damping and absorbing borders, smooth disturbances, bilinear heights, normals, volume and elapsed time. Seeded rainfall is deterministic. Fixed and regression-tested wet-cell queries whose division rounds up at the last cell. The supplied 14-test physics file is unchanged; all tests pass without adjustments.
- Both `water.surface` and `water.fill` carry the five simulation controls through their water material, with serde/reflection defaults for older documents. The existing `water.fill` tests pass, so both nodes are supported. Added lake default and invalid-type/range tests.
- Runtime simulations share the animation clock, run at 1/60 with at most four steps per frame, rasterize water footprints and ground-derived depths from the existing sheet geometry, apply seeded rain, and publish versioned snapshots for one R32Float upload per frame. Fixed stale simulations when replacement geometry has no usable footprint, reset stale material bindings, and capped dimensions against floating-point resolution rounding. Shared render data initializes simulation snapshots for alternate renderer configurations too.
- The forward water shader samples displacement in the vertex stage and finite-difference slopes in the fragment stage. Simulation pipelines use a dedicated texture-bearing layout; ordinary water keeps its uniform-only layout. Added/verified texture packing, uniform layout, old-material defaults, and shader validation/GLSL ES 3.00 translation tests.
- `water.disturb` and `water.height` are routed through the world API and documented in the `world_api` guide. Disturbances above 0.03 m request the existing splash burst.

Checks passed, all with `CARGO_TARGET_DIR=/tmp/codex-target-sim`:

- `cargo test -p ve-world --test water_sim` — 14/14, including a final rerun after the sampling fix.
- `cargo test -p ve-world --lib water_sim` — 2/2 (rain determinism/rate and grid-edge sampling).
- `cargo test -p ve-render` — all unit, integration and doc tests passed, including shader parse/validation/GLSL ES 3.00 translation.
- `cargo test -p ve-modeling --test water --test water_fill` — 7/7 and 12/12.
- `cargo check -p ve-runtime --target wasm32-unknown-unknown` — passed. Existing warnings remain in unrelated geometry, terrain and runtime files.

Formatted touched Rust files with `rustfmt`; `git diff --check` passed. No browser or headset verification was attempted; live appearance, GPU uploads and splash playback remain for Claude to check. No wasm bundle was rebuilt. No `cargo xtask` or `cargo fmt` was run, as this handoff directs; the strict milestone gate remains pending. Nothing was staged or committed.
