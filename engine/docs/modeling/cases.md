# Acceptance case lists (v1)

Each case has an ID, an input, and the expected outcome. A gate passes only when every case in its
list passes and the evidence (commands run, results, sizes) is recorded in the plan's status section.
Lists for G1a to V2 are complete for starting work. Lists from G4 on are **draft**: they must be
expanded and reviewed before their gate starts. Tolerances: lengths 1e-6 m, volumes 1e-4 relative
unless a case says otherwise. "Mock" means the labelled deterministic mock provider.

## G1a: document, commands, history
| ID | Input | Expected |
|---|---|---|
| G1a-01 | Empty document, `add_node box` | Revision 1; one node with a generated ID; spec defaults filled |
| G1a-02 | Same command built by hand and by mock | Byte-identical canonical JSON |
| G1a-03 | Batch whose third command fails | Document unchanged, revision unchanged, `rejected` lists the failure with the command index |
| G1a-04 | `connect` that creates a cycle | `graph.cycle` naming the nodes; unchanged |
| G1a-05 | `connect` output `solid` to a `profile` port | `graph.port_type` |
| G1a-06 | `set_param` radius -1 | `param.range` with the range |
| G1a-07 | `set_param` NaN, then infinity | `param.non_finite` each |
| G1a-08 | `remove_node` still read by another node | Rejected; with `cascade: true` both go |
| G1a-09 | Undo after a batch of 5 commands | One step restores the previous document exactly, IDs included |
| G1a-10 | Redo after undo | The document equals the one before the undo |
| G1a-11 | New batch after undo | Redo stack cleared |
| G1a-12 | Same `transaction` key sent twice | Second returns `already_applied`; revision unchanged |
| G1a-13 | `base_revision` behind current | `revision.conflict` with changed nodes; nothing applied |
| G1a-14 | `editor_set_position` | Revision advances, evaluation hash unchanged |
| G1a-15 | Rename a label | ID unchanged; evaluation hash unchanged |
| G1a-16 | 1,000-command batch, then 1,001 | 1,000 accepted; 1,001 rejected with `budget.commands`, `limit` 1,000, `observed` 1,001 |
| G1a-17 | Delete then add: new node | New ID never equals a deleted ID |
| G1a-18 | Kernel round trip of a solid with unsupported attributes | Explicit rejection or reported loss, never silent |
| G1a-19 | Property test: random valid command sequences, undo all | Equals the empty document |
| G1a-20 | Property test: replay a recorded history | Equals the live document at every revision |
| G1a-21 | One batch adds a profile and a revolve by alias, links them and names an output | Committed; the revolve reads the profile; the output names the revolve |
| G1a-22 | Alias used before its definition, defined twice, or used in a later batch | `graph.unknown_node` (first and third) and `graph.duplicate_id` (second); nothing applied |
| G1a-23 | Property test: undo everything then redo everything | Equals the final document |

## G1b: persistence and bridge
| ID | Input | Expected |
|---|---|---|
| G1b-01 | Save, reload | Equal document: IDs, layout, revision and the ID counter (the undo history is not saved: a reloaded session starts with empty history) |
| G1b-02 | A torn write (half of a save lands) on the older of the two slots | The newest intact save loads; the damaged copy is reported as a warning; loading writes nothing |
| G1b-03 | Truncated or bit-flipped file | One slot damaged: the other loads with a `store.corrupt` warning. Both damaged: `store.corrupt` error. A flipped byte is caught by the checksum |
| G1b-04 | File with schema 99, with a valid and with an unrecognised checksum | `store.newer_version` for both, on load and on save; file bytes unchanged; a save never overwrites it |
| G1b-05 | File with an unknown operator | `store.unknown_op` listing node IDs; file kept; other nodes viewable |
| G1b-06 | Frozen schema-1 fixture, migration test hook | Migrates to current schema; result equals expected document |
| G1b-07 | Migration step that fails | `store.migration_failed`; previous save kept |
| G1b-08 | Quota exceeded (simulated) | `store.quota`; previous save kept; message names bytes needed |
| G1b-09 | Document at the 2 MiB limit, then over | Accepts, then refuses with a clear error |
| G1b-10 | Viewer bridge: send a batch from the page | The engine document changes; the page shows the new revision |
| G1b-11 | Bridge: malformed batch | Rejected with diagnostics; no engine error |
| G1b-12 | Two tabs, same document | Each save is whole; a later load is exactly one tab's last saved document, never a mix. Edits in two tabs are not merged: the last save wins (generation 3 over 2) |
| G1b-13 | Floating-point values saved and reloaded | Bit-identical (parsing is exact), so checksums and document equality hold |
| G1b-14 | A reloaded or migrated document with an ID counter behind its IDs | New nodes never get an ID already in use |
| G1b-15 | Read-only document (newer file) in the page | Opens with the reason; edits and saves are `store.read_only`; the stored file is unchanged; the panel says READ-ONLY |

## V0: editor shell

Implemented and gated 2026-10-12: [V0 report](v0-report.md). V0-06 used a temporary Rust serde
field rename to prove the schema check fails; V0-08 screenshots were reviewed against the report's
composition rubric. The legacy viewport is retained as a comparison route. Headed pointer lock and
headset behavior remain untested; browser checks use SwiftShader.

| ID | Input | Expected |
|---|---|---|
| V0-01 | Load the editor | Canvas renders the gallery; no console errors |
| V0-02 | Send a command from the UI | Engine document and the view both update |
| V0-03 | Resize and device-pixel-ratio change | Canvas and layout follow |
| V0-04 | Focus moves between a text field and the canvas | Keys go to the focused element; pointer lock released on blur |
| V0-05 | Two editor instances in one browser | No interference in state or storage keys |
| V0-06 | Change a Rust command schema field without regenerating | The TS build fails |
| V0-07 | Page unload and reload | Engine instance recreated cleanly; no leaked workers or listeners (counted) |
| V0-08 | Old three.js viewport and new canvas compared on the same scene | Match the agreed rubric before the old one is removed |
| V0-X | Experiment X6 report | Lifecycle, focus, pointer lock and two-instance results recorded; iframe fallback decided if any fail |

## G2: worker, limits, status panel

Implemented and strict-gated 2026-10-12: [G2 evidence](g2-report.md). Native/WASI evaluator parity,
real worker/browser checks and native cancellation cover the cases below. G2-01's former F1
dependency is corrected to the primitive/boolean fixture; F1 requires G3. Cache eviction uses the
production algorithm with a reduced test cap. Upload CPU time and window frame gaps are measured;
headset GPU/frame time is untested, and the software-browser run is not a 60 fps result.

| ID | Input | Expected |
|---|---|---|
| G2-01 | Evaluate the primitive/boolean fixture in the worker | Same geometry as native evaluation/direct kernel (tolerance above); F1 needs G3 profile operators and is checked there |
| G2-02 | Forced 15-second job | Terminated at the hard limit; `eval.worker_timeout`; last scene kept; restart succeeds |
| G2-03 | Worker killed mid-job | `eval.worker_crash`; restart; scene kept |
| G2-04 | Ten edits within 100 ms | Latest result shown; independent earlier edits are all present in the final document |
| G2-05 | Two edits on the same node in flight | Later one conflicts or merges per the rule; never silently lost |
| G2-06 | Preview result arrives after the final | Discarded: `eval.stale_result` |
| G2-07 | Result for an older revision | Discarded |
| G2-08 | Cancel during a boolean tree | Status `Cancelled` observed; cancelled kernel object not cached |
| G2-09 | Cache over 128 MiB | Eviction by use; cap never exceeded |
| G2-10 | Over-budget graph (limits file) | Refused before allocation with `budget.*` |
| G2-11 | Upload larger than 8 MiB | Spread across frames; frame time stays inside the cap |
| G2-12 | Reload during a job | Clean start; no stale result applied |
| G2-13 | Frame and input loop during a stuck job | Window stays responsive (frame pacing measured) |
| G2-14 | Status panel | Shows worker state, queue length, revision, last failure and recovery for G2-02 and G2-03 |
| G2-15 | 100 regenerations | Live handles, buffers and cache bytes bounded after warm-up (recorded) |
| G2-X | Experiment X1 report | Worker WASM size, startup, termination and transfer costs recorded; packaging decision |

