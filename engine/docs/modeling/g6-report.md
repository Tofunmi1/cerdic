# G6 polygon tools: report (2026-10-14)

**Status: implemented against its case table, with the limits below. Not accepted:** ADR 0019 and ADR 0020
are Proposed, and the owner has not seen G6 evidence in a headset or on a physical GPU. The strict gate passed
on this tree: `cargo xtask check --strict`, 549 passing tests, native and WASI parity (20 modeling lines),
all browser suites under SwiftShader (log `engine/target/g6-strict-01.log`). Nothing is committed.

## What exists

**`ve-geometry` (pure Rust, deterministic, cancellable).** Topology: edge incidence, face and vertex
adjacency, components, boundary loops, Euler number, a validator that reports slivers, non-manifold edges and
winding conflicts instead of rejecting the mesh, ear-clip triangulation of simple planar faces (0.1 percent
tolerance, never a fan). Operators, each editing a copy and committing only after validation:
`flip_component`, `flip_faces`, `weld` (sorted-cell grid), `merge_coplanar`, `dissolve_edge`,
`dissolve_vertex`, `dissolve_faces`, `delete_faces`, `extrude_faces`, `inset_face`, `bridge_loops`,
`bevel_edges` (1 to 4 segments, circular profile), the source `grid`, the individual modes `inset_each` and
`extrude_each`, and a shared crossing check for the faces an edit creates. Every refusal is a typed code that
names the offending face, edge or vertex.

**The node language (`ve-graph`, `ve-modeling`).** Port types `Poly` and `Selection`; fifteen `poly.*`
operators: `grid`, `from_solid`, `to_solid`, `select`, `flip_faces`, `weld`, `merge_coplanar`,
`dissolve_faces`, `dissolve_edge`, `dissolve_vertex`, `delete_faces`, `extrude_faces`, `inset_faces`,
`bridge_loops`, `bevel_edges`. `poly.select` chooses faces, edges, vertices or loops by rule (ADR 0020), with
`selection.lost`, `selection.ambiguous` and `selection.stale`. Polygon meshes draw from their triangulated
faces with no volume unless closed, take `solid.material`, save, reload, group and instance like any node.
The MCP guide teaches the operators with one worked example.

## Evidence

| Case | Evidence |
|---|---|
| G6-01 to G6-37 (geometry) | `ve-geometry/tests/g6_*.rs`, including real sliver fixtures (hollow lamp shade, hollow goblet) |
| G6-38 crossing check | `g6_36_self_intersection.rs` |
| G6-39, G6-40 bevel | `g6_39_bevel_edges.rs`: exact chamfer volume, rounded between circle and chamfer, four edges on shared faces |
| G6-41, G6-42 grid, individual modes, merge | `g6_41_*`, `g6_42_*` |
| G6-43, G6-44 node language | `ve-modeling/tests/g6_graph.rs` (12 tests; bridge tunnel volume exactly 4) |
| G6-17 persistence | `g6_persistence.rs` (save, reload, undo, redo, group, instance, override) |
| G6-18 parity | corpus, native and WASI identical, 4 polygon cases |
| G6-45 MCP | `web/tests/mcp.mjs` polygon block: built, edited by ID, impossible inset refused |
| G6-46 gallery | `poly_facade`, `poly_panel`, `poly_bevel_block`, `poly_tunnel`: analytic volumes and six SwiftShader views each (images in `engine/target/reports/gallery/`, git-ignored) |

## Measurements

100,000-face sheet (317 x 317 quads) through the node language, one local release run, Apple Silicon,
including planning, triangulation and validation (`g6_large.rs`): grid 112 ms, weld 658 ms, flip 350 ms,
delete 348 ms, extrude one face 370 ms, inset one face 316 ms. A debug-build run of the same test takes about
20 s. These are single observations, not latency guarantees.

Release WASM, Brotli quality 9, against the G5 report:

| Artifact | G5 raw | G6 raw | Delta | G5 Brotli | G6 Brotli | Delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Runtime | 34,499,928 | 34,510,143 | +10,215 | 4,116,598 | 4,119,722 | +3,124 |
| Evaluator | 1,405,870 | 1,810,103 | +404,233 | 394,881 | 511,350 | +116,469 |

The evaluator grew by about 29 percent raw: the polygon toolkit (topology, triangulation, fifteen operators and
their checks) is compiled into it. G5 follow-ups (colour regions, smooth loft) between the two measurements
are in the delta, so it is not all G6.

## Bugs found by building it (all fixed, all with regression tests)

- The validator rejected two real solids (lamp shade, goblet) over sliver faces; it now reports.
- `inset_face` puts its ring and cap in place of the face, so later face numbers shift; the crossing check
  first looked at the wrong faces and `inset_each` now works from the highest face down and returns where
  each cap went.
- A multi-edge bevel decided the profile direction on a face other bevels had already rewritten; it is now
  decided on the input mesh (found only by beveling four edges that share a top and bottom face).
- The kernel can report an empty trailing run for coincident copies (colour regions); skipped.

## Limits and what is not done

- Bevel v1: disjoint edges, planar faces, three faces at each end, 1 to 4 segments. Non-planar extrusion,
  nonconvex inset, bridge with resampling and bevel edge networks are not in G6.
- Colour regions do not survive polygon edits; the result takes the source's first material.
- A node whose mesh cannot be triangulated (a face more than 0.1 percent of its extent out of plane, or a
  non-simple face) fails, even as an intermediate step.
- Individual-mode edits re-validate the mesh per face and are capped at 4,096 faces per command.
- Face-growing edits are planned with a bounded allowance, not a count, because selections resolve at
  evaluation; the evaluated triangle count is still checked against the quality budget.
- The tunnel gallery piece reads as one long bar from outside; it shows the join, not a hollow tunnel.
- No real-model run on polygon prompts (no provider credit), no headset or physical-GPU measurement, no
  viewport face picking (V5), and G7 (OpenSubdiv adapter) is a separate gate.
- The 4,096, 262,144, 1,024 and 1,000,000 limits are budgets chosen here, not measured ceilings.

## Decisions waiting for the owner

Accept or amend ADR 0019 and ADR 0020; commit the combined tree (G5, material regions, polygon tools, V3 and
the X3 documents, 163 paths at the time of writing); decide whether the added scope (`merge_coplanar`, `grid`, individual
modes, derived `role` tags, `delete_faces`) stays as built.
