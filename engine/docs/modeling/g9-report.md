# G9 fidelity and AI progress report

Date: 2026-10-15. G9 is in progress. This report records the slice implemented in this workspace; it does not claim G9a, G9b or G9c complete.

## Implemented and evidenced

- **Planner estimates (A1):** chained booleans use bounded additive growth and position-only deformers preserve triangle estimates. Eight chained
  sphere cuts pass Preview; a 32 × 32 array and a dense sphere followed by three subdivision levels refuse in preflight. The gallery-wide
  estimated-versus-actual audit remains open.
- **Quality refinement (A2):** an accepted Preview automatically requests Final after 400 ms idle. A Preview `budget.triangles` refusal retains
  the staged candidate for Final; other failures still reject it. The editor shows a refining state, preserves Preview when Final fails, and
  retries the quality that failed after a worker crash. Browser acceptance passed the Preview/Final flow, the Preview-budget escalation, worker
  crash/timeout recovery, Preview retention after Final failure, a 100-request cache soak and large-result transfer checks. To keep this resource
  test representative, its repeated soak runs on the small baseline scene. The two 318-segment sphere cases each have 202,248 triangles: one
  exceeds the 200,000 Preview cap, while remaining below the 2,000,000 Final cap.
- **Concave subdivision (A3):** non-convex faces are triangulated before Catmull-Clark, with Point, Face and Corner provenance retained where
  supported. The evaluator reports `poly.subdivide.concave_face_triangulated`; convex cages keep their prior face flow. Geometry and real graph
  tests cover a closed concave heptagonal prism, convex quads and warning emission. Modeling parity includes the graph, and native/WASI output
  matches for all 38 modeling corpus lines.
- **Batch editing (A4):** the bounded graph batch is 1,000 commands / 1 MiB. The current 386-command study-room fixture is 43,465 encoded bytes, applies
  atomically and undoes in one action. Native `Session::apply` measured 88.7 ms in the focused run. This does not measure provider latency.
- **Study-room profile (A5):** local headless Chromium with SwiftShader staged the current 386-command scene. There were 193 evaluated nodes
  and 41 live meshes. The latest browser run measured 240 ms Preview worker time / 338 ms Final worker time, and 8.34 s / 19.77 s browser wall
  time to accepted Preview / Final. Final ended with zero staged meshes. Raw measurements and the Engine Viewer screenshot are saved under ignored
  `target/reports/g9/`. This is one large atomic batch, not a multi-batch cache study.
- **Viewer defaults and grading (B4):** the product-viewer FOV defaults to 45 degrees and the slider reaches 35 degrees. Viewer grading adds a
  bounded, renderer-only transform (exposure, contrast, saturation, tint) and five named presets, applied in the forward shader after
  tonemapping so windows and XR share one grade with no extra pass; neutral is the identity and every control clamps to documented ranges. The
  browser viewer test `web/tests/g9-grading.mjs` confirms preset selection reaches the renderer and saves `target/reports/g9/grading-*.png`. Bloom
  and depth of field remain unimplemented (deferred, not measured; see ADR 0023).
- **Authored and emissive lighting (B1):** `light.point@1`, `light.spot@1` and `light.sun@1` evaluate as typed outputs and are staged atomically
  with the geometry revision. The forward shader shades at most 16 points and 8 spots in High (2/2 Low, 4/4 Medium); spots are unshadowed and
  the 1,088-byte fixed uniform block is below WebGL2's minimum guaranteed limit. Lights are nearest-first with deterministic packed-value tie
  ordering. `solid.material@1(emits_light=true)` adds one point light at each output part's bounds centre, with intensity from surface area and
  emissive peak (capped at 20) and range from its bounds (clamped to 1–1,000 m). `cargo xtask quick ve-graph ve-modeling ve-render ve-runtime`
  passed (373 tests, clippy and rules). `cargo xtask web-test dev g9-study-room` passed under SwiftShader; the fixture reached one Final revision
  with one sun, one point, one spot and three derived emitters. Preview and Final were accepted in 4.30 s and 9.18 s wall time. The ignored
  screenshot is `target/reports/g9/study-room.png`. The screenshot is an integration check, not owner-scored B8 visual acceptance.
