# Handoff: G9 expanded: fixes, visual fidelity, then AI composition

Date: 2026-10-14. Read first: `handoff-common.md` (rules; `cargo xtask quick` while working, the content-cached
`cargo xtask check --strict` once at the end from a fresh log under `engine/target/reports/`), `docs/plans/track2-modeling.md`
(G9 and G10 sections), `docs/modeling/limits.md`, `handoff-g8-finish.md`. G8 has a working foundation but its current report still marks
acceptance open; it remains a prerequisite before claiming the G9-G10 release sequence complete.

For the current continuation scope and latest B3/B6 state, use [`handoff-g9-remaining.md`](handoff-g9-remaining.md) together with
[`docs/modeling/g9-report.md`](modeling/g9-report.md); older counts and statuses below are historical.

## 1. Scope decision and why

The owner reviewed renders of two hand-written test models (a pistol-shaped prop and a furnished study room) and judged the visual
fidelity poor, "the work of an amateur 3D artist". The owner then asked to **add the fixes to G9**. So G9 now has three parts, in this order:

- **G9a. Fixes** found by building those models (section 3). Small, first.
- **G9b. Visual fidelity** (section 4): lighting, ambient occlusion, procedural materials, edge rounding, post-processing, camera defaults.
  This pulls forward the parts of G10 (procedural appearance) that the renders need. Anything in G10 not listed here stays in G10.
- **G9c. AI composition and the 60-task evaluation** (section 5): the original G9, last, because real-model runs need provider credit.

Write ADR 0023 recording this scope change (the plan says changing a decision needs an ADR), update the milestone table and the G9 and G10
sections of `docs/plans/track2-modeling.md`, and keep ADRs Proposed for the owner to accept. Do not cut scope for schedule reasons; the
owner's deadline is 11 Nov 2026 16:30 CEST and the order below is chosen so the most visible gain lands first.

## 2. Baseline evidence (what the owner saw)

- Programs, now in the repo: `crates/ve-modeling/tests/gallery/visual/pistol_prop.txt` (7 parts, rounded edges via polygon cage, creased
  subdivision and back to a solid) and `crates/ve-modeling/tests/gallery/visual/study_room.txt` (30 parts; 382 commands; four batches
  split at `#--split--`). They are not in the gallery lists; add them in G9b/B8.
- Renders from the review are in `engine/target/reports/show/{pistol,pistol_round,room}/` (git-ignored; regenerate with the gallery
  machinery in `web/tests/gallery.mjs` or a short Playwright script; the viewer's `viewer_look_at` and `viewer_configure(true, fov, false, 2)`
  set a pose and field of view).
- What looked amateur, by impact: (1) lighting: one sun plus a flat ambient term, no bounced light, no ambient occlusion, no contact shadows;
  glowing parts (lamp bulb, laptop screen, window) light nothing; (2) materials: one flat colour and one gloss value per solid, no grain, fabric,
  brushed metal, wear or variation; (3) sharp CG edges (rounding via crease subdivision gave only a small gain by itself because lighting and
  material could not show it); (4) no clutter, small detail or variation; the viewer's default 90 degree field of view distorts (a product shot
  is about 40); (5) no post-processing (ACES tonemapping and MSAA only).
- Renderer facts (verified): `ve-render/src/shaders/forward.wgsl` is metallic-roughness PBR (GGX), one sun with a shadow map, **up to 8 point
  lights** (`MAX_POINT_LIGHTS`, one uniform block), no spot lights, constant ambient plus a procedural studio-sky reflection, vertex inputs are
  **position and normal only** (locations 0 and 1; model matrix, base colour, params and emissive travel per instance at locations 3 to 9),
  materials are plain numbers (`VeMaterial`: base colour, metallic, roughness, emissive; "no textures yet"). `present.wgsl` is the present pass.
  Bevy's `pbr`, `core_pipeline` and bloom were removed from the fork on purpose (ADR 0001); ported snippets are fine (MIT/Apache), GPL code is not.

## Current implementation progress

