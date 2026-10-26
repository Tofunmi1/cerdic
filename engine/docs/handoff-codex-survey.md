# Handoff: `ve_world::survey` and the `survey` tool (ADR 0070 port 1)

Idea from Trompe's camera planner (`engine/third_party/trompe/src/core/planner.ts` when the vendor job has landed; the staging copy is `/private/tmp/claude-501/-Users-mac-Work-victoria-vr-hack/9d098cda-d740-48c5-ac1e-a0a723c575c1/scratchpad/ext/trompe/src/core/planner.ts`, Apache-2.0): sample where a person can stand, build candidate cameras, keep greedily the cameras that see the most of the surface well (a set cover). Ours runs on plain geometry with ray casts, not GPU probe renders. Read the planner first, then write our version; do not translate it line by line.

## Part 1: the module (tests already written, do not weaken)
`crates/ve-world/tests/survey.rs` (11 tests; if one looks logically wrong, say so in Status). New module `crates/ve-world/src/survey.rs` (a module of `ve-world`, not a crate; split into `survey/` files under 400 lines each), exported as `ve_world::survey`:
```rust
pub type Triangle = [[f64; 3]; 3];
pub struct SurveyOptions { pub eye_height: f64 /*1.65*/, pub spacing: f64 /*4.0 m between standing points*/, pub fov_deg: f64 /*90*/, pub yaws: usize /*8 headings per point*/,
  pub pitch_deg: f64 /*6*/, pub max_views: usize /*12*/, pub target_coverage: f64 /*0.95*/, pub max_distance: f64 /*30 m*/, pub samples_per_m2: f64 /*2.0*/, pub seed: u64 /*7*/ }
impl Default for SurveyOptions { .. }
pub struct SurveyCamera { pub position: [f64; 3], pub target: [f64; 3], pub label: String }
pub struct SurveyPlan { pub cameras: Vec<SurveyCamera>, pub coverage: f64, pub predicted: Vec<f64>, pub standable_points: usize, pub surface_samples: usize }
pub fn plan_cameras(triangles: &[Triangle], options: &SurveyOptions) -> SurveyPlan
```
Behaviour:
- **Standable points**: sample the up-facing triangles (geometric normal y >= 0.7) by area (about `samples_per_m2 / 2` per m2, deterministic from `seed`); keep a point when nothing is within `eye_height + 0.3` above it and the floor is the first thing under the eye (ray from the eye straight down hits at `eye_height` +- 0.1); thin by Poisson-disk to `spacing` (same x,z within `spacing` and y within 2 m clash); drop points that are not on the main connected walkable areas only if that is cheap, otherwise skip it (state in Status).
- **Candidates**: per point `yaws` headings at `pitch_deg`, fov `fov_deg`, standing at the eye position.
- **Surface samples**: all triangles sampled by area at `samples_per_m2` (floors and walls alike, any facing). A sample is seen by a candidate when it is inside the fov, within `max_distance`, not behind a surface (ray cast against the triangles; use a grid or BVH, not brute force beyond a few thousand triangles) and on the side facing the camera (two-sided: a wall is seen from whichever side the camera is on). Quality = |cos(angle between normal and view)| / distance.
- **Greedy cover**: reachable = samples seen by any candidate (that is the denominator of `coverage`); repeatedly keep the candidate that adds the most quality-weighted new coverage (a sample counts as covered when it has been seen at >= 50% of its best possible quality or at >= 0.12 absolute); stop at `max_views`, at `target_coverage`, or when nothing adds. `predicted[i]` is the coverage after camera i; `coverage` is the last predicted value (0 when no cameras).
- **Aerial fallback**: when there are no standable points, return four to eight cameras on a ring around the geometry's bounding box centre (radius 1.2 x the box diagonal, height at the box top + 0.5 diagonal / 3, looking at the centre), labelled `aerial_1`...; coverage computed the same way.
- Labels of standing cameras are `stand_1`, `stand_2`, ... in order. `target` is the position plus the unit look direction. Everything deterministic from the options. No unwrap/expect/panic; no new crates.

