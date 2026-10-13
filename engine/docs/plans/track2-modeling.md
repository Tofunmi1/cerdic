# Track 2: an editable procedural modeling engine with a visual editor

**Status: revised draft 3, 2026-10-12. Decisions in section 8 are settled by the owner; implementation
milestones are all pending.** Nothing below claims a feature already works. Track 1 remains the
runtime base. Follow [engine rules](../../AGENTS.md), the [runtime plan](track1-runtime.md) and the
accepted [ADRs](../adr/). Background: [code/language audit](../../../docs/research-handoff.md) and
[advanced modeling research](../../../docs/advanced-modeling-handoff.md), both dated snapshots:
check the current checkout before treating an old finding as an outstanding bug.

## At a glance

| Stage | Gates | What you can see when it lands |
|---|---|---|
| Foundation | G0, G1a, G1b | Baseline report; edit, undo, save and reload on primitives through typed commands |
| Editor shell | V0 | React editor shell hosting the engine canvas; panels; live document view |
| Responsive engine | G2 | Worker status panel: queue, revision, failure and recovery; first gallery entries |
| Language and first editor | G3, V1, V2 | "Make a hollow vase" end to end; node graph with typed sockets; outliner and inspector; gallery of vase, twisted extrusion, offset plate |
| Reuse | G4, V3, V4 | Four-wheel assembly; viewport gizmos and parameter handles; group authoring and library |
| Surfaces and polygons | G5, G6, G7 | Lofts, sweeps, bends; inset, bridge, restricted bevel; subdivision with creases |
| Embedded logic | L0-L4 | Flow ports and rules in the same graph; a door that opens when the key is inserted, in Play mode; replayable sessions |
| Logic editor and AI | L5-L7 | Blueprint-style flow wires and a debugger that steps backward; validator and simulated playthrough; AI writes rules |
| Organic and AI | G8, G9 | Creature from fields; 60-task AI evaluation |
| Appearance and release | G10, G13 | Materials everywhere; release report |
| Optional | G11, G12, V5-V7, L8, L9 | Stylized rendering and strokes; sketches; tool-specific edit modes, XR editing, polish; state machines; interaction packs |

Every gate adds something to the gallery (`?scene=gallery&viewer=1`) or to the editor, so each is
checked by looking at it as well as by tests.

## 1. Outcome and scope

Build a modeling engine and visual editor in which an LLM and a person use the same typed commands to
create and edit procedural models. Preserve construction history, meaningful parts and parameters.
Support several geometry representations and reusable groups so complexity comes from composition and
established algorithms. Bevy stays the runtime/rendering consumer.

**Core release:** document, history and persistence; bounded worker evaluation; typed node language;
groups, instances and attachment frames; profiles, curves, sweeps and lofts; polygon operations and
subdivision; organic fields; procedural appearance; selection, inspector and AI repair loop; **the
visual editor (section 4B): node graph, outliner, inspector and a viewport with gizmos; and embedded
logic (L0-L7): Blueprint-style behaviour in the same language, document and editor, with a Play mode,
a replayable debugger, a validator and AI authoring (contracts in `docs/modeling/logic.md`, ADR 0008).**

**Optional extensions:** stylized rendering, Grease-Pencil-style strokes, constrained sketches, tool
edit modes inside the editor, XR editing, state machines and interaction packs (L8, L9). They have
contracts below but do not delay the core release.

**Deferred:** rigging/skinning, simulation, sculpt brushes, automatic retopology, exact B-rep CAD,
collaboration, unrestricted code execution (logic has no user code at all), imported models, external
generation services, networking, physics events, NPC dialogue beyond the host-effect hook. Deferred but wanted, each needing a fixture before it gets a gate: Minkowski
rounding and shelling, convex hull, SVG/DXF drawing export (E1, after G12).
The goal is broad, editable modeling coverage; arbitrary production-ready assets, good topology,
rigging and animation cannot be promised by adding nodes.

**Release fixtures:** editable vase/pipe; four-wheel assembly with a lofted or subdivided body; an
organic creature; a parameterized building facade. On each, demonstrate a follow-up edit made in the
node graph, one made in the viewport, undo/redo and save/reload. Optional: ink illustration and a
dimensioned bracket. Judge visual quality from several views, not only numbers; document unsupported
requests.

## 2. Delivery rules

1. One active implementation milestone. Write contracts and acceptance cases before code.
   Experiments are disposable; rewrite production adapters, never promote `spikes/` code.
2. Register an operator `ready` only after its advertised conformance cases pass. Capability schemas
   expose ready operators and approved groups, with limits and unsupported cases.
3. Expose kernel functions when a fixture needs them. Do not front-load every available API.
4. Prefer an existing compatible library behind a small adapter. A failed experiment leads to a
   narrower capability or explicit deferral; writing a new kernel or solver needs its own justified
   plan. One production backend per responsibility. Where no library offers an operator (expected for
   polygon bevel, inset and bridge) the operator is deferred or gets a small written plan with its own
   corpus; it is never added silently under another milestone.
5. Estimates do not enforce budgets. Validate before allocating, meter during execution, cap retained
   results, terminate non-cooperative jobs. Keep XR and frame work independent.
6. Every milestone has inspectable evidence: geometry gates add gallery cases; infrastructure gates add
   a visible status, failure or recovery demonstration; editor gates are driven through the real UI in
   browser tests. Measure release artifacts too.
7. New libraries need pinned source, dependency review and a real native and browser build under our
   toolchain. Upstream WebAssembly support alone does not establish compatibility. Licence terms are
   not a selection criterion (section 8); build fit, correctness and size are.
8. **The editor never owns state.** UI state (layout, panel sizes, hover, camera) may live in the UI or
   in a separate editor layer of the document; semantics live only in the authored document and change
   only through commands.

## 3. Architecture and ownership

```text
Chat / text editor / node graph / inspector / outliner / viewport gizmos / XR manipulation
      -> typed CommandBatch(base_revision, transaction_id)
      -> authoring session: committed Document + history + optional pending candidate
      -> parse/typecheck/plan -> bounded evaluation worker -> candidate GeometrySet
      -> revision check + atomic commit -> runtime projection -> Bevy/custom renderer
                                  -> persistence of authored document
```

