# ADR 0020: Polygon selections, derived tags and the polygon nodes of the language

- **Status:** Proposed; implemented and covered by cases G6-38 to G6-47
- **Date:** 2026-10-14
- **Decision owners:** Spatial Lab maintainers
- **Builds on:** ADR 0019 (G6 operators are authored on `PolyMesh`)
- **Adds scope beyond the G6 plan table:** `poly.merge_coplanar`, `poly.grid` and the individual mode of
  inset and extrude (needed to build the plan's facade fixture), and `poly.delete_faces`

## Context

ADR 0019 settled where the G6 operators live. G6 still needed the language around them: a polygon type
in the graph, a way for a persistent command to say which faces to edit without storing an ordinal, and
a way to get from a solid to a polygon mesh with flat faces, because a solid's mesh is all triangles.
Checking the plan's building-facade fixture showed two gaps: no source of quad meshes except a solid, and
inset and extrude acting on one face or one patch when a facade edits a grid of cells.

## Decision

**Two port types.** `Poly` is an editable polygon mesh (open or closed, any face size). `Selection` is a
set of faces, edges, vertices or boundary loops of one mesh. Polygon values are drawn from their properly
triangulated faces (never a fan) with no volume unless the mesh is a closed, consistently wound manifold.
A polygon mesh can be an output and can take `solid.material`; `poly.to_solid` returns a solid and the
kernel refuses anything that is not a closed manifold.

**Selections are rules, not ordinals.** `poly.select` takes a mesh and returns a `Selection` chosen by a
rule: all, normal (direction and angle), box (element centre inside a box), attribute (an integer face or
point attribute equal to a value) and sharp (edges whose faces turn apart by an angle). The element kind
is face, edge, vertex or loop. Choosing nothing refuses with `selection.lost`; `expect="one"` refuses with
`selection.ambiguous` when several match. The selection travels with a fingerprint of the mesh it was made
on; an edit applied to any other mesh refuses with `selection.stale`. An edit given a selection of the
wrong kind refuses with `poly.selection_kind`. Face ordinals exist only inside one evaluation.

**Derived tags.** Inset and extrude add a face attribute `role` (created as zero when absent, copied
through every later edit by the operators' attribute rules): 1 for the inner or moved cap, 2 for ring and
side faces. A later `poly.select by=attribute attribute=role value=1` finds the caps without any ordinal.
Solids converted with `poly.from_solid` keep their colour regions as a `region` face attribute.

**Flat faces from solids.** `poly.merge_coplanar` merges each connected patch of coplanar faces by
repeated, validated `dissolve_faces`; a patch that cannot become one simple loop refuses with that
operator's code instead of being repaired. Bevel needs it because corners of a triangulated solid touch
more than three faces.

**Sources and individual mode.** `poly.grid` makes a flat quad disc facing +z. `poly.inset_faces` insets
every selected face on its own, and `poly.extrude_faces` takes `individual=true` for the same. Any face
that breaks its single-face contract refuses the whole command naming that face. At most 4,096 faces per
individual command, because each edit re-validates the mesh.

**Crossing check.** Extrude, inset, bridge and bevel test the faces they create against every face they
share no vertex with (grid over face boxes, strict-interior triangle tests, a one-million-pair budget,
cancellation polled). Contact is not a crossing. A crossing refuses with `poly.self_intersection`.

**Planning.** Polygon nodes are estimated from their inputs' sizes; face-growing edits add a bounded
allowance (32,768 triangles) because selections are only known at evaluation. A grid is estimated exactly.

## Not covered (recorded, not hidden)

- Colour regions do not survive polygon edits: the result takes the source's first material. Selection by
  the `region` attribute works on the converted mesh.
- Bevel is the v1 contract: disjoint edges, planar faces, valence-three corners, one to four segments.
  Non-planar extrusion, nonconvex inset, bridge with resampling and bevel edge networks are not in G6.
- `poly.bridge_loops`, `dissolve_edge` and `dissolve_vertex` resolve their element by a rule on the input
  mesh each evaluation; a loop is a boundary loop ordinal of that mesh, so persistent loop selection is
  only as stable as the rule (for example a box around the loop).
- A node that cannot triangulate its mesh (a face more than 0.1 percent of its extent out of plane, or a
  non-simple face) fails even when only an intermediate result; it is not drawn as a fan.
- Weld, merge and individual edits re-validate the whole mesh per step; very large selections are refused
  by budget rather than made fast.
- Viewport face picking (V5) is not part of G6.
- All visual evidence is SwiftShader; no headset or physical GPU.

## Consequences

- A model or a person can build and edit hard-surface forms through the same typed nodes, with stable IDs.
- Existing documents are unchanged: the new operators are additive.
- Changing the selection rules or the derived tags needs a new ADR.

## Validation

`g6_*` suites in `ve-geometry`; `g6_graph.rs`, `g6_persistence.rs` and `g6_large.rs` in `ve-modeling`;
the parity corpus (four polygon cases, identical natively and in WASI); `web/tests/mcp.mjs` (polygon
block); `web/tests/gallery.mjs` (four pieces); the strict gate named in `g6-report.md`.