## G3: language to model
| ID | Input | Expected |
|---|---|---|
| G3-01 | Text for F1 | Parses to the same document as the mock commands; IDs assigned once |
| G3-02 | Canonical serialisation twice | Identical bytes |
| G3-03 | Unknown operator `solid.revolv` | `parse.unknown_op` suggests `solid.revolve` |
| G3-04 | Missing required parameter | `parse.missing_param` with type and unit |
| G3-05 | Wrong port type | `graph.port_type` |
| G3-06 | Forward reference and cycle | Rejected with node names |
| G3-07 | `profile.offset` of a rectangle, delta -0.1 | Area equals the analytic offset area (tolerance 1e-6 relative) |
| G3-08 | `solid.extrude` twist 90 degrees, height h, unit square | Volume equals the untwisted volume within 1e-3; bounds correct |
| G3-09 | `solid.revolve` of a rectangle about Y, 360 degrees | 64 segments: faceted analytic volume within 1e-6 and circle within 2e-3; 128 segments: circle within 1e-3 (ADR 0010) |
| G3-10 | Revolve 180 degrees | Half the full volume within 1e-3 |
| G3-11 | Self-intersecting profile | Defined error, no crash |
| G3-12 | Zero-area profile and zero height extrude | `kernel.invalid_input` or `result.empty` as specified; distinct outcomes |
| G3-13 | F1: `check.min_wall` passes; widen opening in both profiles | Passes; node IDs unchanged |
| G3-14 | F1: widen only the inner profile | `constraint.min_wall`; document unchanged; widening the outer profile alone thickens the wall (ADR 0010) |
| G3-15 | Mock: "make a hollow vase" | Produces the F1 document |
| G3-16 | Mock: "widen its opening" | One batch changing the existing nodes' parameters |
| G3-17 | Stale revision from the provider | `revision.conflict`; repaired once or reported; cap of one repair |
| G3-18 | Provider returns executable code or an unknown field | `provider.invalid_patch`; nothing applied |
| G3-19 | Provider key | Never present in the browser bundle or in logs (search test) |
| G3-20 | Incremental evaluation after changing `segments` | Only the dependent nodes re-run (counts) |
| G3-21 | 53 spike programs | Each classified supported, changed or deferred, with a reason in a table; supported ones run |
| G3-22 | Parser and loader fuzz targets | Run for the agreed time with no crash |
| G3-23 | Gallery | Contains vase, twisted extrusion and offset plate, reviewed from the fixed views |

## V1: outliner, inspector, history
| ID | Input | Expected |
|---|---|---|
| V1-01 | Edit a parameter in the inspector | The command equals the one a mock would send; document equal |
| V1-02 | Value out of range | Refused with the diagnostic text; field shows the nearest valid value |
| V1-03 | Drag-scrub a slider | One transaction, one undo step |
| V1-04 | Select in the outliner | The object highlights in the viewport; the reverse works |
| V1-05 | Select a lost feature | `selection.lost` shown, nothing else selected |
| V1-06 | History list undo and redo | Matches G1a behaviour |
| V1-07 | Diagnostics list click | Selects the node and scrolls to it |
| V1-08 | Widgets for every ready operator | Generated from the specs; a spec change changes the widget with no UI code change |
| V1-09 | Keyboard walkthrough | All panels reachable; focus visible |
| V1-10 | Rename a part in the outliner | Label changes, ID does not |

## V2: node graph editor v1
| ID | Input | Expected |
|---|---|---|
| V2-01 | Build F1 in the graph by UI | Document equals the mock-command document |
| V2-02 | Drag a link to an incompatible port | Refused, reason shown; no change |
| V2-03 | Drag a link that closes a cycle | Refused with `graph.cycle` text |
| V2-04 | Search to add | Lists only `ready` operators with cost and readiness; fuzzy match |
| V2-05 | Edit in the text view | Graph updates; document equals; and the reverse |
| V2-06 | Error with a source span | Highlights both the text and the node |
| V2-07 | Move a node | Zero evaluations (instrumented count) |
| V2-08 | Frames and reroutes | Saved in the editor layer; reload restores them |
| V2-09 | Mute and bypass | Evaluation matches the spec (bypass passes the first input when types allow) |
| V2-10 | Copy and paste a subgraph | New IDs, internal links preserved, external links dropped |
| V2-11 | "View this node's output" pin | Viewport shows that node's result; unpin restores |
| V2-12 | F3 (200 nodes) pan, zoom, drag | No frame over 33 ms on the baseline machine |
| V2-13 | Undo and redo through the graph | Matches G1a |
| V2-14 | Keyboard add and connect | Possible without a mouse |
| V2-X | Experiment X7 report | 200 and 500 node results, typed socket and frame support; adopt or replace |
| V2-15 | Editor screenshots | Reviewed and stored as goldens |

## G4: reuse and assembly
G4 is built in slices; the gate passes when all four lists pass. **G4a** (small operators, mirror and
pattern operators) is done and has its own report. Design: ADR 0013. The wheel is the fixture: a
`group.input`-free group of a tyre (torus), a hub (cylinder) and four spokes (box, radial), exposing
`radius` (the tyre's major radius) and `width` (the hub's height); its output is the union.

### G4b: groups and instances
| ID | Input | Expected |
|---|---|---|
| G4b-01 | `create_group` from the wheel nodes with an output member | One group `g_`, version 1; the members carry the group; no member is a top-level part |
| G4b-02 | `create_group` where a member reads a node outside the group | Rejected `group.leaks` naming the member and the outside node; document unchanged |
| G4b-03 | `create_group` where a node outside the group reads a member | Rejected `group.leaks`; document unchanged |
| G4b-04 | `add_instance` of the wheel four times, each followed by `xform.move` | Four instance nodes with distinct IDs, each pinned to version 1, output type solid |
| G4b-05 | `expose_param` on a member's parameter, then `set_instance_param` on one instance | The override is stored on that instance only; the others and the definition are unchanged |
| G4b-06 | `set_instance_param` with a value outside the member's range, or an unexposed name | Rejected with the member parameter's own diagnostic (`param.range`) or `group.unknown_param`; unchanged |
| G4b-07 | `clear_instance_param` | The override is gone; the instance follows the definition again |
| G4b-08 | `set_param` on a member (a definition edit) | Every instance reflects it; no instance data and no ID changes |
| G4b-09 | Instance inside a group of the group it sits in, directly or through a second group | Rejected `group.recursive` naming the groups in the loop; unchanged |
| G4b-10 | Interface change (`expose_param`, `unexpose_param`, `set_group_output`, add or remove a `group.input`) | `group.version` increases by one; instances keep their pinned version |
| G4b-11 | Evaluating a document with an instance pinned behind its group | Refused `group.outdated` naming the instance; other nodes untouched |
| G4b-12 | `upgrade_group` on an outdated instance | Pinned version moves up; overrides for parameters that are still exposed stay, the others are dropped and reported; every other node, group and ID is byte-for-byte unchanged |
| G4b-13 | `remove_node` of an exposed member or of the output member | Rejected `group.in_use` unless the interface command removing it came first |
| G4b-13b | `remove_node` of a `group.input` (after disconnecting what reads it) | Allowed, and an interface change: the version increases by one |
| G4b-14 | `remove_group` with instances, then without | `group.in_use` naming the instances; then removed with its members |
| G4b-15 | Undo and redo of every group command | The document returns exactly, IDs and version included (G1a rules) |
| G4b-16 | Save, reload, migrate | Groups, membership, instances and overrides survive; a document without groups loads unchanged |
| G4b-17 | A group with `group.input` members, an instance with links to them | Links by position, type checked on connect (`graph.port_type`); a missing link is `graph.missing_input` at evaluation |
| G4b-18 | Nested groups: a rack group of two wheel instances, instanced twice | Evaluates; expansion depth 2 is reported |
| G4b-19 | `MAX_GROUP_DEPTH` exceeded; expanded nodes over the budget | Refused before any kernel call with `budget.group_depth` or `budget.expanded_nodes`, limit and observed value |
| G4b-20 | `to_text` of a document with groups | Prints (G4e); an instance behind its group has no text form and is `text.unsupported` until upgraded: never a text that reads back wrongly |

### G4b evaluation
| ID | Input | Expected |
|---|---|---|
| G4b-E1 | Wheel x4 with equal parameters | Four solids with equal volume; the prototype's operators run once (`evaluated_nodes` counts one wheel plus four moves, not four wheels) |
| G4b-E2 | Wheel x4, one with `radius` overridden | That instance's volume differs by the analytic amount; the other three are unchanged and still share one evaluation |
| G4b-E3 | Definition edit (`set_param` on a spoke) | All four volumes change; operators re-run for the changed definition only once |
| G4b-E4 | Edit one instance's `xform.move` | Only that instance's move re-runs; the prototype and the other instances come from cache |
| G4b-E5 | Output of an instance under `solid.material` | The material reaches the part; the part's node is the material node |
| G4b-E6 | A kernel or constraint failure inside a member | Diagnostic names the instance, with the member as `related` |
| G4b-E7 | Muted and bypassed instance | Same as any node: empty result, or input passed through |
| G4b-E8 | Evaluation hash | Changes when a member, an override, a version or the group output changes; unchanged by layout and by renaming a group |

