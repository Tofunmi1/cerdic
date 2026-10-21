# Handoff to Codex: `ve-chain`, a pinned Verlet chain for secondary motion

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use
`#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no
`Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`
from `engine`.** Create the new crate `crates/ve-chain` (pure Rust, no Bevy, no I/O, no dependencies except `thiserror`; compiles to wasm32) and add it to the workspace
members in `engine/Cargo.toml`. Do not edit any other crate: other engineers are editing `ve-modeling`, `ve-graph`, `ve-bevy`, `ve-runtime`.

## Why
Tails, ears, hair, cloth edges and tentacle tips should lag behind the body and spring back, which reads as life. The usual technique is a spring or Verlet chain: the root is
pinned to a bone, the other points move on their own with inertia, damping and gravity, are pulled back towards their rest pose, and keep their segment lengths. Later the
runtime will feed the result into skinned bones; this crate is the solver only.

## API (the tests in `crates/ve-chain/tests/chain.rs` use exactly this)
```rust
pub struct RootPose { pub position: [f64; 3], pub rotation: [f64; 4] /* x, y, z, w unit quaternion */ }
pub struct ChainSpec {
    pub rest: Vec<[f64; 3]>,  // each segment's offset from the point before it, in the root's frame (so |rest[i]| is its length)
    pub stiffness: f64,       // 0..=1: how strongly points are pulled back to the rest pose each second (0 hangs free)
    pub damping: f64,         // >= 0, per second: velocity lost to drag
    pub gravity: [f64; 3],    // m/s^2, world space
    pub iterations: usize,    // length-constraint passes per step, 1..=32
}
pub struct Chain { .. }
impl Chain {
    pub fn new(spec: ChainSpec, root: RootPose) -> Result<Chain, ChainError>; // starts exactly in the rest pose at `root`
    pub fn step(&mut self, root: RootPose, dt: f64);
    pub fn points(&self) -> &[[f64; 3]];   // root first, then one point per segment (rest.len() + 1 points)
}
pub enum ChainError { Empty, ZeroLength { index: usize }, NotFinite(&'static str), TooLong { max: usize /* 64 */ }, OutOfRange(&'static str) }
```
Behaviour:
- Point 0 is the root and is set to `root.position` each step. The rest pose in the world is `root.position` plus the cumulative sum of `rotate(root.rotation, rest[i])`.
- Each other point: position-Verlet with `velocity = (p - previous) * exp(-damping * dt)`, plus `gravity * dt^2`, plus a pull towards its rest-pose point of
  `stiffness * (rest_point - p) * min(1, 60 * dt)`-style strength that is frame-rate independent (document the formula you choose, tests only check the behaviour).
- Then `iterations` passes of distance constraints so every segment keeps its rest length (|rest[i]|), pinning point 0.
- **dt is clamped** to at most 1/30 s per internal step; a larger `dt` runs several clamped sub-steps (at most 8, the rest dropped) so a long frame never explodes the chain.
- Deterministic: no random numbers, no threads, no time; same inputs give bit-identical points on every platform (only `f64` arithmetic in a fixed order; `sqrt`, `exp` are fine).
- Validation: empty `rest`, a zero-length segment, any non-finite number, more than 64 segments, `stiffness` outside 0..=1, `damping` negative or `iterations` outside 1..=32 are errors.

## Tests, already written
`crates/ve-chain/tests/chain.rs`. Do not weaken them. If a test's premise is wrong, say so in the Status and explain; do not change it quietly. Add your own unit tests for the
quaternion rotation helper (identity, a quarter turn, the inverse) in the crate.

## Acceptance
`cargo test -p ve-chain` passes, `cargo clippy -p ve-chain --all-targets -- -D warnings` clean, `cargo check -p ve-chain --target wasm32-unknown-unknown` passes, no file over 400 lines.

## Status (Codex: fill in when you finish, then stop)
- State: Implemented. `ve-chain` is included in the workspace through the existing `crates/*` member glob; no redundant member entry was added.
- What was built: A pure Rust `ve-chain` crate with validated chain construction, normalized quaternion root poses, deterministic Verlet integration, frame-rate-scaled rest-pose return, damping and gravity, root interpolation, and pinned segment-length constraints. Time is split into at most eight substeps, each no longer than 1/30 s; excess time is dropped. Invalid `step` time or root values are ignored because `step` is infallible. Added identity, quarter-turn and inverse quaternion tests. Existing integration tests were retained; only lossless cast/expected-value expressions were adjusted to satisfy Clippy. No other crate was edited.
- Commands run and results: `rustfmt --edition 2024 --config-path rustfmt.toml crates/ve-chain/src/lib.rs` (pass); `cargo test -p ve-chain` (pass: 1 unit and 8 integration tests); `cargo clippy -p ve-chain --all-targets -- -D warnings` (pass); `cargo check -p ve-chain --target wasm32-unknown-unknown` (pass). `cargo xtask quick ve-chain` stopped at its workspace-wide fmt check because unrelated in-progress files, including `ve-bevy` and vendored Bevy files, are unformatted. `cargo xtask rules` reports pre-existing overlength files `crates/ve-geometry/src/triangulate.rs` (403 lines) and `crates/ve-modeling/src/result.rs` (407 lines). Strict gate pending.
- Could not verify / open questions: The workspace `quick` and `rules` gates remain blocked by the unrelated workspace issues above. The `ve-chain` package acceptance checks, including wasm compilation, pass. No open design questions; stiffness blends by `1 - (1 - 0.05 * stiffness)^(60 * dt)` per substep.