| Boundary | Responsibility |
|---|---|
| `ve-core` (exists) | General `Name`/`Registry` utilities; no geometry policy |
| `ve-geometry` (new, pure) | Typed geometry, attributes, units, identity/provenance, backend-neutral packing contracts |
| `ve-graph` (new, pure) | Document and commands, parser/IR, `OperatorSpec`, planning, evaluation contracts, groups and versions, editor-layer metadata |
| `ve-modeling` (new, pure) | Operator registrations and geometry-library adapters; split modules as they grow |
| `ve-kernel` / `ve-kernel-sys` / `cpp/` | Safe solid API and audited C ABI; incremental Manifold exposure |
| Worker entry (new) | Job protocol, evaluator and kernel lifetime, bounded cache, transferable outputs |
| `ve-runtime` / `ve-render` | Authoring-session host, result projection, selection mapping, GPU packing, XR and rendering |
| Editor shell (root app, React/TS) | Panels, node graph, outliner, inspector, gizmos; talks to the engine only through the typed bridge |
| Server | Provider adapter proposes commands from generated schemas; keys stay server-side |

Pure crates join `ENGINE_AGNOSTIC` in `xtask/src/rules.rs`; no Bevy or web types in their APIs.
`ve-graph` defines evaluator interfaces; `ve-modeling` supplies implementations by registration; the
worker composes them, so there is no dependency cycle. Library math types and opaque handles stay
private. Evaluator WASM may be packaged apart from the window renderer; record that against ADR 0001
before implementing. It is shared engine source with a worker entry, not a second rendering engine.

### 3.1 Document, transactions and identity
- One authoritative, versioned authored document, **owned by the Rust engine**: stable node, group,
  instance and part IDs, typed parameters, links, outputs, materials, seed, units, schema versions.
  Entity IDs, kernel handles, GPU buffers, caches, camera and hover state are not scene data.
- `OperatorSpec` lives beside the graph types in the existing registry. Help, TS types, tool schemas,
  inspector widgets and node-search entries are all generated from it; no hand-kept second language.
- Text parses to the same typed IR as structured patches. IDs persist explicitly; renaming labels or
  reformatting text never recreates identities. Source spans and drafts are preserved.
- An **editor layer** in the document holds node positions, collapsed state, frame colours and reroutes.
  It is versioned, undoable and saved, but excluded from evaluation hashes, so moving a node never
  re-evaluates anything.
- A batch builds a candidate against `base_revision`; validate and evaluate without touching the
  committed document; commit only on success and matching revision. Failure keeps the last valid
  document and result. A newer pending edit supersedes older work only by accumulating independent
  commands or by an explicit conflict; stale AI patches get conflicts.
- Undo/redo records successful transactions with deterministic regeneration and bounded history.
  Drags coalesce into one transaction; previews are disposable UI state. Undo, reload and group
  upgrades invalidate pending work. A retried transaction cannot apply twice.
- Save authored data and a dependency manifest, never native memory. Versioned migrations, atomic
  replacement, quota and corruption diagnostics; unknown operators or versions keep the file and
  report incompatibility. localStorage for small documents, IndexedDB when needed. Meshes are
  optional, version-keyed caches.
- Stable authored identity is guaranteed; stable face indices after booleans are not. Select by named
  part, socket or semantic feature. Return `SelectionLost` or `SelectionAmbiguous` when topology
  prevents a unique mapping; never silently select another face.
- The root app's current `shared/commands.ts` (create/update/delete of box, cylinder, plane) is a
  vertical slice. It stays until V0, then is replaced by commands generated from the Rust document.

### 3.2 Geometry and language contracts

| Type | First gate | Contract |
|---|---|---|
| `Solid` | G1a | Valid closed solid through Manifold; explicit empty result and status |
| `PolyMesh` | G1a, editing G6 | Polygon faces plus corner attributes; derived adjacency; boundaries allowed |
| `Profile2`, `Curve3` | G3 basic, G5 advanced | Oriented profiles and holes; curves with domains, sampling tolerance, frame convention |
| `Instances` | G4 | Shared prototype, stable instance ID, transform, overrides; no automatic realization |
| `Surface` | G5 | Parametric surface or control data plus tessellation policy; openness is valid |
| `Field3<T>` | G8 | Bounded pure expression representation; scalar fields are not distance guarantees |
| `Strokes` | G11 | Layers, drawings, strokes, points with width, opacity, colour, depth policy |
| `Sketch` | G12 | 2D entities and constraints, solution state, diagnostics; explicit conversion to profile |

`GeometrySet` holds components; conversions state tolerance and information loss. `Surface -> PolyMesh
-> Solid` includes tessellation and solid validation, each fallible. Assembly composition keeps parts
and instances; boolean union is a separate operation. A render mesh is a derived triangle buffer,
never the only representation of an editable cage or surface.

Metres, radians and the engine's Y-up at public boundaries; convert backend axes once in adapters.
f64 for authoring and modeling where supported, explicit f32 at render and kernel boundaries,
published range and tolerance limits, non-finite values rejected. Specify winding, normal transforms
and reflection. The runtime currently refuses mirrored instances: either support mirrored geometry
through a conversion or change the runtime deliberately; never an unchecked negative scale.

Attribute domains: point, edge, face, face corner, curve point, instance. Operators declare each
attribute as preserved, interpolated, created or invalidated; IDs and material labels are discrete.
Provenance is tracked apart from numeric interpolation. `ve-kernel::Mesh` carries only positions and
triangle indices, so adapters must carry or rebuild metadata explicitly or report its loss.

Every operator declares version, port types, units, defaults and ranges, geometry preconditions,
attribute rules, selection effects, determinism, preview and final policy, cost model and limits.
Diagnostics carry code, node, port and feature IDs, source span, observed limit and repair suggestion.
Empty valid solid, invalid input, exhausted budget and kernel failure are distinct outcomes.

Graph semantics: pure DAG at first; reject cycles, recursive groups and incompatible ports. Groups have
stable interface IDs and pinned versions; upgrades are explicit transactions. Bounded repeat only with
statically bounded expansion. Attribute fields later bind to an explicit geometry domain; a spatial
SDF and a per-element field are different socket kinds.

## 4. Evaluation, cancellation and rendering

One worker and a latest-wins pending queue to start. The worker evaluates immutable candidates; the
window owns revision and history and stays free for input and XR. Jobs and results carry document ID,
revision, request ID, worker generation, quality and dependency hash; check them all before projection;
a late preview never replaces a newer final result. A crashed or timed-out worker is a typed failure;
restart it and keep the last committed scene.

Cooperative cancellation between nodes and inside supported kernels. The vendored Manifold exposes
`ExecutionContext` and `WithContext`; coverage varies by operation (`Status()` observes it, `GetMeshGL()`
does not), so expose only verified behaviour. A synchronous WASM call blocks the worker from reading a
cancel message; without a proven shared-memory signal use window-side worker termination for stuck
calls. No threads, SharedArrayBuffer or isolation headers by accident.

Cache immutable results by operator and backend version, input hashes, parameters, seed, tolerance,
quality and relevant geometry context. Keep reusable geometry apart from instance identity bindings.
Cache only successful complete results, with a byte cap and eviction. A cancelled kernel object is
never recycled as a healthy cached result.