### G4c: instance sets and explicit realization

Implemented 2026-10-13. All seven cases pass in `ve-modeling/tests/g4c_sets.rs` and
`web/tests/instances.mjs`; strict gate passed with 360 native tests and all browser suites.
See [G4c report](g4c-report.md) and [ADR 0014](../adr/0014-instance-sets.md).
| ID | Input | Expected |
|---|---|---|
| G4c-01 | `inst.array`, `inst.radial` and `inst.repeat` of a solid or an instance | An instance set that shares one prototype; no geometry is unioned |
| G4c-02 | 10,000 cheap instances (a 12-triangle box) | Valid as a set; shown with shared geometry |
| G4c-03 | `inst.realize` of the 10,000 | Accepted or refused by predicted triangles and bytes (`budget.triangles` / `budget.memory`) before any union |
| G4c-04 | `inst.realize` of a few instances | One solid equal to the union of the placed copies |
| G4c-05 | Instance set with a mirrored transform | Supported by flipping winding, with positive volume; never an unchecked negative scale |
| G4c-06 | Render and pick of a 10,000-instance set | One mesh upload, many placements; picking returns the instance index |
| G4c-07 | Count products near overflow | Refused with the limit, no wraparound |

### G4d: named parts, sockets and attachment

Implemented (2026-10-13): all G4d-01 through G4d-05 cases have native coverage. The strict gate
passed, including socket inspector selection and group listing. See
[G4d report](g4d-report.md) and [ADR 0015](../adr/0015-assembly-frames.md).
| ID | Input | Expected |
|---|---|---|
| G4d-01 | Socket (named frame on a member or part) with position and orientation | Exposed by name through the group; selectable |
| G4d-02 | `xform.attach` of a part's socket to another socket | The part moves so the frames coincide; pivot is the socket; handedness preserved |
| G4d-03 | Move the parent | Attached parts follow; the unrelated ones do not |
| G4d-04 | `xform.align` of a face normal or axis | Defined pivot and result for parallel and anti-parallel inputs |
| G4d-05 | Attach with a missing socket or a type mismatch | Refused with the socket named |

### G4e: text form for groups
| ID | Input | Expected |
|---|---|---|
| G4e-01 | `group wheel(radius, width) { ... }` and `wheel@1(radius=0.4)` | Reads to the same commands the editor sends; prints back identically |
| G4e-02 | A model builds the wheel group and four instances through MCP | Evaluates; matches the G4b-E1 volumes |

### V3: viewport editing

Case list established before implementation (2026-10-13). See [ADR 0016](../adr/0016-viewport-editing.md)
for the authored-command and preview contract.

| ID | Input | Expected |
|---|---|---|
| V3-01 | Drag the translate gizmo's X arrow on a transform node | One batch/undo step; matches inspector `set_param`; no document mutation during drag |
| V3-02 | Drag translate on a non-transform node | Insert transform after node, rewire consumers and outputs atomically; undo restores graph |
| V3-03 | Drag rotate ring and scale handle | Correct quaternion/Euler composition, units and handedness; one commit per release |
| V3-04 | Axis/plane/ring drag with grid, angle snap and origin/centre/socket pivot | Exact snapping; pivot stays fixed; pure math tests include parallel rays |
| V3-05 | Drag a `profile.polygon` point | Updates the inspector's `points`; invalid drag shows diagnostic and restores handle |
| V3-06 | Drag a radius or revolve-axis handle | Same authored parameter as inspector; preview tier during drag and final evaluation on release |
| V3-07 | Escape, pointer cancel or lost capture during drag | No command or document change; preview is cleared |
| V3-08 | Undo, MCP edit or stale revision during drag | Drag cancels or reports `conflict`; stale base is never committed |
| V3-09 | Removed or provenance-ambiguous feature selection | `selection.lost` / `selection.ambiguous` with candidates; no unrelated selection |
| V3-10 | Handle in group definition or exposed instance parameter | Edits definition with banner, or edits the instance override; identity is preserved |
| V3-11 | Handle on instance set or individual placement | Set parameters are editable; a placement-matrix edit is refused visibly |
| V3-12 | Frame selected part, set member or socket | Camera frames the selected bounds and updates orbit focus |
| V3-13 | Toggle wire, normals, bounds and diagnostic overlays | Display changes only; document and evaluation hash remain unchanged |
| V3-14 | Gizmo while evaluation is busy or failed | Last good scene remains interactive; commit queues or refuses with a visible reason |
| V3-15 | Six fixed screenshot views under SwiftShader | Reviewed and stored as report artifacts; renderer caveat stated |

Evidence (2026-10-13; headless Chromium on SwiftShader throughout, no real GPU, no headset). Tests are in
`engine/web/tests/` unless a Rust path is given. "Gap" is stated where a row is only partly covered.

