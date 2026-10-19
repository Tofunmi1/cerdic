# Handoff: G5 surfaces (curves, sweep, loft, revolve, deformers)

## Current state of G5 (2026-10-13, newest; where it disagrees with later sections, this wins)

**G5 is started, not done.** The foundation is in the working tree, **uncommitted** and mixed with the V3 work
(about 64 changed or new paths; the same files, `cases.md`, the plan, `evaluation.ts`, `mcp`, `web.rs`, carry both
streams, so a clean split into two commits needs per-hunk staging; one combined commit is the practical option).
Rust tests for `ve-graph`, `ve-modeling` and `ve-interact` pass, clippy `-D warnings` and `cargo fmt` are clean.
**`cargo xtask check --strict` passed after A–C** (431 nextest tests, parity, WASM build and browser suites on
SwiftShader). G5 remains incomplete: D still needs a configured provider/key, and the result is recorded in the
explicitly incomplete draft `docs/modeling/g5-report.md`.

**Decisions already made (do not redo).**
- **X2 was run and Curvo was rejected** (`docs/modeling/experiments.md`, section X2): no cancellation hook, unchecked
  panic paths, a 1.35 s adaptive loft over the preview budget, about 107 dependencies, no measured size delta. The
  spike moved to `../spikes/x2-curvo` (it is 638 MB with its `target/`, which the root `.gitignore` ignores).
- **ADR 0017** (draft): a small deterministic in-house subset, owned by us. Cubic Bézier sampling is bounded to 512
  steps; NURBS and adaptive tessellation are deferred. Read its "Not met by this subset" section.
- Initial limits: 4,096 curve points and 64 loft sections (`limits.md`; the reserved 65,536 and 256 need a nested-list
  budget first).

**What exists** (`ve-graph/src/builtin/curves.rs`, `ve-modeling/src/curves*`, `plan/estimate.rs`, `ParamType::Points3`).
- `curve.polyline@1(points, closed)`, `curve.bezier@1(points, steps, closed)` and
  `curve.circle@1(center, radius, segments)`: open paths plus closed sampled circles, authored polylines and
  endpoint-matched single cubic Béziers as `Curve3` intermediate values. A desktop SVG overlay draws their controls;
  this overlay is not rendered per eye in VR.
- `solid.sweep@1(curve, profile)`: open paths and closed sampled circles, one strictly convex closed profile loop,
  rotation-minimizing double-reflection frames with closed-loop seam correction, fan caps only on open paths, a
  curvature-radius guard (`curve.sweep_curvature`), repeated-point and reversal refusal, the unchanged sufficient
  path-distance check, and a budgeted triangle-grid pass (`curve.sweep_self_intersection`) for non-adjacent mesh
  intersections.
- `solid.loft@1(sections..., spacing)`: 2 to 64 convex loops of equal point count, even spacing along Y, capped.
- Preflight triangle and memory estimates before any kernel call, including 24 MiB for the sweep-intersection grid;
  cancellation is polled per ring, segment and grid cell.
- 39 G5 integration tests across the `g5_*.rs` suites, plus three frame unit tests; MCP guide text
  and one MCP browser case.
- 15 G5 diagnostics are in `docs/modeling/diagnostics.md`.

**Case status** (`docs/modeling/cases.md`, G5 section):

| Case | State |
|---|---|
| G5-01, 02, 04, 05, 06, 07, 08, 09, 10 (partly), 15 to 18, 20 to 25 | pass or partial in focused Rust tests (G5-22 in MCP browser test, SwiftShader) |
| **G5-03** loft vase, cavity and open rim | six-section vase and straight/curved pipes pass; a section-gap bound catches a 0.005 m thin wall hidden between the old 0.035 m mesh samples, under matching-vertex linear interpolation assumptions |
| G5-11 save, reload, undo, redo, group, instance | `g5_persistence.rs`: canonical save/reload preserves IDs/output, shared evaluation, history, per-instance override and group-definition edit all pass |
| G5-12 native and WASI parity corpus | 12 modeling outputs match byte-for-byte under `cargo xtask parity`, including closed polyline/Bézier sweeps, loft and the three deformers |
| G5-13 gallery (vase, pipe, curved panel, coil spring, turbine blade) | all five programs pass Rust evaluation and six SwiftShader views in `web/tests/gallery.mjs`; representative three-quarter views inspected; screenshots at `target/reports/gallery/` |
| G5-14 MCP builds and edits a pipe and a lofted vase | scripted MCP client builds both through the viewer and edits their parameters by stable node ID; no external natural-language model/provider was exercised |