Budget source bytes, parsed and expanded nodes, group depth, field instructions and cells, vertices,
faces, realized instances, temporary and cached bytes, job time and GPU uploads. Check count products
for overflow. Native and WASM allocation limits are not fully predictable, so worker recovery stays
required after preflight.

Keep shared instances in render and picking paths. Stage uploads within a frame budget and swap a
complete candidate; track CPU and GPU disposal; forbid mixed-revision picking. Procedural materials
must target shaders the custom renderer supports.

## 4B. The visual editor

**Shape.** The editor is a React/TypeScript shell in the root app. It hosts the engine canvas (the
Bevy-fork runtime, replacing the three.js viewport over time) and panels around it. It never touches
engine internals: it submits `CommandBatch`es, subscribes to document, evaluation and selection
events, and queries read-only views, all through one typed bridge generated from the Rust schemas.
Panels are docked with simple resizable splits; the layout is UI state.

**Panels.**
| Panel | Contents |
|---|---|
| Node graph (Blender Geometry Nodes style) | Pan/zoom canvas; nodes with typed, colour-coded sockets (value and field sockets look different); links with live validity (type and cycle check while dragging); search-to-add built from the registry, showing readiness and cost; frames; reroutes; node groups with enter/exit and a breadcrumb; mute and bypass; multi-select, copy/paste with ID remap, auto-layout, minimap, keyboard navigation; parameter widgets generated from `OperatorSpec` (units, ranges, drag values) |
| Per-node feedback | Evaluation state, cost, diagnostics on the node and port, geometry stats on hover, a "view this node's output" pin that shows that node's result in the viewport |
| Text view | The same IR as text, side by side with the graph; edits in either side become the same commands; source spans link errors to both |
| Outliner (Unity/Unreal hierarchy) | Parts, instances, groups and sockets as a tree; selection, rename, visibility, lock; drag to reparent where the graph allows |
| Inspector | Selected object or node: parameters, transform, material slot, semantic labels, diagnostics, "edit definition vs edit this instance" for groups |
| Library | Our own groups, templates and materials; no imported assets; shows version and dependencies |
| Viewport (scene view) | Orbit and fly camera (the existing viewer), shaded/wire/normals/bounds/diagnostic overlays, translate/rotate/scale gizmos with local/world, snapping and pivot choice, parameter handles (drag a profile point, loft section, revolve axis or radius), frame selected |
| Preview mode | The existing desktop viewer (mock headset, fisheye and headset lens) and Enter VR for the same document; Play mode (L3) runs the embedded logic on a clone |
| History and status | Transaction list with undo/redo, worker queue, last revision, failure and recovery |

**Rules.**
- Selection is one model: viewport picking, outliner, node graph and inspector all select the same
  authored identity, mapped through provenance. A lost or ambiguous mapping is shown, never guessed.
- Every edit is a command. Drags and slider scrubs coalesce into one transaction and show a cheap
  preview evaluation; the final result is committed on release.
- The graph view and the text view cannot disagree: both render the same document.
- The UI must stay responsive while evaluation runs: evaluation is in the worker; the editor virtualises
  large graphs and never blocks on a result.
- Layout metadata is in the editor layer (section 3.1), so moving nodes does not invalidate caches.
- Accessibility basics: keyboard reachable panels, visible focus, readable contrast; the node graph has
  a keyboard add/connect path, not mouse only.

**Library choice.** Start the node canvas on `@xyflow/react` (React Flow, MIT as listed on npm; its
performance at large sizes is unverified) behind a thin adapter so it can be replaced by a custom
canvas if the stress test X7 fails. Docking uses a small own split-pane component unless X6 shows a
reason to adopt a library.

## 5. Milestones and acceptance gates

G0-G10 and G13 plus V0-V4 form the core release (G1 means G1a and G1b). G11, G12 and V5-V7 are
optional extensions. Default execution is serial; prerequisites below are not instructions to run
things in parallel. Each gate needs recorded evidence, not just a passing build.

| ID | Deliverable | Prerequisites |
|---|---|---|
| G0 | Baseline, contracts, case lists, ADRs | Current Track 1 checkout |
| G1a | Geometry containers, document, typed commands, transactions, in-memory history | G0 |
| G1b | Persistence, migrations, corruption and quota handling, bridge for the viewer | G1a |
| V0 | Editor shell hosting the engine canvas, typed bridge, panel frame, live document view | G1b, X6 |
| G2 | Worker packaging, limits, cancellation, recovery, projection, status panel | G1b, X1 |
| G3 | Typed graph and first end-to-end authoring slice | G2 |
| V1 | Outliner, inspector, history panel, generated widgets | G3, V0 |
| V2 | Node graph editor v1 and text view | G3, V0, X7 |
| G4 | Groups, instances, semantic parts and sockets | G3 |
| L0 | Logic contracts in code: port kinds, output ports, variables, schema 2 | G4 |
| L1 | Logic IR, flow validation and the text `rule` form | L0 |
| L2 | Play runtime: pure interpreter, tick, timers, replay log | L1, X8, X9 |
| L3 | Interaction bridge, Play and Edit worlds, live parameter drivers | L2, G2 |
| L4 | Rebuild-class actions through the worker | L3 |
| V3 | Viewport editing: selection sync, gizmos, parameter handles | G4, V1 |
| V4 | Group authoring, enter/exit, library panel | G4, V2 |
| L5 | Flow editor view and the replay debugger | L1, V2, L3 |
| L6 | Scenario validator and simulated playthrough | L2 |
| L7 | AI authoring of logic and its evaluation | L5, L6, G9 |
| L8 | Optional state machines | L7 |
| L9 | Optional declarative interaction packs | L7 |
| G5 | Surface construction and deformers | G4, X2 |
| G6 | Polygon topology and restricted editing | G5, X3 |
| G7 | Subdivision surfaces | G6, X3 |
| G8 | Organic fields and meshing | G4; follows G7 by default |
| G9 | Broader AI modeling and follow-up evaluation | G5-G8 |
| G10 | Procedural appearance and core gallery | G9 |
| G11 | Optional stylized rendering and strokes | G10 |
| G12 | Optional constrained sketches | G5, X5 |
| V5 | Optional tool edit modes: polygon edit, stroke draw, sketch edit | V3 and the matching G gate |
| V6 | Optional XR editing: grab and scale produce coalesced commands | V3 |
| V7 | Optional editor polish: shortcuts, themes, onboarding, large-graph performance | V4 |
| G13 | Release hardening and compatibility report | G0-G10, V0-V4, L0-L7, included extensions |

