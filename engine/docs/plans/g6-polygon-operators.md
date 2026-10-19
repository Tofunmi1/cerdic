# G6 operator implementation plan

This plan follows [ADR 0019](../adr/0019-polygon-and-subdivision-backend.md). Build only after the
owner has reviewed the complete X3 handoff. The working surface is `PolyMesh`; no PMP object or face
index enters a saved command.

## Shared contracts first

Before operators, implement deterministic edge incidence, face/vertex adjacency, connected components,
boundary loops, edge classification and Euler characteristic in `ve-geometry`. Rebuild adjacency after
each candidate edit. Structural validation rejects invalid indices, fewer than three distinct corners,
repeated corners and malformed face ranges. The validation report also records faces below the relative
`f32` area-resolution threshold, nonmanifold edges, inconsistent winding, and boundary edges that
could not form loops. These are legal mesh conditions; each operator must refuse only when the condition
touches its input contract, and must name the offending face or edge. Never collapse a valid sliver in
the shared validator. Sorted edge keys and fixed traversal order are required for native/WASI parity.
Cancellation checks and work counters belong in all topology walks. Components are vertex-connected, so
two shells sharing a vertex belong to one component in v1.

Selections in persistent commands are predicates over face semantic tags, origin labels, plane/normal
queries or named sets. Resolve against the candidate input before editing; zero matches returns
`selection.lost`, multiple matches returns `selection.ambiguous`. Never persist a face ordinal. Keep
the viewport's picked ordinal only as transient state and convert it to a semantic tag before creating
a command.

Initial typed refusals use the `poly.*` namespace. Proposed codes below become stable only when added
to the generated diagnostic schema with the implementation. Every command edits a candidate copy and
commits only after geometry, attribute, budget and expected-topology validation all pass.

## Operators

| Order | Operator | Initial input contract and algorithm | Typed refusal conditions | Attribute/selection propagation | Cases |
|---|---|---|---|---|---|
| 1 | `poly.flip_faces` | Reverse corner order for selected faces. Whole vertex-connected components are always accepted. A partial selection is accepted only if it does not increase inconsistent-winding edges, unless `allow_inconsistent=true`; that choice is returned as an explicit warning result. Refuse a selected sliver face. | `poly.selection_empty`, `poly.selection_invalid`, `poly.flip_increases_inconsistency`, `poly.face_degenerate` | Face tags remain; corner values reverse with corners; point/edge attributes unchanged. | G6-10, G6-19, G6-20 |
| 2 | `poly.weld` | Deterministic distance clusters from a sorted-cell spatial grid, union-find with stable minimum-index representative, then rewrite faces. Preflight candidate distance pairs against the supplied budget. Reject if any face collapses below three unique vertices or the result introduces nonmanifold topology. | `poly.weld_collapses_face`, `poly.weld_pair_budget`, `poly.nonmanifold`, `poly.attribute_conflict` | Never merge corner attributes across charts. Point values come from the minimum-index representative; collapsed edge values must agree or refuse. Conflicting semantic face tags refuse. | G6-08 |
| 3 | `poly.dissolve_edge` | One interior edge with exactly two incident faces. Concatenate their boundary loops after removing the shared edge; require a simple loop with at least three unique vertices. Refuse if either source face is a sliver. | `poly.boundary_edge`, `poly.nonmanifold`, `poly.winding_conflict`, `poly.loop_invalid`, `poly.face_degenerate`, `poly.attribute_conflict` | Face attributes must agree; otherwise refuse and name the edge. Corner UVs copy each original corner in loop order. | G6-09, G6-13, G6-15 |
| 4 | `poly.dissolve_vertex` | Interior manifold vertex: merge its incident faces into one simple loop and remove the vertex. Boundary vertices are supported only at valence two, where the two boundary edges are joined. | `poly.vertex_boundary_unsupported`, `poly.nonmanifold`, `poly.loop_invalid`, `poly.face_degenerate`, `poly.attribute_conflict` | Merged face keeps a shared tag only if all source faces agree; corner values copy corner-by-corner without crossing seams. | G6-21, G6-22, G6-23 |
| 5 | `poly.dissolve_faces` | Merge a connected, consistently wound selected patch by removing interior edges. The result must be one simple loop; holes and pinches are unsupported. | `poly.selection_disconnected`, `poly.loop_invalid`, `poly.nonmanifold`, `poly.winding_conflict`, `poly.attribute_conflict` | Shared face data carries only when all source faces agree; corner values follow source corners. | G6-24, G6-25 |
| 6 | `poly.extrude_faces` | Connected, consistently wound selected patch. Duplicate its boundary, offset the cap by a finite distance along a computed or explicit direction, and stitch one side face per boundary edge. Initial release supports a single planar convex face and a connected planar patch; later general surfaces require local frames. | `poly.selection_empty`, `poly.patch_nonplanar`, `poly.direction_invalid`, `poly.self_intersection`, `poly.nonmanifold` | Retained cap inherits source face tags; side faces get deterministic child tags and inherit source-face values. Boundary corner seams are copied, not collapsed. | G6-03, G6-12, G6-15 |
| 7 | `poly.inset_face` | One planar convex n-gon. Offset its edges inward in the face plane and create a ring plus inner cap. Reject nonconvex or self-intersecting offsets in v1 rather than silently splitting the face. | `poly.face_nonplanar`, `poly.face_nonconvex`, `poly.inset_too_large`, `poly.self_intersection` | Inner face keeps the source tag; ring faces receive deterministic child tags. Per-corner UVs are copied/interpolated within each source face only. | G6-04, G6-05, G6-15 |
| 8 | `poly.delete_faces` | Remove selected faces and retain the newly open boundary. Refuse deletion of every face, newly isolated vertices, and selections touching nonmanifold edges. | `poly.delete_all_faces`, `poly.vertex_isolated`, `poly.nonmanifold`, `poly.attribute_conflict` | Point data remains attached to original positions; surviving face/corner data is copied. Edge-domain data refuses until edge remapping is defined. | G6-34 |
| 9 | `poly.bridge_loops` | Two simple boundary loops on one or two components, with equal vertex counts in v1. Choose cyclic alignment deterministically by minimum squared distance with lowest-index tie break; orient loops oppositely and stitch corresponding edges. No resampling in v1. | `poly.loop_not_boundary`, `poly.loop_mismatch`, `poly.loop_intersection`, `poly.winding_conflict` | New faces get child tags from both loop tags plus operation ID. Corner values interpolate only between corresponding loop corners inside each chart. | G6-06, G6-07, G6-13, G6-15 |
| 10 | `poly.bevel_edges` | Selected edges must each have exactly two incident faces. V1 accepts disjoint edges only, width > 0 and <= a preflighted local inradius bound, and 1–4 segments. Construct bevel strips in a local cross-section; reject connected edge networks until their corner rules are specified. | `poly.boundary_edge`, `poly.nonmanifold`, `poly.bevel_too_large`, `poly.bevel_network_unsupported`, `poly.self_intersection` | Original face semantic tags continue on inset faces; bevel faces receive derived tags. Corner UVs remain split at source seams. | G6-11, G6-13, G6-15 |

