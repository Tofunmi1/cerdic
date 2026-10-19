# Experiment reports

One section per experiment (X1 to X9 in the plan). Spike code is throwaway and lives under `spikes/`
(never imported by the engine); the findings live here. Machine for every number: Apple M1, 16 GB,
macOS 26.1, headless Chromium from Playwright 1.64, release builds, warm HTTP cache.

## X2: Curvo as the G5 geometry adapter (2026-10-13) — answered: do not embed in the evaluator

**Question.** Can Curvo provide G5's curves, loft, sweep, revolve and adaptive tessellation without
breaking the panic-free Rust/WASM evaluator or its geometry budgets?

**Build.** Isolated spike `spikes/x2-curvo` against Curvo 0.3.2 (MIT), Rust 1.97.1, Apple M1. The
probe constructs an analytic circle, interpolated cubic curve, periodic profile, sweep and revolve;
it compiled for `wasm32-unknown-unknown`. A native 256-section loft with Curvo default adaptive
tessellation took 1,354 ms and produced 139,503 vertices / 277,440 triangles. The reported 10,006,632
bytes estimates only point and face payload (`24 B` per each); it excludes allocator capacity, normals,
UVs, hash maps and surface construction data. An exact analytic-circle radial sample had zero measured
error at 257 points. The first periodic interpolation check sampled `[0,1]` instead of the curve's knot
domain; that result was invalid and was replaced by the analytic-circle check.

The minimal WASM probe with circle, interpolation, sweep and revolve was 726,498 bytes raw and 144,425
bytes Brotli q9. This is an absolute spike artifact, not a delta against the engine baseline; no size
claim is made for Curvo's incremental cost in Spatial Lab. `cargo tree --target wasm32-unknown-unknown`
reported about 107 package entries in the normal dependency tree. Curvo's Bevy integration is optional
and was not enabled. A source scan found hundreds of `unwrap`/`expect`/panic spellings under `src/`,
including unchecked conversions in curve and surface evaluation paths; the raw count includes doc
examples and is not a count of reachable production panics. The API does not carry our
`ExecutionContext`, so long adaptive tessellation cannot cooperate with the worker's cancellation.

**Decision.** The WASM build and accuracy smoke check pass, but the production-safety and cancellation
requirements do not. Do not add Curvo to the evaluator dependency graph. Implement a deterministic,
bounded in-house subset with polyline and cubic Bézier curves and rotation-minimizing frames; make
unsupported control topology and self-intersections explicit diagnostics. NURBS remains deferred.

**Not measured.** No fair baseline/feature size delta; no repeated benchmark distribution; no memory
profiler; no degeneracy fuzzing; no Curvo/WASM runtime or browser integration. The native loft is one
warm run and its result exceeds the 200,000-triangle preview budget, so it cannot be used as-is for a
preview. Curvo may be reconsidered after an auditable panic-safe subset, cooperative cancellation,
budget-aware tessellation and a measured size delta exist.

## X3: OpenSubdiv and PMP for G6/G7 (2026-10-14) — answered with a split choice

**Question.** Which library can provide subdivision or polygon operators without violating the
evaluator's WASM, exception, determinism, budget and cancellation constraints?