**G0 — Baseline and decisions.** Record checkout, toolchain and library versions, existing gates and
skips, native and browser release sizes, a named desktop benchmark environment. Define command and
geometry examples, numeric limits, the worker protocol and fixture rubrics. Draft ADRs (free numbers)
recording section 8: document and geometry ownership, evaluator packaging, the licence policy change
(this supersedes the permissive-only wording of ADR 0003 and the `deny.toml` allowlist is extended as
needed), the editor shell and bridge, and Bun as the project package manager (the engine's npm use in
`xtask/src/web.rs` moves to Bun). *Accept:* reproducible baseline; decisions needed for G1, G2 and V0
resolved; budget profile and fixture specs checked in. Also checked in so later gates can be judged
pass or fail: each milestone's concrete case list (inputs, expected outputs or errors, tolerances),
the typed command set with examples, the diagnostic code table, the numeric limits. A gate whose
cases are not listed is not ready to start.

**G1a — Editable document.** Minimal geometry containers and adapters; typed commands: add and remove
node, connect and disconnect, set parameter, transform, set material, group reference, editor-layer
changes; atomic revisions; in-memory history. *Accept:* manual edits and equivalent mock commands give
the same document; invalid batches change nothing; undo and redo keep IDs and parameters; a retried
transaction cannot apply twice; a stale base revision is rejected as a conflict; layout-only changes
do not change the evaluation hash. Kernel round trips keep supported data and reject unsupported
attribute loss explicitly.

**G1b — Persistence and bridge.** Versioned save format, atomic replacement, migrations, dependency
manifest, corruption, version and quota diagnostics, and a thin bridge so the viewer can issue
commands and show the document. *Accept:* reload keeps IDs, parameters, layout and history
boundaries; unknown operators or newer versions keep the file and report incompatibility; corrupted
and over-quota stores fail visibly without losing the previous save; a migration runs on a frozen
old-format fixture.

**V0 — Editor shell.** React/TS editor in the root app: engine canvas component (instance lifecycle,
resize, focus and pointer-lock handling), bridge generated from the Rust schemas, docked panel frame,
and a live read-only document view. The old three.js viewport stays until the engine canvas matches
it, then is retired together with the old command slice. *Accept:* the editor loads, the canvas
renders the gallery, a command sent from the UI changes the engine document and the view; two
instances of the page do not interfere; bridge types fail the build when the Rust schema changes.

**G2 — Responsive evaluation.** Worker entry, protocol, bounded queue and cache, staged projection,
per-result selection map, and a status panel (worker state, queue length, last revision, last failure
and recovery). Start with existing primitives and booleans as the first gallery entries. *Accept:*
forced long or stuck job, worker crash, cancel, reload and rapid edits keep the last valid scene;
stale results cannot commit; restart succeeds; frame and input loops stay responsive. Record worker
and renderer artifact overhead and allocations. Audit item B02 is here, before expensive modeling.

**G3 — First language-to-model slice.** Rewrite the spike language against typed IR and registry with
generated schemas, deterministic serialisation, diagnostics, planning and incremental evaluation.
Expose only needed `CrossSection` (including offset), extrude (twist and top scale) and revolve, so
the gallery gains a vase, a twisted extrusion and an offset plate. A labelled mock provider and an
existing server adapter where available; keys stay server-side; the provider returns bounded typed
patches, never executable code; one capped repair attempt. *Accept:* "make a hollow vase" gives
profile, revolve, a selectable result; "widen its opening" changes the existing parameter and ID; a
manual edit; undo; reload. Wall-thickness constraints preserved. All 53 spike programs classified
as supported, deliberately changed or deferred with reasons; supported semantics compared without
importing spike code. Parser and loader fuzz targets.

**V1 — Outliner, inspector, history.** Tree of parts and instances; inspector widgets generated from
`OperatorSpec` (units, ranges, drag values); history list with undo and redo; diagnostics list
linking to nodes. *Accept (browser tests through the real UI):* editing a parameter in the inspector
equals the same command sent by mock; invalid values are refused with the diagnostic; undo and reload;
selection in the outliner highlights the object in the viewport and the reverse.

**V2 — Node graph editor v1.** Canvas, typed sockets, link validation while dragging, search-to-add
from the registry, parameter widgets on nodes, frames and reroutes, per-node status and diagnostics,
the "view this node's output" pin, mute and bypass, copy and paste with ID remap, minimap, and the
side-by-side text view. *Accept:* building the vase graph by UI gives the same document as the mock
commands; a type-mismatched or cyclic link is refused with the reason shown; text edits and graph
edits stay in sync; moving nodes causes no re-evaluation (instrumented counts); a 200-node graph pans
and zooms without dropped frames on the named desktop; golden screenshots of the editor reviewed.

**G4 — Reuse and assembly.** Versioned groups with exposed ports and parameters, nested namespaces,
shared prototype evaluation, instance overrides, explicit realization, named parts and attachment
frames; align, attach and mirror through typed transforms with defined pivot and handedness.
*Accept:* a wheel group instanced four times; an instance override and an explicit group upgrade each
keep unrelated identities; only affected dependencies recompute and unchanged prototypes are shared;
recursive groups and excessive expanded work are rejected. Ten thousand cheap instances may be valid;
realization is refused by predicted geometry and memory, not an arbitrary count.

**V3 — Viewport editing.** Picking and selection sync with the outliner, inspector and graph through
provenance; translate, rotate and scale gizmos with snapping and pivot; parameter handles for profile
points, loft sections, revolve axis and radii; frame selected; overlays (wire, normals, bounds,
diagnostics). *Accept:* dragging a gizmo produces one coalesced command and one undo step; a
parameter handle edits the same parameter the inspector shows; selecting a lost or ambiguous feature
shows `SelectionLost` or `SelectionAmbiguous`; preview during a drag is cheap and the final result is
committed on release; handles never edit disposable geometry.

**V4 — Groups in the editor.** Create a group from selected nodes with exposed ports, enter and exit
with a breadcrumb, edit definition versus edit instance, explicit upgrade prompts, and a library panel
of our own groups and templates. *Accept:* making the wheel group by UI equals the G4 fixture; editing
the definition updates all instances and editing one instance does not; upgrade conflicts are shown
before they are applied.

**G5 — Surfaces.** The selected curve and surface adapter: lines, arcs, Béziers, NURBS where proven,
compatible lofts, rotation-minimizing sweep frames, revolve, caps, adaptive tessellation. Editable
control data. Bounded twist, taper and bend with correct normals and validity checks. *Accept:* vase,
pipe and a curved body or panel; planar holes, periodic seams, repeated points, degenerate sections
and near-180-degree turns have specified behaviour; approximation checks and analytic volumes only for
valid known cases; unsupported trims and self-intersections are rejected explicitly. Arbitrary
trimmed-surface booleans and general constant-thickness shelling are outside this gate.

