# ADR 0017: Bounded in-house curve and surface adapter for G5

- **Status:** Draft decision; implementation in progress
- **Date:** 2026-10-13
- **Decision owners:** Spatial Lab maintainers
- **Supersedes:** D3's conditional Curvo selection in `track2-modeling.md`

## Context

G5 needs editable 3D curves and surface construction while evaluating untrusted, model-authored
documents in a deterministic Rust/WASM worker. The output must pass through the existing Manifold
kernel, triangle and memory preflights, cancellation protocol, scene cache, groups and instances.
Authored control points stay in the document; evaluated geometry is transient.

X2 (`docs/modeling/experiments.md`) tested Curvo 0.3.2. It is MIT and builds for
`wasm32-unknown-unknown`; its exact-circle smoke sample was accurate. But its normal dependency tree
was about 107 package entries, the probe's absolute WASM artifact was 726,498 B raw / 144,425 B
Brotli, a 256-section adaptive loft produced 277,440 triangles in one 1.35 s native run, and the
library has unchecked internal paths that are not covered by this evaluator's input bounds. It also
does not accept the engine's cooperative cancellation context. The probe did not establish a size
delta or make Curvo safe for the engine's runtime contract.

## Decision

G5 uses a small, deterministic implementation owned by this repository. Curvo is not a runtime or
development dependency of `ve-modeling` for this gate. Revisit only through a new X2 result and ADR.

The supported initial curve language is a sampled `Curve3` evaluation value built from authored
`curve.polyline`, cubic `curve.bezier` controls and `curve.circle`. The first graph schema bounds authored point lists
to 4,096 because it shares the graph's current list-value cap; expanding this to the reserved 65,536
curve-point limit requires an explicit nested-list budget. Cubic sampling is bounded to 512 fixed
subdivisions. Adaptive chord-tolerance sampling remains future work. Evaluation values are cache
values derived from node parameters, not document payloads or WASM-owned kernel handles.
Editable controls remain typed parameters in the graph.

Sweeps and lofts produce an indexed mesh in Rust, then convert it through `Solid::from_mesh` before
publishing an evaluated solid. A sweep transports a stable frame along its sampled path using the
rotation-minimizing double-reflection method; it does not use a Frenet frame at low-curvature or
inflection points. Closed paths distribute the residual seam rotation before welding the seam. Open
paths receive caps only where the profile topology is supported. Preflight computes conservative
triangle and memory bounds before allocating rings or calling Manifold.

`solid.sweep` supports open sampled paths and closed sampled circles, with one strictly convex,
closed profile loop; open paths receive fan-triangulated caps and closed paths receive distributed
seam correction and no caps. The first `solid.loft` supports two to 64 single,
strictly convex profile loops with matching point counts and even spacing along Y. Other profile
shapes and section correspondence are rejected. Open-path rotation-minimizing frames use double
reflection and reject sampled local curvature radii at or below the profile's radial extent.
Evaluation polls cancellation between generated path rings.

Curve nodes are intermediate graph values and are not valid solid outputs. The desktop viewer draws
polyline, Bézier and sampled-circle paths in its SVG overlay; authored polyline/Bézier controls can be
dragged in a camera-facing plane. The overlay is not rendered per eye in WebXR, and the camera-plane
drag is not a full axis-constrained 3D control cage.

Until robust constrained caps and loop correspondence are proven, profiles with holes, concave outlines,
mismatched sections and unsupported trims fail with node-scoped diagnostics rather than being repaired
silently. Near-reversal and curvature-radius checks reject tubes that would fold through themselves, and
repeated consecutive path points are refused.

`xform.twist`, `xform.taper` and `xform.bend` sample edge midpoints and face centroids against their
`tolerance` parameter. They locally split only marked edges; adjacent faces share the same midpoint,
so refinement stays conforming without T-junctions. Up to two rounds allow at most 16x triangle growth.
An unmet sampled tolerance refuses with `geometry.deformation_tolerance`. A finite-difference orientation
check refuses sampled Jacobian or face-normal flips with `geometry.deformation_fold`, then
`Solid::from_mesh` validates the complete result. This remains sampled evidence, not a continuous
Hausdorff error guarantee. Bend is limited to +/-90 degrees and refuses when its bend radius is not
greater than half the input X width.

**Self-intersection** (`curve.sweep_self_intersection`) retains the existing sufficient path-segment test
(`curves/overlap.rs`) unchanged. After mesh generation, a uniform spatial grid checks non-adjacent swept
triangles, excluding shared vertices and neighboring path rings; this catches cap/tube, bend-vertex and
parallel-overlap fixtures that the sufficient rule misses. The grid polls cancellation and has explicit
cell-reference and pair-count caps; preflight reserves 24 MiB per sweep for this pass. A separated
near-miss is covered. Triangle intersection uses bounded numeric tolerances, so this is still not a formal
exact-predicate proof for arbitrary degenerate or near-coplanar input.

## Not met by this subset (gate blockers, recorded rather than hidden)

Closed sampled paths, periodic seam correction, hollow vase composition and hollow pipes now have focused
acceptance coverage. Planar profiles with holes are still refused, and the hollow wall bound only applies to
matching corresponding section vertices under the assumptions below; it is not a general shell-thickness operator.
Other remaining gaps are:

- ~~bounded twist, taper and bend operators pass focused volume/silhouette/refusal tests (partial G5-09);
  local conforming refinement, sampled face checks and thin-box/twisted-loft fixtures pass, but a continuous
  error bound remains unproven;~~
- ~~the six-section hollow vase and straight/curved pipe compositions (G5-03) pass in `g5_hollow.rs`; for matching
  corresponding section vertices and linear loft interpolation, the outer-minus-inner gap is linear in height,
  so its minimum occurs at a section. This assumes equal point counts and correspondence, inner inside outer at
  every section, and an inner loft raised above the floor. It does not establish a universal wall bound through
  floor/rim transitions or arbitrary lofts.
  G5-08 specifies that profiles with holes
  are **refused**, so hollow forms are composed with `solid.difference` rather than annulus caps. The inner loft is
  raised above the floor and extended beyond the rim; the inner pipe sweep extends beyond both ends.

The documented G5 cases and strict validation pass, except provider-backed G5-14 is not reproducible from this
repository: no provider is configured and the two manual runs were recorded only in chat. The matching-section
wall argument above remains conditional, not a runtime wall-thickness guarantee. This ADR remains Draft pending
maintainer review and acceptance.

NURBS, arbitrary trimmed surfaces, constant-thickness shelling and unrestricted self-intersection
repair are deferred. This is an explicit bounded subset: the node system can be extended later without
making current surface operators accept geometry they cannot validate.

## Consequences

- No new third-party geometry dependency, license notice or dependency-surface growth is added.
- The curve/sweep/loft algorithms, sampling, frame transport, triangulation and validity rules remain
  testable in pure Rust and share native/WASI operation order.
- Explicit preflight and cancellation checks fit the worker; deterministic sampling makes cache keys
  stable from authored controls plus quality.
- The first G5 release has less surface-kernel breadth than Curvo: no NURBS, no arbitrary trims, and
  only matching section topology. MCP must teach only operators whose conformance cases pass.
- A 256-section loft still requires a conservative cost cap and measurement; the Curvo timing is a
  warning workload, not a performance target or a direct comparison.

## Validation required before Accepted

All G5 rows in `docs/modeling/cases.md`; native/WASI parity; worker cancellation and preflight tests;
strict gate; a g5 report with measured preview/final size and timing; and a review of the generated
vase, pipe and curved-panel views under SwiftShader. No headset claim is implied.