- **Baked ambient visibility (B2, partial):** deterministic cosine-weighted 8-ray Preview / 32-ray Final visibility is baked per prototype vertex
  against a median-split triangle BVH, then transferred from evaluator WASM through the worker and mesh upload as vertex attribute 2. The
  forward shader multiplies ambient and sky terms only; direct lights and mesh geometry are unchanged. One isolated planar part is assigned
  full visibility without a BVH. A covered-floor unit case darkens while an open floor stays lit, the >1,000,000-triangle refusal is tested as
  `budget.ao_triangles`, and AO is included in modeling native/WASI parity (38 lines identical). A one-scene AO cache keys all output positions,
  indices, part boundaries and quality; unchanged scenes reuse per-part channels, while geometry or quality changes rebake. It stores at most
  32 MiB of channels and otherwise skips retention. On an Apple M1 MacBookPro17,1 with 16 GiB RAM, an ignored release benchmark of curved grid
  meshes measured 100,352 triangles at 0.52 s bake / 0.002 s cache hit / 23 MiB sampled RSS (19 MiB over the pre-bake sample), and 999,698
  triangles at 5.97 s bake / 0.023 s cache hit / 214 MiB sampled RSS (173 MiB over baseline). RSS was sampled every 250 ms during Final/32-ray
  baking; these are one-machine measurements, not Quest costs. After the cache addition, `g9-study-room` passed under SwiftShader at 4.41 s
  Preview / 9.48 s Final wall time, with worker times about 164 / 273 ms. Known limits: no explicit ground-plane occluder exists, instances bake on their prototype without considering other placements, and the
  bake/cache still need visual review and headset cost measurement. This is integration evidence, not owner-scored B8 visual acceptance; B2
  remains open.
- **Procedural materials (B5, partial):** `solid.material@1` now has typed `recipe` choices (`none`, `wood`, `planks`, `brushed_metal`,
  `leather`, `fabric`, `brick`, `concrete`, `marble`, `painted_wall`) plus bounded object-space `texture_scale` from 0.1 to 50. The evaluated
  material remains typed/serializable; recipe ID and scale use the reserved instance lanes and stable placement index seeds subtle per-instance
  hue/brightness variation. The WGSL path uses bounded value-noise samples, pattern-driven roughness, and a small height-gradient normal perturbation;
  it loads no textures or UVs. The real study-room fixture assigns recipes to floor, wall, desk, sofa, rug and books, and passes under SwiftShader
  (5.41 s Preview / 10.63 s Final wall time, worker 178 / 284 ms). The pistol prop fixture assigns brushed metal to its slide and leather texture
  to its grip; evaluator tests confirm both typed recipes survive the real model graph. A second browser fixture renders all ten choices (`none`
  plus nine patterns) in one Final revision; its SwiftShader capture at ignored `target/reports/g9/material-recipes.png` shows distinct grain,
  seams, weave, brick and marble patterns. Modeling tests cover recipe evaluation, scale retention, canonical save/load, undo/redo and pistol
  evaluation. Limits: every tier currently shares the same
  pattern cost (two value-noise samples for all active recipes, plus one for brushed metal); per-instance variation is not yet visually scored,
  and no recipe-by-recipe six-view before/after set or owner rubric score is recorded. Quest GPU cost is unmeasured. B5 remains open for image
  review, tier tuning and fixture acceptance.

- **Rounded profiles and solids (B6, partial):** `profile.fillet@1` uses the polygon offset kernel to round planar profile corners before
  extrusion. `solid.round@1` uses Manifold sphere erosion followed by dilation, with an explicit 6–24 segment setting and bounded input/output
  triangle counts. Analytic tests check profile area/volume, rounded-box volume and preserved outer bounds; thin-core refusal and real pistol/room
  fixture evaluation are covered. The pistol slide and two furniture tops now use this operator. The study-room fixture is consequently 386
  commands (43,465 serialized batch bytes, 193 evaluated graph nodes); its previous 382-command / 42,667-byte figures describe the earlier fixture.
  The alternative approaches were checked against the current APIs: `poly.bevel_edges` is limited to disjoint edges and cannot round the connected
  box edge network without a topology-contract expansion; G8 field offsets accept fields, not arbitrary existing solids. Minkowski is therefore
  used for the bounded convex primitive pieces only. `solid.chamfer@1` now adds a flat 45-degree chamfer from an octahedron Minkowski. Neither operator
  provides general concave-solid filleting or per-edge selection; selected-edge work stays with `poly.bevel_edges@1`, limited to disjoint edges. The actual
  Engine Viewer browser run passed under SwiftShader: Preview 8.34 s and Final 19.77 s wall time (worker 240 / 338 ms);
  the fresh capture is `target/reports/g9/study-room.png`. These are one-machine measurements.