**G6 — Polygon tools.** Polygon and corner storage plus adjacency; selected-face extrusion, inset,
bridge loops, weld, dissolve, orientation change, restricted bevel. X3 lists which operators the chosen
library provides; the rest (expected: bevel, inset, bridge) are deferred or covered by a small
separate plan with its own corpus, agreed before G6 starts. Never begin G6 with an unassigned
operator. *Accept:* boundary, closed and non-manifold cases follow the contract; attribute domains and
semantic selections are preserved where defined; bevel failures are typed and transactional;
connectivity, winding and Euler changes validated; open meshes need not be solids.

**G7 — Subdivision.** Catmull-Clark with supported boundary and crease rules and face-varying
attributes, using the backend X3 selects (OpenSubdiv CPU first). Keep the cage; preview and final
levels. *Accept:* extraordinary vertices, creases, boundaries and UV seams match reference cases
within tolerance; refinement growth is preflighted and cancellable. Manifold smooth and refine stay a
distinct tangent-based triangle operation, not a Catmull-Clark substitute.

**G8 — Organic fields.** Bounded field tape (primitive distances, blends, transforms, repeat,
deterministic noise), explicit bounds and resolution, cell and instruction budgets, Manifold `LevelSet`
adapter; tape evaluated in Rust or C++ with no per-sample JS crossing. Blends, deformations and noise may
lose the exact-distance property; expose the guarantee. *Accept:* sphere and torus analytic checks;
blends by convergence or reference samples; clipped and unbounded domains and oversized grids
rejected; boundary behaviour checked; creature fixture with a multi-view silhouette review and named
regions, without claiming animation-ready topology.

**G9 — AI composition and editing.** Select capabilities and groups by task with dependency closure,
supply a bounded summary of current IDs and parameters, targeted diagnostic repair, optional budgeted
render critique. The validator owns correctness; the LLM cannot bypass contracts or limits.
*Accept:* freeze at least 60 creation and edit tasks across modeling families, including impossible
requests, ambiguous edits and stale revisions. Report first-try validity, repaired validity, semantic
and visual success, edit locality, latency and provider cost separately. Version model, prompt and
schema; cap calls, tokens, time and critique images. Judge attachments against declared sockets;
intentionally disconnected assemblies are valid. Keep a deterministic mock corpus. Freeze rubrics and
thresholds before evaluating; compare overlapping tasks with the spike baseline. A metrics table
alone does not establish useful modeling. AI-proposed edits appear in the editor as a reviewable
pending transaction (graph diff plus viewport preview) the user accepts or rejects.

**G10 — Appearance and gallery.** Typed bounded procedural material subset, object and triplanar
coordinates, stable material slots, deliberate normals and tangents, optional UV atlas after X4;
texture and shader limits and real renderer support; MaterialX informs contracts only. A material
editor panel in the inspector. *Accept:* core fixtures show material boundaries, seams, mirrored cases
and follow-up edits correctly; all survive save, reload, undo and redo; supported recipes and
limitations published with screenshots.

**G11 — Optional styles and strokes.** Toon material mode, outlines with a defined open and closed mesh
policy; layered 3D and on-plane strokes with widths, opacity, colours and filled contours. Static
drawings first; animated frame editing is separate. Specify depth, transparency, picking and pixel
versus world width. *Accept:* a multi-width, depth and occlusion gallery; stereo views with correct
per-eye projections (eyes need not match pixel for pixel); stroke-count benchmark on named hardware;
headset performance unverified until measured.

**G12 — Optional sketches.** A constraint solver chosen by X5; points, lines, arcs, circles,
dimensions, degrees of freedom, named conflict and redundancy diagnostics; solved sketches convert to
profiles; the last valid solution is kept; cancellable. SVG/DXF export is deferred item E1. *Accept:*
frozen under-, fully- and over-constrained and ill-conditioned corpus; a dimensioned bracket
regenerates predictably. No solver selected means the milestone is deferred, not rewritten ad hoc.

**V5 — Tool edit modes (optional).** Polygon edit mode over G6, stroke drawing over G11, sketch
editing with live constraint feedback over G12; each ships with its gate's fixture. *Accept:* each
tool's edits are commands, coalesce, undo and save.

**V6 — XR editing (optional).** Controller grab, move and two-hand scale (existing `ve-interact`)
produce coalesced transform commands; selection and handles usable in headset; on-wrist status.
*Accept:* emulated-runtime flows plus headset checks recorded as untested until run.

**V7 — Editor polish (optional).** Shortcut map, themes, onboarding example, 500-node graph
performance, panel layouts saved. *Accept:* measured large-graph interaction, keyboard-only walkthrough.

**L0 — Logic contracts in code.** `PortSpec` gains a kind (`Data` or `Flow`), `OperatorSpec` gains
named output ports, the document gains typed `variables` (initial values), flow links are stored in the
same `inputs` map, and the schema becomes 2 with a real migration step 1 to 2 (adds empty variables)
tested against a frozen schema-1 fixture. `event.*`, `flow.*`, `act.*` and `state.*` operator families
are reserved as draft. *Accept:* old documents load and migrate; a flow link into a data port and the
reverse are refused with `graph.port_type`; commands for variables and flow links follow the G1a case
style; nothing evaluates yet.

**L1 — Logic IR and validation.** Event, flow, action and state operators registered with typed payloads;
flow validation in `ve-graph` (acyclic flow, no fan-in without a merge, bounded `repeat`, typed event
payloads, variable types); the text `rule` form compiles to nodes and links and prints back
identically; effect class and param class (live or rebuild) on every action. *Accept:* the door and the
vending-machine fixtures round-trip text, graph and commands; every `logic.*` diagnostic has a failing
case; the parser fuzzes clean.

**L2 — Play runtime.** A pure `ve-logic` crate: world clone, 60 Hz tick, event queue, rule matching in
authored order, chains with yield points, `wait` and timers, cascade depth 16, 10,000 steps per tick,
re-trigger policy, seeded random streams, input log with per-tick state hash. *Accept:* the same
document, seed and log give the same hash on every tick (property tests, native and wasm); chains
always terminate within the budgets (adversarial cases: emit loops, huge repeats, event floods);
overflow and budget diagnostics fire as specified; one tick of the door fixture costs under 1 ms.

**L3 — Interaction bridge.** Engine interaction (hover, select, grab, insert, enter) becomes typed
events targeted by part and socket names; a Play and Edit toggle in the viewer and editor; Play runs
on a clone and stopping discards it; `act.animate`, visibility and material actions drive live
parameters directly; host effects queue to the page. *Accept:* in the browser the escape-room door
fixture opens when the key is inserted, by mouse and by emulated controller; Edit mode is unchanged
afterwards (document equality); no kernel call happens per frame during animation (instrumented).

