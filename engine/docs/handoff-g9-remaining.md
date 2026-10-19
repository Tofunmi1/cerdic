# Handoff: finish the remaining G9 fidelity and AI work

Date: 2026-10-15. This handoff continues G9; it does not declare G9 complete. Read
[`handoff-common.md`](handoff-common.md), [`handoff-g9-fidelity-and-ai.md`](handoff-g9-fidelity-and-ai.md),
[`docs/modeling/g9-report.md`](modeling/g9-report.md),
[`docs/modeling/cases.md`](modeling/cases.md),
[`docs/plans/track2-modeling.md`](plans/track2-modeling.md),
[`docs/modeling/limits.md`](modeling/limits.md),
[`docs/modeling/diagnostics.md`](modeling/diagnostics.md), and Proposed
[`docs/adr/0023-g9-fidelity-before-ai.md`](adr/0023-g9-fidelity-before-ai.md) first. Inspect the current tree and existing diffs before editing;
the report and handoff have older historical figures in places, so prefer the latest dated entries in `g9-report.md` and verify live code/tests.

## Goal

Finish the remaining work in this order: B3 environment lighting, B4 presentation and post effects, B6 general edge treatment, B7 composition
aids, B8 visual evidence/review, then G9c AI composition and its evaluation. Keep correctness and resource budgets in deterministic typed contracts.
Record limits honestly. Do not claim G9 complete until the acceptance criteria in the plan and the exit criteria below are met.

Already present: authored point/spot/sun lights and emissive-derived lights (B1); baked AO with cache (B2, still partial); ten procedural material
recipes (B5, still partial); 45-degree viewer FOV default; `profile.fillet@1`; bounded Manifold-based `solid.round@1`; and a hemisphere ambient term
with sky/ground colours in `VeAmbientLight`, now extended to an authored `scene.environment@1` (see the B3 status below). The latest report records
B6/B3 validation and actual Engine Viewer captures. B6 currently
rounds the pistol slide and two study-room table tops. The current study room is 386 commands / 43,465 serialized bytes. Do not restore old 382 /
42,667 figures as current facts.

## Work order

### 1. Finish B3: environment lighting

- Extend the current two-colour hemisphere with a small low-order SH irradiance representation (or document why a simpler stable model wins).
- Add explicit optional warm ground bounce and serializable scene environment/lighting preset values. Keep scene appearance data typed and saved with
  the document; do not hide authored scene state in renderer-only globals.
- Make the preview/final behavior deterministic and bounded. Preserve native/WASI parity for any evaluator-side output.
- Add tests for default values, serialization/reload, normal response, and any clamping/refusal. Render the study room and pistol with before/after
  views through the Engine Viewer.
- State that this is an approximation; do not call it global illumination.

Status 2026-10-15: the remainder is implemented in the workspace. `scene.environment@1` produces a typed, serialized `SceneEnvironment`; the
sky/ground colours pack as a first-order (L1) spherical-harmonic band plus an optional warm ground bounce, evaluated in the forward shader. Tests
are `crates/ve-render/src/light_tests.rs` (defaults, band packing, normal response, bounce clamp) and `ve-modeling/tests/g9_environment.rs`
(defaults, serialization round trip, out-of-range `param.range` refusal, `scene.environment.duplicate`). The Engine Viewer before/after captures of the
study room and pistol are saved by `web/tests/g9-environment.mjs` (which passes under SwiftShader) at
`target/reports/g9/environment-{study-room,pistol-prop}-{before,after}.png`. Still open: owner visual review (B8) of the captures. See
`docs/modeling/g9-report.md` for the dated entry.

### 2. Finish B4: grading and restrained post effects

- Keep the 45-degree viewer default and existing adjustable FOV.
- Add bounded exposure, contrast, saturation, and tint controls in the rendering path. Prefer a predictable grading transform with unit tests for
  neutral settings, boundaries, and finite output. Persist controls at the right scope (viewer preset vs authored scene) and make the distinction
  explicit.
- Investigate the existing render graph and present path before adding passes. Add bloom only to High desktop if it can be isolated cleanly; keep
  it off on Low/Medium and XR/headset configurations until measured. Depth of field is optional and should not block useful grading/presets.
