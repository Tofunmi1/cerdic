# Handoff: survey modes (walk, orbit, auto): do not stand on roofs

Found live (2026-10-26): `survey` on the cottage (walls and a roof, no ground) put every camera standing on the roof slopes looking at the sky, because the roof faces are up-facing and have headroom. Trompe avoids this with an "enclosed" check (playable spaces are enclosed; rooftops drop out). Read `engine/third_party/trompe/src/core/planner.ts` (`enclosed`, `mainAreas`) and our `crates/ve-world/src/survey.rs` and `survey/`.

## Tests (already written, do not weaken)
`crates/ve-world/tests/survey.rs`: the existing 11 plus 4 new ones at the end (`an_object_with_no_ground...`, `walk_mode_still_stands...`, `walk_mode_on_a_floating_object...`, `a_small_standable_patch...`). If one looks logically wrong, say so in Status with numbers.

## Build
- `SurveyMode { Auto, Walk, Orbit }` in `ve_world::survey`, `SurveyOptions::mode` (default `Auto`, keep `Default` and every existing field).
- **Walk**: today's behaviour (standing cameras where a person can stand; the aerial ring only when there is nowhere to stand).
- **Orbit**: always the aerial ring (labels `aerial_N`): cameras around the geometry's bounding box centre at two or three heights (a low ring near the middle height and a high ring), looking at the centre, enough of them for the view budget; coverage computed as for standing cameras.
- **Auto** (default): Walk when the level offers a real place to walk, otherwise Orbit. A "real place to walk": the standable points, grouped into connected regions (points within about `spacing * 1.6` of each other at similar height, `|dy| < 0.6`), include a region whose area (points times `spacing^2`, or the area of its triangles) is at least 12 m2 AND whose height is within the lower 60% of the geometry's bounding-box height (or the box is under 6 m tall and the region is at its base); roof slopes and small patches on top of a block do not count. Points in regions that do not qualify are dropped before candidates are built (so a level with a floor AND a roof keeps only the floor regions). Where nothing qualifies, Auto behaves as Orbit.
- Update the plan JSON from `ve-runtime`'s `survey_plan` export (`mode` option: "auto" | "walk" | "orbit", invalid values are an error naming the choices; the answer includes the `mode` actually used), the MCP `survey` tool schema (a `mode` enum property; keep each tool definition under 1,300 characters and the total under 13,000: `bun tools/tool-surface-test.mjs`), the `snapshot {tour}` options, the `survey` guide topic (one short paragraph on modes) and `tools/tool-surface-test.mjs` (add a check that `survey` exposes `mode`; do not weaken the others).

## Rules
- AGENTS.md rules. No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging. Do not rebuild wasm; Claude does.
- Run `cargo test -p ve-world --test survey`, `cargo check -p ve-runtime --target wasm32-unknown-unknown`, `bun tools/tool-surface-test.mjs`, `bun tools/orientation-test.mjs` (the sandbox may refuse ports: say so), `cd web && bun test`, `bunx tsc -b --pretty false` (repo root).

## Status
Implemented 2026-10-26: `SurveyMode`, default Auto, connected-region filtering (1.6 × spacing, |dy| < 0.6, at least 12 m2, low bounding-box height), preserved Walk standing behavior, and Orbit with low/high rings using the full view budget. Plans report the mode actually used. Runtime JSON parsing, MCP survey/tour schemas, shared argument validation, the TypeScript plan, and the survey guide now support modes. Added mode assertions to the tool-surface checks, two web tests, and five separate Rust integration tests. The supplied `tests/survey.rs` is unchanged.

Checks used `CARGO_TARGET_DIR=/tmp/codex-target-octa`:
- `cargo test -p ve-world --test survey`: **14 passed, 1 failed**. The failing `walk_mode_on_a_floating_object_has_nowhere_to_stand_so_it_falls_back_to_the_ring_like_before` contradicts the requested preservation of today's Walk behavior: its cottage roof normals have y ≈ 0.781 and 0.842, above the existing 0.7 threshold, with clear headroom. Seed 7 leaves **3 standable roof points** (floor y = 2.626861, 2.664201, 2.686784), producing **5 standing cameras, coverage 0.902439**. Thus it does have somewhere to stand under the existing sampler. Auto rejects all three (the lower-60% cutoff is y = 2.52) and returns **8 aerial cameras, coverage 1.0**. Kept the test and Walk behavior unchanged rather than special-casing the fixture.
- `cargo test -p ve-world --test survey_modes`: **5 passed** (floor/roof filtering at translated heights, disconnected small-region removal, small-region fallback, orbit budget/heights/centre targeting, and Walk fallback with no upward faces).
- `cargo check -p ve-runtime --target wasm32-unknown-unknown`: **passed**, with existing warnings in unrelated code. This was a check only; no wasm bundle was rebuilt.
- `cd web && bun test`: **39 passed**, 759 assertions. Repo-root `bunx tsc -b --pretty false`: **passed**.
- `bun tools/tool-surface-test.mjs` and `bun tools/orientation-test.mjs`: **blocked by sandbox port binding**. Bun reported EADDRINUSE for ports 8792 and 8794; independent socket probes on those ports and an ephemeral port returned `Operation not permitted`. Stopped the waiting test processes. An offline evaluation of the actual tool definitions confirmed **11,681 characters total**, largest **1,068** (snapshot), and the exact auto/walk/orbit enums on survey and snapshot tour. The full MCP scripts still need a run where local ports are permitted.

No staging, commits, `cargo xtask`, `cargo fmt`, or wasm rebuild. The strict gate remains pending per the handoff's check limits.