**L4 — Rebuild actions.** Actions on rebuild parameters start worker jobs through the G2 path, coalesced
to 4 per second per node, old geometry kept until the swap; `animate` on a rebuild parameter is
refused. *Accept:* a rebuild action changes the geometry in Play without a frame stall; stale results
never swap in; the play world's picking stays consistent across the swap.

**L5 — Flow editor and debugger.** The V2 canvas shows flow ports and white wires, event hats and
rule groupings; the text and graph views stay in sync; the debugger offers breakpoints, pin watches,
live highlighting of fired wires, per-rule counts and step costs, an event log, a variable watcher
and stepping backward by replay. *Accept:* a session recorded in Play replays tick for tick in the
debugger; stepping back one tick restores the exact state hash; a breakpoint pauses at the right node;
graphs stay readable (auto layout of the fixtures reviewed by eye).

**L6 — Scenario validator and playthrough.** Always-on static analysis (section 9 of `logic.md`) and a
bounded simulated playthrough using reversible actions only. *Accept:* seeded defect fixtures (a key
sealed behind its own lock, an unreachable objective, an emit cycle, conflicting writers) are each
reported with the right code and nodes; the door fixture's objective is reached; the validator cost on
200 rules stays under the editor's frame budget.

**L7 — AI authoring of logic.** Tools generated from the registry (`describe_events`,
`describe_conditions`, `describe_actions`, `draft_rules`, `validate`, `simulate_playthrough`); output
as typed commands shown as a reviewable pending change. *Accept:* a frozen corpus of at least 40 logic
tasks (create, edit, impossible, ambiguous) judged by a clean validator and a playthrough that solves
the stated objective; first-try, repaired and unsupported rates reported separately; thresholds frozen
before evaluating.

**L8 — State machines (optional).** `state_machine.*` for modes, compiled to rules. *Accept:* the door
as closed, opening, open and locked matches the rule version tick for tick.

**L9 — Interaction packs (optional).** A declarative pack format (data only) for events, conditions
and actions the AI can create and save as a template. *Accept:* a vending machine pack made by the
mock provider validates, runs in Play and survives save and reload.

**G13 — Release hardening.** Parser and loader fuzzing, adversarial budgets, native sanitiser and
browser parity gates, release size and memory review, long-session recovery including the editor,
migrations, dependency notices. *Accept:* documented release commands, core fixture rubric satisfied,
limitations matrix, measured benchmark report; included extensions meet their gates; excluded ones
are labelled deferred.

## 6. Porting experiments and adoption

Time-box each to 2-3 working days before reassessing; this is not an implementation estimate. Report:
commit, features, transitive dependencies, native and browser build, no-exception compatibility
where relevant, numerical cases, time and memory, raw and Brotli size delta, metadata preservation,
cancellation behaviour, maintenance risk.

| ID | Question and candidates | If the evidence fails |
|---|---|---|
| X1 | Evaluator-only WASM in a worker with the real C++ toolchain; termination, restart, buffer transfer | Resolve packaging before G2; no heavy evaluation on the window |
| X2 | Curvo for loft, sweep, revolve and adaptive surfaces against a fixture and tolerance corpus | A documented limited profile and polyline sweep subset; defer the rest |
| X3 | OpenSubdiv CPU (primary for subdivision) and PMP (polygon and remeshing): subdivision rules, creases, corner data; output a table of G6 and G7 operators as provided, missing or partial | Narrow operators or rules with diagnostics; missing operators get a small plan (rule 4) |
| X4 | xatlas for UV charts with vertex remapping and corner provenance | Keep object and triplanar coordinates; defer unwrap |
| X5 | neoscad sketch-solver (primary) against a frozen 40-sketch corpus, WASM build | Defer constraints; keep dimensioned procedural profiles |
| X6 | Engine canvas inside a React shell: wasm instance lifecycle, resize, focus, pointer lock, two instances | Keep the engine page in an iframe with a message bridge |
| X7 | `@xyflow/react` for the node graph: typed sockets, frames, reroutes, groups, 200 and 500 node performance, custom edge validation | A custom canvas node graph behind the same adapter |
| X8 | A throwaway flow interpreter (events, branch, wait, animate, emit) on the door fixture: cost per tick native and wasm with 200 rules, hash stability over 100k ticks, behaviour under event floods and emit loops | Lower the budgets in `limits.md`; if one tick cannot stay under 1 ms at 200 rules, move the interpreter to its own worker and accept the extra latency (record in ADR 0008) |
| X9 | Deterministic math: the same expression and animation values (trigonometry, interpolation) native versus wasm with a pure `libm`, over a seeded sweep of inputs | Restrict expressions to arithmetic and table-driven easing; document the difference |

Choose on required semantics and measured integration cost, not headline features. An experiment that
narrows a core capability revises that fixture and acceptance contract explicitly; report the
reduced scope rather than marking the original gate passed. Experiments never relax global build
flags or add native-only dependencies silently. Native ASan/UBSan and browser parity are separate
checks. New C ABI functions: checked lengths, explicit ownership, typed status, release paths. Avoid
the deprecated Manifold `Compose` (use `BatchBoolean` with `Add`). Minkowski, broad remeshing and hull
wait until a fixture justifies their cost.

## 7. Verification and performance evidence

| Area | Required evidence |
|---|---|
| Documents and language | Canonical round trip; migration; atomic errors; history; IDs; conflict and idempotency; malformed-input fuzzing |
| Geometry | Type-specific invariants, domain lengths, finite data, bounds, topology, area and volume where meaningful, adversarial cases |
| Native/WASM parity | Same document and command semantics; geometry compared with stated tolerances and topology expectations |
| Determinism | Canonical IR, IDs and seed streams exact; repeatable per pinned backend; no demand for universal byte-identical tessellation |
| Incrementality | Instrumented node counts and cache hits; local edits recompute dependents only; quality, attribute or version changes invalidate correctly; layout edits recompute nothing |
| Worker and runtime | Stale preview and final races, cancel, restart, crash, upload limits, disposal, selection revision consistency |
| Editor | Browser tests drive the real UI and compare the resulting document with the mock-command document; link rejection, undo, reload, selection sync; reviewed editor screenshots |
| Visual and LLM | Fixed views and lighting, reviewed goldens, frozen prompts, semantic success distinct from compilation success |
| XR | Emulated lifecycle, picking and stereo tests plus real headset latency, memory and controller checks; untested hardware marked as such |

G0 defines an executable budget profile and fixture manifest: hardware and browser versions, build
mode, warm and cold cache, graph nodes, tessellation and field resolution, instance counts, output
size. Initial goal: p95 warm preview under 150 ms for the named interactive fixtures, with cold and
final times separate; editor interaction (pan, zoom, drag) at the display refresh on a 200-node graph.
A node count alone is not a workload. Measure main-thread stalls and upload cost. Concrete queue, job,
memory and upload caps are set in G0 and enforced even when the performance goal fails.