**What to build next, in order.**

1. **Closed paths (G5-01, G5-07): circle, authored polyline and endpoint-matched cubic Bézier are covered.**
   Closed sweeps distribute frame holonomy, stitch to the first ring without duplicate rings or caps, and include
   wraparound triples/segment adjacency in curvature and overlap checks. Pappus volume, torus bounds, nonplanar
   seam-frame continuity and closed output edge counts pass. Closed Bézier currently supports one cubic whose
   final endpoint equals its first; multiple-piece periodic splines remain future work.
2. **Hollow things by composition (G5-03): section-gap bound added under explicit assumptions.** For matched
   corresponding vertices with linear interpolation, outer-minus-inner separation is linear between sections, so
   its minimum occurs at a section. `g5_hollow.rs` includes a 0.005 m wall missed by the former 0.035 m sampling.
   This is not a universal runtime guarantee at arbitrary floor/rim transitions or malformed correspondence.
3. **Deformers (G5-09): local conforming refinement and fold checks added.** Edge/centroid errors mark shared edges;
   neighboring triangles split those same edges, preventing T-junctions. Two rounds preserve the 16x preflight cap.
   Sampled Jacobian/face orientation and final Manifold validity are checked. Thin hard-bend and twisted-loft cases
   pass; a sampled tolerance can still be refused and is not a whole-face Hausdorff bound.
   Preflight reserves the fixed 16x triangle growth.
4. **Remaining evidence.** G5-11 now also distinguishes instance overrides from group-definition edits. G5-12
   parity covers twelve modeling outputs. G5-13 renders supported approximations for all five gallery forms; the coil
   uses an authored polyline rather than a helix factory, and the blade uses a convex twisted loft. G5-14 exercises
   scripted MCP tool calls through the real viewer page and stable-ID edits, but no external model/provider. Keep
   this distinction explicit; the repository has no configured LLM provider.
5. **V3 curve editing hook.** `web/viewer.js` draws polyline, cubic Bézier and sampled-circle curves in the desktop
   SVG overlay; Bézier/polyline control points drag in a camera-facing plane and commit `set_param(points)` through
   the existing document history. `web/tests/v3-curve-points.mjs` covers preview, commit, undo and circle visibility.
   The overlay is desktop-only, not per-eye; point drag follows camera-plane motion, not an arbitrary 3D axis.
6. **Measured evidence.** `g5_performance.rs` observes a 64-section, 32-point loft at 4,096 triangles in both
   Preview and Final; a local release test measured 10.55 ms and 7.85 ms respectively (one run, native Apple
   Silicon). Current release WASM is 34,499,210 raw / 4,115,645 Brotli bytes for runtime and 1,375,139 /
   383,943 for evaluator. Compared with the recorded G4c build, cumulative deltas are +34,950 / -38,012 bytes
   runtime and +134,055 / +38,582 evaluator; intervening G4d/V3 and this G5 work are not separable from that
   baseline. These measurements are observations, not release guarantees.
7. **Close the gate.** Update the existing draft `docs/modeling/g5-report.md` after remaining G5 cases and
   validation are complete; include these measurements, current self-intersection limits and the strict-gate result.
   The strict gate passed after A–C. Move ADR 0017 to Accepted only after every row of "Validation required before
   Accepted" is met, including provider-backed G5-14.

**Review follow-ups (2026-10-13, after a review of this tree).** The review found one size-cap gate failure, now
fixed, and three implementation gaps. A to C are now implemented and focused-tested; D remains open.

- **Fixed:** `cargo xtask rules` failed because `ve-graph/src/value.rs` (417 lines) and `builtin.rs` (428) were over the
  400-line cap. The deformer specs moved to `builtin/deformers.rs`, `points`/`points3` merged into one helper and the
  `value.rs` tests moved to `value_tests.rs`. Rules, clippy and the `ve-graph`/`ve-modeling` tests pass. Run
  `cargo xtask rules` after every G5 code change; it is now included in the report's validation list.
- **A. Self-intersection:** a bounded uniform grid checks non-adjacent swept triangles after mesh construction;
  the sufficient path-distance rule remains unchanged. End-cap/tube contact, bend-vertex touch and parallel overlap
  refuse with `curve.sweep_self_intersection`; a separated near-miss builds. The grid polls cancellation, caps cell
  references and candidate pairs, and preflight reserves 24 MiB per sweep.
