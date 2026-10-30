# Handoff: the water simulation does not visibly evolve in the browser (bug report and fix)

Follows `docs/handoff-codex-water-sim.md` (done: 14 physics tests, nodes, runtime, shader, `water.disturb`, `water.height`). Claude ran it live in a headless Chrome on a rebuilt viewer with `web/dist-bevy-gpu/worlds/sim_test.txt` (a `water.fill` lake with `sim=true`, `sim_resolution=128`, `sim_damping=0.4`, in a flat 100 x 100 tile dished at (50, 50)) and found:
- `water.disturb {x: 50, z: 50, radius: 1.2, amount: 0.12}` answers ok and the splash burst appears (flat disc particles) but no ripple rings spread on the surface;
- `water.height {x: 55, z: 50}` returned 0.013540183193981647 both 0.5 s and 2 s after the poke (identical to 15 digits), and `water.height` at the centre right after a +0.12 poke was NEGATIVE (-0.0116);
- two snapshots 9 seconds apart of the still lake were pixel-identical (`changedFraction` 0), so even the procedural wave animation did not advance between snapshots;
- the results are bit-identical between two runs of the whole script.
So either the animation clock does not advance in that page (or the snapshot tool leaves it paused: see `src/engine/snapshot.ts`, `hooks.seek/play`, and `crates/ve-runtime/src/animation/part_1.rs` where `material.water_time = clock.seconds`), or the simulation is not stepping, or `water.height`/the shader read something other than the live simulation, or the simulation was built from a footprint with no water cells. Find out which, fix it, and make it testable.

## Tests (already written, do not weaken)
`tools/water-sim-live-test.mjs` (`bun tools/water-sim-live-test.mjs` from `engine/`; it needs a browser and the static server on 127.0.0.1:8801, so Codex cannot run it: Claude does). It needs two additions to the runtime that you build:
1. A world-API method `water.status {}` returning `{parts: [{name, resolution, cell, steps, time, max_abs, volume, wet_cells}]}` for every simulated water part (steps = simulation steps taken since the sim was built, time in sim seconds, `max_abs` the largest absolute displacement in metres, `volume` the sum of displacement times cell area, `wet_cells` the number of water cells; empty list when no part simulates). Add it to the `world_api` guide topic.
2. `water.disturb` takes `splash: false` to skip the splash burst (default true).
Rust tests for both (parameter parsing, the status shape, `splash` false requests no burst).

## What to find and fix
- Whether the viewer's animation clock advances when nothing is playing, and whether `snapshot` pauses it and leaves it paused. A water simulation must step with real time in the viewer (a lake keeps moving), unless the user paused the clock. If snapshots pause the clock for deterministic motion sheets, restore the previous run state afterwards. Do not break `snapshot {times, states}` behaviour (their tests are in `tests/snapshot-diff.test.ts`, `web/tests`).
- `water.height` must return the procedural swell plus the live simulated displacement at that point and time.
- The shader must show the simulated displacement and slope (texture upload once per frame, `R32Float`). Check that the water texture bind group is actually bound for sim water and that the texture is updated when the version changes; add a unit test around the version/upload logic if it lacks one.
- A disturbance applied by `water.disturb` before the first step, between steps, and while the page was just loaded must all work.
- Splash discs: leave as they are.

## Rules
- AGENTS.md rules. No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging. Edit files with your edit tools. Do NOT rebuild wasm; Claude does after review.
- Run `cargo test -p ve-world --test water_sim`, `cargo test -p ve-render`, the `ve-modeling` water tests (`--test water --test water_fill`), `cargo check -p ve-runtime --target wasm32-unknown-unknown`, `cd web && bun test` (repo `web/`), and `bunx tsc -b --pretty false` at the repo root.

## Status

Implemented, with native regression coverage. Follow-up browser verification and one runtime fix are recorded below.

