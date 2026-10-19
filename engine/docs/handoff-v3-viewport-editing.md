# Handoff: V3 viewport editing (gizmos, parameter handles, overlays)

Read `handoff-common.md` first (rules, gate mechanics, architecture). Prerequisites are met (G4, V1).
Plan text: `docs/plans/track2-modeling.md` section 5, "V3". Related: ADR 0012 (the viewer page is the
editing surface), V6 (optional XR editing) builds on what you make here, so keep the maths shared.

## 1. What V3 must deliver (plan text, then my reading)
"Picking and selection sync with the outliner, inspector and graph through provenance; translate, rotate
and scale gizmos with snapping and pivot; parameter handles for profile points, loft sections, revolve
axis and radii; frame selected; overlays (wire, normals, bounds, diagnostics)."

*Accept:* dragging a gizmo produces **one coalesced command and one undo step**; a parameter handle edits
**the same parameter the inspector shows**; selecting a lost or ambiguous feature shows `selection.lost` or
`selection.ambiguous`; preview during a drag is cheap and the final result is committed on release;
**handles never edit disposable geometry**.

Loft sections arrive with G5 (not built); do handles for what exists (profile points, revolve, radii,
move/rotate/scale values, array spacing, socket frames) and leave a clean extension point.

## 2. What already exists (verified)
- **Selection sync is done for nodes**: `web/panels/selection.js` (page) and
  `ve-runtime/src/selection.rs` (engine, thread-local). A viewport click gives `(node, instance)` (G4c);
  panels highlight the whole set or one instance. Provenance from a part back to the nodes that build it
  is `partsFor` in `web/panels/graph.js` (pure; the page holds the graph).
- **Sockets are selectable in the inspector** (G4d, `panels/inspector.js`, `selection.selectSocket`) but
  **no frame gizmo is drawn** (ADR 0015 says drawing frame gizmos and freehand socket picking are V3).
- **Camera**: orbit, zoom, `viewer_frame(min, max)` (frames a box), `viewer_look_at`. Frame-selected needs
  only the selected parts' bounds; the page can compute them from the evaluated meshes or the engine can
  expose per-part bounds.
- **Interaction crate** `ve-interact` (pure): pointer, `grab::follow_one/follow_two`, snap hook, gesture
  machine. In `ve-runtime/src/interact.rs` a grab moves a **disposable entity** (`Interactable.grabbable`,
  `placement_of(transform)`). This is exactly the "never edit disposable geometry" trap: a gizmo must not
  reuse that path to change the document.
- **Parameter widgets** are generated from specs and caption-scrub already commits **once on release**
  (`panels/widgets.js numberField`), so scrub is one transaction today. There is **no history coalescing**
  in `ve-graph` (grep `coalesc` finds nothing). If you want live updates while dragging you must add it
  deliberately (see 4.3).
- **Rendering**: a custom forward WGSL pass (`ve-render/src/{pipeline,pass,material}.rs`, shaders in
  `shaders/forward.wgsl`) with hover/selected glow. There is **no line or overlay pipeline and no
  wireframe**. That is the largest engine piece of V3.
- **Preview vs final** are already different cache keys (G2: quality tiers, 200,000 preview triangles in
  `limits.md`). Use the preview tier while a handle drags.
- `doc_stage` exists but commits-after-geometry semantics, not a drag preview. Do not repurpose it blindly.

## 3. Cases to write first
`cases.md` has only a two-line draft for V3 near the end (the "later gates" section). The plan says a gate
whose cases are not listed is not ready, so expand it into a table before building.
Proposed (renumber freely, keep the ids stable once written):

| ID | Input | Expected |
|---|---|---|
| V3-01 | Drag the translate gizmo's X arrow on a part | One transaction, one undo step; the document equals the one made by the same `set_param` through a mock; no document traffic during the drag |
| V3-02 | Same drag, but the selected node is not a transform | An `xform.move` is inserted above it, consumers and outputs rewired, in one batch; undo removes it entirely |
| V3-03 | Drag the rotate ring and the scale handle | Same as V3-01 with `xform.rotate`, `xform.scale`; handedness and units correct (radians in the document, degrees shown) |
| V3-04 | Snapping (grid, angle) and pivot choice (origin, part centre, socket) | Snapped values exact; pivot does not shift unrelated geometry |
| V3-05 | Drag a profile point handle | `profile.polygon` `points` changes, same value the inspector shows; out-of-range drag is refused with the diagnostic and the handle returns |
| V3-06 | Drag a radius or revolve axis handle | Same parameter as the inspector; preview uses the preview tier; the final result is committed on release |
| V3-07 | Escape or pointer cancel during a drag | Nothing committed; document equal to before |
| V3-08 | Document changes under a held handle (undo, MCP edit, stale revision) | The drag is abandoned or refused with `conflict`; never applies to a stale base |
| V3-09 | Select a node that was removed, or a click on geometry built by several nodes | `selection.lost` / `selection.ambiguous` shown with candidates; nothing else selected |
| V3-10 | Handles on a node inside a group | Edits the definition and says so (the inspector banner does this today), or edits the instance override when the handle belongs to an exposed parameter |
| V3-11 | Handles on an instance-set member | Gizmo moves the set's parameters (array spacing), not one placement; an individual-instance gizmo is refused with a clear message |
| V3-12 | Frame selected | Camera frames the selection's bounds; works for sets and sockets |
| V3-13 | Overlays: wire, normals, bounds, diagnostics | Each toggles; none changes the document or the evaluation hash; diagnostics overlay marks the node's part |
| V3-14 | Gizmo while the evaluation is busy or failed | Gizmo stays usable on the last good scene; commit is queued or refused visibly |
| V3-15 | Screenshots (six fixed views, SwiftShader) | Reviewed by eye and stored as goldens |