- Add a small set of useful studio/viewer presets and a test that confirms selection reaches the renderer. Do not copy a large post stack or add
  dependencies without a measured need.
- Measure browser cost with the existing Engine Viewer scripts. SwiftShader is integration evidence only, not headset performance evidence.

Status 2026-10-15: viewer grading is implemented. `crates/ve-render/src/grade.rs` holds a bounded, renderer-only transform (exposure 0.25–4,
contrast and saturation 0–2, each tint channel 0.5–1.5) with five named presets, applied in the forward shader after tonemapping and carried in the
view uniform so windows and XR share one grade with no added pass; neutral is the identity and controls clamp (non-finite becomes neutral). The
runtime exports `set_grading`, `set_grading_preset` and `grading_status`, and the Engine Viewer has a Grade selector. Tests:
`crates/ve-render/src/grade_tests.rs`; browser evidence `web/tests/g9-grading.mjs` (passes under SwiftShader, saves `target/reports/g9/grading-*.png`).
Still open: bloom and depth of field, which are deferred with the reason in ADR 0023 (they cannot be isolated cleanly in the straight-copy present
path without new targets and passes, and bloom was conditional on that isolation); owner review of the grading captures (B8).

### 3. Finish B6: usable edge treatment beyond the first pass

- Keep `profile.fillet@1` and `solid.round@1` behavior covered. Existing `solid.round` is a whole-solid rolling-ball operation with triangle caps;
  it is not selected-edge beveling. Preserve its thin-core refusal and test topology/volume/bounds on varied convex inputs.
- Design an explicit `poly.bevel_edges` edge-network contract. The current operator deliberately accepts disjoint edges; do not silently relax that
  topology rule. Cover adjacent edges, vertex fans, boundary/nonmanifold edges, seams/attributes and atomic refusal before enabling networks.
- Add `solid.chamfer` only with a clear geometric contract and tests that distinguish it from round/fillet. Do not expose a name that implies
  arbitrary CAD filleting if only primitive convex solids are supported.
- Apply useful results to additional furniture/prop edges. Measure triangle counts and evaluator/kernel time across a representative range, and
  save before/after Engine Viewer images. Keep hard caps and report worst observed costs.

Status 2026-10-15: the convex Minkowski chamfer is implemented. `Solid::chamfer` (C++ `ve_solid_chamfer`, `Manifold::Hull` of the six axis points
passed to `MinkowskiDifference`/`MinkowskiSum`) cuts convex edges and vertices with a flat 45-degree facet, refuses a width at or above half the
thinnest bounding dimension, and is capped at 20,000 input / 250,000 output triangles; it is exposed as the Ready `solid.chamfer@1`. Tests:
`crates/ve-kernel/tests/chamfer.rs`, `tests/g9_rounding.rs`; parity corpus now prints 22 kernel / 39 modeling lines. The `poly.bevel_edges` v1 contract
is pinned (not widened) by `crates/ve-geometry/tests/g9_bevel_edges_contract.rs`: vertex fans, non-manifold edges, winding conflicts and unwelded seams
refuse with a typed code naming the element, and a mixed selection refuses atomically. Engine Viewer before/after images of a stool are saved at
`target/reports/g9/chamfer-{sharp,cut}.png` (`web/tests/g9-chamfer.mjs`, 136 → 782 rendered triangles). Kernel cost on this machine (release,
`crates/ve-kernel/examples/chamfer_measure.rs`): a 1 m box at width 0.02–0.10 is 44 triangles in 9.6–11.4 ms; a 76-triangle stool union at width 0.01
is 616 triangles in 147 ms. Still open: edge networks (deliberately
disabled), per-edge selection, and owner review of the captures (B8). Concave and cylindrical edges are not chamfered.

`cargo xtask check --strict --no-cache` now passes fresh. Finishing it also refreshed the stale engine/xr pixel goldens (they predated the B1/B3
lighting) after review, and moved `g9-material-gallery` from port 5198 to 5201 because it collided with `g9-environment` in the parallel browser pool.

### 4. B7: deterministic composition tools and kits