- **B. Hollow section bound:** under equal point counts, matching vertex correspondence, linear section interpolation,
  inner-inside-outer at every section, and a raised inner loft, the outer-minus-inner gap reaches its minimum at a
  section. A 0.005 m thin spot that the former 0.035 m sampling misses is covered. This does not validate arbitrary
  floor/rim transitions at runtime.
- **C. Deformers:** shared-edge local refinement samples edge midpoints and face centroids, with two rounds and the
  existing 16x growth cap. Sampled face/Jacobian orientation and final solid validity are checked. Hard-bent thin box,
  twisted loft and unmeetable tolerance cases pass; the tolerance remains sampled, not a face-wide guarantee.
- **D. Provider-backed G5-14 stays open** until the owner configures a provider and key; keep the report's wording that
  the MCP test is scripted.

After A to C, the acceptance table, diagnostics, ADR 0017 and the incomplete G5 report have been updated. Run
focused Rust suites and `cargo xtask rules` after G5 changes. The strict gate has passed once after A–C. D remains
open until the owner configures a provider and key; keep the MCP test labelled as scripted meanwhile.

**Do next.** Configure a provider/key to close D and collect clean post-V3/G5 and post-curve-edit timing evidence.
Rerun the strict gate after any further G5 code change. V3 and
G5 remain interleaved in one tree; the owner commits.
Keep Rust files under 400 lines and functions under 80.

---

## 0. Update 2026-10-13, after V3 (read this before section 1)

**Where the repo is.** G4 and V3 are implemented and strict-gated, **but V3 is uncommitted** (about 19 modified
and several new files: `web/viewer.js`, `web/panels/*`, `ve-interact/src/gizmo*`, `ve-runtime/src/gizmo.rs`,
`document_exports.rs`, `ve-modeling/tests/v3_pivot.rs`, `web/tests/v3-*.mjs`, `support-v3.mjs`, ADR 0016,
`docs/modeling/v3-report.md`). Commit it first (the owner commits; no attribution lines) so G5 starts from a
clean tree. Gate evidence: 387 native tests, all browser suites, `target/v3-strict-02.log`. Not yet reviewed by
anyone but me: the G4d commits `eca20cd` (frames, attach) and `2a8ec35` (socket inspector).

**Why G5 is next.** The engine builds hard-surface things (boxes, cylinders, tori, extrude with twist and taper,
revolve, booleans, patterns, groups, instance sets, sockets) but nothing smoothly curved: no curve type, no
sweep, loft or bend. Vases, pipes, handles, car-like bodies and curved panels all wait on G5. It is the biggest
single gain in what a prompt can produce before the 11 Nov deadline. Organic creatures (G7 subdivision, G8 fields)
and game or CAD export are separate; see section 8 of this file for a proposed follow-on.

**What V3 changes for you.**
- A new curve or control-point node should be editable by handle. `ParamType::Points2` handles exist
  (`profile.polygon`, drawn and dragged on the XY plane). A `Points3` param needs a handle type too; add it to
  `web/panels/transform-commands.js` (`parameterHandleCommand`) and the `parameterHandles` and `profilePoints`
  hooks in `web/index.html` rather than a new UI path. Test it like `web/tests/v3-profile-points.mjs`.
- Curves are not drawn. The overlay layer (`web/viewer.js`, SVG over the canvas) can outline a curve as a
  polyline from sampled points. Decide that explicitly; do not leave an invisible node. The overlay is desktop
  only (not per eye); say so if you add to it.
- Frames ride on values (ADR 0015). A sweep or loft result should carry end-cap sockets (`start`, `end`) so
  `xform.attach` works on pipes and legs. Add that to the cases.
- Feature picking resolves clicks down single-input chains (`featureCandidates` in `web/panels/graph.js`); a new
  single-input operator needs no change, a multi-input one is reported ambiguous automatically.

**Lessons from V3 that will save you gate runs.**
- Browser tests: use `web/tests/support-v3.mjs` (one shared browser per file, a context per viewer, `select`,
  `settle`, `beginDrag`, `lookAt`, `screenOf`, `viewportClick`). Launching a second browser right after closing
  the first failed with a dead DevTools pipe. Hold a mouse press across rendered frames (SwiftShader).
- Playwright treats a perfectly horizontal SVG line as hidden: wait with `state: "attached"`.
- Clicking the outliner row of the selected node deselects it; do not call `select` twice on one node.
- Browser errors give no stack in bun: print `step()` markers per block so a timeout names its block.
- `lastFailure` in the evaluation state is history by design (G2 case g2_14); use `failureCurrent` for "what
  is wrong now". Do not clear `lastFailure`.