- **Environment ambient (B3, implementation):** the two-colour sky/ground hemisphere is packed as a first-order (L1) spherical-harmonic band
  (intensity-scaled mean and half-difference) and evaluated in WGSL along the world normal, retaining baked AO and the procedural reflected studio
  sky. An optional warm ground bounce adds to downward-facing normals and clamps off when the strength is negative. The environment is now authored,
  typed and serialized: `scene.environment@1` produces a `SceneEnvironment` output carrying `sky_color`, `ground_color`, `intensity`, `reflection`,
  `bounce` and `bounce_color`, threaded through evaluation, the worker, projection and the WGSL block like authored lights; at most one environment
  is allowed (`scene.environment.duplicate`), and out-of-range values refuse as `param.range` at authoring. The uniform block is 1,088 bytes. Tests:
  `crates/ve-render/src/light_tests.rs`, `ve-modeling/tests/g9_environment.rs`. Engine Viewer before/after captures of the study room and pistol prop are saved
  (`target/reports/g9/environment-study-room-before.png`/`-after.png`, `environment-pistol-prop-before.png`/`-after.png`) by `web/tests/g9-environment.mjs`;
  owner-scored visual comparison (B8) remains open.

- **Engine-viewer evidence correction (2026-10-15):** the earlier gallery captures were taken from the retired React shell and are discarded.
  The replacement scripts serve `engine/web/dist` through `tools/serve-lan.mjs`, open the Engine Viewer with `?viewer=1&eval=1`, and stage
  the material gallery/study fixture through the actual evaluator and projection bridge. The first Engine Viewer capture exposed a framing bug:
  auto-frame put a wide model outside the bounded studio room, behind its wall. The review cameras now stay inside the studio and use a wide FOV;
  the current captures show all ten material meshes and the furnished room in the Engine UI. Both focused browser scripts pass under SwiftShader.
  This fixes the capture path and camera placement; it does not close the owner visual-scoring or Quest GPU-cost gaps.

- **Latest B6/B3 validation (2026-10-15):** `cargo xtask quick ve-graph ve-kernel ve-modeling ve-render ve-runtime` passed all 538 tests,
  Clippy, formatting and source rules; `cargo xtask cpp` passed sanitizer tests, WASI compilation and clang-tidy. `cargo xtask parity` passed
  (20 kernel and 38 modeling lines byte-identical). The refreshed `cargo xtask web-test dev g9-study-room` passed with the actual Engine Viewer,
  shader and fixture, under SwiftShader; `cargo xtask web-test dev g9-material-gallery` also passed all ten recipe choices in Final. No physical
  headset test was run.

- **Authored environment (B3, 2026-10-15):** added `scene.environment@1` (a `Environment` port producing a serialized `SceneEnvironment`) plus the
  L1 sky/ground ambient band and optional ground bounce. `cargo xtask quick` passed Clippy, formatting, source rules and the full test run across the
  changed crates and dependents; the TypeScript schema check and viewer typecheck also passed. `cargo xtask web-test dev g9-environment` passed under
  SwiftShader against the real Engine Viewer: it stages the study-room and pistol fixtures with no environment, then appends the environment, and both
  reach Final with the environment live in the committed projection. Captures: `target/reports/g9/environment-{study-room,pistol-prop}-{before,after}.png`
  (with `environment-profile.json`). The stage row in `docs/plans/track2-modeling.md` is updated. Owner visual review remains open.

- **Viewer grading (B4, 2026-10-15):** added a bounded viewer grading transform and five named presets in `crates/ve-render/src/grade.rs`, applied in
  the forward shader after tonemapping and carried in the view uniform; `set_grading`, `set_grading_preset` and `grading_status` runtime exports and a
  Grade selector in the Engine Viewer. Focused tests (`crates/ve-render/src/grade_tests.rs`) cover identity, presets, clamping, non-finite handling and
  finite output. `cargo xtask web-test dev g9-grading` passed under SwiftShader, saving `target/reports/g9/grading-{neutral,mono,warm}.png`. Bloom and
  depth of field are deferred with the reason recorded in ADR 0023.