- Inspect the existing `xform.*`, `inst.*`, grouping, and graph evaluation contracts first. Avoid implementing duplicate placement functionality.
- Add a bounded scatter/placement operator over a documented domain (for example, a profile/area or explicit point set) with seed, count, spacing,
  and transform-jitter limits. Define collision/overlap behavior; make it deterministic across native and WASI. Prefer instance sets over fusing every
  repeated mesh when the current renderer supports them.
- Add bounded position/rotation/scale variation with a stable authored seed and stable element IDs/order. Re-evaluating or saving/reloading the same
  graph must produce the same result.
- Build a few useful group kits from existing operators (for example window, desk, shelf) and expose only deliberate parameters. Add tests for
  seed stability, budget refusal, bounds, save/reload, and native/WASI parity.
- Measure dense cases and refuse work before allocation/evaluation exceeds declared caps.

Status 2026-10-15: implemented `inst.scatter@1` (seeded rejection scatter over a width x depth rectangle in XZ, `min_spacing`, yaw and scale
jitter, 16,384-point cap, typed `scatter.unsatisfiable` instead of returning fewer points), `inst.jitter@1` (per-element shift, yaw, scale; element
`i` keyed only by seed and `i`), and the kits in `crates/ve-modeling/kits/{desk,shelf,window}.txt`, exported as `ve_modeling::KITS`. Randomness is
SplitMix64, so native and WASI print identical corpus lines (kernel 22 / modeling 40). Tests: `tests/g9_scatter.rs`, `tests/g9_kits.rs`. The MCP
`get_guide` now includes each canonical kit source and call example; `list_operators` exposes the same kit catalog, covered by `web/tests/mcp.mjs`.
The fresh strict run remains part of B8 validation. Engine Viewer captures of the three kits and two scatter seeds exist
(`web/tests/g9-kits.mjs`, `target/reports/g9/kits-*.png`, 148/388/208/1,500 rendered triangles, SwiftShader);
the shelf books are one colour and the kits are plain boxes, so owner review under B8 should judge them. Kits expose colour/seed parameters only;
size parameters would need expression support the language does not have.

### 5. B8: reproducible visual review package

- Add the pistol, furnished study room, creature, and vase to a focused review gallery. Use six named, fixed camera views per model and both Preview
  and Final where supported. Capture before/after images for each B item so changes can be isolated.
- Capture through the actual Engine Viewer (`engine/web/dist`, `?viewer=1&eval=1`, evaluator/projection bridge). Do not use the retired React
  shell or create a second mock viewer. Reuse/fix `web/tests/g9-study-room.mjs` and `web/tests/g9-material-gallery.mjs` as appropriate.
- Store reproducible scripts and machine-readable measurements in the repo; image/report outputs belong under ignored `engine/target/reports/g9/`.
  Record fixture revision/hash, view name, quality, renderer, and relevant build revision with each capture.
- Use a short rubric: edge readability, material distinction, lighting/contact depth, silhouette, and composition. Prepare a comparison sheet and
  leave score fields for the owner. Do not invent owner scores or call machine image similarity a human acceptance score.
- Ask the owner to score only after the complete concrete review package exists. Record scores and requested changes in `g9-report.md`.

Status 2026-10-15: the reproducible package is generated by `web/tests/g9-review.mjs` and registered in the browser gate. It uses the Engine
Viewer at port 5203 and creates 60 Preview/Final images across six fixed views for the pistol, study room, field creature, vase, and kit/scatter
composition. The machine-readable manifest includes fixture hashes, camera poses, renderer, revision/dirty state, triangle counts, and evaluator
wall times. Open `target/reports/g9/review/index.html` (or an individual model page) for the five-category rubric; scores are intentionally blank.
The package is ignored build evidence, not a committed binary artifact. `cargo xtask check --strict --no-cache` passed on 2026-10-15 with
`VE_WEB_JOBS=1`; no stages were cached. All browser checks passed serially, including `g9-review` (253 s). The earlier pooled run hit timeouts in
the outliner click and continuous-canvas screenshots under simultaneous SwiftShader load; the serial run passed those tests.