- **A1 primary repro fixed:** boolean estimates now use additive growth with a capped intersection allowance instead of multiplying by four at every
  chained operation. Position-only twist, taper and bend preserve the input triangle estimate. `g9_a1_planner.rs` proves eight sphere cuts
  pass at Preview, a 32 × 32 array that exceeds the actual Preview triangle budget is refused during preflight, and a dense sphere subdivided
  three levels is refused before evaluation. The all-gallery estimate versus actual-count audit remains outstanding, so A1 is not fully closed.
- **A4 complete for the current fixture:** the typed batch limit is now 1,000 commands / 1 MiB. The study room's 382-command text applies
  atomically in one transaction and undoes with one action. The encoded transaction is 42,667 bytes; native Session apply took 72.9 ms in
  the latest focused run on this machine. This is not a browser or provider end-to-end measurement. Limits, diagnostics, command and MCP
  guidance are updated.
- **A2 implementation added:** accepted Preview results automatically request Final after 400 ms of inactivity. If a staged edit exceeds the
  Preview triangle budget, Rust keeps that candidate alive for the Final retry; other Preview failures still reject it. The status displays
  `refining`, and worker recovery retains the failed request's quality. The browser acceptance now checks Preview-to-Final, a model that exceeds
  Preview but fits Final, Preview retention on a Final crash, cache stability after both qualities are warm, and bounded large-result upload.
  `cargo xtask web-test dev evaluation` passed in 210 s with SwiftShader after the 100-request soak was kept on the small baseline scene;
  the just-over-budget and large-transfer fixtures use 318 segments (202,248 triangles), avoiding the 524,288-triangle test artifact.
- **A3 implemented:** concave faces are triangulated before Catmull-Clark, while convex cages keep their original face flow; face and corner
  source values are remapped, and the evaluator emits `poly.subdivide.concave_face_triangulated`. Geometry and graph tests cover a closed
  concave heptagonal prism, unchanged convex quads, and the warning. The modeling parity corpus now includes the graph; native and WASI output
  match on all 38 lines.
- **A5 measured:** the study-room browser profile reports 100.8 ms Preview and 55.3 ms Final evaluator time for 188 evaluated nodes and 382
  commands (42,667 encoded bytes). End-to-end wall time from the staged edit was 4.35 s to accepted Preview and 8.77 s to accepted Final;
  Final mesh transfer was 159,208 bytes and render upload took 4.00 s. There were 41 live meshes and zero staged meshes after either quality.
  This run used local headless Chromium with SwiftShader; it is not a headset or GPU result. See `docs/modeling/g9-report.md` and the ignored
  JSON/screenshot in `target/reports/g9/` for the raw measurements.
- **B4 partial:** the desktop viewer now defaults to a 45-degree vertical FOV, with the slider extended down to 35 degrees. Colour grading,
  bloom, depth of field and presentation presets remain open.
- Remaining in G9a: A1's all-gallery estimate audit. A5's cache-reuse assertion and one room profile now have browser evidence; more batches
  and a controlled before/after comparison against the historical 45 s report remain open.
- **B1 implementation added:** typed `light.point@1`, `light.spot@1` and `light.sun@1` outputs now flow through evaluation, revision-staged projection
  and the forward shader. The 1,040-byte WebGL2 uniform block holds up to 16 point and 8 unshadowed spot lights, with 2/2, 4/4 and 16/8
  Low/Medium/High caps, nearest-first selection and deterministic full-value tie ordering. `solid.material@1(emits_light=true)` adds one bounded
  point light at the output part's bounds centre, scaled from surface area and emissive peak. The 373-test affected-crate quick gate and
  `cargo xtask web-test dev g9-study-room` passed; the browser fixture commits one sun, one point, one spot and three emissive-derived lights
  in the same revision. The screenshot is in ignored `target/reports/g9/study-room.png`. Keep the cost formula a visual heuristic, not physical
  photometry; its Quest cost and owner-scored visual acceptance remain open. Continue G9b with B2 AO, then B5 materials as ordered below. G9c
  remains untouched.