- **Solid chamfer and the edge-network contract (B6, 2026-10-15):** added `ve_solid_chamfer` to the C++ kernel, the safe `Solid::chamfer`, the
  `solid.chamfer@1` Ready operator, its plan estimate and corpus line. It erodes and dilates with an octahedron (`Manifold::Hull` of the six axis
  points), so it cuts convex edges and vertices with a flat 45-degree facet and leaves concave edges sharp; it is capped at 20,000 input and 250,000
  output triangles and refuses a width at or above half the thinnest bounding dimension. Tests: `crates/ve-kernel/tests/chamfer.rs` (bounds, volume
  below a same-size round, determinism, refusals) and `tests/g9_rounding.rs` (graph evaluation, bounding box, thin-core refusal). The `poly.bevel_edges`
  v1 contract is now pinned by `crates/ve-geometry/tests/g9_bevel_edges_contract.rs`: vertex fans, non-manifold edges, winding conflicts and unwelded
  seams refuse with a typed code naming the element, and a mixed valid/invalid selection refuses atomically. `cargo xtask cpp` passed sanitizers,
  WASI compilation and clang-tidy; `cargo xtask parity` now prints 22 kernel and 39 modeling lines byte-identical; `cargo xtask web-test dev g9-chamfer`
  passed under SwiftShader, saving `target/reports/g9/chamfer-{sharp,cut}.png` for a stool (136 to 782 rendered triangles). Kernel cost on this machine
  (release, `crates/ve-kernel/examples/chamfer_measure.rs`): a 1 m box at width 0.02–0.10 is 44 triangles in 9.6–11.4 ms; a 76-triangle stool union at
  width 0.01 is 616 triangles in 147 ms, so cost grows with the convex-edge count rather than the format. Edge networks remain
  disabled and owner visual review remains open under B8.

- **Golden refresh and browser-pool fix (2026-10-15):** the `engine.mjs` and `xr.mjs` pixel goldens predated the authored-light and ambient-band
  work, so the full browser stage failed on stale images while the focused captures passed. The goldens were regenerated once with `UPDATE_GOLDEN=1`
  (`web/tests/golden/*.png`) and reviewed: the frames are structurally unchanged and differ only in the B1/B3 lighting, which the owner should confirm
  under B8. `g9-material-gallery.mjs` moved from port 5198 to 5201 because it collided with `g9-environment.mjs` in the same parallel pool and made
  the suite flaky with `EADDRINUSE`. After both fixes the focused rerun of `engine`, `xr`, `xr-layer`, `g9-environment`, `g9-material-gallery` and
  `evaluation` passed.

- **B8 review package and clean strict gate (2026-10-15):** `web/tests/g9-review.mjs` produces an Engine Viewer review package at ignored
  `target/reports/g9/review/index.html`, with one page per fixture and a JSON manifest. It captured 60 images: six fixed views at Preview and
  Final for each of the pistol, furnished study room, field creature, ceramic vase, and combined desk/shelf/window plus scatter composition.
  The manifest records source hashes, fixed camera poses, requested and committed quality, rendered triangle counts, evaluation wall time,
  Chromium SwiftShader, and dirty revision metadata. Each page has blank fields for edge readability, material distinction, lighting/contact depth,
  silhouette, composition, and notes, plus JSON export. The review images show the study-room interior, though its near wall still hides some
  contents from three-quarter; the kit composition exposes the flat-colour books and simple box-like assets. Focused B3 environment, B4 grading,
  and B6 chamfer comparisons are linked in the package. The common runner also captures same-camera B1 authored-light off/on, B5 procedural
  recipes none/on, and B7 scatter off/on pairs; their camera poses and triangle counts are recorded in the manifest. The B1 pair changes 69.7%
  of RGBA channel values, B5 changes 19.7%, and B7 changes 6.3%; these are pixel-difference diagnostics only, not visual acceptance scores. B2
  AO still has no isolated pair because it is baked into mesh vertex data and the runtime provides no AO toggle. These remain review-package
  gaps, not completed visual acceptance. The expanded runner passed directly (`bun web/tests/g9-review.mjs`, `exit=0`) and `cargo xtask rules`
  passed afterward. The full strict gate above passed before these additive capture-only assertions were added. Owner scores and confirmation of the refreshed XR
  goldens remain pending; no headset or headset emulator was tested.

  `VE_WEB_JOBS=1 cargo xtask check --strict --no-cache` passed on 2026-10-15 with no cached stages. Rust tests, Clippy, formatting, rules, C++
  checks, dependency checks, WASM Clippy, native/WASI parity, and all 39 browser scripts passed. Browser checks ran serially in 1,748 s;
  `g9-review` passed in 253 s. An earlier four-worker attempt hit load-sensitive timeouts in the outliner click and canvas screenshots; both passed
  in the serial run. Full log: ignored `target/reports/g9/strict-b8-review-serial.log`, ending in `exit=0`.

## Validation completed in this workspace

- `cargo xtask quick ve-modeling ve-geometry ve-runtime`: passed, 307 tests, Clippy and source rules.
- `cargo xtask parity`: passed; ve-kernel 20 lines and ve-modeling 38 lines match native/WASI.
- `cargo xtask web-test dev evaluation`: passed in 210 s under headless Chromium/SwiftShader.
- `cargo xtask web-test dev g9-study-room`: passed; measurements above.
- `cargo xtask web-test dev viewer`: passed; default FOV assertion included.
- The web build/typecheck stage passed as part of each focused browser command. The schema generator emits existing `ts-rs` warnings for serde
  attributes it does not parse; generated schema check and TypeScript project build pass.
