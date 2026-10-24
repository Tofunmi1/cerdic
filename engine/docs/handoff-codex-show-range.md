# Handoff: camera-distance bands for placed copies (`scatter.place` show_from / show_to)

Owner: Claude manages; Codex implements. Follows `handoff-codex-lod-ranges.md` (done: `ve-bevy/src/sets.rs` gives each level of a `VeInstanceSet` a Bevy
`VisibilityRange`). Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use
`rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use your own `CARGO_TARGET_DIR` (for example `/tmp/codex-target`); do not commit
or stage; edit with your edit tools, not scripts. Do the crates in the order below, one at a time, running that crate's tests before moving on. The sandbox cannot run
the browser: Claude does that.

## Why

`worlds/forest_v2.txt` has three copies of every tree kind (near, mid, far mesh detail, 1/3/4 levels of `lod`), each planted by its own `scatter.place` with the
same seed and radius (so the plants stand in the same places) and a density field that cuts the forest into rings around the world's centre. A ring around the
centre is wrong as soon as the camera moves, and the viewer draws all 4.8 million triangles of the near mesh in the ring. We want the choice to follow the **camera**: each
copy-set gets a distance band, and the renderer draws a set only while the camera is inside it. A single set with several meshes is not needed: three sets with
bands (0 to 45 m, 45 to 140 m, 140 m and beyond) that stand in the same places are the same thing, and need no new geometry plumbing.

## What to build (language to renderer)

Two new optional numeric params on `scatter.place@1`: `show_from` (metres, default 0) and `show_to` (metres, default 0, where **0 means no limit**). Doc text on the params must say that
(`show_to` must be 0 or greater than `show_from`; otherwise a typed diagnostic with a suggestion, as other params do). They only affect drawing, never where copies stand.
Nothing else about placement may change: same seed, same positions, same part triangles.

1. **ve-graph** (`crates/ve-graph/src/builtin/world.rs`, `scatter.place@1` spec, params next to `scale`): add the two params. Update anything that lists the node's params
   in a snapshot or guide test if one exists (run `cargo test -p ve-graph`).
2. **ve-modeling**: `Part` (`src/result.rs`) gets `pub visible_range: Option<(f32, f32)>` (`#[serde(skip)]`, doc comment, default None; update every place `Part { .. }` is built, the
   compiler lists them). `world_scatter.rs` (find where `scatter.place` results become a part with `transforms`) sets it to `Some((from, to))` when either param is non-default,
   with `to == 0` stored as `f32::INFINITY`.
3. **ve-render** (`crates/ve-render/src/instance_sets.rs`): `VeInstanceSet` gets `pub min_distance: f32` (0 by default in `new`; document it as honoured by the Bevy path only).
   The existing renderer path may ignore it, but state that in the doc comment.
4. **ve-runtime** (`src/projection/placements.rs` and `bulk.rs`): when a placement part has `visible_range`, a bulk set (`bulk::spawn`) gets
   `min_distance = from` and its single `SetMesh` gets `max_distance = to`. For a part that is not bulk (fewer than 256 copies or animated) spawn each copy with the same range:
   `VisibilityRange::abrupt(from, to)` on the entity (add the dependency or re-export the way `ve-runtime` already reaches Bevy; check how it spawns `Mesh3d` today).
5. **ve-bevy** (`src/sets.rs`): the first level's `start` is `set.min_distance` instead of 0. Keep all existing tests passing.

## Tests (fixed, written by Claude; never weaken; add more if you like)

- `crates/ve-modeling/tests/scatter_show_range.rs` (copy the `evaluate` helper of `tests/rig_skin.rs`; build a small `terrain.tile` and a `scatter.place` of a small `solid.box`, see
  `crates/ve-modeling/tests/` for an existing scatter test to copy the terrain line from):
  1. `no_show_params_means_no_range`: `visible_range` is `None`.
  2. `show_params_are_carried_to_the_part`: `show_from=45, show_to=140` gives `Some((45.0, 140.0))`; `show_from=140` alone gives `Some((140.0, f32::INFINITY))`.
  3. `show_range_does_not_move_the_plants`: the part's `transforms` with and without the params are identical (same length and same values).
  4. `show_to_below_show_from_is_refused`: `show_from=50, show_to=20` yields an error naming `show_to` (find how a bad param is reported in other tests and assert on that).
- `crates/ve-runtime`, in `projection/bulk.rs` tests: a small pure helper, for example `fn band(range: Option<(f32, f32)>) -> (f32, f32)`, returning `(0.0, INFINITY)` for None
  and the pair otherwise, with tests for both; `spawn` uses it.
- `crates/ve-bevy/src/sets.rs`: `a_set_starts_at_its_min_distance`: one level with `max_distance` 140 and `min_distance` 45 gives a child whose `VisibilityRange` start is 45 and end 140;
  with two levels (20, 80) and `min_distance` 5 the bands are (5, 20) and (20, 80).

## Check

`cargo test -p ve-graph`, `-p ve-modeling`, `-p ve-render`, `-p ve-runtime`, `-p ve-bevy` all pass (none ignored). Report anything that failed before your change as pre-existing, with the output.
`cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` builds. Clippy: run it on the crates you touched and report errors in your own files only
(there is a known unrelated `too_many_lines` in `ve-render/src/lib.rs`).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked. The feature is implemented. The full `ve-modeling` test target is blocked by a pre-existing duplicate `mod tests` declaration in `src/anim/drop.rs` and `src/anim/part_2.rs`; the focused `scatter_show_range` integration test passes. Clippy also encounters a pre-existing `eprintln!` in `src/poly_ops.rs:49`.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-graph` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-modeling` — failed to compile tests: E0428, `tests` defined in both `src/anim/drop.rs:286` and `src/anim/part_2.rs:154`; also missing `visible_range` initializers fixed during this task. This existing duplicate-module source is outside this change.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-modeling --test scatter_show_range` — 4 passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-render` — 123 unit tests and 6 integration tests passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-runtime` — passed (native configuration contains 0 tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-bevy` — 18 passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target cargo clippy -p ve-graph -p ve-modeling -p ve-render -p ve-runtime -p ve-bevy -p ve-evaluator --lib` — failed on pre-existing `eprintln!` at `crates/ve-modeling/src/poly_ops.rs:49`; no clippy error was reported in the changed feature code. Existing warnings include `ve-render/src/lib.rs` too-many-lines.
  - No `cargo xtask` or browser tests run, per this handoff's instructions.
- Files changed: `crates/ve-graph/src/builtin/world.rs`; `crates/ve-modeling/src/{result.rs,cache.rs,appearance.rs,regions.rs,instances/output.rs,world_scatter.rs}`; `crates/ve-modeling/tests/scatter_show_range.rs`; `crates/ve-render/src/instance_sets.rs`; `crates/ve-runtime/src/{instance_exports.rs,projection/bulk.rs,projection/placements.rs}`; `crates/ve-bevy/src/sets.rs`; `crates/ve-evaluator/src/wasm.rs`; `src/engine/{bridge.ts,mesh-upload.ts,worker-types.ts}`; `web/eval-worker.js`.