Findings:
- The idle viewer clock already defaults to playing and advances from `Time<Real>`; the native test now runs the actual clock and simulation systems together and verifies this, including explicit pause/resume and speed control.
- `water.fill` and river sheets carry `finish.water` with `recipe=None`. The old animation updater only assigned `water_time` to `recipe=Water`, leaving these sheets' material time at zero. It now updates both forms of water.
- The live test uses `dist-bevy-gpu`. Its Bevy water material had neither a simulation texture binding nor simulated displacement/slope; the existing R32Float upload and wave shader only served the custom forward renderer. Bevy now receives shared, versioned R32Float images, samples displacement in the vertex shader and slope in the fragment shader, and uses the same seeded swell table/time as `water.height`. Image updates run once per changed version before asset events; the handle stays stable until dimensions change. Water material cache keys exclude time, preventing a new cached material every frame.
- Ordinary snapshots do not pause the clock. Motion sheets with `times` did leave it paused. Motion sheets now restore camera, previous time and playing state in `finally`, including capture errors. Restoration uses one atomic clock request, since separate seek/play requests would overwrite each other in the runtime queue.
- The native footprint fixture has 1,024 wet cells, accepts a positive poke before its first simulation frame and between steps, changes live sampled heights, and holds while paused. A negative total `water.height` alone does not establish a negative applied disturbance: the result includes signed procedural swell.

Added `water.status {}` with all requested fields (empty `parts` when no water simulates), strict boolean parsing for `water.disturb.splash` (default true), suppression of the burst for `splash:false`, and world_api guide documentation. Requests initialize/synchronize the grid before querying or forcing it, including a request before the first simulation update. Splash particle rendering is unchanged.

Passed with `CARGO_TARGET_DIR=/tmp/codex-target-simlive`:
- `cargo test -p ve-world --test water_sim` (14 tests) and `cargo test -p ve-modeling --test water --test water_fill` (19 tests; run together in one Cargo invocation).
- `cargo test -p ve-render -p ve-bevy` (all existing renderer tests plus the new version/upload regression).
- `cargo test -p ve-runtime --test water_live` (3 native tests using the actual browser clock/simulation sources).
- `cargo test -p ve-bevy --test water_shader` (composes the actual water vertex/fragment shader with Bevy's real imports and validates the resulting WGSL using Naga).
- `cargo check -p ve-runtime --target wasm32-unknown-unknown`, also with `--features bevy-webgpu`.
- `cd engine/web && bun test` (40 tests), `bun test tests/snapshot-diff.test.ts` at repository root (5 tests), and root `bunx tsc -b --pretty false`.

Existing unrelated warnings remain in geometry, modeling, toon exports and runtime exports. The strict gate is pending.

## Follow-up verification — 2026-10-27

The browser run confirmed the root cause: the water entity spans multiple render chunks, and ECS query order changed between frames. Since the simulation signature flattened chunks in query order, it changed every frame and rebuilt the grid, resetting steps and discarding disturbances. Sort each water part's chunks by entity ID before signature construction and rasterization. `water.status` includes the part ID so this regression is directly testable; unchanged geometry must keep the same ID.

`water.height` now reports `height` (the existing combined result), `simulation` (the sampled live displacement), and `wave` (the procedural swell), so a simulation test can distinguish a spreading ripple from ordinary animated waves. The live test checks the simulated component 5 m from the poke. It uses a 2 mm minimum because the tested 0.36 m impact produced about 2.8 mm of simulated displacement there after two seconds.

Rebuilt the `web-dev` WASM runtime with the workspace-pinned `wasm-bindgen` 0.2.129 and refreshed `web/dist-bevy-gpu`. `bun tools/water-sim-live-test.mjs` passed all checks in headless Chromium, including stable part identity, at least 30 sim steps per second, still-water baseline, disturbance, propagated height, volume, damping, rendered snapshot change and no page errors. The close camera and wider second impact make the rendered-ripple check resolve the water rather than the whole 100 m landscape. Native `cargo test -p ve-runtime --test water_live` also passed (3 tests). This verifies the browser renderer path, but not an actual headset session.