| ID | Covered by | Gap |
|---|---|---|
| V3-01 | `v3-gizmo.mjs` (real pointer drag, one undo), `v3-transform-commands.mjs` (the edit equals the inspector's `set_param`) | |
| V3-02 | `v3-transform-commands.mjs` (rewires, outputs, undo equals the original), `v3-gizmo.mjs` | |
| V3-03 | `v3-gizmo.mjs` (rotate ring, scale handle), `v3-transform-commands.mjs` (composition, the gimbal-lock rule), `ve-modeling/tests/v3_pivot.rs` (a mirrored part) | Rotations are composed by inserting a rotation node, never decomposed to Euler angles (ADR 0016) |
| V3-04 | `ve-interact/tests/v3_gizmo_math.rs` (snapping, parallel rays), `v3_pivot.rs` (the pivot stays fixed through the real kernel, with a control), `v3-frames.mjs` (socket pivot) | World axes only: no local-axis handles |
| V3-05 | `v3-profile-points.mjs` | |
| V3-06 | `v3-radius-handle.mjs`, `v3-parameter-handles.mjs` (revolve angle) | The preview during a drag is a projected ghost; a preview-tier rebuild per drag step is not built |
| V3-07 | `v3-gizmo.mjs` (Escape), `v3-drag-guards.mjs` (lost capture, `pointercancel`, window blur, Escape) | `pointercancel` and `blur` are dispatched on the real elements; a browser cannot be told to send them |
| V3-08 | `v3-drag-guards.mjs` (a real drag with the revision moved under it; a handle pressed while an edit is evaluating), `v3-transform-commands.mjs` (the three conflict reasons) | The document is changed through the document API; an undo or an MCP edit moves the revision the same way but is not separately driven |
| V3-09 | `panels.mjs` (`selection.lost`), `panels-graph.mjs` (`featureCandidates`), `v3-scope.mjs` (a real click: `selection.ambiguous` with the operands, then one chosen; a single-input chain resolves to its feature) | Ambiguity is by construction (operands of a combining node), not by face provenance, which the evaluator does not record |
| V3-10 | `v3-scope.mjs` (a handle on a group member edits the definition, inside the group, with the banner; every use changes; no version bump) | A handle that edits an instance's exposed-parameter override is not built |
| V3-11 | `v3-scope.mjs` (a real click picks placement 1 and offers no handle; the `inst.array` spacing handle edits the pattern node), `v3-parameter-handles.mjs` (`xform.array`) | `inst.repeat` step handle exists but has no browser case |
| V3-12 | `v3-frames.mjs` (F frames the selected part from far away) | Framing a set member or a socket is not separately tested |
| V3-13 | `v3-gizmo.mjs` (bounds, wire, normals), `v3-scope.mjs` (a real node failure: the diagnostic overlay outlines it with nothing selected, then stops when a later evaluation succeeds; the revision and the evaluation hash do not move) | |
| V3-14 | `v3-drag-guards.mjs` (refused with the reason while an edit evaluates), `v3-scope.mjs` (after a failed evaluation the last good scene is selectable and a real drag commits) | |
| V3-15 | `v3-gizmo.mjs` writes the six views to `target/reports/v3-view-*.png` | Reviewed by eye on SwiftShader only |

Socket frame gizmo (ADR 0015, ADR 0016): `v3-frames.mjs` checks the frame axes numerically (the foot
socket's quarter turn about Z), a disc at the projected origin, a real click selecting the socket in the
viewport and the inspector with no command sent, and the move handles starting at the socket when it is the pivot.

**V3:** a gizmo drag is one coalesced command; a handle edits the inspector's parameter; preview during
a drag, commit on release; lost and ambiguous selections shown.
**V4:** making the wheel group by UI equals the G4 fixture; edit definition versus edit instance;
upgrade conflict shown before applying.
### G5: curve and surface construction

Cases fixed before implementation (2026-10-13); unsupported topology is a specified refusal, not a
silent approximation. Adaptive tessellation is quality-tier bounded and preflighted.

| ID | Input | Expected |
|---|---|---|
| G5-01 | Sweep a circular profile along a circular path | `g5_closed.rs`: closed solid; volume within 1% of Pappus; bounds match path and profile radii |
| G5-02 | Loft equal polygon sections at equal spacing | Volume, bounds and cap topology match the corresponding extrusion within 1e-5 relative volume |
| G5-03 | Loft six compatible sections into a vase | `g5_hollow.rs` checks the composed vase and pipes; for matching corresponding vertices with linearly interpolated sections and the inner loft raised above the floor, the minimum outer-minus-inner gap is bounded by the minimum section gap. A 0.005 m section gap at an unsampled height is caught while both 0.035 m neighboring samples exceed 0.04 m. This bound does not cover mismatched correspondence, non-linear interpolation or floor/rim transitions. |
| G5-04 | Sweep a round profile along a cubic Bézier elbow | Rotation-minimizing frames stay continuous; no cross-section inversion or pinching; output is closed |
| G5-05 | Tube radius exceeds local bend radius at a near-180° turn | Refuse with `curve.sweep_curvature`, node, tube radius and estimated curvature radius |
| G5-06 | Consecutive repeated path points or a zero-length segment | Refuse with a node-scoped `geometry.degenerate_curve`; document is unchanged |
| G5-07 | Sweep around a closed periodic path | `g5_closed.rs`: circle, authored closed polyline and endpoint-matched cubic Bézier sweeps have welded output with no boundary edges; nonplanar circle frame seam agrees within 1e-6 radians |
| G5-08 | Loft sections with different point counts, holes, or a zero-area section | `g5_refusals.rs`: different counts and hole loops are refused with `surface.incompatible_sections`; empty section is refused with `surface.degenerate_section` |
| G5-09 | Bounded bend, twist and taper on a tessellated solid | `g5_deformers.rs`: edge and face-centroid samples drive up to two local, shared-edge conforming refinement rounds (16x maximum growth); hard-bent thin box and twisted loft remain one valid solid; sampled face orientation is checked and unmeetable tolerance is refused. The sampled tolerance is not a continuous Hausdorff bound. |
| G5-10 | Sweep or loft predicted above quality triangle budget | Refuse before mesh allocation/kernel call with `budget.triangles`, limit and observed count |
| G5-11 | Curve/surface graph save, reload, undo, redo, group and instance | `g5_persistence.rs`: canonical save/reload, shared prototype evaluation, history, one-instance override and group-definition edit pass |
| G5-12 | Native and WASI corpus for curve/sweep/loft results | Twelve modeling outputs match byte-for-byte in `cargo xtask parity`, including circle, open/closed polyline and Bézier sweeps, loft and twist/taper/bend |
| G5-13 | Gallery: vase, pipe, curved panel, coil spring, turbine blade, staircase, lamp, goblet, bridge, sword, toy car, smooth-vase comparison and fused multi-colour car | All thirteen programs are checked in under `crates/ve-modeling/tests/gallery/`, pass native evaluation and six fixed SwiftShader views in `web/tests/gallery.mjs`; representative images are generated into ignored `target/reports/gallery/`. Programs cover sweeps, lofts, deformation, groups, instances, repeats, booleans and material regions. These are authored regression fixtures, not provider-generated results. |
| G5-14 | MCP prompt builds a pipe and lofted vase, then edits each by stable ID | Scripted MCP client builds/edits both through the viewer page and preserves IDs; no external natural-language model/provider is configured or tested |

**Additional foundation cases (not substitutes for G5-01 through G5-14):**

| ID | Input | Expected | Status |
|---|---|---|---|
| G5-15 | A polyline curve is selected as the only output | Rejected as a render output because curves are intermediate values | Pass |
| G5-16 | Sweep a 48-point circle along a straight 2 m polyline | Closed Manifold solid; volume within 1% of the analytic cylinder | Pass |
| G5-17 | Sweep a circle along a cubic Bézier path | Closed solid follows the curve endpoint | Pass |
| G5-18 | Sweep whose predicted triangles exceed Preview budget | Refused as `budget.triangles` in planning | Pass |
| G5-20 | Loft two equal rectangle sections at equal spacing | Volume and Y bounds match the corresponding extrusion | Pass |
| G5-21 | Loft three equal-count circle sections with changing radii | Closed convex solid; section height and analytic frustum volume match | Pass |
| G5-22 | MCP text path builds an open Bézier sweep and a matching-section loft | Both programs parse, evaluate and return no failure through the viewer bridge | Pass (SwiftShader) |
| G5-23 | Double-reflection frame transport through a smooth inflection and along a straight path | Orthonormal frame, positive continuity and fixed straight-path orientation | Pass (unit tests) |
| G5-24 | Sweep a tube wider than the sampled path's local bend radius | Node-scoped `curve.sweep_curvature` includes tube and estimated curvature radii | Pass |
| G5-25 | A swept tube passes through itself (interior crossing, cap/tube contact, bend-vertex touch, parallel overlap, and near-miss) | Existing sufficient path test remains unchanged; a budgeted triangle-grid pass catches cap/tube, bend-vertex and parallel overlap cases. A 0.05 m clear near-miss builds. Dense valid tubes (straight and winding, 200 points) are not refused (only box-overlapping triangle pairs count against the pair budget; cells are sized to the average triangle). Regression cases are in `g5_surfaces.rs` and `g5_mesh_overlap.rs`; numerical tolerance and configured cell/pair budgets remain limits. | Pass (focused Rust tests) |
| G5-26 | Loft with `steps` above 1 (four-section vase) | Straight (`steps=1`) output is bit-identical to the old loft; blended output passes through every section, keeps total height, overshoots the belly by under 15 %, stays within 25 % of the straight volume, and its extra rows are in the triangle estimate (a 64-section, 256-point loft at `steps=8` is refused in Preview). A blended row that stops being convex or flips winding is refused. `g5_loft_blend.rs`, `loft_blend` unit test | Pass |
| G5-27 | Two-colour boolean, move, mirror, array, radial, repeat | One fused solid, one part per colour (`Part.region`); volume shares sum to the solid's volume; a move keeps both colours; the patterns keep both; coincident copies (empty kernel runs) do not break it; single-colour and unpainted documents take the old path unchanged. `material_regions.rs`, kernel `origin_runs.rs`, parity corpus (16 modeling lines identical natively and in WASI) | Pass |
| G5-28 | Cutter colour on a difference | An uncoloured cutter leaves the body colour on the cavity wall (one part); a coloured cutter inlays its colour on the cut faces (two parts, green faces are the cavity wall) | Pass |
| G5-29 | `solid.material` on a placement set | The set stays a set (same number of placements of one prototype) and every placement carries the surface. `material_instances.rs` | Pass |
| G5-30 | Deformers on boolean output with zero-area or sliver faces, and on small or thin solids | A grooved blade bent 0.07 rad, a hollow shade twisted 0.45 rad and a sub-millimetre plate twisted are not refused as folded; faces thinner than the f32 grid are skipped by the orientation check and the fold test is scale-free. `g5_deformers.rs` | Pass |
| G5-31 | Five chained unions of overlapping boxes in Preview | Thirteen coincident boxes enter each union stage; five chained stages evaluate to the 12-triangle box under the 200,000 Preview budget. Each union estimate is the input estimate sum plus a capped 16,384-triangle intersection allowance, avoiding repeated fourfold growth. `g5_performance.rs` | Pass |
## G6: polygon topology and restricted editing (in progress; graph integration remains gated)

Contracts and operator order are in [`g6-polygon-operators.md`](../plans/g6-polygon-operators.md).
Persistent selection uses semantic tags/predicates, not face ordinals. All refusal cases leave the
document and last good render unchanged. Status 2026-10-14: every operator exists in `ve-geometry`
(topology, flip, weld, merge coplanar, dissolve edge/vertex/faces, delete, extrude, inset, bridge,
bevel, grid, individual modes, crossing check) and is reachable through the node language
(`poly.*` nodes with `poly.select`), evaluated, drawn, saved, grouped and instanced. Rows below name
the tests that hold each case; the gallery pieces are `poly_facade`, `poly_panel`, `poly_bevel_block`
and `poly_tunnel`. Remaining limits are in `docs/modeling/g6-report.md`.

| ID | Input | Expected |
|---|---|---|
| G6-01 | Closed cube through `poly.from_solid` and lossless `poly.to_solid`; lamp shade and goblet sliver fixtures through validation and current polygon operators | Cube volume and winding are preserved; slivers are reported, not globally rejected; selection-based edits away from slivers succeed and edits touching them refuse with a named face; global weld either succeeds or names a face it would collapse |
| G6-02 | Concave planar and non-planar n-gons sent through triangle-only solid conversion | Concave simple planar face ear-clips without filling its notch; deviation up to 0.1% of face extent is accepted; grossly non-planar or non-simple face returns `poly.triangulation_unsupported`; no silent fan |
| G6-03 | Extrude a cube's top face by 0.2 m | Closed, consistently wound result; Euler characteristic unchanged; volume increases by analytic top-area × distance |
| G6-04 | Inset a quad by more than half its width | Typed `poly.inset_too_large` naming the semantic face; document unchanged |
| G6-05 | Inset an open-boundary face and a face with an interior hole boundary | Matches the v1 contract: unsupported boundary/hole form refuses with a typed diagnostic |
| G6-06 | Bridge equal-length boundary loops with opposite orientation | One valid stitched surface; loop edges consumed; no non-manifold edge; expected component/Euler delta recorded |
| G6-07 | Bridge loops with different vertex counts | `poly.loop_mismatch`; no implicit resampling or topology repair |
| G6-08 | Weld duplicate vertices by a bounded tolerance | Vertex count drops; no zero-area face or collapsed loop; corner/UV values remain split at seams |
| G6-09 | Dissolve an edge between two coplanar adjacent faces | One simple merged loop replaces the two faces; non-manifold and repeated-vertex results refuse |
| G6-10 | Flip the orientation of one whole connected component | Euler and absolute volume unchanged; component winding reverses consistently; a partial selection cutting an interior edge refuses |
| G6-11 | Bevel a supported manifold edge, then a boundary/non-manifold edge and an oversized width | First succeeds with declared topology delta; unsupported edges and width return typed errors transactionally |
| G6-12 | Open mesh through face extrusion and viewport output | Valid polygons render; no volume is claimed; no Manifold-solid assumption or kernel error |
| G6-13 | Edge incident to three faces, inconsistent winding, and two shells touching at one vertex | Validation reports conditions without globally rejecting the mesh; each operator follows its own contract and names a relevant offending edge; vertex contact counts as one component; no silent repair |
| G6-14 | Select faces by operand-origin/tag, then change upstream dimensions and remove the source | Stable semantic selection follows the tagged region; removal produces `selection.lost`; multiple matches produce `selection.ambiguous` |
| G6-15 | Corner UV attributes through extrusion, inset, bridge and weld | Source seams remain split; no interpolation crosses chart boundaries; undefined propagation refuses or reports loss |
| G6-16 | 100,000-face input through every operator | Preflight predicts face/vertex/memory work; operation completes under the documented budget or refuses before allocation; cancellation is polled in bounded loops |
| G6-17 | Save/reload, undo/redo, group and instance a polygon-edit graph | Stable operation and semantic IDs; undo restores exact topology and attributes; redo reproduces output |
| G6-18 | Same polygon operator corpus on native and WASI | Stable canonical topology/attribute output and matching diagnostics, byte-identical where floating output is canonicalized |
| G6-19 | Flip a partially selected miswound face to repair its shared-edge winding | Inconsistent-edge count decreases; output validates structurally and names the repaired face/edge in the result |
| G6-20 | Flip a partial selection that would add winding conflicts, then repeat with `allow_inconsistent=true` | Default refuses transactionally with `poly.flip_increases_inconsistency` and the first new edge; explicit override succeeds and returns a warning listing the remaining conflict |
| G6-21 | Dissolve a valence-four interior vertex in a planar fan | Incident faces become one simple face; the vertex is removed; boundary and Euler changes match the contract |
| G6-22 | Dissolve a boundary vertex with exactly two boundary edges | Vertex is removed and its boundary edges join into one valid boundary loop |
| G6-23 | Dissolve a pinched or nonmanifold vertex | Typed `poly.nonmanifold` naming the vertex; input mesh remains unchanged |
| G6-24 | Dissolve a connected 2x2 quad patch | One simple n-gon replaces the patch; all interior edges disappear; corner data follows source corners |
| G6-25 | Dissolve a patch whose union contains a hole or is disconnected | Typed `poly.loop_invalid` or `poly.selection_disconnected`; input mesh remains unchanged |
| G6-26 | Extrude one planar face of a closed cube along its outward normal | Closed, consistently wound result; Euler stays 2; source attributes follow cap and side provenance |
| G6-27 | Extrude with zero, tangential, inward, or non-finite offset | `poly.direction_invalid`, named source face; input unchanged |
| G6-28 | Extrude one face in a 100,000-face mesh; cancel before work | Large input completes; pre-cancelled operation returns cancellation without mutation |
| G6-29 | Inset a convex quad by a valid distance with point/face/corner attributes | Ring and cap created; Euler unchanged; data follows source provenance |
| G6-30 | Inset a nonconvex face, invalid face index, or beyond the inradius | Named typed refusal; input unchanged |
| G6-31 | Inset one face in a 100,000-face mesh; cancel before work; inset real sliver fixtures | Large input completes; cancellation is observed; touching slivers refuses by face ID |
| G6-34 | Delete one cube face, delete every face, and delete a selection that would orphan a point | First leaves one valid boundary loop; latter cases refuse as `poly.delete_all_faces` and `poly.vertex_isolated` |
| G6-35 | Bridge equal-count loops from two open quad components | One closed, consistently wound component; loop correspondence deterministic; Euler preserved |
| G6-36 | Bridge unavailable or unequal boundary loops | `poly.loop_not_boundary` or `poly.loop_mismatch`; mesh unchanged |
| G6-37 | Bridge two four-edge loops in a mesh above 100,000 faces; cancel before work | Large input completes; cancellation is observed; no input mutation |
| G6-38 | An extrusion pushed through another surface, short of it, onto a coplanar overlap, and an inset that stays inside its face | `poly.self_intersection` naming the new face for the first and third; the second and fourth build; contact is not a crossing. `g6_36_self_intersection.rs` |
| G6-39 | Bevel one cube edge as a chamfer (1 segment), rounded (2 to 4), two disjoint edges, and the four vertical edges that share top and bottom faces; refusals for a shared vertex, an oversized width, bad width or segments, flat, boundary and four-face-corner edges | Euler number kept; chamfer removes exactly w squared over two times the edge length; rounded removes between the circle's and the chamfer's area; attributes follow their source elements; 100,000-face mesh and cancellation. `g6_39_bevel_edges.rs` |
| G6-40 | Multi-edge bevel on faces rewritten once per edge | The profile direction on a shared face is decided from the input mesh, not from the half-edited face (regression for the four-edge block) |
| G6-41 | `poly.grid` and individual inset and extrude | A grid is an open quad disc facing +z; every cell insets and its inner cap extrudes (24 cells give 24 + 96 + 96 quads, Euler 1, one boundary loop); one bad face refuses the whole command naming it; selections over 4,096 faces refuse. `g6_41_grid_and_individual.rs` |
| G6-42 | `poly.merge_coplanar` | A triangulated box becomes six quads (Euler 2); a curved mesh is unchanged; a patch with a hole is refused, not repaired. `g6_42_merge_coplanar.rs` |
| G6-43 | The node language end to end | Round trip through a polygon mesh; raised panel (volume 1 + 0.8 x 0.8 x 0.2); chamfer volume; open sheet drawn with no volume and a colour; facade chain (inset every cell, select role 1, extrude individually); dissolve edge and vertex; bridge tunnel volume exactly 4; refusals `selection.lost`, `selection.ambiguous`, `poly.selection_kind`, `selection.stale`, `poly.delete_all_faces`. `g6_graph.rs` |
| G6-44 | Derived tags and selection by attribute | Inset and extrude tag what they make (`role` 1 for the inner or moved cap, 2 for ring and sides) and a later `poly.select by=attribute` finds them. `g6_graph.rs` |
| G6-45 | MCP builds and edits a polygon chain and an impossible inset is refused | Built through the page, edited by stable node ID, IDs unchanged; the inset refusal carries `poly.inset_too_large` and leaves the document unchanged. `web/tests/mcp.mjs` (SwiftShader) |
| G6-46 | Gallery | `poly_facade`, `poly_panel`, `poly_bevel_block` and `poly_tunnel` evaluate with their analytic volumes (panel 1.128, tunnel 4, rounded block between circle and chamfer, facade open with none) and render from six SwiftShader views. `gallery.rs`, `web/tests/gallery.mjs` |
| G6-47 | 100,000-face sheet through the node language | grid, weld, flip, delete, extrude and inset each complete (release run: 112, 658, 350, 348, 370 and 316 ms on one Apple Silicon machine, including planning, triangulation and validation); a job cancelled between nodes is discarded. `g6_large.rs` |
| G8-01 | Sphere analytic and convergence | `g8_fields.rs`: volume and area errors shrink as edge length halves |
| G8-02 | Torus analytic and topology | `g8_fields.rs`: analytic volume tolerance and Euler characteristic 0 |
| G8-03 | Primitive/reference parity | `g8_fields.rs`: plane, ellipsoid and tube instructions match the C++ interpreter bit-for-bit; unequal ellipsoids and multi-segment tubes report `LowerBound` |
| G8-07 | Bounded repeat | `g8_fields.rs`: folded coordinates honor per-axis copy counts and periods; over-cap copy counts are refused; Rust/C++ samples and native/WASI production output match |
| G8-10 | Inexact field distance warning | `g8_graph.rs`: offset and shell on a smooth blend return `field.distance_inexact` warnings for the authored nodes; offset on an exact sphere returns no warning |
| G8-09 | Bounded polyline tube | `g8_fields.rs`: two-point tube matches analytic capsule volume, a multi-segment tube agrees bit-for-bit with C++, repeated consecutive points are refused, and point count is capped |
| G8-04 | Production adapter native/WASI parity | Seven field cases (sphere, torus, ellipsoid, smooth blend, creature-like capsule blend, polyline tube, bounded repeat) run through the Rust modeling evaluator; `cargo xtask parity` reports identical 37-line native and WASI output |
| G8-05 | Smooth union reference samples | `g8_fields.rs`: Rust expression and C++ tape outputs match bit-for-bit; guarantee is `None` |
| G8-11 | Clipped field bounds | Kernel and graph tests refuse a sampled inside boundary point as `field.clipped`; crossings thinner than the grid can evade the lattice check |
| G8-12 | Grid, padded storage and tape work limits | `g8_fields.rs`: oversized requests refuse before sampling; enforced caps are in ADR 0022 |
| G8-14 | Cancel during field sampling | `cpp/tests/field_test.cpp`: a 64-instruction field cancellation returns `VE_ERR_CANCELLED`; native runs measured 0.7–3.3 ms from request to return and assert under 500 ms. Maximum-cap and WASI latency remain unmeasured |
| G8-15 | Field result usable as a solid | `g8_graph.rs`: meshed sphere feeds `solid.union` and yields a nonempty result |
| G8-16 | Field graph persistence and instancing | `g8_persistence.rs`: grouped field subgraph instances save/reload with identical meshes; an instance radius edit preserves node IDs and undo/redo restores identical meshes |
| G8-17 | Field regions | Deferred for G8: the current `LevelSet` adapter does not produce per-triangle primitive labels. Use separate `field.to_solid` outputs for separately coloured pieces; do not claim named field-region support. Decision recorded in ADR 0022. |
| G8-18 | Field gallery models | `gallery.rs` evaluates repeated bent tubes and a creature built from smooth-blended ellipsoids, tube limbs/tail and separate eyes; `web/tests/gallery.mjs` captures six SwiftShader views for each through the real worker. At 0.026 m authored body spacing, the Preview render removes the prior sawtooth limb faceting. A regression test enforces the 200,000-triangle Preview cap, 12% volume tolerance and 0.06 m bounds tolerance against Final. Noise, regions and follow-up prompt edits are not covered. |

## Character sculpting

| ID | Input | Expected |
|---|---|---|
| CH-01 | Surface falloff on two nearby disconnected shells | Brush affects only the connected surface containing the nearest stroke point; the second shell remains byte-stable. `ve-modeling/tests/poly_sculpt.rs` |
| CH-02 | Mask, sculpt, unmask, sculpt across chained nodes | `sculpt_mask` persists as a point float attribute; a fully masked vertex does not move, and unmask restores its response. `ve-modeling/tests/poly_sculpt.rs` |
| CH-03 | Sculpt a human, then skin, grow hair and fit an outfit | Source vertex mapping survives the render-vertex/UV-seam to shared polygon-point conversion; all downstream character tools accept the edited body. `ve-modeling/tests/poly_sculpt_human.rs` |
| CH-04 | Update head and eye landmarks from sculpted points | Scalp points are transformed back into grounded body space; eye centres and radii recalculate from their mapped vertices. `ve-modeling/src/human/rig.rs` unit test |
| CH-05 | Blend and export a character shape key | `rig.morph` preserves the neutral mesh, applies the chosen weight, emits glTF POSITION deltas, and refuses mismatched topology. `ve-modeling/tests/rig_morph.rs`; browser export coverage |

## G9a: foundation fixes
| ID | Input | Expected |
|---|---|---|
| G9a-01 | Eight sequential spherical cuts from one box at Preview | Evaluates within Preview; chained boolean estimate does not multiply by four per operation. `ve-modeling/tests/g9_a1_planner.rs` |
| G9a-02 | 32 × 32 array of a dense sphere | Refused as `budget.triangles` during planner preflight. `ve-modeling/tests/g9_a1_planner.rs` |
| G9a-03 | Concave heptagonal prism through Catmull-Clark | Concave cap n-gons triangulate before refinement, result stays closed and converts to a valid solid; convex quad cages keep their faces. `ve-geometry/tests/g9_concave_subdivision.rs` |
| G9a-04 | Study-room fixture, 386 typed commands | Commits as one transaction; one undo removes the complete fixture. Apply time recorded; `ve-modeling/tests/g9_a4_large_batch.rs` |
| G9a-05 | A Preview triangle-budget refusal that fits Final | The staged edit remains pending for Final, then commits only after the Final projection succeeds; unrelated Preview errors still reject the candidate. `web/tests/evaluation.mjs` |
| G1a-16 (updated) | 1,000-command batch, then 1,001 | 1,000 accepted; 1,001 rejected with `budget.commands`, limit 1,000 and observed 1,001. `ve-graph/tests/g1a_history.rs` |

## G9b-1: authored lights (implementation in progress)
| ID | Input | Expected |
|---|---|---|
| G9b-01 | Point, spot and sun graph nodes plus a solid | Each light evaluates as a typed scene output without creating mesh parts; spot/sun directions normalize; `light.zero_direction` and `light.invalid_cone` are node-scoped refusals. `ve-modeling/tests/g9_lights.rs` |
| G9b-02 | Study-room fixture plus one sun, point, spot, and emissive window/laptop/lamp materials | All room parts and six authored/derived lights reach one committed Final viewport revision; screenshot saved by `web/tests/g9-study-room.mjs`. Browser gate passed under SwiftShader; visual review remains open. |
| G9b-03 | More lights than shader capacity, with equal-distance lights | Per-tier caps select nearest lights and ties sort by all packed values, so output is independent of ECS query order. `ve-render/src/light.rs`; focused GPU/browser coverage pending. |
| G9b-04 | 129 emissive output parts | Evaluation refuses `budget.lights` after combining explicit and material-derived sources; no partial Evaluation is returned. `ve-modeling/tests/g9_lights.rs` |

## G9b-2: baked ambient occlusion (implementation in progress)

| ID | Input | Expected result |
|---|---|---|
| G9b-05 | Isolated floor and a floor beneath a nearby ceiling | The open floor remains unoccluded and the covered centre darkens; `ve-modeling/src/ao.rs` |
| G9b-06 | One planar 100,000-face sheet | AO is all 1.0 without building the BVH; no geometry or topology changes |
| G9b-07 | More than 1,000,000 source triangles | Evaluation refuses with `budget.ao_triangles` before building the acceleration structure |
| G9b-08 | Existing modeling parity corpus with AO channel included | All 38 native/WASI JSON lines are byte-identical |
| G9b-09 | Study-room browser fixture | AO travels through evaluator WASM, worker transfer, upload and WGSL; headless Chromium/SwiftShader integration passes. Screenshot evidence remains to be owner-reviewed. |
| G9b-10 | Curved 100,352- and 999,698-triangle meshes, release build | Final/32-ray bake times and sampled RSS are recorded in `docs/modeling/g9-report.md`; `ao::tests::b2_measure_100k_and_1m_triangle_scenes` |
| G9b-11 | Re-evaluate unchanged scene, then change quality and geometry separately | The cached AO channel is reused only for unchanged scene geometry and quality; both changes force a rebake. `ao::tests::unchanged_scene_reuses_occlusion_but_geometry_and_quality_changes_rebake` |

## G9b-5: bounded procedural material recipes (implementation in progress)

| ID | Input | Expected result |
|---|---|---|
| G9b-12 | Every recipe accepted by `solid.material@1` with a bounded `texture_scale` | Typed material value keeps recipe and scale through evaluator output; unsupported recipe strings are refused by the graph choice validator; `tests/material_instances.rs` |
| G9b-13 | Study-room floor, wall, desk, rug, sofa and books assigned distinct recipes; pistol slide and grip use metal/leather | The room passes evaluator/WASM/projection/browser; the pistol graph evaluates with recipe materials attached; `web/tests/g9-study-room.mjs`, `tests/material_instances.rs`. Owner scoring and six-view visual evidence remain open. |
| G9b-14 | All ten recipe choices shown together in the procedural material gallery | One Final browser revision projects all ten meshes in the Engine Viewer (`engine/web/dist`, SwiftShader); screenshot saved at ignored `target/reports/g9/material-recipes.png`; canonical save/load and undo/redo preserve a recipe and scale. `web/tests/g9-material-gallery.mjs`, `tests/material_instances.rs`. Per-recipe six-view image review and Quest GPU cost remain open. |
| G9b-15 | Fillet a 2D square, round a convex box, and round the pistol slide and furniture tops | Analytic profile area and solid bounds/volume hold; invalid radius vs. core thickness refuses; real pistol and room fixtures evaluate. `tests/g9_rounding.rs`, `crates/ve-kernel/tests/round.rs`. General concave solids and selected-edge bevels are unsupported. |
| G9b-16 | Ambient irradiance on upward/downward-facing normals | Sky/ground colours pack as a first-order (L1) spherical-harmonic band (intensity-scaled mean and half-difference) and the WGSL ambient term evaluates it along the world normal; an optional warm ground bounce adds to downward-facing normals and clamps off when negative. `crates/ve-render/src/light_tests.rs`. Render capture and headset cost still need review. |
| G9b-17 | Authored `scene.environment@1` with a solid | The environment is a typed, serialized graph output (`SceneEnvironment`); `sky_color`/`ground_color`/`intensity`/`reflection`/`bounce`/`bounce_color` survive a JSON round trip; defaults match `VeAmbientLight`; out-of-range values refuse as `param.range` at authoring and a second environment refuses as `scene.environment.duplicate`. `ve-modeling/tests/g9_environment.rs` |
| G9b-18 | Study room and pistol prop rendered before and after the environment | The authored environment reaches the Engine Viewer through evaluator WASM, worker transfer, projection and WGSL; `web/tests/g9-environment.mjs` passes under SwiftShader and saves before/after captures at `target/reports/g9/environment-{study-room,pistol-prop}-{before,after}.png`. Owner visual review remains open. |

## G9b-6: viewer grading and presentation (implementation in progress)
| ID | Input | Expected |
|---|---|---|
| G9b-19 | Neutral, mono, warm and out-of-range grading controls | Neutral is the identity; contrast, saturation and tint mirror `forward.wgsl`; every control clamps to its documented range and a non-finite control becomes neutral; the graded output stays finite for finite input. `crates/ve-render/src/grade_tests.rs` |
| G9b-20 | Study room graded neutral vs. mono through the Engine Viewer | `set_grading_preset`/`set_grading` reach the renderer; the viewer preset changes the rendered image, out-of-range controls clamp, and an unknown preset is rejected. `web/tests/g9-grading.mjs` saves `target/reports/g9/grading-{neutral,mono,warm}.png`; owner visual review remains open. |

## BX: Blender extraction (ADR 0024)

| ID | Case | Expected result and test |
|---|---|---|
| BX-1 | `MikkTSpace` tangents through the C ABI | A quad whose U follows X gets tangent +X and sign +1; swapping U and V gives +Y and sign -1; a UV sphere gets unit tangents perpendicular to its normals; the same input is bit-identical; mismatched lengths, a bad index count, an empty list, an out-of-range index and a NaN refuse with typed errors. `crates/ve-kernel/tests/tangents.rs` |
| BX-2 | Native and WASI parity of the vendored Blender code | `tangents corners=N hash=...` over a carved mesh prints the same hash natively and under WASI (kernel corpus 23 lines). `crates/ve-kernel/examples/corpus.rs` |
| BX-3 | Sanitizers and static analysis on vendored code | `cpp/tests/tangents_test.cpp` passes under ASan and UBSan and `cpp/src/tangents.cpp` passes clang-tidy; `lib/mikktspace` is Apache-2.0 and unmodified (`VENDOR.md`). `cargo xtask cpp` |

## G9b-8: deterministic composition and kits

| ID | Case | Expected result and test |
|---|---|---|
| G9b-25 | Seeded scatter | The same seed gives identical placements, a different seed changes them, placements stay inside the width x depth rectangle on the XZ plane, and `min_spacing` is honoured between every pair. `tests/g9_scatter.rs` |
| G9b-26 | Scatter refusals | Spacing that cannot fit refuses with `scatter.unsatisfiable` (never fewer points); a count over 16,384 is refused by the operator contract; a scatter stacked into a set over the instance limit refuses as `budget.instances` before placement. `tests/g9_scatter.rs` |
| G9b-27 | Per-element jitter | `inst.jitter` varies shift, yaw and scale in place; element `i` depends only on the seed and `i`, so adding elements never moves earlier ones. `tests/g9_scatter.rs` |
| G9b-28 | Native and WASI parity | One scatter-then-jitter corpus line prints identical transforms natively and under WASI (kernel 22 / modeling 40 lines). `ve-modeling/examples/corpus.rs` |
| G9b-29 | Kits | `desk`, `shelf` and `window` evaluate to their stated footprint, the shelf `arrangement` seed is stable and changes the books, and every kit saves and reloads with identical geometry. `tests/g9_kits.rs`, `kits/*.txt` |

## G9b-7: edge treatment beyond the first pass (implementation complete; owner review pending)
| ID | Input | Expected |
|---|---|---|
| G9b-21 | Chamfer a convex box and compare with a round of the same size | `Solid::chamfer` erodes then dilates with an octahedron: the bounding box holds, the volume drops, and the flat facet removes more material than the sphere's arc; a width that consumes the core, or a zero, negative or non-finite width, refuses. `crates/ve-kernel/tests/chamfer.rs` |
| G9b-22 | `solid.chamfer@1` in the node language | The Ready operator evaluates through the graph: a 1 m box chamfered by 0.12 keeps its bounding box on every axis and loses only edge material, a 0.2 m core with a 0.1 m width refuses as `kernel.invalid_input`, and native and WASI print the same corpus line. `tests/g9_rounding.rs`, `ve-modeling/examples/corpus.rs` |
| G9b-23 | The `poly.bevel_edges` edge-network contract | A vertex fan, a non-manifold edge, a winding conflict and an unwelded seam each refuse with a typed code naming the element; a selection mixing valid and invalid edges refuses atomically and leaves the input byte-identical. `crates/ve-geometry/tests/g9_bevel_edges_contract.rs` |
| G9b-24 | A sharp and a chamfered stool rendered in the Engine Viewer | Both are captured at `target/reports/g9/chamfer-{sharp,cut}.png`; the chamfer raises the rendered triangle count (136 to 782) and leaves the concave leg-to-seat seams sharp. `web/tests/g9-chamfer.mjs` |

## G9b-9: owner visual review package

| ID | Input | Expected |
|---|---|---|
| G9b-30 | Pistol, study room, field creature, ceramic vase and kit/scatter scene at six fixed views and Preview/Final | `web/tests/g9-review.mjs` captures viewport-only Engine Viewer PNGs and writes fixture hashes, renderer, git revision, quality, triangle counts and evaluation times to ignored `target/reports/g9/review/`. Per-fixture HTML sheets leave the rubric scores blank; SwiftShader output is not headset/performance evidence. |

| G7-01 | CPU Catmull-Clark kernel, one closed cube level | 8 cage points produce 26 refined vertices and 24 quad faces; an independent face/edge/vertex rule implementation agrees per parent within 1e-6; vertex and face provenance are complete. `crates/ve-kernel/tests/subdivision.rs` |
| G7-02 | Uniform levels and cage preservation | Four sequential cube refinements remain bounded and each axis extent is non-increasing; the graph emits both unchanged cage and refined output; second-level provenance remains covered. `crates/ve-kernel/tests/subdivision.rs`, `crates/ve-modeling/tests/g7_graph.rs` |
| G7-04 | Open quad boundary | Edge-and-corner boundary holds cage corners; edge-only boundary moves them along the boundary. `crates/ve-kernel/tests/subdivision.rs` |
| G7-05..06 | Edge crease and corner sharpness | Per-parent edge and corner points are compared against the semi-sharp blend at sharpness 0, 0.5, 1, 2 and 10 at level 1; exact multilevel decay remains open. `crates/ve-kernel/tests/subdivision_creases.rs` |
| G7-03 | Extraordinary vertices, valence 3 and 5 | Closed triangular bipyramids at valence 3 and 5 match their own immediate parent vertex/edge/face using provenance, against an independent Catmull-Clark reference; results remain finite/bounded. Exact infinite-level limit positions remain a documented extension. `crates/ve-kernel/tests/subdivision.rs` |
| G7-07 | UV seam interpolation | Paired `u`/`v` F32 corner channels interpolate as one FVar channel with linear boundaries; exact-bit welding preserves separate values at a shared cage vertex. A two-level identity square stays equal to position, per-face cube UV islands remain split, the normalized-UV `poly.grid` source survives two-level subdivision, and seam output matches WASI. The `uv_subdivided_grid` gallery fixture also runs through the browser evaluator and viewport. `crates/ve-geometry/tests/g7_attributes.rs`, `crates/ve-modeling/examples/corpus.rs`, `web/tests/gallery.mjs` |
| G7-08 | Attribute propagation | Face attrs copy; up to 8 F32 point channels (32 million refined floats maximum) interpolate and keep names; F64 has `poly.subdivide.f64_unsupported`; integer point attrs return names and warnings; other corner and edge attrs retain typed refusals. Hole tags retain `poly.subdivide.holes_unsupported` because pinned OpenSubdiv does not propagate a child-face hole contract. `crates/ve-geometry/tests/g7_attributes.rs`, `crates/ve-geometry/tests/g7_subdivision.rs` |
| G7-09 | Growth and quality preflight | Kernel exact growth cap exists; graph bounds level 1 at six times input triangles and multiplies by four per further level. |
| G7-10 | Cancellation | Pre-cancel is tested and interpolation polls between levels. Deterministic asynchronous cancellation between levels is untested; `RefineUniform` itself is uninterruptible. Conservative growth caps are enforced and cap-sized latency remains a documented measurement limit. |
| G7-11 | Cage immutability | The adapter returns a new mesh; tests verify the source cage remains unchanged. |
| G7-12 | Curved quad conversion | Subdivided cube triangles close as a manifold/solid; shortest diagonal is deterministic and >90° folds refuse. |
| G7-13 | Invalid topology | Non-manifold and a reversed cube face both refuse with the typed edge diagnostic. `crates/ve-geometry/tests/g7_subdivision.rs` |
| G7-14 | Native/WASI parity | Modeling corpus includes a creased cube with two F32 point channels and six distinct face UV islands; native and Node WASI JSON lines match byte-for-byte, alongside existing sharpness, boundary and dropped-attribute cases. |
| G7-15 | Large mesh / level 4 timing | Release grid: 99,856 faces, level 1, native 94 ms / WASI 141 ms; four-face level 4 below 1 ms on both. This does not measure cap-sized uninterruptible calls, so the 2M cap remains conservative and unretuned. |
| G7-16 | Save/reload/undo/group/instance | Subdivision graph retains evaluated mesh through save/reload and undo/redo; a grouped, exposed-size subdivided block produces two instances and preserves both through edit history. `g7_graph.rs`, `g7_persistence.rs` |
**G8:** sphere and torus level sets match analytic volumes; blends by convergence; oversize grids
refused; creature silhouette review from six views.
**G9:** 60 frozen tasks with rubrics and thresholds; deterministic mock corpus; stale revision and
impossible request cases; pending AI transaction reviewable before acceptance.
**G10:** material boundaries and seams on all fixtures; mirrored cases; save, reload, undo.
| G10-01 | `solid.texture@1` on an unpainted solid | Evaluates with default PBR values and preserves the texture source in serialized material output. `crates/ve-modeling/tests/materials.rs` |
| G10-02 | `solid.texture@1` on a painted solid | Geometry, colour, metallic and roughness remain unchanged; the texture source is attached. `crates/ve-modeling/tests/materials.rs` |
| G10-03 | Texture program lifecycle | Source survives document JSON save/reload, undo and redo; malformed DSL is rejected by the texture parser before bake. |
| G10-04 | UV bake, viewer and glTF | A textured mesh has continuous intended seams, visible baked channels in the viewport, and valid glTF image/texture/material references. |
| G10-05 | glTF UV fallback | A closed mesh exports a bounded `TEXCOORD_0`; UV discontinuities stay split during vertex welding and the file passes `gltf-validator`. `tests/gltf.test.ts` |
| G10-06 | Texture channel bake | One program bakes albedo, roughness, metallic, height-derived normals and emissive; 3D channels use an object-position map, packed glTF roughness uses G, metallic uses B, and UV seams have padding. Browser WebGL2 test pending. |
| G10-07 | Blender polygon triangulate and remove doubles | Triangulating a cube produces 12 triangles with Euler characteristic 2 and unchanged volume; selected duplicate vertices merge while unselected geometry remains untouched. `crates/ve-kernel/tests/mesh_ops.rs` |
| G10-08 | Blender polygon UV pack | Packed cube unwrap is deterministic and every corner UV remains inside the unit square. `crates/ve-kernel/tests/uv.rs` |
| G10-09 | Progressive polygon LOD | Each requested level simplifies the prior level at the configured face ratio; collapse-prone settings are refused with a recovery suggestion. `crates/ve-modeling/tests/g10_blender_ops/part_2.rs` |
**L0:** schema-1 fixture migrates to schema 2 with empty variables; a flow link into a data port and a
data link into a flow port are each `graph.port_type`; variable commands (define, set initial value,
remove) are atomic and undoable; an old build opening a schema-2 file reports `store.newer_version`.
**L1:** the door fixture round-trips text, graph and commands; every `logic.*` code has a failing
document (unknown event, wrong payload type, unknown or mistyped state, flow cycle, flow fan-in
without merge, `repeat` over 64); `rule` text prints back identically; an action on a rebuild
parameter inside `animate` is `logic.rebuild_hot`.
**L2:** same document, seed and log give the same per-tick hash (property test, native and wasm);
emit loop stops at depth 16 with `logic.cascade_depth`; a 5,000-event flood is bounded by the queue
with `logic.queue_overflow`; `wait` resumes on the right tick; `ignore`, `restart` and `queue(8)` each
behave as specified; two writers in one tick follow queue and authored order; missing target aborts only
its chain with `logic.target_missing`; one tick of the door fixture under 1 ms.
**L3:** door fixture opens on key insert by mouse and by emulated controller; Play does not change the
authored document; stopping Play discards state; no kernel call per frame while animating; host effect
queued to the page and its result event returns.
**L4:** a rebuild action swaps geometry without a stall; coalescing at 4 per second; stale results
never swap in; picking stays consistent across the swap.
**L5:** a recorded session replays tick for tick; step back restores the exact hash; breakpoint pauses at
the node; watches show "no data" before first run; fired wires highlight; auto layout reviewed.
**L6:** seeded defects (sealed key, unreachable objective, emit cycle, write conflict) are each reported
with the right code and nodes; the door objective is reached; validator cost bounded on 200 rules.
**L7:** at least 40 frozen logic tasks with a clean validator and a solving playthrough as the rubric;
impossible and ambiguous requests included; first-try, repaired and unsupported rates separate.
**L8, L9:** per the plan; lists due before those gates are started.
**G11, G12, V5-V7:** per the plan; lists due before those gates are started.
**G13:** fuzz time, soak results, parity run, size report, migration fixtures, limitations matrix.

## G12: close-view procedural building asset

| ID | Input | Expected |
|---|---|---|
| G12-BA-1 | `building.asset@1` for masonry, glass, trim, roof and doors | Each material group has finite geometry and one UV per triangle corner; the same seed is deterministic, higher detail adds trim, all twelve grammar families and long/short footprints are covered, dimensions/styles outside the contract refuse, and the requested floor count controls the height. `crates/ve-modeling/tests/building_asset.rs`; all five material groups are included in native/WASI modeling corpus parity. `web/demos/architecture_gallery.txt` composes twelve differently proportioned examples with a road and street furniture. |
| G12-SS-1 | `street.road@1` kit parts | Asphalt, lane markings, kerbs and pavement each produce finite UV-mapped geometry; cardinal axis and placement are tested; invalid dimensions refuse. `crates/ve-modeling/tests/streetscape.rs`; the four parts are in native/WASI parity. |
| G12-SS-2 | `street.prop@1` reusable street-object kit | Body/accent geometry is finite and UV mapped for ten prop families; lights expose a lens group; incompatible parts, unknown kinds and invalid dimensions refuse. `crates/ve-modeling/tests/streetscape.rs`; representative props are in native/WASI parity. |