The latest capture run adds same-camera B1 authored-light off/on, B5 procedural recipes none/on, and B7 scatter off/on comparisons in the common
review page. Each comparison records fixture-source SHA-256, camera pose, quality, triangle count, and wall time; its before/after PNGs are ignored
outputs in `target/reports/g9/review/`. B3 environment, B4 grading, and B6 chamfer have focused before/after pairs linked from the review pages.
B2 AO still lacks an isolated pair because its occlusion is baked into mesh vertex data and the runtime has no AO toggle; add a deliberate
comparison control before claiming that comparison. The review sheet exposed the flat-colour books and simple box-like kits; the study-room
framing shows the room interior, though its near wall still occludes part of the scene in the three-quarter view. Do not treat the image gallery or
strict gate as owner visual acceptance. Owner scores and confirmation of regenerated XR goldens remain pending; no headset or headset emulator was
used. `bun web/tests/g9-review.mjs` passed after the comparison changes (`review-capture.log`, `exit=0`), and `cargo xtask rules` passed. The full
uncached strict gate above passed before these additive capture-only assertions were added.

### 6. G9c: AI workflow and frozen 60-task evaluation

- Audit current `ve-provider`, MCP, editor pending-transaction flow, existing experiments and provider tests before writing a second framework.
  Keep typed commands and server-side provider adapters as the only path from model output into the graph. Provider output remains untrusted and
  must pass schema validation, graph validation, evaluator limits, and explicit user acceptance.
- Implement capability selection with dependency closure, concise bounded scene context (stable IDs, relevant parameters, diagnostics), one
  targeted repair attempt, optional budgeted image critique, and a reviewable pending edit showing graph diff plus viewport preview. Verify stale
  revision, impossible request, ambiguous edit, malformed response, and refusal paths.
- Freeze at least 60 tasks covering creation and follow-up edits, including impossible/ambiguous requests and stale revisions. Include rubric,
  thresholds, schemas, prompt version, model/provider version, budgets, and task corpus hash before evaluating. Keep deterministic mock results
  separate from real-provider results.
- Report separately: first-try validity, repaired validity, semantic success, visual rubric results, edit locality/unchanged IDs, latency, token
  use and cost. Include failed/time-out tasks in denominators and publish raw structured results plus a compact report.
- Run the mock corpus first. The previously pasted Agent Router key must not be reused. Real-model evaluation is blocked on fresh, explicitly
  configured funded provider credentials/credit; do not log or commit secrets. If none is available, finish and report mock evaluation only.

## Cross-cutting rules

- Every visual feature needs explicit Low/Medium/High and XR behavior, a bounded cost, fallback/refusal, focused tests, and browser evidence. Never
  infer Quest performance from SwiftShader. No headset result exists for this work until run on a headset.
- For evaluator operations, preserve deterministic native/WASI output and sorted traversal. For renderer-only settings, test neutral and bounded
  behavior and note platform scope.
- Keep source files within the repository's 400-line limit, functions under 80 lines, and Clippy clean. Do not silence warnings; use a justified
  `#[expect]` only when needed. Add tests in focused new files where practical. Do not add neural assets, downloaded models/textures, MCP servers,
  backends, deployment, or unrelated domain modules.
- Do not commit. Preserve unrelated and pre-existing user changes. Update `cases.md`, `diagnostics.md`, `limits.md`, ADR 0023, the plan, and
  `g9-report.md` when contracts or status change; resolve historical stale statements instead of adding contradictory current claims.

## Validation and exit criteria

During work, run `cargo xtask quick <affected crates>` and `cargo xtask rules`. For evaluator geometry run `cargo xtask parity`; for C++ kernel
changes run `cargo xtask cpp`. For browser changes run the focused Engine Viewer scripts, including at minimum:

```sh
cargo xtask web-test dev g9-study-room
cargo xtask web-test dev g9-material-gallery
```

At completion run `quick`, `rules`, parity and the focused browser scripts above; do not run the strict gate (policy in `handoff-common.md`;
the owner runs it at the G9 milestone, saving its log under `engine/target/reports/g9/`). Record all skipped/unavailable checks explicitly. G9 remains open until the owner visual scores are recorded, remaining implementation
and mock evaluation acceptance passes, and any real-provider/headset limitations are stated without implying those runs happened.

## Final report format

Report: what changed, links to code/docs/review images, measured costs and environment, tests/gates with actual pass/fail results, owner score status,
remaining risks/limits, provider/headset runs performed or not performed, and confirmation that no commit was created.
