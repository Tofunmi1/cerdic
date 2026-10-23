# Handoff to Codex: the band scheduler (ADR 0058, pure logic in `ve-world`)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-bands`; delete it when done.
Read `docs/adr/0058-*.md` first. Measured motivation: a wood of 44 oaks, 84 pines and 4,600 grass tufts draws about 780,000 triangles (the headset budget is
about 300,000), because every copy draws the full mesh; the scheduler decides, per spatial chunk of a set, how it is drawn.

## Task
New module `crates/ve-world/src/bands.rs` (declare `pub mod bands;` in `lib.rs`; no engine dependencies, deterministic, no allocation surprises):
```rust
pub enum Band { Near, Mid, Far, Culled }                       // Clone, Copy, Debug, PartialEq, Eq
pub struct BandConfig { pub near: f32, pub mid: f32, pub far: f32, pub hysteresis: f32, pub near_budget: usize }   // Clone, Copy, Debug
pub struct ChunkInfo { pub centre: [f32; 3], pub radius: f32, pub copies: usize }                                  // Clone, Copy, Debug
pub struct Scheduler { .. }                                    // Scheduler::new(config) ; Scheduler::update(&mut self, &[ChunkInfo], &[[f32; 3]]) -> Vec<Band>
pub struct BandCounts { pub near_copies: usize, pub mid_chunks: usize, pub far_chunks: usize, pub culled_chunks: usize }
pub fn summary(chunks: &[ChunkInfo], bands: &[Band]) -> BandCounts
```
Rules (all pinned by the contract tests):
- Distance of a chunk = max(0, nearest eye distance to its centre minus its radius). `d < near` is Near, `d < mid` Mid, `d < far` Far, otherwise Culled. No eyes: everything Culled.
- Hysteresis: a chunk that was in a band keeps it when it moves *outward* until its distance exceeds that band's outer edge times `(1 + hysteresis)`; moving inward
  it changes at the edge itself. State lives in the scheduler per chunk index; the chunk list may grow or shrink between updates (new chunks start with no history).
- Near budget: the total `copies` of Near chunks never exceeds `near_budget`. Chunks that would be Near are admitted nearest first (ties: lower index) while they
  fit; the rest are Mid. A chunk that alone exceeds the budget is never Near. The budget beats hysteresis.
- Everything is deterministic: same history and input give the same output.

## Contract (written first, do not weaken)
`crates/ve-world/tests/bands.rs`. Report a test that looks wrong in Status; do not edit it.

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-bands cargo test -p ve-world
CARGO_TARGET_DIR=target-codex-bands cargo clippy -p ve-world --all-targets -- -D warnings   # must be clean for your files
CARGO_TARGET_DIR=target-codex-bands cargo check -p ve-world --target wasm32-unknown-unknown
```

## Status
Implemented `ve_world::bands` and declared it in `lib.rs`. The scheduler applies nearest-eye sphere distance, per-index outward hysteresis, deterministic nearest-first near-budget admission, and band cost summaries. The contract test file was left unchanged.

Checks run with `CARGO_TARGET_DIR=target-codex-bands`:
- `cargo test -p ve-world`: passed (all active tests; timing tests remain ignored).
- `cargo clippy -p ve-world --all-targets -- -D warnings`: blocked by `tests/bands.rs:74`, where `i as f32` triggers `clippy::cast_precision_loss`. `cargo clippy -p ve-world --lib -- -D warnings` passes for library code.
- `cargo check -p ve-world --target wasm32-unknown-unknown`: passed.
- `cargo xtask quick ve-world`: failed at workspace-wide format check due to existing diffs across unrelated engine and vendored Bevy files.
- `cargo xtask rules`: failed on existing over-400-line files `ve-graph/src/builtin/fields.rs`, `ve-graph/src/builtin/world.rs`, and `ve-modeling/src/human_hair.rs`.

No behavioral assertion in the contract suite looks wrong. The test `a_chunks_radius_brings_it_closer` has a naming typo, and the cast in the near-budget test is the all-targets Clippy blocker; neither test was edited. The dedicated `target-codex-bands` directory was removed after verification.