**Pinned sources.** The isolated probe in `spikes/x3-polygon/` uses OpenSubdiv 3.7.0, commit
`9dab8a47bfbb1388ec8388fe61f5f916e6123f38`, under Pixar's Tomorrow Open Source Technology License 1.0;
and PMP 3.0.0, commit `f2fb04f4a4188a5c1ab137e83b96e62fa99c639f`, under MIT plus its employer disclaimer.
OpenSubdiv core is C++ with no dependencies beyond the standard library. PMP's processing library uses
its bundled Eigen 3.4.0 headers (primarily MPL-2.0); the native PMP build also succeeded with Eigen's
`EIGEN_MPL2_ONLY` guard enabled. Viewers and their GLFW/GLEW/ImGui dependencies were disabled. No
upstream code is copied into the engine by this probe. Primary sources: [OpenSubdiv repository and
build options](https://github.com/PixarAnimationStudios/OpenSubdiv/tree/v3_7_0), [OpenSubdiv topology
descriptor](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/v3_7_0/opensubdiv/far/topologyDescriptor.h),
[Sdc rules](https://github.com/PixarAnimationStudios/OpenSubdiv/blob/v3_7_0/opensubdiv/sdc/options.h),
[PMP repository and license](https://github.com/pmp-library/pmp-library/tree/3.0.0), and [PMP
algorithms](https://github.com/pmp-library/pmp-library/tree/3.0.0/src/pmp/algorithms).

**Build and compatibility.** Both configure and build as native static libraries with optional viewers,
GPU APIs, docs, examples and tests disabled. OpenSubdiv also builds for `wasm32-wasip1` with
`-fno-exceptions -fno-rtti`; one upstream translation unit (`bfr/tessellation.cpp`) needed an
out-of-tree `<cmath>` include because WASI's libc++ did not expose `std::abs` through its existing
includes. This local probe-only patch is not an engine patch. A small Catmark program runs under Node's
WASI and returns the same 641 vertices and 600 faces as native. The experiment did not measure a full
large-mesh WASI run or byte-identical positions. The engine's existing `ve-kernel-sys/build.rs` uses
the wasi-sdk C++ toolchain for WebAssembly builds, then links its WASI sysroot libraries for the
`wasm32-unknown-unknown` Rust target. The probe used that C++ (`wasm32-wasip1`) build route; it did
not produce a standalone C++ artifact targeted directly at `wasm32-unknown-unknown`.

PMP 3.0.0 does **not** build under the engine's `-fno-exceptions -fno-rtti` WASI C++ settings:
`SurfaceMesh` and algorithms contain `throw` paths, and property lookup uses `dynamic_cast`. The build
reported both `cannot use 'throw' with exceptions disabled` and `dynamic_cast requires -frtti` errors.
A broad source scan found 197 `throw`, `assert`, `dynamic_cast` or `abort` spellings under `src/pmp/`;
the inspected subdivision/remeshing/hole-fill/decimation/utility implementations account for 23 of
those. These are source counts, not a full reachable-path audit; `surface_mesh.h` and property access
also contain exception/RTTI paths.
There is no evidence here that an exception-enabled PMP archive can safely cross this engine's C ABI;
do not link it into the evaluator as-is.

**Benchmark.** `spikes/x3-polygon/probe.cpp` constructs the input mesh and measures setup plus one
Catmark subdivision step; ten release repetitions on an Apple M1, macOS 26.1, clang 17. A 40 x 25
cell mesh (2,000 input triangles) produced 6,000 faces; PMP median was 0.515 ms (0.505–0.610),
OpenSubdiv 0.330 ms (0.326–0.416). At 250 x 200 cells (100,000 input triangles), both produced
300,000 quad faces. The final ten-run PMP median was 24.95 ms (24.65–30.62); OpenSubdiv median
was 16.07 ms (15.85–21.03). Peak process RSS from `time -l` in those runs was 59.5 MB for PMP
and 47.0 MB for OpenSubdiv.
These are single-process, single-machine smoke numbers, include mesh/topology setup, and do not
establish evaluator latency. Small-mesh peak memory was not measured.

The OpenSubdiv WASI Catmark probe is 1,832,964 bytes raw / 358,754 bytes Brotli q9. A matching
minimal C++ WASI executable without OpenSubdiv is 1,754,514 / 328,693 bytes, so the linked probe's
increment is 78,450 raw / 30,061 compressed bytes. Both numbers include the WASI C++ runtime, and
the programs are standalone `wasm32-wasip1` executables. The current evaluator release baseline in
`g5-report.md` is 1,405,870 raw / 394,881 Brotli; **this is not a measured evaluator integration
delta**. PMP has no WASM size because its compatible no-exception/no-RTTI build failed. A future
engine integration must measure its actual evaluator delta after the link and exports are defined.

**Capabilities found in pinned public APIs and source.** OpenSubdiv Far/Sdc provides Catmull-Clark
topology refinement and primvar interpolation, boundary rules, edge and vertex creases, and
face-varying channels. It is subdivision evaluation, not a face-editing toolkit. PMP exposes
Catmull-Clark, Loop and quad-tri subdivision; normal/face flipping; hole filling; triangulation;
decimation; uniform/adaptive remeshing; and lower-level halfedge insert/remove/split operations.
Its named APIs do not provide face extrusion, inset, bridge loops, weld, dissolve or bevel. The
operator evidence and limits are in [ADR 0019](../adr/0019-polygon-and-subdivision-backend.md).

OpenSubdiv's topology descriptor carries creases, corners, holes and per-corner face-varying value
indices; Sdc exposes boundary interpolation, crease method and face-varying interpolation rules.
PMP's property model has vertex, halfedge, edge and face domains (halfedges can represent corners),
but its built-in algorithms do not promise our semantic propagation contract. Neither API supplies
our `ExecutionContext` cancellation protocol. Bound topology and subdivision level before entry,
check cancellation between bounded phases, and do not claim interruption inside a library call.

**Operator table (source/API inspection at the pinned commits):**

| Operator | OpenSubdiv | PMP | Result | Failure and cancellation evidence |
|---|---|---|---|---|
| Face extrusion | Missing | Missing as a named operator; halfedge insert/split primitives only | In-house on `PolyMesh` | No operator preconditions supplied by either library; our narrow planar-patch contract and typed refusals apply. Neither is cancellable inside a call. |
| Inset | Missing | Missing | In-house on `PolyMesh` | No offset/self-intersection contract in either API; restrict v1 to planar convex faces and refuse unsafe offsets. |
| Bridge boundary loops | Missing | Missing | In-house on `PolyMesh` | No loop correspondence or winding contract provided; equal-count loops and typed mismatch/winding refusals. |
| Weld by distance | Missing | Missing as a named op; low-level vertex/edge edits exist | In-house on `PolyMesh` | No tolerance clustering or attribute seam rule; deterministic clustering, collapsed-face and non-manifold validation required. |
| Dissolve edge/vertex/face | Missing | Missing as a named op; halfedge removal primitives exist | In-house on `PolyMesh` | No safe merge contract; require simple resulting loops and validate incidence/winding. |
| Orientation change | Missing | Partial: `pmp::flip_faces` reverses faces | PMP provides a primitive, but whole-component selection and semantic propagation remain ours | PMP has no selection/topology policy in this helper; constrain to components and revalidate. No cancellation callback. |
| Restricted bevel | Missing | Missing | In-house on `PolyMesh` | No bevel API or typed width refusal. Start with isolated manifold edges, bounded width/segments and transactional refusal. |
| Catmull-Clark | Far topology refinement and `PrimvarRefiner` | `catmull_clark_subdivision`, one step | Both provide | OpenSubdiv requires valid topology descriptor and bounded level; PMP has input/algorithm exceptions. Neither offers our mid-call cancellation callback. |
| Boundary and crease rules | Sdc exposes vertex boundary interpolation, uniform/Chaikin crease methods, edge/corner crease inputs | PMP feature flags in its subdivision source; no fractional crease weights/rule selection comparable to Sdc | OpenSubdiv provided; PMP partial | Numerical parity and boundary/non-manifold fixtures not run in X3. Bound levels and refuse unsupported topology before entry. |
| Face-varying data / UV corners | Descriptor channels index values per face corner; Sdc exposes interpolation modes | Halfedge property domain can store corner values, but built-in subdivision does not establish our seam-preserving propagation contract | OpenSubdiv provided; PMP partial | UV seam interpolation not measured; require a chart-seam reference fixture before G7. No cancellation callback. |
| Fill holes | `holeIndices` excludes faces from subdivision; this is not a hole-filling operator | `fill_hole` for simple boundary loops, triangulation + remeshing + fairing | PMP provided | PMP preconditions exclude non-manifold holes and invalid/non-boundary halfedges, with exceptions. Whole operation is not interruptible. |
| Remeshing / decimation | Missing | Uniform/adaptive remeshing and decimation provided | PMP provided, but not evaluator-compatible as probed | Remeshing requires triangles; invalid preconditions can throw. Iterations are bounded parameters, but no `ExecutionContext` polling. |

The failure column describes available API behavior, not exhaustive fuzz results. In particular, neither
library was runtime-probed on non-manifold input, PMP property survival through these operators was not
tested, and the source scan is not proof that all panic/abort paths have been found.

**Decision.** Use OpenSubdiv's CPU Far path as the proposed G7 Catmull-Clark backend, gated on a
small engine adapter proving bounded growth, cancellation boundaries, boundary/crease/UV cases and
native/WASI parity. Build G6 editing operators on our `PolyMesh` plus deterministic adjacency and
typed contracts. Keep PMP as a measured reference and possible isolated remeshing adapter; do not
make it an evaluator dependency unless a reviewed no-exception/no-RTTI port or safe boundary is
demonstrated. PMP's mesh-editing primitives may inform algorithms, but the engine operator code must
be authored and validated against our own cases. This narrows D2 in the plan by ADR 0019.

**Not measured.** OpenSubdiv crease, boundary and face-varying numerical fixtures; PMP exception-enabled
WASI; either library's behaviour on deliberately non-manifold inputs; per-attribute propagation through
PMP algorithms; concurrent or threaded modes; peak WASM linear memory; full evaluator-link size delta;
large WASI timings; and browser rendering. Source/API inspection is not runtime acceptance. G6 must
not start until the owner has reviewed this X3 result, ADR 0019, the operator plan and G6 cases.

## X10: which control surface should a model use? (2026-10-12) — answered: no difference worth a third surface

**Question.** Does a model build and edit geometry more reliably through the line-per-node text
language, schema-constrained typed commands, or a JavaScript builder with loops and variables?

**Build.** Spike `spikes/x10-surfaces` (throwaway). One evaluator and one operator vocabulary; three
surfaces lowered to the same graph, each with the same conventions text and the same two worked
examples (generated from one graph so they cannot differ). 16 creation tasks and 8 follow-up edits,
chosen to be harder than the earlier spike: dimensioned mechanical parts (a 14-tooth gear, a bolt and
nut, a 14-tread spiral stair), architecture (a pointed-arch window in a wall, a three-arch bridge), a
vase with a 3 mm minimum wall, a twisted tapered tower, a rounded plate, organic parts (a mushroom
with 12 spots), assemblies with 12 repeated gondolas, a bookcase with books standing on the second of
five shelves, and every output carrying a named part and a PBR material (steel, brass, copper, glass,
porcelain, emissive flame, matte black, walnut, stone). Each task has 7 to 11 automatic checks
(sizes, tooth count, genus, wall thickness by section opening, material class, spacing, containment,
and so on); edits are checked against the model's own earlier result (unchanged parts must stay
unchanged). One repair attempt with the error text. Models: Claude Sonnet and Claude Haiku through
the Claude Code CLI as a plain completion endpoint, one run per cell.

**Checks were wrong three times and were corrected before reading results**, which changed the
numbers: a bridge's arches open onto the ground (genus 0, not 3), "rim 140 mm" can mean the outside or
the opening, and two crude lamp and wheel checks. The stored programs were re-scored with the
corrected checks without calling the models again.

| Model, phase | Surface | Valid first try | Valid after repair | All checks pass | Mean checks passed |
|---|---|---:|---:|---:|---:|
| Sonnet, create (16) | text | 100% | 100% | 94% | 98% |
| | typed commands | 94% | 100% | 94% | 99% |
| | JS builder | 94% | 100% | 94% | 99% |
| Sonnet, edit (8) | text | 100% | 100% | 100% | 100% |
| | typed commands | 100% | 100% | 100% | 100% |
| | JS builder | 100% | 100% | 100% | 100% |
| Haiku, create (16) | text | 94% | 94% | 75% | 90% |
| | typed commands | 88% | 88% | 63% | 82% |
| | JS builder | 88% | 94% | 75% | 90% |
| Haiku, edit (7 or 8) | text | 100% | 100% | 75% | 92% |
| | typed commands | 100% | 100% | 86% | 94% |
| | JS builder | 86% | 100% | 71% | 91% |

Cost and size (Sonnet, create): text 1,536 output tokens and $0.019 per task, JS builder 1,504 and
$0.019, typed commands 2,105 and $0.026 (the JSON is wordier). Edits: typed commands 920 output tokens,
text 1,185, JS builder 1,049, because commands carry only the change. Edit drift: about 80% of node
content unchanged and 100% of node names kept on every surface (JS builder 85% for Haiku). Whole
experiment: 142 results, 150 model calls, $1.60.

**What failed was the same on every surface.** Sonnet's only creation miss on each surface was one
task (mushroom for text, bookcase for commands, a frame that does not reach the axle for the JS
builder), and Haiku's misses were the gear (all three surfaces), the lantern's four panes, a thin
vase wall and similar modeling errors. Four Haiku runs (bookcase, Ferris wheel) produced no reply
within 240 s twice; they are model time-outs spread over all three surfaces, not parse failures.

**Reading.** With 16 tasks per cell one task is six points, so every difference between surfaces
above is inside the noise; the model and the task decide the outcome, not the surface. Sonnet is near
the ceiling on these tasks, so the experiment cannot rank surfaces for strong models; it does rule out
a large gap for either model. The JS builder gives loops, but nothing here needed them badly enough to
win, and it costs the properties the plan relies on (no code, bounded evaluation, stable addressing).

**Decision.** Keep text for authoring and typed commands, addressed by node ID, for edits (ADR 0011);
do not build a code surface. Not measured: a vision-model critique loop, query tools, multi-turn
conversations, long documents, other providers, repeated runs for variance. Spike code and raw
results are under `spikes/x10-surfaces/` (`results/`, `results-rescored/`).

## X6: the engine canvas inside a React shell (2026-10-12) — answered, adopt with two engine fixes

**Question.** Can the engine canvas live as one panel of a React 19 editor (lifecycle, resizing, pointer
coordinates, focus), or does it need an iframe?

**Build.** Spike `spikes/x6-react-shell`: Vite 7 and React 19 from the repository root, `StrictMode` on,
a three-panel layout (a field and buttons, the engine pane, buttons) with a resizable pane. The engine is
the real runtime (the dev build, 62 MB) loaded as `ve_runtime.js` plus its wasm, the wasi shim and the
`snippets/` directory (the WebXR bridge script, which the glue imports and which is easy to forget).

**What the experiment found in the engine (both fixed, both with regression tests):**
1. **Any resize killed the engine.** The runner changed the window's resolution but never sent Bevy's
   `WindowResized` message, which is what makes the camera recompute its target size. The render targets
   kept the old size while the surface took the new one, a validation error followed, and Bevy quit the
   app, permanently. This was true of the engine's own page too (a browser window resize or opening
   devtools would have frozen the viewer), and had gone unnoticed because no test resized a rendering
   page. Fixed in `ve-platform-web` (the message, plus the scale-factor message when it changes);
   `web/tests/resize.mjs` resizes six times including 64 by 48 and fails without the fix.
