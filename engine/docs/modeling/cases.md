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
| ID | Input | Expected |
|---|---|---|
| G2-01 | Evaluate F1 in the worker | Same geometry as the in-window result (tolerance above) |
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
| G3-09 | `solid.revolve` of a rectangle about Y, 360 degrees | Volume equals pi (r2^2 - r1^2) h within 1e-3 (segments 64) |
| G3-10 | Revolve 180 degrees | Half the full volume within 1e-3 |
| G3-11 | Self-intersecting profile | Defined error, no crash |
| G3-12 | Zero-area profile and zero height extrude | `kernel.invalid_input` or `result.empty` as specified; distinct outcomes |
| G3-13 | F1: `check.min_wall` passes; widen opening in both profiles | Passes; node IDs unchanged |
| G3-14 | F1: widen only the outer profile | `constraint.min_wall`; document unchanged |
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

## Draft lists (expand before each gate starts)
**G4:** wheel group instanced four times evaluates the prototype once; one instance override leaves
the others; explicit group upgrade preserves unrelated IDs; recursive group rejected; realization of
10,000 cheap instances refused or accepted by predicted size; mirror and attach honour pivot and
handedness; socket attachment moves with its parent.
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
**G11, G12, V5-V7:** per the plan; lists due before those gates are started.
**G13:** fuzz time, soak results, parity run, size report, migration fixtures, limitations matrix.