## 4. Proposed design (change it with an ADR if you disagree: ADR 0016)
### 4.1 Where the maths lives
A pure module in `ve-interact` (say `gizmo/`): axis, plane and ring drag maths from a pointer ray, snapping,
pivot, screen-constant sizing, constraint to the part's local or world axes. Unit-test it natively against
exact expected numbers (it is the same maths V6 will drive from controllers). `ve-runtime` draws the
handles and feeds pointer rays; the page never does the 3D maths.

### 4.2 From drag to command
The page owns commands (typed-command rule). Engine emits `gizmo_begin`, `gizmo_update`, `gizmo_end`
(or polled state) with the *delta in document units*. On release the page builds one batch:
- selected node is `xform.move/rotate/scale`: `set_param` on it (compose with the current value);
- otherwise insert a transform after it: `add_node`, `connect` it to the selected node, `connect` every
  consumer input that read the selected node to the new node, move `outputs` that named it. This rewiring
  is easy to get wrong; implement it once as a builder with tests (a Rust helper in `ve-graph`, exposed
  like the other `doc_*` calls, is better than JS so the MCP and the page share it), and label the
  transaction ("Move part").
- Instance sets (G4c): the gizmo edits the set's own parameters; it never edits a placement matrix.
During the drag the engine moves a **preview transform** on the displayed entity (cheap, no kernel call,
no document change); on release the evaluation result replaces it. That satisfies "preview is cheap".

### 4.3 Parameter handles that change geometry
A radius or a profile point changes the mesh, so preview needs re-evaluation. Options, in order of
preference: (a) throttle (about 4 per second, same figure as the plan's rebuild-action rate) preview-tier
evaluations of a **candidate** document that is never committed, drop stale ones (the G2 protocol already
rejects late preview results), commit on release; (b) show only a ghost outline until release. Measure
(a) with a real handle drag before choosing, and record numbers. Adding history coalescing is **not**
needed for this design because nothing is committed until release; do not build it unless a case needs it.

### 4.4 Drawing
Add a small overlay pass (lines, unlit, depth-tested option) to `ve-render`: handles, bounds, normals,
wire. Keep handle meshes procedural (no assets). Stereo: the pass must work per eye (check
`ve-render/src/stereo.rs`) even though V6 is optional, or XR will regress.

### 4.5 Picking handles
Handle picking must win over part picking and must not fire `select_on_click`. Reuse the ray-vs-shape
code in `ve-interact` (see how `pick` works in `ve-runtime/src/interact.rs`), add handle shapes, and
give handles a priority in the pick result.

## 5. Order of work
1. Write ADR 0016 and the V3 case table (cases.md), plus a status line in the plan.
2. Pure gizmo maths in `ve-interact` with native tests (V3-04 maths, V3-07).
3. Overlay line pass in `ve-render`; draw bounds first (cheapest, proves the pass).
4. Translate gizmo end to end (V3-01, V3-02) including the insert-after builder and its undo.
5. Rotate and scale, snapping, pivot.
6. Parameter handles: radius, revolve axis, profile points (needs a per-operator handle registry; keep it
   data-driven off `OperatorSpec` where possible so a new operator does not need new UI code).
7. Sockets: draw frames, pick them, attach-aware move.
8. Frame selected, wire and normals overlays, diagnostics overlay, `selection.lost/ambiguous` UI.
9. Browser test `web/tests/gizmo.mjs` driven by real pointer events (not by calling the API), screenshots,
   report `docs/modeling/v3-report.md`, add to `xtask/src/web.rs`, strict gate.

## 6. Risks and traps
- SwiftShader frames are slow (the G4c probe showed ~6 fps at 120,000 triangles); the first Playwright
  click test failed because the mouse was released between slow frames. Hold the pointer across frames
  and wait for frames, as `web/tests/instances.mjs` does.
- Pointer lock: viewer mouse look locks the pointer on click (`web/viewer.js`). Gizmo dragging must work
  while the mouse is released, as hover and click do today; define the interaction between lock and drag.
- The selection poll in `selection.js` runs every 100 ms; a drag must not fight it.
- Euler order is X, Y, Z (ADR 0015 for frames, `xform.rotate` for nodes). A rotate gizmo that writes
  Euler angles needs a decomposition with a stated behaviour near gimbal lock; test it.
- Mirrored parts (negative determinant) flip handle handedness; test a mirrored part.
- Do not claim headset behaviour: nothing here is tested on a Quest.

## 7. Done means
All V3 cases pass, strict gate passes, report with software-render caveats, plan status paragraph, ADR
0016, the MCP guide untouched unless a new command is added, no `#[allow]`, no attribution lines.
