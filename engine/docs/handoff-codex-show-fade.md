# Handoff: cross-fade at the edge of a camera-distance band (`scatter.place` show_fade)

Owner: Claude manages; Codex implements. Follows `handoff-codex-show-range.md` (done). Crates you may edit, in this order, one at a time with that crate's tests before moving on: `ve-graph`,
`ve-modeling`, `ve-render`, `ve-runtime`, `ve-bevy`, plus the evaluator wasm glue and TypeScript files the previous round touched (`crates/ve-evaluator/src/wasm.rs`,
`src/engine/{bridge,mesh-upload,worker-types}.ts`, `web/eval-worker.js`). **Do not touch `ve-measure`** (another helper is working there). Rules: never run `git checkout`, `git clean`,
`git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; own `CARGO_TARGET_DIR` (for example `/tmp/codex-target-fade`);
do not commit or stage; edit with your edit tools, not scripts. The sandbox cannot run the browser; Claude does that. Note: `ve-modeling`'s lib test target currently fails to compile because of a duplicate
`mod tests` in `src/anim/drop.rs` and `src/anim/part_2.rs` (not yours, leave it); test with `--test <file>` integration targets there.

## Why

`scatter.place` has `show_from` / `show_to`: a set of copies is drawn only while the camera is inside that band. At the band edge a tree switches abruptly to the next mesh (popping).
Bevy's `VisibilityRange` can fade over a margin: `VisibilityRange { start_margin, end_margin, use_aabb }` where a margin is a range of distances over which the entity cross-fades
(dithered) in or out. Today `ve-bevy/src/sets.rs` uses `VisibilityRange::abrupt(start, end)`.

## What to build

1. **ve-graph**: `scatter.place@1` gets `show_fade` (number, metres, default 0, 0 to 200; doc: "Width of the cross-fade at each end of the band, in metres; 0 switches abruptly. Neighbouring bands should use the same fade and meet in the middle of it.").
2. **ve-modeling**: `Part.visible_range` stays `Option<(f32, f32)>` for compatibility; add `pub visible_fade: f32` (`#[serde(skip)]`, default 0, update every `Part { .. }` initializer) and the matching cache `Value` field; carry it like `visible_range` (including through
   `inst.vary`, every material region, and the budget `drawn_copies` stays as is). Refuse `show_fade` larger than half the band width when `show_to` is set (typed diagnostic naming `show_fade`).
3. **ve-render** `VeInstanceSet`: `pub fade: f32` (0 by default; Bevy path only).
4. **ve-runtime**: pass it to the set (bulk and per-entity paths) and through the instance export / TypeScript bridge exactly as the range travels (`eval_visible_range` returns `[from, to]`; add `eval_visible_fade(part) -> f32` or extend the vector, your choice).
5. **ve-bevy** `sets.rs`: when `fade > 0`, level ranges become cross-fades centred on the band edges: a band `[s, e)` with fade `f` is `VisibilityRange { start_margin: (s - f/2)..(s + f/2), end_margin: (e - f/2)..(e + f/2), use_aabb: false }`, except that a start of 0 and an
   infinite end stay hard (no margin there). Check that Bevy applies the dither cross-fade for the materials this crate uses (`StandardMaterial`, and `ToonMaterial`, `DetailMaterial`, `TerrainMaterial`, `WaterMaterial` in this crate): look in `third_party/bevy` for how the visibility
   range crossfade is wired in the mesh pipeline and say in the Status which of our custom materials will fade and which will not (they would then just switch at the middle of the margin; that is acceptable but must be stated).

## Tests (fixed, written by Claude; never weaken; add more if you like)

- `crates/ve-modeling/tests/scatter_show_fade.rs` (copy the helpers of `tests/scatter_show_range.rs`): `show_fade=10` with `show_from=40, show_to=130` gives `visible_fade == 10.0` on the part and on every material region of a `flora.tree` shape; no param gives 0.0; `show_fade=60` with the same band is refused naming `show_fade`
  (band width 90, half is 45); `inst.vary` keeps the fade; plant positions do not change with the fade.
- `crates/ve-bevy/src/sets.rs` tests: a set with levels `max_distance` 40 and 130, `min_distance` 0, `fade` 10: the first child has `start_margin` 0..0 and `end_margin` 35..45; the second has `start_margin` 35..45 and `end_margin` 125..135; with `fade` 0 both are abrupt as before.
- Keep every existing test passing: `cargo test -p ve-graph`, `-p ve-render`, `-p ve-runtime`, `-p ve-bevy`, and the focused `ve-modeling` targets (`--test scatter_show_range --test scatter_show_budget --test scatter_show_fade`).
- `cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` builds.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked. Implementation and required Rust/wasm checks passed. The optional `bun run typecheck` still reports seven unrelated existing TypeScript errors (texture program nullability in `src/engine/mesh-upload.ts:67`, DOM iterable support in `snapshot.ts`, and stale test expectations in `shader-expr.test.ts` and `surface-signals.test.ts`).
- Which of our custom materials cross-fade: `StandardMaterial` uses Bevy's stock `pbr.wesl`, which calls `visibility_range_dither`. `ToonMaterial`, `DetailMaterial`, `TerrainMaterial`, and `WaterMaterial` use custom fragment shaders that do not call it; they switch abruptly at the midpoint of the fade margin.
- Tests run and their output:
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo test -p ve-graph` — passed (44 unit tests and all integration/doc tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo test -p ve-modeling --test scatter_show_range --test scatter_show_budget --test scatter_show_fade` — passed (12 tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo test -p ve-render` — passed (123 unit tests, 6 integration tests, doc tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo test -p ve-runtime` — passed (unit/doc tests; no tests defined).
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo test -p ve-bevy` — passed (20 unit tests, doc tests).
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` — passed.
  - `CARGO_TARGET_DIR=/tmp/codex-target-fade cargo check -p ve-evaluator --target wasm32-unknown-unknown` — passed.
  - `bun run typecheck` — failed with the unrelated pre-existing errors listed above.
  - Browser tests were not run here; the handoff assigns them to Claude. `cargo xtask` was not run per the handoff instruction; the strict milestone gate remains pending per repository policy.
- Files changed: `crates/ve-graph/src/builtin/world.rs`; `crates/ve-modeling/src/{result.rs,cache.rs,appearance.rs,instances/output.rs,ao.rs,emitters.rs,budget.rs,world_scatter.rs}`; `crates/ve-modeling/tests/scatter_show_fade.rs`; `crates/ve-render/src/instance_sets.rs`; `crates/ve-runtime/src/{projection/placements.rs,projection/bulk.rs,instance_exports.rs}`; `crates/ve-evaluator/src/wasm.rs`; `crates/ve-bevy/src/sets.rs`; `src/engine/{bridge.ts,mesh-upload.ts,worker-types.ts}`; `web/eval-worker.js`.