- `cargo xtask check --strict --no-cache` passed on 2026-10-15 with all stages fresh: 606 workspace tests, native/WASI parity and all 33 pooled
  browser scripts plus three serialized timing checks. The browser stage took 761 s under headless Chromium/SwiftShader. Evidence is in the ignored
  `target/reports/g9/strict-final2.log` with `exit=0`. The first attempt found the stale 90-degree panel assertion and load-sensitive large-upload
  timeout; both were corrected and the complete gate then passed. No physical headset, provider-backed AI or G9b/G9c completion is claimed.

## 3. G9a: fixes (do first; each with a test)

**A1. The planner overestimates chained operations and refuses small, legitimate models.**
`ve-modeling/src/plan/estimate.rs`: `solid.difference` and `solid.intersect` are estimated as inputs times 4, so every cut in a chain multiplies the
estimate again; `xform.twist|taper|bend` times 16; `xform.mirror` times 2; `poly.subdivide` times 6 times 4^(levels-1). Repro: the slide in
`pistol_prop.txt` with five chained differences (before I merged the cutters into one union) was estimated at 419,264 triangles and refused at the
200,000 Preview budget; the real mesh is a few thousand. Fix: make estimates additive or bounded by known counts (as `estimate_union` already
does with an allowance), and rely on the **actual** triangle count checked after each operator (the evaluator already checks the finished count)
for true blow-ups. Acceptance: a chain of 8 cuts on a box passes at Preview; a genuine blow-up (large array of arrays, deep subdivide of a dense mesh)
is still refused before it runs; add both as tests, and a test that compares planner estimates with actual counts on every gallery program and
fails if any estimate is below the actual count.
Status: the repeated-multiplication failure is fixed and the eight-cut, large-array and dense-subdivide preflight cases pass in
`g9_a1_planner.rs`. The gallery-estimate audit remains open.

**A2. The page never asks for Final quality, and a manual Final request hung.**
`grep` finds no caller of `evaluation.request('final')` in `src/` or `web/`; only `'preview'` is requested, so everything on screen is limited to
the Preview tier (200,000 triangles, no resolution upgrade). Also: calling `window.__ve.evaluation.request('final')` by hand on a document whose Preview
had failed never returned to `worker === 'idle'` with `failureCurrent === false` within 300 s; reproduce and find out why before building on it.
Fix: after a Preview result settles and the document is idle for a short time (recommendation: 400 ms), request Final, show a small "refining"
state in the status panel, replace the Preview geometry without flicker (see `web/tests/flicker.mjs`), and keep the Preview result if Final fails
(show the diagnostic, do not blank the model). G8's field Preview resolution scaling already exists (`handoff-g8-finish.md` F2). Acceptance: a model that exceeds the
Preview budget but fits Final appears after refinement; a Final failure keeps the Preview and reports the code; tests in `web/tests/`.

**A3. `poly.subdivide` fails on concave polygon faces.**
Repro: `solid.extrude` of a concave heptagon, `poly.from_solid`, `poly.merge_coplanar`, `poly.subdivide` gives
`poly.triangulation_unsupported: self-intersecting polygon` (the face point of a concave n-gon lies outside it). Fix (recommendation): triangulate any
non-convex face before subdividing and report a warning `poly.subdivide.concave_face_triangulated` through the evaluation warnings channel; also
add the case to the guide ("do not merge coplanar faces of a concave cap before subdividing"). Test with a concave extrusion, a convex one (unchanged
output), and parity native versus WASI.

**A4. The 200-command batch limit (implemented at 1,000 commands / 1 MiB).**
The 382-command study room applies as one atomic transaction and one undo entry. Its serialized batch is 42,667 bytes; native Session apply
measured 72.9 ms in the latest focused run. `limits.md`, the MCP guide and `tools/mcp/server.mjs` now publish the limit. Browser/provider
end-to-end timing remains unmeasured.

**A5. Evaluation time of a scene of this size.**
The study room took about 45 s to evaluate in the browser worker on a loaded machine (software rendering host). Profile where the time goes per
batch (does batch 2 recompute batch 1? is the cache keyed correctly? are large unions of boxes the cost?). Target and record numbers in the report;
fix anything that recomputes unchanged nodes.

