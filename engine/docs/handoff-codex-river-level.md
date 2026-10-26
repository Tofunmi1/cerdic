# Handoff: a river whose water runs downhill (`carve_river_descending`, `river.carve level="descending"`)

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-world` (`src/river/stream.rs` or a new `src/river/descending.rs`, the export in `src/river.rs`), then `ve-graph` (the `river.carve@1` spec in `builtin/world.rs`), then `ve-modeling` (`src/world_river.rs`).** Do not touch `ve-render`, `ve-bevy`, `ve-runtime`, `ve-evaluator`, `tests/river.rs` (finished: the old behaviour stays and must keep passing), `src/material_presets.rs`.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-river`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone and fix clippy findings in your own code. `ve-world` stays pure arithmetic and deterministic (no `powf`/trig that differ between native and wasm in anything that decides the result; `river/stream.rs` already has a polynomial `tangent`). In `ve-modeling` integration tests only: run them with `--test <name>` (the lib test target may not compile for reasons that are not yours).

## Why (found 2026-10-25 by reading `river/stream.rs`, confirmed by the owner's screenshot of `canyon_v2`)

`carve_river` sets each cell's water height to `bank_top - water_level_offset`, where `bank_top` comes from the ground height at (or beside) that very cell. On terraced or ridged ground (steps of tens of metres) the water surface therefore steps up and down with the terrain: a translucent sheet climbing the cliffs. The bed is cut the same way, so the channel floor is not a descending valley either. The flow speed is `depth * 0.2 + a small slope term`, not derived from the water surface. A real river's surface only falls along its course, is level across its width, and sits a fixed depth above its bed. The old behaviour was chosen so tiles of one world agree (`tests/river.rs` `a_river_depends_only_on_the_ground_near_it_so_tiles_agree`), so it must stay as `level="local"` and remain the default; the new mode is a whole-path profile like `carve_road` (read `river/road.rs`: profile along the path, smoothing, grade limit) and, like a road, must be given the whole path on the whole grid.

## What to build

1. **`ve-world`: `pub fn carve_river_descending(ground: &mut Ground<'_>, path: &Path, profile: &RiverProfile, cancel: &dyn Fn() -> bool) -> Result<RiverOut, RiverError>`**, exported from `river.rs` next to `carve_river`. Same validation and errors as `carve_river` (reuse its checks: fewer than 2 points or none inside the grid is `BadPath`, bad parameters `BadParameter`, `Cancelled`). `meander_flatten` is ignored in this mode (say so in the doc). Algorithm, in your own words but with these properties:
   - Resample the path as `carve_river` does. For each sample take the reference height = the original ground height at the path point; the target water level = reference - `water_level_offset`.
   - Make the level only ever fall along the path (running minimum from the first sample), smooth it with a wide window (about six channel widths of path length) and re-impose the running minimum, then limit the fall to 5 percent (`0.05` m per metre of path) by lowering the *upstream* side where the ground steps down faster than that (so a river that meets a drop cuts a gorge upstream instead of falling off a cliff).
   - Water depth is `depth - water_level_offset` (at least 0.1 m). Bed = level - water depth.
   - For each cell within `width/2 + bank_width` of the path (nearest sample, as `carve_river` does): the water height is the level of that sample (so it is constant across the width), the channel floor (within `width/2`) is cut to the bed, and outside it the ground is cut to `min(original, bed + (distance - width/2) * tan(bank_slope))`; never raise the ground outside the channel floor; beyond the banks nothing changes. `channel_mask` and `bank_mask` as `carve_river` defines them; `water_height` off the channel is the ground height (the existing convention).
   - Flow: `flow_dir` is the path direction inside the channel (zero outside). `flow_speed` inside the channel is `0.3 + 12 * grade + 0.05 * water depth` m/s with `grade` the local fall of the level per metre of path, at most 8 m/s; zero outside.
   - Deterministic, cancellable (poll `cancel` per row as `carve_river` does), bounded memory (a few vectors the size of the path samples).
2. **`ve-graph`: `river.carve@1` gets a parameter `level`**: choice `"local"` | `"descending"`, default `"local"`. Doc: local follows the ground beside each cell and lets tiles of one world agree; descending makes the water fall steadily along the whole path and sit level across the river, which is right for terraced, ridged or mountainous ground and for lakes in valleys (build it on the whole grid). Parameter names are read with `Param::as_f64` and strings with `Param::Str`; refuse unknown choices with a typed diagnostic. Whole numbers arrive as integers in saved documents (a slider once failed in the browser for that reason).
3. **`ve-modeling`: `world_river.rs` calls `carve_river_descending` when `level` is `"descending"`** and `carve_river` otherwise, and stores the same layers (`channel`, `bank`, `water_height`, `flow_x`, `flow_z`, `flow_speed`). `river.water` needs no change.

## Tests (written by Claude; never weaken)

- `crates/ve-world/tests/river_descending.rs`: 10 tests on a stepped grid (a 12 m ridge across the river and a 20 m step): water only falls (both directions along the path), level across the width, bed a fixed depth below the surface, never above the ground, fall at most 5 percent, nothing touched outside the banks or raised beside the channel, flow along the path and faster where steeper, deterministic, errors and cancellation, close to the old behaviour on smooth ground.
- `crates/ve-modeling/tests/river_level.rs`: 5 tests through the node on terraced ridged ground: default unchanged, descending never climbs, level across the width, the old mode really does climb on this fixture (so the test means something; if it does not, say so in your Status: the fixture needs steeper terraces, do not weaken), unknown level refused.
- `crates/ve-world/tests/river.rs` and the `g12_world` tests in `ve-modeling` must still pass unchanged.

## Check

`cargo test -q -p ve-world --test river_descending --test river`, `cargo clippy -q -p ve-world --all-targets -- -D warnings`, `cargo check -q -p ve-world --target wasm32-unknown-unknown`, `cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test river_level --test g12_world`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked: implementation is complete; two package checks remain red on unrelated existing findings below.
- Algorithm used, in three lines:
  - Resample original ground along the path, make a running-minimum water profile, smooth it over six channel widths, then reapply the minimum and 5% grade limit.
  - Cut a fixed-depth bed beneath the level profile, shape banks without raising terrain, and keep water height at ground level outside the channel.
  - Derive flow direction from the path and speed from local profile grade; poll cancellation per grid row. Smoothing uses a sliding window with linear work in sample count.
- Tests run and their output:
  - `cargo test -q -p ve-world --test river_descending --test river`: passed, 10 descending and 8 existing river tests.
  - `cargo clippy -q -p ve-world --all-targets -- -D warnings`: failed on existing findings in `building.rs`, `flora/simplify.rs`, and `flora/colonize/grid.rs`; no findings in the new river code.
  - `cargo check -q -p ve-world --target wasm32-unknown-unknown`: passed.
  - `cargo test -q -p ve-graph`: passed all test targets.
  - `cargo test -q -p ve-modeling --test river_level`: passed all 5 tests.
  - `cargo test -q -p ve-modeling --test river_level --test g12_world`: `river_level` passed when run separately; `g12_world` had 19 passed and 1 unrelated failure, `species_differ_and_levels_of_detail_halve_the_triangles` (`69416 then 55442 then 11598`).
  - `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`: passed.
- Files changed: `crates/ve-world/src/river/stream.rs`, `crates/ve-world/src/river.rs`, `crates/ve-graph/src/builtin/world.rs`, `crates/ve-modeling/src/world_river.rs`. Supplied tests were not edited.