- Text-form angles are degrees; the document stores radians. `solid.box` is centred at the origin.
- `cargo xtask check --strict` is about 10 to 15 minutes; run the specific suites first
  (`cargo xtask web dev`, then `bun web/tests/<name>.mjs`) and the gate once.
- Do not use `sed`, heredocs or scripts to edit files (a slip this pass left a `.bak` file); Edit and Write only.

**Faster loop (optional, not started).** `cargo xtask check --strict` runs every stage serially; a `quick`
command (fmt, clippy and tests for touched crates, named browser suites) and selectable browser suites in
`xtask/src/web.rs` would cut iteration time without weakening the final gate. Ask the owner before building it.

---

Read `handoff-common.md` first. Prerequisites: G4 (done) and experiment X2 (answered; see the X2 report below).
The initial G5 slice is now underway: `Points3`, `curve.polyline`, fixed-step `curve.bezier`, and a
bounded open-path `solid.sweep` are implemented. See the dated status in `docs/plans/track2-modeling.md`.
These operators do not complete the acceptance scope below. Plan text:
`docs/plans/track2-modeling.md` section 5 "G5", section 3.2 (types `Curve3`, `Surface`), section 6 (X2),
decision D3 in section 8.

## 1. What G5 must deliver
"The selected curve and surface adapter: lines, arcs, Béziers, NURBS where proven, compatible lofts,
rotation-minimizing sweep frames, revolve, caps, adaptive tessellation. Editable control data. Bounded
twist, taper and bend with correct normals and validity checks."

*Accept:* vase, pipe and a curved body or panel; planar holes, periodic seams, repeated points,
degenerate sections and near-180-degree turns have specified behaviour; approximation checks and analytic
volumes only for valid known cases; unsupported trims and self-intersections are rejected explicitly.
Arbitrary trimmed-surface booleans and general constant-thickness shelling are **outside** this gate.

Why it matters to the product: the LLM can only build what the language can express. Today it has boxes,
cylinders, spheres, cones, tori, capsules, polygon extrude (with twist and top scale), revolve, offset,
booleans, patterns. Sweeps, lofts and curved panels are what make furniture, pipes, vases, handles and
organic-ish bodies possible. `docs/modeling/spike-classification.md` lists spike prompts that were deferred
only because G5 is missing (pipe elbow, stethoscope tube, coil spring, turbine blade loft).

## 2. What exists (verified)
- Kernel (`ve-kernel`): `Profile::{polygon, from_loops, offset, offset_round, difference, extrude(height,
  divisions, twist, top_scale), revolve(segments, angle)}`, `Solid::{from_mesh, to_mesh, transform, ...}`.
  There is **no curve type, no sweep, no loft, no bend**. `Solid::from_mesh` validates a triangle mesh into a
  Manifold solid, which is how you can build sweeps and lofts yourself (see 4.2).
- Language: `PortType::Curve` is declared and unused. `ParamType::Points2` carries polygon points (max 4,096).
  Ready operators include `profile.polygon/rect/circle/offset/smooth/difference`, `solid.extrude`,
  `solid.revolve`. `XFORMABLE` is solids only.
- Limits already reserved in `docs/modeling/limits.md`: curve points 65,536; loft sections 256; revolve
  segments 512; extrude divisions 256; 2,000,000 final / 200,000 preview triangles.
- Frames and instance sets exist: a sweep along a curve can later carry sockets at its ends (ADR 0015).
- Fixtures: `docs/modeling/fixtures.md` row "Vase / pipe" (closed solid; rim opening visible from top; wall
  at least the declared minimum; widening the opening keeps node IDs; the pipe follows its curve with no
  twist flips and no pinched cross-section). `ve-modeling/tests/gallery/` has `vase`-class programs from G3.
- Draft acceptance in `cases.md` (end of file): sweep of a circle along a circle matches the analytic volume
  (Pappus: area times centroid path length); loft of equal sections equals the extrusion; repeated points,
  near-180-degree turns, periodic seams and degenerate sections give the specified outcome; bend, twist
  and taper keep normals valid.

