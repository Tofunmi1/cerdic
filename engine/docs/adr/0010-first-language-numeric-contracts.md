# ADR 0010: first language numerical contracts

Status: accepted for G3 implementation. The owner authorized implementing the handoff and continued
the work; optional questions about the contradictory vase fixture were unanswered. These corrections
are mathematical requirements, recorded explicitly rather than preserving the example's errors.

- The frozen vase has six nodes, not eight. Its inner top radius is 0.040 m at y=0.31 m: the old
  0.044 gave only 2 mm radial clearance at the 0.30 m rim, below the requested 3 mm wall.
- Widening only the **inner** opening is the failing edit. Widening only the outer opening thickens
  the wall. The passing paired edit changes outer 0.050→0.070 and inner 0.040→0.064 m.
- The wall check measures the cross-section `outer` minus `inner` (any two profiles, not only
  axisymmetric ones): it opens that region with a disc of the required diameter and fails when
  anything is lost, reporting the largest thickness that passes. The region is the physical wall, so
  the open rim of a vase and its contact with the axis are not walls and are not judged. Full
  eroded-profile containment would wrongly reject the cavity's deliberate extension above the rim.
  An inner profile that covers the outer one leaves no wall and fails with thickness 0. (G3 first
  limited the check to one-loop axisymmetric profiles; the opening test never needed that.)
- A 64-facet circle has relative volume deficit `1 - sin(2π/64)/(2π/64) = 0.0016056`, which cannot
  meet the example's 0.001 circular-volume tolerance. Keep exactly 64 requested facets; compare
  against the closed-form faceted volume within 1e-6 and the circle within 0.002. A 128-facet case
  separately meets the original 0.001 circular tolerance. Half-revolve uses the same angular step.
- Extrusion stands along Y; the profile's second coordinate maps to -Z. `divisions` counts vertical
  slabs. Manifold's consistently chosen side diagonal showed a 0.004065 relative volume excess for
  a square twisted 90° with 128 slabs, above the acceptance tolerance. Our factory uses Manifold's
  cap triangulation and a symmetric four-triangle fan per side quad; the centre averages both
  diagonals, removing this first-order bias. It builds a checked Manifold from the resulting f64
  mesh. No vendored source is changed. Twist and angle are radians on the
  typed command/C ABI boundary; source syntax uses degrees by default and permits explicit `rad`.
  Canonical text prints `rad` to preserve stored floating-point values exactly.

Conformance and gallery evidence are required before operators become `ready`. This ADR does not
authorize later modeling or editor gates.
