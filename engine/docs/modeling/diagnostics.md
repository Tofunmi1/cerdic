# Diagnostic codes (contract v1, draft for G1a)

One shape everywhere (commands, parser, planner, evaluator, store, worker, editor):
`{ code, severity, message, command?, node?, port?, param?, related?, feature?, span?, limit?, observed?, suggestion? }`.
`command` is the index of the failing command in its batch; `related` lists other nodes that took
part (the members of a cycle, the source of a mistyped link).
`code` is a stable dotted string (generated into a TS union); `message` is for people and may change;
`span` is `{ line, col, len }` into the text source; `limit` and `observed` are numbers with a unit
string. Severity: `error` blocks a commit, `warning` does not, `info` is progress or a no-op.

| Code | Severity | Meaning | Typical suggestion |
|---|---|---|---|
| `parse.syntax` | error | Text does not match the grammar | Show the expected form |
| `parse.unknown_op` | error | Operator not registered or not `ready` | Closest names by edit distance |
| `parse.unknown_param` | error | Parameter not declared by the operator | List valid parameters |
| `parse.missing_param` | error | Required parameter absent | Name it with its type and unit |
| `parse.duplicate_name` | error | Two statements define the same node name | The second definition's span |
| `parse.forward_reference` | error | A statement reads a node defined later | Define inputs first |
| `parse.unknown_name` | error | A statement reads a name no statement defines | |
| `provider.invalid_patch` | error | A provider reply is not exactly one bounded patch (prose, code, extra fields, empty or oversized); nothing was applied, no repair is attempted | |
| `provider.unsupported` | error | The provider does not handle the request | What it does handle |
| `provider.failed` | error | The provider failed to answer | |
| `param.type` | error | Wrong type | Expected type |
| `param.range` | error | Outside declared range | Nearest valid value and the range |
| `param.unit` | error | Unit mismatch or missing unit | Expected unit |
| `param.non_finite` | error | NaN or infinity | |
| `graph.unknown_node` | error | Reference to a missing node | |
| `graph.duplicate_id` | error | ID already used | |
| `graph.port_type` | error | Output type does not fit the input port | Types accepted by the port |
| `graph.port_arity` | error | Too few or too many inputs | Required count |
| `graph.cycle` | error | Link would create a cycle | The nodes on the cycle |
| `graph.group_recursion` | error | Group contains itself, directly or not | |
| `graph.group_version` | error | Group version missing or incompatible | Available versions |
| `graph.unknown_port` | error | The operator has no such input or output port | Valid ports |
| `graph.in_use` | error | Removing a node that other nodes or outputs still use | The readers; or use cascade |
| `graph.unknown_material` | error | Reference to a missing material | |
| `graph.unused` | warning | Node not reachable from any output | |
| `budget.source` | error | Source text over the byte limit | `limit`, `observed` |
| `budget.nodes` | error | Node count over limit (parsed or expanded); also output, material and ID limits | |
| `budget.commands` | error | More than 10,000 commands in a batch | Reduce the batch size |
| `budget.expansion` | error | Predicted geometry or instance expansion over limit | Predicted size |
| `budget.geometry` | error | Vertex, face, grid-cell or point count over limit | |
| `budget.memory` | error | Predicted temporary or cached bytes over limit | |
| `budget.time` | error | Job exceeded its time budget | Preview vs final policy |
| `kernel.invalid_input` | error | Kernel rejected an argument | |
| `scatter.unsatisfiable` | error | `inst.scatter` could not place `count` points at `min_spacing` within its draw budget | Message names how many fit; enlarge the area, lower the spacing or the count |
| `socket.name` | error | Socket name is empty or contains unsupported characters | Use letters, numbers, `_`, `-` or `.` |
| `socket.duplicate` | error | Socket name already exists, or a mirrored copy would collide | Choose a unique name |
| `socket.invalid_frame` | error | Socket matrix cannot form a bounded proper rigid frame | Use a finite, noncollapsed transform |
| `socket.unknown` | error | Requested socket is absent from a value | The diagnostic lists available sockets on the named input |
| `budget.sockets` | error | Predicted socket metadata exceeds the per-value cap | Reduce copied sockets below the reported limit |
| `budget.lights` | error | More than 128 authored plus emissive-derived scene lights are reachable | Keep the combined light count at or below 128 |
| `budget.ao_triangles` | error | More than 1,000,000 prototype/output triangles would enter the deterministic ambient-occlusion bake | Reduce Final scene geometry below the AO budget |
| `light.zero_direction` | error | A sun or spot light direction is zero | Set a non-zero direction vector |
| `light.invalid_cone` | error | A spot light's inner angle is not smaller than its outer angle | Set `0 < inner_angle < outer_angle < π` |
| `scene.environment.duplicate` | error | A document authors more than one `scene.environment` output | Keep a single scene environment |
| `kernel.not_manifold` | error | Input mesh is not a valid closed solid | Which node produced it |
| `kernel.too_large` | error | Result too large for the kernel | |
| `result.empty` | info | Valid but empty (for example a difference that removes everything) | Not an error; distinct from failure |
| `constraint.min_wall` | error | A declared thickness constraint is violated | Which pair of nodes |
| `geometry.floating_part` | warning | Disconnected pieces with no declared socket relation | The piece IDs |
| `geometry.unsupported_case` | error | Valid input outside the operator's published contract | The documented limits |
| `poly.triangulation_unsupported` | error | Polygon is non-simple, degenerate or non-planar and cannot be safely triangulated by the current adapter | Keep it as a polygon or make it planar/simple |
| `poly.sculpt_mask_attribute` | error | Existing `sculpt_mask` is not one finite F32 value per mesh vertex | Rename the conflicting channel or restore its point-float layout |
| `poly.selection_empty` | error | Polygon operator requires at least one selected face | Select a face or resolve a semantic selection |
| `poly.selection_invalid` | error | Face selection contains a duplicate or out-of-range ordinal | Refresh the selection against the current mesh |
| `poly.selection_disconnected` | error | Dissolve selection contains multiple edge-connected patches | Select one connected face patch |
| `poly.patch_nonplanar` | error | Face extrusion selection is not planar within the scale-relative v1 tolerance | Select a planar patch or flatten it first |
| `poly.face_nonplanar` | error | Inset source face is outside the scale-relative planarity tolerance | Flatten the face before insetting |
| `poly.direction_invalid` | error | Extrusion offset is non-finite, near-zero, tangential, or points against the source face normal | Provide a finite outward offset with a normal component |
| `poly.inset_distance_invalid` | error | Inset distance is non-finite or not positive | Provide a finite positive distance |
| `poly.inset_too_large` | error | Requested inset collapses or crosses the source face's inradius | Reduce the inset distance |
| `poly.face_nonconvex` | error | V1 inset accepts only strictly convex polygon faces | Use a convex face or a future general-offset backend |
| `poly.selection_splits_component` | error | Whole-component orientation edit selected only part of a vertex-connected component | Select the full component or use `poly.flip_faces` |
| `poly.flip_increases_inconsistency` | error | Partial face flip would increase the count of inconsistent-winding edges | Select a repair selection or explicitly allow the reported conflicts |
| `poly.weld_collapses_face` | error | Weld would leave a face with fewer than three unique corners or a repeated corner | Reduce tolerance or preserve the named face |
| `poly.weld_pair_budget` | error | Spatial-grid candidate-pair preflight exceeds the supplied work budget | Reduce tolerance or raise the operation budget |
| `poly.weld_grid_overflow` | error | A position divided by tolerance cannot be represented by the bounded spatial cell key | Increase tolerance or move the mesh closer to its local origin |
| `poly.tolerance_invalid` | error | Weld tolerance is non-finite or not positive | Supply a positive finite distance |
| `poly.nonmanifold` | error | Polygon edit touches or creates an edge or vertex with non-manifold incidence | Repair the named element or choose a supported selection |
| `poly.winding_conflict` | error | Polygon edit would create an inconsistent-winding interior edge | Preserve opposite traversal across the named edge |
| `poly.face_degenerate` | error | Polygon edit touches or would introduce a face below the relative f32 area-resolution threshold | Avoid the named face or reduce the edit |
| `poly.attribute_conflict` | error | An edit would merge face data or edge values that disagree | Reconcile the named attributes before editing |
| `poly.vertex_unknown` | error | Requested vertex does not exist in the polygon mesh | Refresh the vertex selection |
| `poly.vertex_isolated` | error | Vertex is not referenced by any polygon face | Select a referenced vertex |
| `poly.delete_all_faces` | error | Face deletion would leave a polygon mesh with no faces | Keep at least one face or delete the whole mesh value |
| `poly.vertex_boundary_unsupported` | error | Boundary vertex does not have exactly two boundary edges | Select an interior vertex or one supported boundary vertex |
| `poly.edge_unknown` | error | Requested undirected edge is absent from the current polygon mesh | Refresh the edge selection |
| `poly.boundary_edge` | error | Edge dissolution requires exactly two incident faces | Choose an interior manifold edge |
| `poly.loop_invalid` | error | Dissolved patch boundary repeats a vertex, self-intersects, contains a hole or cannot form one simple loop | Select a simpler edge or patch |
| `poly.loop_not_boundary` | error | Requested bridge loop does not identify one of the mesh's current boundary loops | Refresh boundary-loop selection |
| `poly.loop_mismatch` | error | Bridge loops have different vertex counts in v1 | Match loop counts or resample in a future explicit operation |
| `poly.bridge_pair_budget` | error | Deterministic loop-alignment candidate count exceeds one million | Reduce loop size or use an explicit correspondence in a future version |
| `poly.self_intersection` | error | A face an extrusion, inset or bridge just made crosses a face it shares no vertex with (touching is not crossing) | Shorten the offset, shrink the inset or move the loops apart; the named face is the new one |
| `poly.budget_exceeded` | error | A crossing check, individual-mode selection (over 4,096 faces), grid (over 262,144 faces) or merge (over 1,024 patches) exceeds its work budget | Reduce the input or split the edit |
| `poly.topology_unexpected` | error | After an edit the mesh's Euler number, component count or set of non-manifold, mis-wound or degenerate faces is not what the operator promises | Report it with the input; the document is unchanged |
| `poly.grid_invalid` | error | Grid size or count is not positive and finite, or has more than 262,144 faces | Use a smaller grid |
| `poly.angle_invalid` | error | A coplanarity angle is outside 0 to 0.5 radians or not finite | Use a small positive angle |
| `poly.edge_degenerate` | error | A bevel edge has zero length | Weld or remove the edge first |
| `poly.bevel_width_invalid` | error | Bevel width is not finite and positive | Supply a positive width |
| `poly.bevel_segments_invalid` | error | Bevel segments are outside 1 to 4 | Use 1 to 4 segments |
| `poly.bevel_too_large` | error | The bevel would cut away 90 percent or more of an adjacent edge, or reaches past the face | Reduce the width; the named edge is the one cut too far |
| `poly.bevel_flat_edge` | error | The two faces of a bevel edge are nearly coplanar (or fold flat), so there is no corner to round | Choose a sharp edge (poly.select by=sharp) |
| `poly.bevel_vertex_unsupported` | error | A bevel edge ends at a vertex that does not have exactly three faces forming a closed fan | Merge coplanar faces first (poly.merge_coplanar) or choose another edge |
| `poly.bevel_network_unsupported` | error | Two selected bevel edges share a vertex | Bevel disjoint edges only in v1 |
| `poly.selection_kind` | error | An edit received a selection of the wrong kind (for example edges for delete_faces) | Set element on poly.select to match the edit |
| `poly.select_rule_unsupported` | error | The selection rule does not apply to that element (for example by=normal on edges) | Use a rule that fits the element |
| `selection.stale` | error | A selection was made on a different mesh than the one it is applied to | Select again on the mesh the edit receives |
| `poly.topology_unexpected` | error | Candidate topology does not match the operator's declared Euler, edge or face delta | Keep the source mesh and report the operation as unsupported |
| `poly.subdivide.nonmanifold` | error | Catmull-Clark cage contains an edge with more than two incident faces | Repair the named edge before subdivision |
| `poly.subdivide.winding_conflict` | error | Adjacent cage faces traverse a shared edge in the same direction | Fix the named edge's face winding |
| `poly.subdivide.edge_attribute_unsupported` | error | Edge-domain attributes have no subdivision propagation policy | Remove or convert the edge attribute |
| `poly.subdivide.holes_unsupported` | error | A non-zero face `hole` tag cannot be propagated by this adapter; no child mesh is published | Keep the hole on the cage or remove the tag before subdivision |
| `poly.subdivide.point_channel_limit` | error | F32 point channels exceed 8 channels or 32 million refined floats | Reduce the channel count or subdivision level |
| `poly.subdivide.f64_unsupported` | error | F64 point attributes cannot cross the f32 subdivision boundary without losing precision | Keep F64 data outside subdivision or explicitly convert it to F32 |
| `poly.subdivide.uv_incomplete` | error | The paired UV corner attributes `u` and `v` are missing, mistyped or have the wrong corner count | Supply both F32 corner attributes with one value per corner |
| `poly.subdivide.corner_attribute_unsupported` | error | Corner-domain attribute has no declared subdivision interpolation rule | Remove it or keep it on the source cage |
| `poly.subdivide.concave_face_triangulated` | warning | Concave n-gon faces were split into triangles before Catmull-Clark refinement; convex faces retain their original cage topology | Review the result if diagonal edge flow matters |
| `poly.subdivide.attribute_dropped` | warning | Integer point attribute was omitted and its name is listed in the evaluation report | Recreate the discrete value after subdivision if needed |
| `poly.triangulation_unsupported` | error | A non-planar n-gon or curved quad folded at least 90 degrees cannot be triangulated safely | Split the n-gon or reduce the fold |
| `geometry.degenerate_curve` | error | A curve has a repeated consecutive point | Remove or move the point |
| `geometry.invalid_curve` | error | A curve's sampling parameters are out of range (the Bézier step count) | The accepted range |
| `geometry.invalid_frame` | error | A sweep path has a repeated point or reverses on itself, so no stable frame exists | Round the reversal |
| `geometry.degenerate_profile` | error | A sweep profile has too few points | At least three points |
| `geometry.unsupported_profile` | error | A sweep profile is outside the supported subset (concave or not one loop) | One strictly convex loop |
| `geometry.unsupported_topology` | error | A sweep needs a valid sampled path and one profile loop | Use an open path or closed sampled circle; holes are unsupported |
| `curve.sweep_curvature` | error | The tube radius is not less than the path's local curvature radius, so it would fold | Names both radii; shrink the profile or ease the bend |
| `curve.sweep_self_intersection` | error | The conservative path-distance test or the bounded swept-triangle grid found a non-adjacent surface intersection | Names the path segments for the sufficient path test or triangle IDs for the mesh test; a separated near-miss remains accepted (ADR 0017) |
| `curve.bend_curvature` | error | Bend radius is too small for the input solid's width | Names bend radius and half-width; reduce angle or use a narrower solid |
| `geometry.deformation_tolerance` | error | Two bounded conforming local-refinement rounds cannot meet the sampled edge and face-centroid tolerance | Names observed deviation and requested tolerance; raise tolerance or reduce deformation |
| `geometry.deformation_fold` | error | The sampled deformation Jacobian flips or collapses a face | Names the affected node; reduce the deformation or use a less aggressive bend |
| `surface.section_count` | error | A loft needs 2 to 64 profile sections | |
| `surface.degenerate_section` | error | A loft section has no closed loop | |
| `surface.unsupported_section` | error | A loft section is not strictly convex | |
| `surface.incompatible_sections` | error | Loft sections differ in loop count or point count, or one is not convex | Same point count on every section |
| `eval.cancelled` | info | Job superseded or cancelled | |
| `eval.worker_crash` | error | Worker died; restarted; last scene kept | |
| `eval.worker_timeout` | error | Worker terminated after the hard limit | |
| `eval.stale_result` | info | Result for an older revision was discarded | |
| `revision.conflict` | error | `base_revision` is not current | `current_revision` and changed nodes |
| `revision.already_applied` | info | Transaction key seen before | The original revision |
| `revision.document` | error | The batch is for another document | |
| `selection.lost` | warning | Authored feature no longer exists | |
| `selection.ambiguous` | warning | Several candidates; none chosen | The candidates |
| `store.corrupt` | error | Saved data failed validation | Previous save kept |
| `store.quota` | error | Storage quota exceeded | Bytes needed |
| `store.newer_version` | error | File written by a newer schema | File kept untouched |
| `store.unknown_op` | error | File uses an operator this build lacks | File kept, shown read-only, node ids listed |
| `store.unavailable` | error | Storage cannot be read or written (blocked, disabled) | Previous save kept |
| `store.read_only` | error | Edit or save refused on a document that is read-only (unknown operators, or it failed to load) | Open with a build that can edit it |
| `store.migration_failed` | error | A migration step failed | Previous save kept |
| `provider.invalid_patch` | error | Provider output did not validate | Counts toward the repair cap |
| `provider.budget` | error | Calls, tokens or time cap reached | |
| `field.invalid_parameter` | error | A field node has a missing, non-finite or invalid parameter | Correct the parameter values |
| `field.clipped` | error | The field is inside a sampled boundary point of its explicit meshing box | Expand the bounds; thinner features can cross between samples |
| `field.empty` | error | No surface crossed the sampled field volume | Expand the bounds or change the field |
| `field.distance_inexact` | warning | Offset or shell was applied to a field without an exact Euclidean distance guarantee | Treat offset width as approximate or start from an exact primitive |
| `budget.field_ops` | error | Field sample, tape-work or padded voxel-slot cap exceeded | Lower grid resolution or simplify the expression |
| `budget.field_cells` | error | Field lattice sample or padded-slot cap exceeded during graph preflight | Use a coarser edge length or smaller bounds |
| `budget.field_tape` | error | Lowered field instruction count exceeds the tape cap | Simplify the field expression |

Logic codes (`logic.*`, defined with the logic track; see `logic.md` section 11): `unknown_event`,
`event_args`, `unknown_state`, `state_type`, `flow_cycle`, `flow_fan_in`, `unreachable_rule`,
`unreachable_objective`, `cascade_depth`, `step_budget`, `queue_overflow`, `target_missing`,
`rebuild_hot`, `write_conflict`, `retrigger`.

Rules: a diagnostic always names the smallest responsible node, port or feature it can; a failure
that cannot be located says so rather than guessing; adding a code is additive, changing a meaning is
a contract version bump.