2. **Hover stayed lit when the pointer left the canvas** for another panel (no event told the engine).
   Fixed: `pointerleave` clears the cursor, and a press captures the pointer so a drag that ends outside
   the canvas still delivers its release and cannot leave a grab held. Case added to `interact.mjs`.

**Results after the fixes:**

| Check | Result |
|---|---|
| Boot under StrictMode (effects run twice) | started exactly once behind a module-level guard; no errors; 1.7 to 2.9 s with the 62 MB dev wasm |
| Canvas sized to its panel | the drawing buffer followed the pane: 360 by 600, then 700 by 600, then 1 by 600 (hidden), then 360 by 600 again; the engine kept running through all of it |
| Pointer coordinates with an offset canvas (panel at x=216) | hover found at the same relative positions in a 360 and a 700 pixel pane (the engine reads the position relative to the canvas) |
| Pointer over another panel | no hover (after fix 2) |
| Panel remount (React unmounts and mounts the pane) | one engine-owned canvas moved into the new mount: still running, same size logic, hover works, `start` called once |
| Canvas out of the document for a second, then back | no errors, still running, size recovered |
| Typing in a React field | "wasd" typed intact; no engine errors (the engine listens for pointer events on the canvas only; keyboard capture stays the shell's job) |
| **Naive pattern** (React renders its own `<canvas id="ve">`, re-created on remount) | the new element is a blank 300 by 150 canvas while the engine keeps rendering into the old detached one: **must not be used** |

**Not measured.** Pointer lock inside the shell (headless Chromium refuses it); the viewer's keyboard
handling next to focused fields; two engines in one page (a second glue instance would be needed; not
required, one engine per page); repeated move cycles for leaks; the release build and a real GPU.

**Decision.** Embed in the React shell (D8 stands; the iframe fallback is not needed). Rules for V0: the
engine canvas is created once outside React and moved into the pane; the engine is booted once per page
load behind a guard; there is no `stop()`, so hide a panel rather than destroy it (add a teardown API
only if a fixture needs it); ship the engine as glue, wasm, shim and the `snippets/` directory.

## X1: evaluator-only WASM in a worker (2026-10-12) — answered, adopt

**Question.** Can the geometry evaluator (kernel plus graph, no renderer) run in a worker built from the
same source, with the real C++ toolchain, and be terminated and restarted when a job sticks?

**Build.** Spike `spikes/x1-worker`: a `cdylib` of `ve-kernel` and `ve-graph` with `wasm-bindgen`
(`--target web`), the same wasi shim as the runtime, Rust 1.97.1, no new dependency, no threads, no
SharedArrayBuffer. The glue needs every `wasi_snapshot_preview1` import repointed to the shim (the
first attempt replaced only the first and the worker failed to load: a build-script detail, not a
design problem).

| Measurement | Result |
|---|---|
| Module size | 688,783 B raw; 569,498 B with custom sections stripped; **183,122 B brotli** (the full runtime is 3,358,137 B brotli, so the evaluator is 5.5 percent of it) |
| Cold start in a fresh worker (fetch, compile, instantiate; warm cache) | 31 to 70 ms total, 12 to 28 ms of it instantiation (4 runs) |
| Jobs (sphere unions through the kernel) | 2 spheres: 5 to 9 ms; 6 spheres: 36 to 56 ms; 10 spheres: 100 to 150 ms; 14 spheres: 152 to 195 ms |
| Transfer of the result (up to 90 KB) | under 1 ms over the job time (the result is moved, not copied) |
| Command path (`ve-graph` batch of 200 nodes) | 13 to 14 ms |
| Memory after 20 more jobs | 12.4 MB before and after: no growth once warm |
| Main thread during a 186 ms job | 60 frames per second kept, longest frame gap 17 ms (the same as idle) |
| Stuck job (`spin`) terminated from the page | worker terminated, the pending call settled as `terminated`; terminate, new worker, init and a first job in **46 to 70 ms**; five restarts, **identical results every time**; main thread longest gap 17 ms throughout |
| Deliberate trap (`unreachable`) | reported as `RuntimeError: unreachable`; the same worker answered the next call (not proof that the instance is safe: policy is still to restart after any trap) |

**Not measured.** Result transfers above 0.1 MB; the cost on a Quest (a worker competes for CPU cores
with the XR frame loop, unmeasured); cooperative cancellation (`ExecutionContext` is not in the C ABI
yet, so only termination was tested); jobs that take seconds; Safari and Quest Browser behaviour.

**Decision.** Two entry points from one source: a separate evaluator module for the worker (183 KB
brotli) rather than loading the 3.4 MB runtime twice. Termination plus restart is a sound recovery
path (under 100 ms). ADR 0006 is accepted with this evidence. G2 must still measure the large-transfer
case, add the ABI for cooperative cancellation, and test on the headset.

### Production G2 follow-through

The production evaluator and worker now exist separately from the disposable spike; details and
reproducible evidence are in [g2-report.md](g2-report.md). Large transferable results, cooperative
native boolean cancellation, hard timeout/crash recovery and cache/upload bounds are covered by the
production tests. Current release evaluator is 217,936 B stripped Brotli q9. Headset CPU cost and
hardware frame time remain open; production software-browser measurements do not replace those tests.