## 4. G9b: visual fidelity (in this order)

Every item needs: a tier policy (Low, Medium, High; what runs on Quest), a measured cost, a typed refusal or fallback, native/WASI parity where it
runs in the evaluator, a test, and before/after images from the review set. **No number measured under SwiftShader says anything about the Quest.** The
M9 headset run (owner plus headset) decides tier defaults; until it exists, mark Quest costs as unmeasured and keep expensive features off the Low tier.

**B1. Lights as graph values, and emissive parts that light the scene.**
Add `light.point@1`, `light.spot@1` and `light.sun@1` operators producing a `Light` value that becomes scene lights (new output kind next to parts; thread it
through evaluator, projection and the runtime: `ve-modeling`, `ve-runtime/src/projection.rs`, `ve-render/src/light.rs`). Add spot lights to `forward.wgsl`
(cone and falloff). Raise the point light count within the WebGL2 uniform-block limit (measure the block size; report the maximum you could reach),
with a per-tier cap and deterministic ordering (nearest or brightest first; the code already sorts). Option on materials: `emits_light=true` creates one
point light at the part's bounds centre from its emissive colour and area-based intensity, capped in number. Sun keeps the only shadow map unless you
measure that a shadowed spot is affordable. Acceptance: the study room's lamp, laptop and window visibly light nearby surfaces; the tier cap works.

**B2. Ambient occlusion baked at evaluation time.**
Add a per-vertex occlusion value to the render mesh (a new vertex attribute; the vertex layout is position and normal only today, locations 0, 1 and 3 to 9
are taken) and multiply it into the ambient and sky terms in `forward.wgsl`. Bake in C++ with deterministic rays (a fixed low-discrepancy sequence, cosine
weighted, 32 to 64 rays, a maximum distance, a ground plane option) against a BVH over the evaluated triangles; native and WASI must agree byte for byte.
Bake over the whole document's static parts so objects shade each other (desk under the window, books in the bookcase); instance sets bake on the
prototype only, say so. Cache by geometry hash and bake only for Final (or when idle). Measure bake time at 100,000 and 1,000,000 triangles. Acceptance:
visible contact darkening in corners, under furniture and in shelf gaps on the study room; no change to the pistol's geometry; budget refusal with a code.

**B3. Better ambient and bounce.**
Partial implementation: `VeAmbientLight` carries separate sky and ground colours and the forward shader blends their diffuse irradiance by world
normal. The remaining work is low-order spherical-harmonic irradiance, optional warm ground bounce, and serializable scene parameters (a small
`scene.environment` or lighting-preset op). Verify with actual rendered image comparisons, not uniform-only coverage.

**B4. Post-processing and presentation defaults.**
Bloom (port a dual-filter bloom, desktop High tier only, off on Quest until measured), colour grading controls (exposure, contrast, saturation, tint) in
the present pass, a default viewer field of view of about 45 degrees (keep the slider), and an optional depth of field on desktop High. Studio lighting
presets for the page (three-point plus soft ground). SSAO only if B2 proves insufficient.

**B5. Procedural materials (the part of G10 the renders need).**
A typed, bounded recipe set evaluated in the fragment shader from object-space 3D noise (no UVs needed, nothing imported): wood grain, planks with
per-plank variation, brushed metal, leather or stipple, fabric weave, brick, concrete, marble, painted wall with subtle noise. A recipe id and a few
parameters travel with the instance (there are free vertex attribute locations; check the packed material lanes in `ve-render/src/material.rs`), with
bump from the noise gradient, roughness variation, and per-instance hue and brightness variation from a hash of the instance index. Define the
language side (`material.*` values or recipe parameters on `solid.material`; decide with the owner and record in the ADR), the budget per tier (octave
caps), and published screenshots of every recipe. Acceptance: the study room's floor, desk, sofa, walls and books and the pistol's grip and slide read as
different materials; save, reload, undo and redo keep them.