- A fresh `cargo xtask check --strict --no-cache` passed on 2026-10-15: 606 workspace tests, native/WASI parity, C++ gates, and all 33 pooled
  browser scripts plus three serialized timing checks. Every gate was fresh (no cached stages); the run used headless Chromium/SwiftShader and
  took 761 seconds in the browser stage. Log: ignored `target/reports/g9/strict-final2.log`, ending with `exit=0`. The first strict attempt
  (`strict-final.log`) exposed FOV assumptions and load-sensitive upload waits; those were corrected and the full clean run passed.
- **After B2 changes:** `cargo xtask quick ve-modeling ve-evaluator ve-render ve-runtime` passed (265 tests, Clippy and rules), `cargo xtask parity`
  passed (20 kernel and 38 modeling lines, byte-identical), and the isolated `g9-study-room` browser test passed. A subsequent full cached strict
  run passed all 619 Rust tests, WASM Clippy and parity; 32 browser scripts passed, but its quality script failed on a stale expectation of 8
  High-tier point lights while the implemented/documented cap is 16. The expectation is corrected and `cargo xtask web-test dev quality` passed
  afterward. The full strict browser stage was not rerun after that correction; this is not a claim of a clean current strict gate. The pooled
  gallery and evaluator scripts were load-sensitive (up to 573 seconds for evaluator soak).
- **AO cache and benchmark follow-up:** `cargo xtask quick ve-modeling ve-evaluator ve-render ve-runtime` passed (266 tests, Clippy and rules),
  WASM Clippy passed, native/WASI parity passed (20 kernel and 38 modeling lines), and `g9-study-room` browser/typecheck passed after the cache
  addition. The ignored release-only 100k/1m study passed with the measurements above. Full strict has not been rerun against the cache changes.
- **B5 focused checks:** the current focused gate passed (`cargo xtask quick ve-modeling ve-evaluator ve-render ve-runtime ve-graph`: 381 tests,
  Clippy and rules); WASM-target Clippy passed; native/WASI parity passed (20 kernel and 38 modeling lines). The three material integration tests
  passed, including save/load/undo/redo. Both `g9-study-room` and `g9-material-gallery` browser runs passed with wasm build, generated schema check,
  TypeScript build and SwiftShader; the gallery accepted all ten recipe choices. The new gallery browser script is registered in the web gate.
  The fresh full `cargo xtask check --strict --no-cache` then passed its 624 Rust tests (one skipped), native and WASI C++ checks, WASM Clippy,
  dependency checks, parity and every browser script except one load-sensitive overload assertion in `tests/perf.mjs`. In that strict run the
  governor reached Low, but the observed sequence was `medium, high, medium, high, medium, high, medium, low` (the script caps transitions at
  six). The focused rerun `cargo xtask web-test dev perf` passed in 63 s with sequence `medium, low`, all five scene-size assertions and headroom
  recovery. This points to frame-pacing sensitivity in the full run, but a clean full strict run is still outstanding.

## Open acceptance and limits

- A1 still needs the gallery-wide estimate-versus-actual audit.
- A5 needs multi-batch/cache profiling and a controlled comparison to the historical 45 s measurement; this single profile does not isolate every
  operator or establish a general speedup.
- G9b has implementation in place for authored/derived lighting (B1), partial AO (B2), L1 sky/ground ambient and optional bounce (B3), viewer
  grading presets (B4), ten bounded procedural material recipes (B5), whole-solid round/chamfer with the pinned edge-network refusal contract
  (B6), and deterministic scatter/jitter plus desk/shelf/window kits (B7). Bloom and depth of field remain deferred as recorded in ADR 0023;
  selected-edge network operations remain unsupported. The B8 gallery and MCP kit discoverability are implemented, and the clean strict gate passed;
  B2 isolated comparison remains open, as do owner scores and XR golden confirmation. Quest GPU cost is unmeasured.
- G9c AI composition and its frozen 60-task evaluation remain open. No provider key or funded credit was configured; no provider-backed model
  results are claimed.
- Continuation handoff for the remaining G9 work: [`handoff-g9-remaining.md`](../handoff-g9-remaining.md).
- Browser evidence used SwiftShader only. No physical headset or headset emulator run was performed in this slice.
- No release-size measurement or physical headset run is recorded.
