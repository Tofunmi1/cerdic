# Handoff: non-repeating texture sampling (ADR 0070 port 2)

Reference (read first, read-only, MIT): `engine/third_party/redgpu/src/landscape/core/shader/library/common/stochasticSampling.wgsl` and the structs `StochasticGridTri.wgsl`, `StochasticSampleResult.wgsl` next to it. It cuts the plane into a triangular grid; a point blends three samples of one texture, each taken at the point shifted by a random offset that belongs to the grid vertex, with the barycentric weights of the triangle. We want it for tiled procedural textures and the terrain material, where repetition shows from a distance. Write our own version from the idea; keep the upstream notice in a comment where the shader adapts its maths.

## Test (already written, do not weaken)
`crates/ve-render/tests/stochastic.rs` (8 tests). If one looks logically wrong, say so in Status with numbers; do not change it.

## Part 1: CPU reference (`crates/ve-render/src/stochastic.rs`)
`pub struct StochasticSample { pub uv: [f32; 2], pub weight: f32 }`; `pub fn stochastic_uvs(uv: [f32; 2], scale: f32, seed: u32) -> [StochasticSample; 3]` (the three shifted sample positions and weights; `scale` is grid cells per uv unit); `pub fn vertex_offset(cell: [i32; 2], seed: u32) -> [f32; 2]` (a deterministic hash of the grid vertex and seed, each component in [0, 1); integer hash, no `sin` tricks, so CPU and GPU agree bit for bit). Weights are the barycentric weights of the containing triangle; to keep contrast the shader may use the variance-preserving blend of Heitz and Neyret as an option, but `stochastic_uvs` returns plain linear weights (the test checks they sum to 1). Non-finite or zero inputs give finite samples (clamp scale to 0.01..1000, replace NaN/inf uv by 0). Document public items; no unwrap/expect/panic.

## Part 2: the shader (`crates/ve-render/src/shaders/stochastic.wgsl`, included into the forward shader like `wind.wgsl`)
- `fn vertex_offset(cell: vec2<i32>, seed: u32) -> vec2<f32>` (same integer hash as the CPU one) and `fn stochastic_cells(uv: vec2<f32>, scale: f32, seed: u32) -> array<...>` or a small struct returning the three uvs and weights.
- Find where the forward shader samples the tiled texture layers (procedural texture programs baked into the texture array, `tile`/world-space projection, the terrain material's layers; look at `crates/ve-render/src/textures*.rs`, `surface_terrain.rs` and the `maps`/`textures` parts of `forward.wgsl`) and add the stochastic path: three samples blended, only for surfaces that opt in. Gradients: use `textureSampleGrad` with the derivatives of the unshifted uv for all three samples (so mip selection does not jump at triangle edges); WebGL2 allows it.
- Opt-in: a flag on the material (`no_repeat`, carried like the other surface parameters; reflection and serde defaults so old documents load). `terrain.material` defaults it on; `solid.texture` defaults it off with a `no_repeat` parameter (and `no_repeat_scale`, cells per tile, default 1.0, range 0.25..8). Quality: the lowest quality tier (see `quality.rs`, `quest_tier.rs`) ignores the flag and samples once. Add the shader to the validation test (`shader_tests.rs`: parse, validate, GLSL ES 3.00 translation) and keep it passing.

## Part 3: the language
- Node parameters on `terrain.material` and `solid.texture` (see `crates/ve-modeling`: `terrain.rs`, `world_ops.rs`, and the texture node code; follow the existing node tests), ranges, defaults, and guide text (one short paragraph in the `world` and `appearance` guide topics in `tools/mcp/guide.mjs`; keep tool definitions untouched).
- Do not rebuild wasm; Claude does after review and looks at it in the browser (a terrain from far away, with and without).

## Rules
- AGENTS.md rules (files under 400 lines, `#[expect]` not `#[allow]`, docs on public items, tests for behaviour). No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-render` (all, including the shader validation), `cargo test -p ve-modeling --test water --test river_level` plus the node tests you add, `cargo check -p ve-runtime --target wasm32-unknown-unknown`.

## Status
Implemented parts 1, 2 and 3.

- Added the documented CPU reference with linear barycentric weights, a wrapping integer vertex hash, exact 24-bit float conversion, and finite-input handling. The WGSL uses the same hash, conversion and triangular grid; its adaptation retains the upstream MIT notice.
- Added forward texture-array sampling of three shifted positions for all four map channels, sharing cells and the original unshifted UV gradients. The opt-in scale travels in the spare material attribute lane, with no extra vertex attributes. Low quality takes the single-sample path.
- Added terrain layer sampling in the forward renderer and the existing Bevy terrain extension, sharing the same grid and terrain functions. Terrain parameters share the existing per-material uniform/bind group, with separate batching/cache keys for distinct terrain looks. Bevy terrain sampling follows runtime quality changes.
- Added reflected/serialized material controls and projection parsing with defaults for old results. `terrain.material` defaults `no_repeat=true`; `solid.texture` defaults it false. Both nodes expose `no_repeat_scale=1`, range 0.25..8. Updated the world and appearance guide paragraphs without changing MCP tool definitions. Split oversized material/spec/shader files by responsibility.
- Added coverage for material defaults, reflection/serde, node defaults/overrides/range rejection, parameter preservation through transforms, quality fallback, terrain batch separation and CPU/shader uniform layout.

Checks passed (all Cargo commands used `CARGO_TARGET_DIR=/tmp/codex-target-stoch`):

- `cargo test -p ve-render`: 177 tests passed, including all 9 existing `tests/stochastic.rs` tests unchanged, plus WGSL parsing, validation and GLSL ES 3.00 translation of the forward variants and standalone stochastic shader. No stochastic test was weakened or found logically wrong.
- `cargo test -p ve-modeling --test water --test river_level --test stochastic_nodes --test g12_texture_size --test material_preset`: 29 tests passed.
- `cargo check -p ve-runtime --target wasm32-unknown-unknown`.
- `cargo check -p ve-bevy`.
- `node --check tools/mcp/guide.mjs`; targeted `rustfmt --edition 2024 --config-path rustfmt.toml` formatting (with child-module traversal disabled).

Not verified here: browser/headset appearance and performance, and actual GPU execution of the hash. The CPU/GPU implementations use identical integer operations and conversion; shader validation/translation passed. Browser review of distant terrain with and without sampling remains for Claude after review. No wasm bundle was rebuilt. The strict gate remains pending; no `cargo xtask` or `cargo fmt` was run, as instructed. Existing unrelated compiler warnings remain. No commits or staging.
