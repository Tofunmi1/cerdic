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
| G1a-16 | 200-command batch, then 201 | 200 accepted; 201 rejected with `budget.commands`, `limit` 200, `observed` 201 |
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

Partially implemented: G4d-04 alignment and checked rigid kernel transformation are in place.
Strict validation passed: 364 native tests, native/WASI parity and every browser suite.
Socket ownership and default
attachment rotation await the owner decisions required by the handoff. See [G4d report](g4d-report.md).
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

**V3:** a gizmo drag is one coalesced command; a handle edits the inspector's parameter; preview during
a drag, commit on release; lost and ambiguous selections shown.
**V4:** making the wheel group by UI equals the G4 fixture; edit definition versus edit instance;
upgrade conflict shown before applying.
**G5:** sweep of a circle along a circle matches the analytic volume; loft of equal sections equals
the extrusion; repeated points, near-180-degree turns, periodic seams and degenerate sections give
the specified outcome; bend, twist and taper keep normals valid.
**G6:** each operator against closed, boundary and non-manifold inputs; Euler change checks; bevel
refuses unsupported cases transactionally; attribute rules enforced.
**G7:** cube subdivides toward the known limit surface; creases, boundaries and UV seams against
reference cases; refinement growth preflighted.
**G8:** sphere and torus level sets match analytic volumes; blends by convergence; oversize grids
refused; creature silhouette review from six views.
**G9:** 60 frozen tasks with rubrics and thresholds; deterministic mock corpus; stale revision and
impossible request cases; pending AI transaction reviewable before acceptance.
**G10:** material boundaries and seams on all fixtures; mirrored cases; save, reload, undo.
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