Measure renderer and evaluator artifacts separately and together, including duplication, instantiation,
shared prototypes and metadata, and the editor bundle. Set an aggregate release budget from the
baseline and record each dependency's delta.

Run 100 regenerations and a 30-minute alternating edit, cancel and reload soak, with the editor open.
Measure live handles, buffers, cache bytes and WASM high-water memory: linear memory may stay grown
after frees. Require bounded steady growth after warm-up and successful worker recovery. Headset
frame-budget claims need real measurements.

## 8. Decisions (settled by the owner, 2026-10-12)

| ID | Decision |
|---|---|
| D1 | **Licences are not a selection criterion.** MPL-2.0 is allowed; OpenSubdiv's Tomorrow Open Source Technology License 1.0 (Apache with a changed trademark section) is allowed. ADR 0005 records this, supersedes the permissive-only wording of ADR 0003 and extends the `deny.toml` allowlist as needed. Standing rule kept from before: no GPL or AGPL code in the engine, principles only; say if that also goes. Supply-chain advisory checks stay. |
| D2 | **Subdivision:** OpenSubdiv CPU is the primary backend, confirmed by X3; PMP for polygon and remeshing work; Manifold smooth stays a separate triangle operation. |
| D3 | **Surfaces:** Curvo is the primary adapter if X2 passes; otherwise the limited subset in section 6. |
| D4 | **Sketches:** neoscad sketch-solver is primary if X5 passes; otherwise G12 is deferred. |
| D5 | **Blender:** no offline worker in this release; browser authoring stands alone. |
| D6 | **Evaluation:** worker isolation early (G2), one authored command system, groups early. |
| D7 | **Fixtures:** vase and pipe (G3, G5), wheel assembly (G4), organic creature (G8), parameterised building facade (G4, G6), optional bracket (G12) and ink illustration (G11). |
| D8 | **Editor shell:** React and TypeScript in the root app, hosting the engine canvas; the three.js viewport and old command slice retire after V0. |
| D9 | **Node canvas:** `@xyflow/react` behind an adapter, validated by X7; own canvas if it fails. |
| D10 | **Document owner:** the Rust engine; the editor mirrors it through generated types. |
| D11 | **Package manager:** Bun everywhere (the engine's npm use moves to Bun in G0). |
| D12 | **Logic lives in the node language** (ADR 0008): rules are the canonical form; flow ports and a text `rule` block are two views of the same nodes; no user code, ever. |
| D13 | **Tick and runtime:** fixed 60 Hz, integer ticks, no wall clock; the interpreter runs inside the engine module on the window thread under a per-tick step budget; the geometry worker only does rebuild jobs. |
| D14 | **Re-trigger default:** `ignore` while a rule's chain is running; `restart` and `queue` (at most 8) are explicit per rule. |
| D15 | **Per-frame writes** are live parameters only (transforms, visibility, material parameters); geometry parameters are discrete, coalesced rebuild requests. |

Primary references (checked 2026-10-12; pin a commit during adoption):
[vendored Manifold API](../../third_party/manifold/include/manifold/manifold.h) and
[execution-context contract](../../third_party/manifold/include/manifold/common.h) (local APIs beat
upstream assumptions; the safe wrapper exposes only a subset); [Curvo](https://github.com/mattatz/curvo)
(MIT per Cargo.toml; README badge disagrees); [PMP](https://github.com/pmp-library/pmp-library);
[OpenSubdiv FAR](https://www.opensubdiv.org/docs/far_overview.html) and its
[licence file](https://raw.githubusercontent.com/PixarAnimationStudios/OpenSubdiv/release/LICENSE.txt);
[neoscad sketch-solver](https://github.com/neoscad/sketch-solver) (young, two commits);
[React Flow on npm](https://www.npmjs.com/package/@xyflow/react) (performance at 500 nodes unverified).
Other candidates and Blender field and group principles are sourced in the research handoff.

Logic research (read 2026-10-12): the Godot team's
[account of removing VisualScript](https://godotengine.org/article/godot-4-will-discontinue-visual-scripting);
Unity's [graph types](https://docs.unity3d.com/Packages/com.unity.visualscripting@1.9/manual/vs-graph-types.html)
and [custom events](https://docs.unity3d.com/Packages/com.unity.visualscripting@1.9/manual/vs-custom-events.html);
how [Blueprint evaluation works](https://zomgmoz.tv/unreal/Blueprints/How-blueprint-evaluation-works)
(community guide) and Epic's
[Blueprint debugger](https://dev.epicgames.com/documentation/en-us/unreal-engine/blueprint-debugger-in-unreal-engine);
Wasmtime [fuel and epoch interruption](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html)
(only relevant if user code were ever allowed, which it is not); Blender's
[simulation zone](https://docs.blender.org/manual/en/dev/modeling/geometry_nodes/simulation/simulation_zone.html);
Scratch's scheduler described in [SchedCheck](https://arxiv.org/pdf/2607.00623); behaviour trees versus
state machines ([Opsive](https://opsive.com/support/documentation/state-designer/concepts/finite-state-machines-vs-behavior-trees/));
a study of when [visual programs are harder to read](https://core.ac.uk/works/16724972);
A-Frame's [event-driven interaction model](https://aframe.io/docs/1.7.0/components/cursor.html). Verse's
effect annotations come from secondary pages only (Epic's own reference did not load). **Not found:**
a benchmark of LLMs writing Blueprint-style graphs, Blueprint VM performance figures, and event-
condition-action rule literature; the logic evaluation (L7) and the interpreter budgets (X8) are
therefore ours to measure.

**Progress:** **G0 documents written 2026-10-12** in `docs/modeling/` (`baseline.md`, `commands.md`,
`diagnostics.md`, `limits.md`, `fixtures.md`, `cases.md`) and ADRs 0004-0007. G0 is **not closed**:
case lists for G4 onward are drafts to expand before each gate starts, the Bun move for
`xtask/src/web.rs` is not done, and X1 and X6 have not run (they gate G2 and V0, not G1a).
**G1a done 2026-10-12** (`ve-graph`, `ve-geometry`): document, 13 typed commands with batch-local
aliases, atomic batches, undo and redo, conflict and idempotency handling, evaluation hash that
ignores layout and labels, attribute domains, polygon mesh, kernel adapters that reject or report
attribute loss; every G1a case has a test named after it (`crates/ve-graph/tests`,
`crates/ve-geometry/tests`); `cargo xtask check --strict` green; wasm clippy clean. Known limits:
operators are `Draft` (nothing evaluates yet), history keeps full snapshots capped at 200 entries or
16 MiB, frames and reroutes arrive with V2.
**G1b done 2026-10-12:** `ve-graph::store` (envelope with schema, generation, checksum and operator
manifest; two-slot save so a torn write cannot damage the newest save; migrations with a frozen
schema-0 fixture; read-only opening of files this build cannot edit; saving never overwrites a newer
file), a `ve-runtime` document host with a `localStorage` backend, `doc_*` wasm functions, the page's
`state.doc` API and a visible document panel (`?doc=1` or `?viewer=1`). Cases G1b-01 to G1b-09
natively (`crates/ve-graph/tests/g1b.rs`), G1b-10 to G1b-12 and the read-only case in
`web/tests/document.mjs`. Release wasm +118 KB brotli (3.24 to 3.36 MB stripped). Decisions made:
the undo history is not saved; two tabs are not merged (last save wins); `serde_json` now parses
floats exactly (`float_roundtrip`) because the default parser failed bit-exact reload.
**Embedded logic planned 2026-10-12** (`docs/modeling/logic.md`, ADR 0008, gates L0-L9, experiments X8
and X9, decisions D12-D15, an escape-room door fixture); no logic code exists yet.
**X1 done 2026-10-12** (`docs/modeling/experiments.md`): the evaluator-only module is 183 KB brotli,
starts in 31 to 70 ms, keeps the main thread at 60 fps during a job, and a stuck job is terminated and
restarted in 46 to 70 ms with identical results; ADR 0006 accepted (two modules, one source). Open for
G2: large transfers, cooperative cancellation in the C ABI, headset CPU cost.
**X6 done 2026-10-12:** the engine embeds in a React 19 shell (persistent canvas pattern; D8 stands, no
iframe). It found and fixed two engine bugs with regression tests: any window or panel resize killed the
engine (missing `WindowResized` message; `web/tests/resize.mjs`), and hover stuck when the pointer left
the canvas (`pointerleave` and pointer capture; `interact.mjs`). Details in `docs/modeling/experiments.md`.
**V0 implemented and strict-gated 2026-10-12:** Rust-derived TS bridge (ADR 0009), React canvas host,
read-only document view and command input, Bun build/test chain, gallery/legacy comparison. Evidence:
[`v0-report.md`](../modeling/v0-report.md). Release stripped Brotli q9 3,370,251 B (+12,114 B from G1b).
Headed pointer lock/headset checks remain untested. Legacy comparison route is retained.

**G2 implemented and strict-gated 2026-10-12:** production evaluator-only WASM worker, stepped pure
evaluator, Rust-owned candidate sessions, bounded latest-request queue with accumulated commands,
mesh LRU, native boolean cancellation and browser termination/restart, atomic projection after asset
readiness, selection revision map and visible status. All 226 native tests and browser acceptance
passed; native/WASI evaluator and kernel parity, C++ sanitizers and advisories passed. Evidence:
[`g2-report.md`](../modeling/g2-report.md). Release stripped Brotli q9: runtime 3,398,760 B
(+28,509 B from V0), evaluator 217,936 B. Large result 2,361,960 B; 100 warm jobs leave cache/live
meshes bounded. Software-browser frame gaps are reported, not claimed as hardware performance.
Audit B02's worker/cancellation paths are covered; Quest CPU/GPU timing remains untested. Next: G3.

**G3 implemented (2026-10-12):** profile operators, extrude and revolve, the wall constraint, the text
form with source spans, the provider boundary with a labelled mock, parser and loader property fuzzing,
the spike classification and the three-piece gallery. All 15 shipped operators are `ready`.
`cargo xtask check --strict` passed (277 native tests and every browser suite). Evidence, cases,
sizes and limits are in [`g3-report.md`](../modeling/g3-report.md); decisions in ADRs 0010 and 0011.
Not claimed: headset behaviour, a real provider, 32 of the 53 spike programs (later gates), and the
comparison of control surfaces for a model (now run: X10). Next: V1 or G4, then the logic gates.

**Amendments after G3 (2026-10-12), not in the original plan.** (1) A test route for models through
MCP and a viewer mode that evaluates documents (`mcp-test-provider.md`), a lit studio scene, smooth
shading across shallow edges, an eased hover glow. (2) `solid.material` (colour, metal, roughness,
glow) pulled forward from G10, and `profile.smooth` (corner cutting) as a cheap stand-in until G5
curves and G7 subdivision. (3) **Order:** G4a, the small operators a model needs to build furniture
and machines (rect, circle, cone, torus, capsule, mirror, array, radial, repeat), is done before V1,
because what a model can build limits everything else; G4 proper (groups, instances, sockets) and
V1 follow. (4) **Decided by the owner (2026-10-13), [ADR 0012](../adr/0012-viewer-page-is-the-editing-surface.md):** the
engine's viewer page, not the React shell, is the editing surface. V1 and V2 are plain-JS panels
there (`engine/web/panels/`); the criteria below are unchanged, tested "through the real UI" of that page.

**V1 implemented (2026-10-13), on the viewer page:** outliner (parts with the nodes that build them,
collapse, diagnostic dots), inspector (widgets generated from `OperatorSpec`: units, kinds, drag
values, label, mute and bypass, delete), history list (labelled transactions, click to step to one),
problems list (links to nodes), and selection shared with the viewport in both directions (a
selected node lights up the parts it builds; a click on a part selects its node). Engine side:
`Session::history()`, `selection_*` exports, a selected-part glow. Not done from the V1 text:
"instances" in the tree (G4 proper) and a per-part pose in the inspector (parts have no pose in the
document yet).

**G4b implemented (2026-10-13):** groups and instances (document, commands, versions and upgrades,
unfolding before evaluation, sharing of equal prototypes). `cargo xtask check --strict` passed. Report:
[`g4b-report.md`](../modeling/g4b-report.md); design: [ADR 0013](../adr/0013-groups-are-subgraphs-expanded-before-evaluation.md).
G4 is not complete: instance sets and realization (G4c), sockets and attachment (G4d) and the text
form for groups (G4e) remain.

G4-G13, V1-V7 and L0-L9 incomplete; X2-X5 and X7-X9 not performed. No gate has passed merely because this
document was revised. Replace this line with per-milestone evidence as work lands:
commit, contract version, cases and commands run, pass, fail and skips, benchmark artifact, known
limitations, next dependency. Run `cargo xtask check --strict` from `engine/`, the separate
`cargo xtask advisories` gate, and explicit release builds with size measurement; strict checks do
not replace release measurements or headset testing.

**Next implementer:** start G0: confirm the Track 1 baseline, agree the vase transaction example,
write the ADRs and case lists, run X1 and X6, then build G1a, G1b, V0 and G2 before the modeling
catalog grows. The research list is not an instruction to vendor every library.
