# ADR 0021: Catmull-Clark adapter and G7 policies

Status: Proposed. G7-attributes implements the recommendations below; maintainer acceptance remains pending.

## Context

G7 needs deterministic Catmull-Clark refinement while preserving an editable `PolyMesh` cage. The pinned
OpenSubdiv 3.7.0 CPU Far sources now compile through the existing CMake kernel for native, `wasm32-wasip1`,
and browser `wasm32-unknown-unknown` targets. The kernel accepts indexed face loops and returns refined
positions, quad topology, immediate vertex-parent elements, and level-zero face parents behind the C ABI.

## Decision proposed

- Keep OpenSubdiv Far behind `ve_kernel`'s C ABI and `ve-geometry::subdivide`; the renderer and graph never
  depend directly on OpenSubdiv types.
- Use uniform Catmull-Clark levels 1 through 4, with preflighted exact face, corner and vertex growth limits.
- Preserve the input cage by returning a new mesh. A result vertex points to its immediate parent vertex,
  edge or face; each result face points to its original level-zero face.
- Reject malformed, non-manifold and inconsistently wound input before the library refinement call. Poll
  cancellation before refinement and between each per-level primvar interpolation; topology refinement itself
  is one uninterruptible library call.
- Face attributes are copied to children through face provenance; integer point attributes are dropped and
  listed in `Subdivided::dropped_attributes`, then surfaced as typed evaluation warnings. F32 point attributes
  are interleaved by name and interpolated as up to eight channels (32 million refined floats maximum). F64
  stays explicitly refused with `poly.subdivide.f64_unsupported`. Paired F32 corner `u`/`v` values use one
  face-varying channel, welded by exact float bits in first-corner order, with linear face-varying boundaries.
  Other corner and edge attributes retain typed refusals. A non-zero face `hole` U32 tag is recognized and
  refuses with `poly.subdivide.holes_unsupported`: the pinned OpenSubdiv refiner marks base holes but exposes
  no child-hole inheritance contract, and the adapter has no hole output representation.
- Treat subdivision as the final polygon-editing step in v1. A probe confirms planar-patch extrusion can refuse
  curved subdivided faces with `poly.patch_nonplanar`, and bevel can refuse selected edge networks with
  `poly.bevel_network_unsupported`, at levels 1 and 2. The graph help and MCP guide say to edit the cage first.
- Curved subdivision quads use their shorter diagonal, with deterministic vertex-index tie-breaking. A quad
  refuses if either triangle reverses relative to the Newell normal or the two triangle normals meet at 90° or
  more. Non-planar n-gons above four corners keep the planar-only refusal.
- `poly.subdivide@1` is reachable from text and MCP, exposes levels/boundary and selection-driven edge/corner
  sharpness, and participates in quality preflight, save/reload and undo/redo.

## Owner decisions required

1. Confirm or amend the shorter-diagonal curved-quad rule and its 90° fold bound.
2. Confirm or amend dropping integer point attributes with a typed name report.
3. Confirm the subdivided, bevelled block gallery demo. A provider-backed real-model run remains deferred until
   provider credit is available.

No policy above is considered accepted by this proposed ADR. Keep ADR 0019 Proposed pending owner review.

## Evidence so far

- One closed cube refines to 26 vertices and 24 quads; a Rust reference implementation checks the Catmull-Clark
  face, edge and vertex point rules per parent within 1e-6. Edge and corner semi-sharp parent points are checked
  at sharpness 0, 0.5, 1, 2 and 10; valence 3/5 samples are paired through provenance.
- Two cube levels produce 96 quads and preserve face provenance.
- Native ASan/UBSan C++ tests, focused Rust tests, clippy, and compilation for both WebAssembly targets pass.
- The subdivision WASI test executable ran under Node WASI; native and WASI modeling corpus outputs match for
  29 lines, including the five sharpness settings, open boundary case and typed warning payload. Release
  benchmarks cover a ~100k-face level-1 cage and near-cap level-3/4 cages.
- Strict checks print `all gates passed`; browser rendering uses SwiftShader and is not headset validation.

## Consequences and remaining work

The graph, MCP, gallery and grouped-instance persistence paths exist; curved quads convert to closed triangle
meshes. UV FVar and point primvar interpolation, edge/corner generic attribute interpolation, and hole topology
remain explicitly refused with tested reasons. The near-cap native release times are recorded in the G7 fixes
handoff; the WASI near-cap timing and peak memory still need measurement. `RefineUniform` remains one
uninterruptible library call. Exact extraordinary limit positions and multilevel crease-decay references remain
future numerical work. ADR status stays Proposed until owner review.