## Part 2: reach it from the tools (after part 1 passes)
- `ve-runtime`: an export `survey_plan(options_json: &str) -> String` next to the other document exports (find where `measure` gets the evaluated meshes in world coordinates and reuse that: all drawn parts' triangles, instances realized as their transforms if cheap, otherwise skipped with a `skipped_instances` count in the answer). JSON in: any of the option names; JSON out: `{ cameras: [{position, target, label}], coverage, predicted, standable_points, surface_samples, triangles, ms }`.
- MCP tool `survey {eye_height?, spacing?, max_views?, yaws?, target_coverage?}` relays to the page, which calls the export; the answer is the JSON above plus `snapshot_args`: the `cameras` array trimmed to the first 6 in the shape `snapshot` takes (`{position, target, label}`), so a model can pass it straight to `snapshot`. Also `snapshot {tour: true | {max_views, ...}}` runs the survey and renders its cameras in sheets of up to 6 (one picture per sheet, up to 2 sheets). Validate arguments in the server before asking the page (numbers finite and in range; error says the range). Keep tool definitions short (total under 13,000 characters: `bun tools/tool-surface-test.mjs`); put the detail in a guide topic `survey` and mention it in the server `instructions` in one sentence (replace a less important sentence to stay under 2,500 characters; `bun tools/orientation-test.mjs` must still pass).
- Do not rebuild wasm; Claude does after review.

## Rules
- AGENTS.md rules (no unwrap/expect/panic in library code, `#[expect]` not `#[allow]`, docs on public items, files under 400 lines, tests for behaviour). No `cargo xtask`, no `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-world` (the new test file and everything else), `cargo check -p ve-runtime --target wasm32-unknown-unknown`, the JS tool tests you can (`bun tools/tool-surface-test.mjs`, `bun tools/orientation-test.mjs`; the sandbox may refuse ports: say so), `cd web && bun test`, `bunx tsc -b --pretty false` (repo root).

## Status
Part 1 is complete and was left unchanged: `ve_world::survey` delivers deterministic standing/headroom sampling, accelerated visibility tests, greedy coverage planning and aerial fallback. Its 11 survey tests pass. Disconnected walkable areas are retained; no main-area filtering was added.

Part 2 is complete, building on the wiring already present in the workspace:
- `ve-runtime` exports `survey_plan`, validates all option fields, and returns cameras, coverage/predictions, sample/triangle counts, elapsed milliseconds and `skipped_instances`. Geometry is retained at projection commit, so rejected/staging geometry does not replace the survey scene. Ordinary drawn triangle meshes are included; instances use their authored world transforms within a 200,000 instance-triangle budget, counted by whole logical placement across prototype chunks. Live animation/physics offsets are outside this snapshot.
- MCP `survey` relays through the page and returns directly usable `snapshot_args` containing at most six cameras. `snapshot` tours capture at most twelve cameras in two labelled sheets, restore the camera on capture failure, and now expose the failure reason. Empty-scene tours return a useful error without moving the camera.
- Shared server/page validation rejects non-finite, out-of-range and conflicting survey/tour arguments before page calls. The `survey` guide covers defaults, ranges, coverage semantics, instance limits and capture examples; one server instruction sentence points to it. Tool definitions total 11,563 characters (each below 1,300); server instructions are 1,037 characters.
- Added MCP regression checks for the survey schema, guide, instruction, numeric ranges, tour limits/conflicts and page relays, plus an empty-scene tour test and capture-failure reason assertion.

Verification used `CARGO_TARGET_DIR=/tmp/codex-target-survey` for every Cargo command:
- PASS: `cargo test -p ve-world --test survey` — 11 passed.
- PASS: `cargo check -p ve-runtime --target wasm32-unknown-unknown` (existing warnings in unrelated code).
- PASS: `cd engine/web && bun test` — 37 tests; root `bunx tsc -b --pretty false`; root `bun test tests/guide-topics.test.ts` — 8 tests.
- PASS: `bun tools/tool-surface-test.mjs` and `bun tools/orientation-test.mjs` with a temporary `/tmp/codex-survey-stdio-only.mjs` preload that disables only `Bun.serve`. Direct runs could not open their local listeners (Bun reported `EADDRINUSE` on 8792 and 8794). The stdio schemas, guides, validation and no-page relay checks passed; a live browser/WebSocket tour remains unverified.
- BLOCKED: unfiltered `cargo test -p ve-world` fails compiling the pre-existing `tests/rooms_layout.rs`, which imports the absent `ve_world::rooms` module. This unrelated module/test was not changed. All other library/integration tests were run separately: 168 passed and 5 timing tests were ignored, in addition to the 11 survey passes above; doc-tests pass (zero examples).

No wasm bundle was rebuilt, no `cargo xtask` or `cargo fmt` was run, and nothing was staged or committed. The strict gate and rebuilt-wasm browser review remain pending for the owner.