These are deliberately narrow v1 contracts. A wider contract requires added cases and a revised plan,
not fallback repair that changes authored topology silently.

Status 2026-10-14: extrusion, inset, bridge and bevel now run a geometric crossing check on the faces they
create (`poly.self_intersection`; contact is not a crossing; one-million-pair budget). Bridge alignment is
bounded to one million distance pairs. All operators are reachable from the node language; persistent
commands choose their elements with `poly.select` rules (ADR 0020), and a loop is chosen by a rule such as a
box around it, resolved on the input mesh at every evaluation.

Additions to the table, decided while building the facade fixture (ADR 0020): `poly.merge_coplanar` (merge
connected coplanar faces by validated dissolves; needed because a solid's mesh is triangles and bevel
requires three faces at each corner), `poly.grid` (a flat quad source mesh), and the individual modes
(`poly.inset_faces` insets every selected face on its own; `poly.extrude_faces individual=true`), plus the
derived face attribute `role` that inset and extrude write (1 for the inner or moved cap, 2 for ring and
side faces). The operator names in the graph are `poly.inset_faces` (plural, always individual) and
`poly.extrude_faces`; the geometry functions are `inset_face`, `inset_each`, `extrude_faces` and
`extrude_each`.

## Assigned v1 operations

`poly.flip_faces`, `poly.dissolve_vertex` and `poly.dissolve_faces` are assigned v1 operators. The
whole-component API remains available as the safe subset of `poly.flip_faces`. Per-face orientation
changes may repair existing inconsistent winding, but may not increase it unless the caller explicitly
opts into that condition and receives a warning.

## Shared implementation and test order

1. Add topology validation and invariant tests for open, closed, disconnected and edge-nonmanifold
   meshes, including isolated vertices, repeated corners, zero-area faces and reversed winding.
2. Implement each table row with its refusal cases first. Run the operation on a cloned candidate mesh,
   then compare topology/attribute invariants before publishing its value.
3. Add operation work estimates for vertices, corners, candidate pairs and output faces. Refuse before
   allocation when Preview/Final budgets cannot be met. Poll cancellation between bounded loops.
4. Add semantic selection persistence and provenance propagation before graph commands. Selection
   identity must survive parameter edits; generated child tags must be stable for equal operation IDs.
5. Integrate `PolyMesh` into `Value`, graph ports, text, MCP docs and render projection only after pure
   geometry operators pass. Open polygon outputs have no volume. Converting a non-triangle `PolyMesh`
   back to a solid uses robust triangulation or a typed refusal; never fan concave/nonplanar faces.
6. Add facade and top-face-extrusion gallery entries with six SwiftShader views. Include the named
   operator/selection in native/WASI parity output. Run focused tests, clippy and `cargo xtask rules`
   after each operator; run strict gate once after the full gate is complete.

## G7 boundary

G7 is a separate acceptance gate. The proposed OpenSubdiv adapter consumes validated topology and
explicit boundary, crease and face-varying rules; preflights total refined faces/vertices before
refinement; and states the maximum uninterruptible library call. Test Catmull-Clark on extraordinary
vertices, open boundaries, sharp edges, UV seams, holes, non-manifold refusal and native/WASI
determinism against checked reference meshes. Do not expose subdivision to the graph until those
cases and a measured full evaluator size delta pass.