**B6. Edge rounding as a real operator.**
Implemented partially: `profile.fillet@1` rounds planar outlines; bounded `solid.round@1` uses Manifold sphere erosion followed by dilation on the
pistol slide and study-room table tops. Its contract is limited to small convex pieces, caps inputs at 20,000 triangles and outputs at 250,000,
and refuses radii at or above half the smallest bounds dimension. `poly.bevel_edges` remains disjoint-edge-only, so extending it to networks would
require a separate topology contract. `field.offset` cannot consume existing solids. Remaining: general edge-network/per-edge selection, a chamfer
operator, before/after six-view evidence, performance under larger meshes and owner scoring.

**B7. Composition aids.** Scatter, clutter and per-instance random variation (position jitter, rotation, scale) as operators, so scenes can be filled
without hand placement; a small kit library of groups (furniture, fence, window) built from existing operators. Keep this behind B1 to B6.

**B8. Visual review gate.** Add the pistol, the study room, the creature and the vase to the gallery list with six fixed views each, a Preview and a Final
render, and a before and after pair for every B item. Write a rubric the owner can score (edge readability, material variety, depth from lighting and
occlusion, composition) and record scores in the report. This is evidence for the owner's judgment, not a pixel golden.

## 5. G9c: AI composition and the 60-task evaluation (original G9, last)

Follow the plan text for G9. In short: capability selection by task with dependency closure, a bounded summary of current IDs and parameters, targeted
diagnostic repair, optional budgeted render critique, AI-proposed edits as a reviewable pending transaction in the editor, and a frozen evaluation of at
least 60 creation and edit tasks with first-try validity, repaired validity, semantic and visual success, edit locality, latency and cost reported separately,
a deterministic mock corpus, and thresholds frozen before evaluating. Additions from this scope change:
- The MCP guide and capability lists must cover lights, materials, rounding, scatter and the batch rule; keep the guide short (the full guide cost about
  $0.24 per request earlier; trimmed guide, an on-demand operator help call and prompt caching are the cost fix).
- Score visual quality with the B8 rubric on a sample of results, not only validity numbers.
- Real-model runs need funded provider credit (the earlier router credit is spent); build and test against the mock corpus first. Do not reuse the router
  key pasted in chat earlier.

## 6. Traps

- Order matters: A1 and A2 first (they block seeing any detailed model), then B1, B2, B5 (largest visual gain), then B6, B3, B4, B7, B8, then G9c.
- Shader limits (WebGL2): no storage buffers; lights are one uniform block; vertex attribute count is limited; per-pixel noise costs fill rate on Quest.
- Determinism: anything computed in the evaluator (AO bake, scatter) uses fixed sequences and sorted structures and must match native versus WASI.
- Do not import textures or models. All patterns are procedural.
- Rust files under 400 lines, functions under 80, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy; C++ gated by clang-format, clang-tidy, ASan,
  UBSan; new tests in new files. `cargo xtask quick` while working; the strict gate once at the end (it caches by content).
- Another agent may be editing the tree; if something unrelated breaks, re-run before assuming it is yours. No commits, no attribution lines, edit tools only
  (formatters and `bun run schema:generate` are fine).

## 7. Done means

A1 to A5 fixed with tests (chain of cuts passes at Preview and real blow-ups still refuse; Final refinement works and keeps Preview on failure; concave
subdivide handled; batch rule decided and documented; evaluation time profiled). B1 to B6 delivered with tier policies, measured costs, parity, tests and
before and after images; B7 and B8 delivered or recorded as limits with reasons; the owner has scored the study room and pistol against the rubric and the
report says what remains. G9c has the mock corpus, the selection and repair machinery and the pending-transaction editor flow, with the real-model numbers
either measured with funded credit or stated as pending. ADR 0023, the plan, `limits.md`, `diagnostics.md`, `cases.md` and the reports agree with the code;
release sizes and timings are recorded; `cargo xtask check --strict` passes from a fresh log with the real `exit=` line. Report honestly: SwiftShader only,
no headset, no real-model run unless credit is provided.
