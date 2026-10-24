# Handoff: distance levels of detail for instance sets in the Bevy path (ve-bevy)

Owner: Claude manages; Codex implements. Crate you may edit: `engine/crates/ve-bevy` only (and its Cargo.toml). Do not touch any other crate, do not run
`git checkout`, `git clean`, `git stash` or `git restore`, never run `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`), never
run `cargo xtask`. Use your own `CARGO_TARGET_DIR` (for example `/tmp/codex-target`). Do not commit or stage. Edit files with your edit tools, not scripts.

## Why

A forest world has about 4.8 million triangles because every tree copy is drawn with its nearest level of detail. The evaluator already builds several
levels per instance set (`VeInstanceSet.meshes`, nearest first, each `SetMesh { mesh, max_distance }`, the last distance is where the set stops being drawn,
infinity means never). `crates/ve-bevy/src/sets.rs` `map_sets` spawns each copy with `set.meshes.first()` only. Bevy has `VisibilityRange`
(`third_party/bevy/crates/bevy_camera/src/visibility/range.rs`, plugin `VisibilityRangePlugin`): an entity is drawn only while the camera is within its range.
This is the same idea as Unreal's Nanite distance selection, done with stock Bevy and no 64-bit GPU atomics, so it runs in WebGPU and on Quest.

## What to build

In `map_sets`, spawn one child per copy **per level**: level `i` covers camera distances `[start_i, end_i)` where `start_0 = 0`, `start_i = max_distance of level i-1`,
`end_i = max_distance of level i` (infinity stays infinity). Give each child `VisibilityRange::abrupt(start_i, end_i)` (check the exact constructor in the
vendored source; level 0 has start 0). A set with one level must behave exactly as now plus a range `abrupt(0, max_distance)` (infinite: no component or an infinite range, your choice, but
state which and why). Make sure `VisibilityRangePlugin` is added by the ve-bevy plugin that owns sets (add it only if the app does not already have it; look how
other ve-bevy plugins register Bevy plugins and avoid adding it twice). Keep `share_set_material` working for all the children. Copies drawn at a level must
still carry the same `Transform` and `ChildOf(entity)`.

Also update the doc comment at the top of `sets.rs` (it says level of detail is not carried over).

## Tests (written by Claude in advance, you may add more but never weaken these): `engine/crates/ve-bevy/src/sets.rs` `#[cfg(test)] mod tests`

Add this module (or equivalent in a `tests/` file if the crate layout needs it) and make it pass:

1. `three_levels_make_three_children_per_copy`: a world with a `VeInstanceSet` of 2 transforms and 3 levels with `max_distance` 20, 80, infinity; after running `map_sets` once there are
   exactly 6 child entities (`ChildOf` the set).
2. `levels_cover_consecutive_distance_bands`: for the children of the first copy, the `VisibilityRange` components, sorted by `start_margin.start`, have (start, end) bands
   equal to (0, 20), (20, 80), (80, infinity) (compare with the fields of `VisibilityRange` you find in the vendored source: `start_margin` and `end_margin` ranges).
3. `a_single_level_set_is_unchanged`: one level, 5 transforms -> 5 children, each with the mesh handle of that level.
4. `replacing_the_set_replaces_the_children`: after changing the component (set `transforms` to 1 copy) and running again, the old children are despawned and
   only `levels` children exist.

Use real mesh handles (`Handle::default()` or `Assets<Mesh>` as the crate's other tests do) so the tests do not need a GPU. If `map_sets` needs resources the test cannot provide, split out
the pure spawning logic into a function the test calls, but keep `map_sets` as the system.

## Check

- `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-bevy` passes (all tests, none ignored).
- `CARGO_TARGET_DIR=/tmp/codex-target cargo clippy -p ve-bevy --all-targets -- -D warnings` clean for the files you touched.
- `CARGO_TARGET_DIR=/tmp/codex-target cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` builds (about 6 minutes; the sandbox cannot run
  the browser, so Claude does the browser check).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked. Implementation is complete. Clippy remains blocked by the existing `clippy::too_many_lines` error in `engine/crates/ve-render/src/lib.rs:add_render_systems` (108 lines); that file is outside the allowed edit area.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target cargo test -p ve-bevy` — passed, 17 passed, 0 failed, 0 ignored. `CARGO_TARGET_DIR=/tmp/codex-target cargo clippy -p ve-bevy --all-targets -- -D warnings` — failed on the `ve-render` lint above, before completing `ve-bevy` checks. `CARGO_TARGET_DIR=/tmp/codex-target cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` — passed.
- Files changed: `engine/crates/ve-bevy/src/sets.rs`; this handoff Status section. Each copy now has a child per LOD with abrupt consecutive visibility ranges. A single-level infinity range is explicit (`abrupt(0, infinity)`) so it remains visible at every distance. `VisibilityRangePlugin` is registered by `VeBevySetsPlugin` only when not already added.
