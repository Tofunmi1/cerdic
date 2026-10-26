# Handoff: `blend_trs` (pose blending) and the `anim` game effect

Owner: Claude manages; Codex implements. Two independent small jobs in two crates; do **A then B** (they do not touch each other). **Crates you may edit: `ve-animstate` (job A: a new `src/trs.rs`, export lines in `src/lib.rs`) and `ve-game` (job B: `src/effect.rs`, `src/parse/helpers.rs` action validation, the action that emits effects in `src/world/state/actions.rs`).** Do not touch other crates (`ve-runtime/src/game.rs` has a `match` on `EffectKind` that will not compile after job B: list it in your Status, Claude fixes it).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-blend`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. Both crates are deterministic arithmetic: no platform trigonometry in anything that decides a result (square root is fine).

## Why

The owner's goal (2026-10-25): an LLM makes characters and animation itself. `anim.machine` (done) carries a state machine on a part; `Machine::step` returns a `Pose` of clip layers with weights. To play it the runtime must blend the poses of several clips on each part: that blend is job A (a pure function, so it is testable and identical on every platform). A game rule must be able to raise a trigger on an entity's machine ("jump", "attack", "hit"): game rules only report requests to the host as `Effect`s, so job B adds an `anim` effect kind. The runtime wiring (parameters, applying the blend, raising the triggers) is the next job, by Claude.

## Job A: `ve-animstate`

`pub struct Trs { pub translation: [f32; 3], pub rotation: [f32; 4], pub scale: [f32; 3] }` (derive `Clone, Copy, Debug, PartialEq`; rotation is a unit quaternion x, y, z, w) and `pub fn blend_trs(items: &[(f32, Trs)]) -> Option<Trs>`, both exported from `lib.rs`:
- Weights are normalised by their sum. Translation and scale blend linearly. Rotation: flip each quaternion onto the hemisphere of the first item with positive weight (negate when its dot with the reference is negative; `q` and `-q` are one rotation), take the weighted sum, normalise.
- Items with weight exactly 0 are ignored. `None` for an empty list, a total weight of 0 or less, any negative or non-finite weight, any non-finite component, or a rotation of zero length (and for a blended rotation whose sum has zero length, fall back to the heaviest item's rotation).
- Doc comment: say why nlerp (not slerp): cheap and deterministic, exact at the midpoint of two rotations, accurate for the small angles between clips of one character.
- Tests: `crates/ve-animstate/tests/blend_trs.rs` (9 tests) plus the crate's existing tests.

## Job B: `ve-game`

`effect:anim:NAME[@SELECTOR]`: a new `EffectKind::Anim`. Read how `effect:sfx:...` is validated (`parse/helpers.rs` `validate_action`: the kind list `sfx | particles | camera_shake | message`) and emitted (`world/state/actions.rs`, `effect.rs`): add `anim` the same way. `NAME` is non-empty (the trigger name); `@SELECTOR` is any valid selector (`a`, `b`, `other`, `tag:x`, `kind:x`); a selector that matches several entities emits one `Effect` per entity with `target` set to that entity; no selector emits one `Effect` with `target: None`. Update the doc comment of `EffectKind`. Everything else about effects is unchanged.
- Tests: `crates/ve-game/tests/anim_effect.rs` (6 tests) plus all existing `ve-game` tests.

## Check

`cargo test -q -p ve-animstate`, `cargo test -q -p ve-game`, `cargo clippy -q -p ve-animstate -p ve-game --lib -- -D warnings`, `cargo check -q -p ve-animstate -p ve-game --target wasm32-unknown-unknown`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: Jobs A and B implemented in the permitted crates.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-blend cargo test -q -p ve-animstate` — passed (35 tests across unit, integration, and doctest targets; includes all 9 `blend_trs` tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-blend cargo test -q -p ve-game` — passed (all targets; includes all 6 `anim_effect` tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-blend cargo clippy -q -p ve-animstate --lib -- -D warnings` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-blend cargo clippy -q -p ve-animstate -p ve-game --lib -- -D warnings` — blocked by six existing warnings in `ve-game/src/parse/reader.rs` and `ve-game/src/world/state/hands.rs` (`unnested_or_patterns` and `collapsible_if`). No warnings were reported in the changed files.
  - `CARGO_TARGET_DIR=/tmp/codex-target-blend cargo check -q -p ve-animstate -p ve-game --target wasm32-unknown-unknown` — passed.
- Files changed: `engine/crates/ve-animstate/src/trs.rs` (new), `engine/crates/ve-animstate/src/lib.rs`, `engine/crates/ve-game/src/effect.rs`, `engine/crates/ve-game/src/parse/helpers.rs`, `engine/crates/ve-game/src/world/state/actions.rs`, and this handoff's Status section.
- Crates outside your scope that stop compiling because of the new `EffectKind` variant: `ve-runtime`, at `crates/ve-runtime/src/game.rs`'s exhaustive `match` on `EffectKind`; Claude owns that follow-up.
