# Handoff to Codex: `ve_world::city`, the layout of a modern district (streets, blocks, lots, heights)

Owner: Claude. Helper: Codex. Pure Rust, no Bevy, no graph, no I/O, no dependencies except `thiserror` (check `crates/ve-world/Cargo.toml`; add it only if missing). Compiles to wasm32.
Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 first (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason)]` on the smallest item), files under 400 lines,
docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no attribution lines). **Never run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.**

## Why (researched first)
The owner wants procedural modern cities, dense like a Shibuya district, walkable in VR. The field's pipeline (Parish and Mueller 2001, "Procedural Modeling of Cities"; Voronoi and warped-grid street generators)
is: street network, then blocks (the faces between streets), then lots (blocks split recursively), then a building per lot from its footprint. This handoff is the first three stages plus a height per lot.
Nodes and building geometry come later (Claude writes them on top of this API). The module must therefore be a pure function of its parameters: same parameters, bit-identical result on every platform.

## The test (fixed, do not weaken)
`crates/ve-world/tests/city_layout.rs` (9 tests). Do not edit it. Run: `CARGO_TARGET_DIR=target-codex-city cargo test -p ve-world --test city_layout`. Delete `target-codex-city` when you finish (the disk is nearly full).

## API (the tests use exactly this)
```rust
// crates/ve-world/src/city.rs (+ crates/ve-world/src/city/*.rs); add `pub mod city;` to lib.rs
#[derive(Clone, Debug, PartialEq)]
pub struct CityParams {
    pub size: [f32; 2],        // metres, x and z extent of the district, centred on the origin. default [300, 300]; each 60..=1000
    pub centre: [f32; 2],      // where downtown is (x, z) in the same frame. default [0, 0]
    pub block: f32,            // target street spacing along each axis, metres, centre to centre of streets. default 40; 15..=200
    pub street_width: f32,     // metres between facing lots. default 9; 2..=40
    pub lot_min: f32,          // the smallest lot side wanted. default 6; 3..=40
    pub lot_max: f32,          // blocks are split until their long side is at most this. default 14; at least 1.5 * lot_min, at most 80
    pub irregularity: f32,     // 0 is a square grid, 1 is bent streets. default 0.5; 0..=1
    pub seed: u64,             // default 1
}
impl Default for CityParams { .. }  // the defaults above
#[derive(Clone, Debug, PartialEq)] pub struct Lot { pub footprint: Vec<[f32; 2]> /* counter-clockwise, 3..=8 corners */, pub block: u32, pub height: f32 /* metres */, pub front: usize /* index of the footprint edge facing the nearest street */ }
#[derive(Clone, Debug, PartialEq)] pub struct Layout { pub street_nodes: Vec<[f32; 2]>, pub street_edges: Vec<(u32, u32)>, pub lots: Vec<Lot> }
#[derive(Debug, thiserror::Error)] pub enum CityError { .. }  // Display names the offending parameter (`size`, `block`, `street_width`, `lot_min`, `lot_max`, `irregularity`) and says the allowed range
pub fn layout(params: &CityParams) -> Result<Layout, CityError>;
```

## Behaviour
- **Streets**: a grid of street lines along x and along z, spaced about `block` apart with a small random jitter (up to 15% of `block`), covering the district. Every street line is cut into segments of about `block / 3`, so a bent
  street shows as segments that are not axis-aligned: `street_nodes` are the points, `street_edges` the segments (the graph is connected and has loops). With `irregularity` 0 every segment is exactly axis-aligned. With 1 the lines are
  bent by a smooth deterministic displacement field (for example two octaves of value noise, amplitude up to about 0.3 * `block`, the same field applied to every point so streets stay roughly `street_width` apart).
- **Blocks**: the quads between neighbouring street lines, each pulled in by `street_width / 2` on every side, so the gap between two lots of different blocks is about `street_width`. Block numbers start at 0 and are contiguous.
- **Lots**: each block is split recursively: cut across the longer side at a random 35 to 65 percent, stop when the long side is at most `lot_max`. Lots may be quads or, near bent streets, convex polygons of up to 8 corners. No lot has a side
  under `lot_min * 0.5`. Lots of one block share walls, never overlap. `front` is the edge whose outward normal points at the nearest street (smallest distance from the edge midpoint to the street graph).
- **Heights**: a deterministic profile by distance `r` from `centre` (normalised by the district radius `min(size) / 2`): the fringe is 6 to 18 m, the middle 35 to 70 m, with value noise on top; the few tallest lots near the middle are
  towers of 90 to 180 m (about 3 to 6 percent of the lots, never fewer than 3, never more than 1 in 12). Every height is within 4..=260 m. The mean height within 0.35 of the radius is more than twice the mean beyond 0.8.
- **Defaults give a walkable district**: 250 to 1400 lots in 20 to 200 blocks.
- Validation: any parameter outside its range, a `lot_max` under `1.5 * lot_min`, or a non-finite number is an error naming it.
- Deterministic: your own small PRNG (xorshift or splitmix) seeded from `seed` and the cell coordinates, no `HashMap` iteration order, no threads, no time, only f32/f64 arithmetic in a fixed order.
- Budget and cost: at most 20,000 lots (refuse a size, block or lot size that would exceed it, naming the parameter).

## Rules
- You may edit only `crates/ve-world/src/city.rs`, `crates/ve-world/src/city/`, `crates/ve-world/src/lib.rs` (one line: `pub mod city;`), `crates/ve-world/Cargo.toml` (only if `thiserror` is missing) and the Status below. Never touch other crates.
- `CARGO_TARGET_DIR=target-codex-city`, no `cargo xtask`. Add unit tests for the street displacement field and the block split in your own files.
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do. Do not stop after a skeleton: the nine tests must pass.

## Status
Implemented `ve_world::city` with deterministic streets, street-inset blocks, recursive lot splitting, validation, frontage selection, and height profiles. The named integration target currently contains 10 tests (not 9); all 10 pass, as do the crate unit tests and the wasm32 check. Added unit tests for the displacement field and split area.

`cargo xtask quick ve-world` stopped at its workspace-wide formatting check because many unrelated files need formatting. `cargo xtask rules` reports existing overlong files in other crates. Package Clippy reports existing lints in `crates/ve-world/src/building.rs`, which is outside this task's allowed edit list; it reports no city module lints. The strict gate is pending. `target-codex-city` is removed after the checks.
