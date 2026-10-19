# ADR 0018: Material regions through booleans, materials on placement sets, smooth loft blending

- **Status:** Proposed; implemented and covered by cases G5-26 to G5-30
- **Date:** 2026-10-14
- **Decision owners:** Spatial Lab maintainers
- **Changes:** the rule "transforms and booleans keep the first input's surface" (G3, `appearance.rs`)

## Context

A `solid.material` gave a whole solid one surface, and every boolean or transform kept the first input's
surface for the whole result. A car, a goblet with jewels or a sword could only be many colours as many
separate outputs, which cannot be moved, mirrored, instanced or grouped as one object. Building six
ambitious test models (staircase, lamp, goblet, bridge, sword, car) showed two more gaps: a material
refused placement sets, so sets had to be realized (fused) before colouring, and a four-section loft is
straight between sections, so a vase belly came out as two cones.

## Decision

**Material regions.** When the inputs of a boolean, a rigid move, a mirror or a pattern carry two or more
different materials (`regions::needed`), each input is converted to a kernel solid whose triangles are
labelled with fresh Manifold origin ids (`Solid::from_mesh_runs`, `ve_reserve_ids`,
`ve_solid_from_mesh_runs`). Manifold keeps the label on surviving faces and gives faces cut by a second
operand that operand's label. `Solid::runs` returns the triangle runs of the result, in the order
`to_mesh` copies them, and the evaluator maps ids back to materials. A value then keeps ordered
`regions` (first triangle plus material). Anything else takes the old path with no labelling, so
single-colour and unpainted documents are unchanged and cost nothing extra.

Resolution rules: a face takes its operand's material; an operand without one takes the first input's, so
an uncoloured cutter leaves the body's colour on the cavity wall, and a coloured cutter inlays its colour.
`solid.material` on top replaces every colour. Empty kernel runs (coincident copies) are skipped.

**Output.** A value with regions is output as one part per region, sharing `node` and `name` and carrying
`Part.region` (0, 1, ...). A part's volume is its signed-tetrahedron share of the closed solid's volume;
the shares sum to the solid's volume. Placement sets split their prototype the same way and share the
placements. The renderer keys a surface by (node, region): `eval_projection_material_region` and
`eval_projection_chunk_part` are new, the old calls are region 0.

**Materials on sets.** `solid.material@1` accepts a placement set and returns a set; the surface goes on
the shared prototype, so the set stays shared (no realize, no extra mesh per placement).

**Smooth lofts.** `solid.loft@1` gains `steps` (1 to 8, default 1). With `steps` above 1 each vertex of the
matching sections follows a cubic Hermite curve through its section positions (tangents from the
neighbouring sections, one-sided at the ends), so the surface passes exactly through every section and is
smooth between them. Rows are budgeted in the triangle estimate before building. A blended row that is
not convex or reverses winding is refused as `surface.unsupported_section`. `steps=1` is bit-identical to
the previous loft, so no existing document changes.

## Not covered (recorded, not hidden)

- Twist, taper and bend, `inst.realize` and sockets (`xform.attach`) keep only the first input's
  material; regions do not survive them.
- A material on top of a multi-colour solid recolours all of it (there is no "recolour one region").
- Union estimates now add a capped 16,384-triangle intersection allowance to the estimated input sum;
  this avoids repeated fourfold growth for chained small unions. Five chained unions of overlapping boxes
  pass below the Preview budget (G5-31). The allowance is a resource estimate, not a proof that arbitrary
  boolean output complexity is bounded by that amount.
- Overshoot of the Hermite blend is bounded only by the convexity refusal; there is no monotone option.
- All visual evidence is SwiftShader; no headset or physical GPU.

## Consequences

- One object can carry many colours and still be one node: movable, mirrorable, groupable, instanceable.
- The kernel ABI grew by three functions; parity (native vs WASI) covers two multi-colour cases.
- Documents saved before this decision evaluate identically.
- Changing the colour rules again needs a new ADR.

## Validation

`origin_runs.rs` (kernel labels survive union, difference, move), `material_regions.rs` (union, move,
difference with and without a coloured cutter, recolour on top, sets, mirror/array/radial/repeat,
coincident copies, unchanged single-colour documents), `material_instances.rs`, `g5_loft_blend.rs`,
`regions.rs` unit tests, parity corpus, SwiftShader renders of a fused four-colour car and a straight
versus blended vase, and the five-union preflight regression (G5-31). The strict gate result is in
`target/next-after-g5-strict-03.log` (458 native tests; all strict gates passed; browser checks used
SwiftShader).
