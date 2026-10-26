# Handoff: motion measurements (`ve_measure::motion`): foot slide, loop seam, bone stretch

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-measure` only** (a new `src/motion.rs`, `pub mod motion;` or the matching `pub use` in `src/lib.rs`, and new `MeasureError` variants in `src/error.rs`). Do not touch other crates (the `check.motion` graph node comes in a later job).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-motion`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code (the crate has a crate-level `#![expect]` for cast lints; keep to its conventions). No panics: every refusal is a typed `MeasureError`. Deterministic, arithmetic only.

## Why

The owner's goal (2026-10-25): an LLM makes characters and animation itself. A model that writes a walk cannot see it, so it needs numbers it can act on: does the foot skate across the ground while it should be planted, does the loop pop at its seam, does a bone stretch. These are the three most common faults of generated motion. This crate already answers geometry questions on plain arrays (`measure_mesh`, `symmetry_error`, `min_wall_thickness`); this adds three functions on sampled motion, in the same style. A later job wraps them in a graph node `check.motion` that samples a clip at 60 Hz and reports in the tool reply.

## API (the tests in `crates/ve-measure/tests/motion.rs` use exactly this)

In `ve_measure::motion`, all `pub` with docs; positions are world metres `[f32; 3]` (y up), frames are uniformly spaced:
- `foot_slide(foot: &[[f32; 3]], dt: f32, contact_height: f32) -> Result<FootSlide, MeasureError>`. A frame is in contact when its y is at most the track's lowest y plus `contact_height` (so a raised floor is measured the same). Speed is the horizontal (x and z) distance between two consecutive frames that are both in contact, over `dt`. `FootSlide { pub contact_frames: usize, pub mean_speed: f32, pub max_speed: f32 }` (derive `Clone, Copy, Debug, PartialEq`); speeds are 0 when no two consecutive frames are both in contact. Refuse fewer than 2 frames, `dt` not finite and positive, a negative or non-finite `contact_height`, any non-finite coordinate.
- `loop_seam(frames: &[Vec<[f32; 3]>]) -> Result<LoopSeam, MeasureError>`: `frames[i]` holds the positions of the same tracked points at frame `i` (root-relative: the caller removes travel). `LoopSeam { pub gap: f32, pub typical_step: f32 }` with `gap` the largest distance any point is from its position in frame 0 at the last frame, and `typical_step` the mean over points and consecutive frame pairs of the distance moved in one frame. `impl LoopSeam { pub fn seamless(&self) -> bool }` is true when `gap <= 1.5 * typical_step` (a full cycle that stops one step short of repeating its first frame), which also holds for a clip whose last frame equals its first and for a still pose (both zero). Refuse fewer than 2 frames, frames with different point counts, frames with no points, any non-finite coordinate.
- `length_drift(a: &[[f32; 3]], b: &[[f32; 3]]) -> Result<f32, MeasureError>`: `a` and `b` are the two ends of a bone over time; the result is the largest `|len_i - len_0| / len_0` where `len_i` is the distance between them at frame `i`. Refuse different lengths, fewer than 2 frames, a first-frame length of zero, any non-finite coordinate.
- New `MeasureError` variants for these refusals with `thiserror` messages that say what to change (for example `"foot_slide needs at least 2 frames, got 1"`); do not reuse the mesh variants for them.

## Tests (written by Claude; never weaken; add more in your own file if you like): `crates/ve-measure/tests/motion.rs`

10 tests: a planted foot against a skating foot (0.5 m/s read as 0.5 within 0.02); only horizontal motion counts; contact is relative to the lowest point; bad input refused (foot); a whole cycle is seamless; a cut-short cycle is not; a clip that ends where it began and a still pose are seamless; bad input refused (loop); a rigid bone does not drift and a 10 percent stretch reads as about 0.1; bad input refused (drift).

## Check

`CARGO_TARGET_DIR=/tmp/codex-target-motion cargo test -q -p ve-measure --test motion` plus the crate's other tests still passing (`cargo test -q -p ve-measure`), `cargo clippy -q -p ve-measure --all-targets -- -D warnings`, `cargo check -q -p ve-measure --target wasm32-unknown-unknown`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: implementation is complete. `cargo clippy --all-targets` is blocked by two diagnostics in the supplied `tests/motion.rs` (`approx_constant` and unfulfilled `float_cmp` expectation); tests were left unchanged.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-motion cargo test -q -p ve-measure --test motion` — passed, 10 tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-motion cargo test -q -p ve-measure` — passed, all crate tests.
  - `CARGO_TARGET_DIR=/tmp/codex-target-motion cargo clippy -q -p ve-measure --all-targets -- -D warnings` — failed on the two supplied-test diagnostics above.
  - `CARGO_TARGET_DIR=/tmp/codex-target-motion cargo clippy -q -p ve-measure --lib -- -D warnings` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-motion cargo check -q -p ve-measure --target wasm32-unknown-unknown` — passed.
- Files changed: `crates/ve-measure/src/motion.rs` (new), `crates/ve-measure/src/error.rs`, `crates/ve-measure/src/lib.rs`, `crates/ve-measure/src/tool.rs` (exhaustive error suggestion), and this handoff status.