## 3. The decision you must make first (X2, then an ADR: 0017 is the next free number, 0016 is V3's)
The plan names **Curvo** (a Rust curve and NURBS crate) as the primary adapter *if experiment X2 passes*, else
"a documented limited profile and polyline sweep subset". X2 was never run. Do it first, time-boxed:
- Verify Curvo's licence (no GPL/AGPL; ADR 0005) and its transitive dependencies; I did not verify them.
- Check it builds for `wasm32-unknown-unknown` with no unwinding assumptions (the workspace is panic-free
  and clippy-strict: `unwrap/expect/panic` are banned in our code, a dependency may still contain them;
  note which paths can panic and fence them).
- Measure: sampling accuracy against analytic circles and Béziers, time and memory for a 256-section loft,
  raw and Brotli size delta of the runtime and evaluator wasm (the reports track these, see `g4c-report.md`).
- If it fails or is awkward, our own subset is small and fully deterministic: polyline and cubic Bézier
  curves, circular arcs, Catmull-Rom through points, rotation-minimizing frames (double reflection), sampled
  to a tolerance. NURBS only "where proven". Do not claim NURBS until a case proves it.
Record the outcome in `docs/modeling/experiments.md` (an X2 section like X1 and X6) and in an ADR; the owner
must hear that the adapter choice is made.

## 4. Proposed design
### 4.1 Types
- `Curve3` value: ordered sample points plus tangent data, closed flag, sampling tolerance, frame
  convention (rotation-minimizing, with an optional start up-vector). Add `Value.curve` next to `mesh`
  and `loops` in `cache.rs`; cache keys already hash op, params and inputs, so nothing else changes.
- Operators (draft names, all `@1`): `curve.line`, `curve.polyline(points)`, `curve.arc`, `curve.bezier`,
  `curve.spline(points)`; `solid.sweep(profile, path, ...)`, `solid.loft(sections...)`, `solid.pipe`
  (circle sweep convenience); deformers `xform.bend`, `xform.twist`, `xform.taper` on solids (bounded
  parameters; `extrude` already has twist and top scale, do not duplicate its semantics).
- Control data stays editable: curves are authored as `Points2/Points3` params (add a `Points3` param type)
  so V3 handles can drag control points.

### 4.2 Building sweeps and lofts
Generate the tube or loft mesh yourself in a pure module (sample path, build frames, place profile loops,
stitch quads into triangles, add caps), then `Solid::from_mesh` to validate and make a valid solid. This
keeps everything in Rust, deterministic, cancellable (poll `ExecutionContext` between rings) and testable
natively. Preflight in `plan.rs` must predict triangles (`sections x loop points x 2` plus caps) before any
kernel call, like every other operator (see `shapes.rs` and `instances/preflight.rs`).
Reject, with typed diagnostics and the node named: self-intersecting sweeps (`from_mesh` fails: map it),
profiles with holes unless every section has equal loop counts, loft sections with different point counts
(or resample with a stated rule, never silently), degenerate (zero-area) sections, repeated path points
(collapse or refuse, specify which), near-180-degree turns (the double-reflection frame is stable but the
profile can fold: detect via the tube radius versus local curvature radius and refuse with the numbers).

### 4.3 Deformers
`bend`, `twist`, `taper` act on a solid by moving mesh vertices, so the mesh needs enough resolution: refine
first (Manifold `refine`/`subdivide` exists, factor capped at 16 in limits) or require the caller to. Output
goes back through `Solid::from_mesh`; recompute normals deliberately and test them (cases say "keep normals
valid"). Bound angles and curvature so a bend cannot self-intersect; refuse with the observed value.

### 4.4 Tessellation tolerance
One rule everywhere: the chord tolerance in metres, derived from the quality tier (`ve-modeling/src/quality`
if present; see how `segments` scale in `shapes.rs`), never a fixed count the LLM must guess. Preview and
final must differ only in density, and cache keys already separate them.

## 5. Cases to write first
`cases.md` has a short draft only. Expand into a table (G5-01...). Minimum:

| ID | Input | Expected |
|---|---|---|
| G5-01 | Sweep a circle along a circle | Volume equals Pappus within the stated tolerance for the chosen segments |
| G5-02 | Loft of equal sections | Equals the extrusion (volume, bounds, triangle count) |
| G5-03 | Loft of different sections, a vase | Closed solid; widest and narrowest radii at the sampled heights; rim opening visible |
| G5-04 | Pipe along a Bézier elbow | No twist flip (frame continuity bound), no pinched section, wall thickness at least the declared minimum |
| G5-05 | Near-180-degree turn, tube radius too large | Refused with the node, the radius and the curvature radius |
| G5-06 | Repeated points, zero-length segment | Specified: collapsed with a warning, or refused (choose and test) |
| G5-07 | Periodic seam (closed path) | Closed ring with no gap or doubled seam; volume correct |
| G5-08 | Profile with a hole, degenerate section | Hole preserved through every section; zero-area section refused |
| G5-09 | Bend, twist, taper | Normals valid (no inverted triangles), volume change within bound, refusal when out of range |
| G5-10 | Predicted triangles over the limit | Refused before any kernel call with `budget.triangles` and the observed number |
| G5-11 | Curve and sweep survive save, reload, undo, redo, group, instance | Equal documents; instances of a group containing a sweep share the prototype |
| G5-12 | Native versus WASI parity corpus lines for the new kernel paths | Equal (add to `ve-modeling/examples/corpus.rs`) |
| G5-13 | Gallery: vase, pipe, curved panel, coil spring, turbine blade | Six views reviewed by eye on SwiftShader |
| G5-14 | MCP: the provider builds a pipe and a lofted vase from prose | Through the real UI path, like `web/tests/mcp.mjs` |

## 6. Order of work
1. X2 experiment (licence, wasm build, accuracy, size). Report to the owner in chat; write
   `experiments.md` section and the ADR (the repo docs are part of the gate).
2. Case table in `cases.md`, then `Curve3` value, `Points3` param, curve operators with tests.
3. Frames (rotation-minimizing) with property tests on random smooth curves.
4. `solid.sweep`/`pipe`, then `loft`, then preflight and refusals, then caps and holes.
5. Deformers.
6. Gallery programs, MCP guide text (`tools/mcp/server.mjs` teaches the operator list; a missing entry
   means the model never uses the operator), text form (`text/` parser must read list-of-points params),
   `docs/modeling/commands.md` if commands change (they should not).
7. Parity corpus, TypeScript export (`typescript.rs`) for new serde types, `bun run schema:generate`.
8. Strict gate, `g5-report.md` with raw and Brotli size deltas, plan status paragraph.

## 7. Traps
- ADR 0010 corrects the vase fixture (six nodes, not eight; inner top radius 0.040 m at y = 0.31 m) and
  its rim and axis contact "are not walls and are not judged". Two optional owner questions about the
  contradictory vase fixture remain unanswered; use the ADR's corrected numbers.
- Any new `Value` field must go through `cache.rs`, `appearance.rs`, `result.rs`, projection and the worker
  transfer if it renders. A curve is not drawn; decide whether curves get an overlay (V3's line pass
  would draw them) and say so rather than leaving invisible nodes.
- Text form: instance statements are recognised by "operator without a dot" (groups). Do not name a new
  operator in a way that breaks that rule.
- Float determinism: native and WASM must agree; use the same operation order and the parity corpus.
- 400-line and 80-line caps will force `curves/`, `sweep/`, `loft/` modules from the start.
- Per-point clippy `cast_precision_loss`: loop counters as `u32` with `f64::from`.
- Do not use any GPL code (Blender's curve code, for example) even for reading; principles only.

## 8. Proposed follow-on after G5 (not in the plan; needs the owner's yes and an ADR)

Today nothing leaves the engine: there is no glTF, OBJ, STL or STEP export (the plan lists only SVG/DXF, deferred,
after G12). For the demo and for any "game asset" claim, a **glTF 2.0 binary (.glb) export** of the evaluated scene
is the smallest useful step. Sketch: a pure crate (no Bevy; `ve-export`) turning the evaluation result into a .glb:
positions, normals, indices, the PBR base colour, metallic and roughness factors from `solid.material`, instances as
one shared mesh plus node transforms (so a 10,000-placement set stays small), sockets as named empty nodes. Output is
local only (a download from the viewer page); nothing is published. Deterministic bytes for equal documents, bounded
size (limits table), a validator pass (glTF validator is a dev-time check, not a runtime dependency), and a browser
case that exports and re-parses the file. Textures and UVs wait for G10; do not import anything (ADR 0003). If the
owner agrees, write the ADR and a case table first, as for any gate.

Visual quality is a separate lever (G10: procedural materials, normals, optionally UV atlas): the gallery looks plain
because materials are three scalars and the studio stage is grey. Do not start it without the owner choosing it.

## 9. Done means
All G5 cases pass, strict gate passes, X2 recorded, ADR written, report with size deltas and software-render
caveat, plan status paragraph, MCP guide teaches the operators, no `#[allow]`, no attribution lines.
