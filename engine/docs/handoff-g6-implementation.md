# Handoff: G6 implementation (after X3), plus work that can run in parallel

Written 2026-10-14. Read `handoff-common.md`, `handoff-next-after-g5.md` (tree state and traps) and
`handoff-x3-g6-polygon-tools.md` (what G6 is, what exists, the X3 outcome) first. This file turns them into work
packages. Authority order if they disagree: ADR 0019, `docs/plans/g6-polygon-operators.md`, the G6 rows in
`docs/modeling/cases.md`, then this file.

## 0. Pre-flight: do not start coding until these are settled
These come from a review of the X3 deliverables on 2026-10-14. They are decisions for the owner or maintainer;
record the answer in the operator plan, do not guess.
1. **Unassigned or narrowed operators.** The plan's rule is "never begin G6 with an unassigned operator", and
   `g6-polygon-operators.md` has only `poly.dissolve_edge` and `poly.flip_component`. Ask for a written
   decision on each: dissolve vertex, dissolve face (assign a row, or defer with a typed refusal and a case),
   and per-face orientation change (assign, or accept whole-component flip as the v1 contract).
2. **Narrow v1 contracts.** Extrude takes one planar convex face or a planar patch, inset a planar convex
   n-gon, bevel disjoint edges only, bridge equal-length loops. They refuse with typed errors, which is the
   right behaviour, and they are enough for the facade fixture. They will not cover subdivided or curved
   meshes (an organic model's first need is non-planar extrusion). Ask whether v1 stops there or whether a
   second contract (local frames for non-planar patches, bevel edge networks, nonconvex inset) is part of G6.
3. **Tree hygiene.** The working tree holds V3, G5, material regions and the X3 docs, about 120 paths, all
   uncommitted. Ask the owner to commit it and to accept or reject ADRs 0017, 0018 and 0019 first, so G6 starts
   from a known baseline.
4. Weld is written as "sorted candidate pairs". At 100,000 faces that must use a deterministic spatial grid
   (sorted cell keys), not all pairs; the budget counts pairs actually tested (the G5 pair-budget lesson).

## 1. Work packages (each ends with tests, clippy, `cargo xtask rules`; one strict gate at the end)
Keep every `.rs` file under 400 lines and every function under 80; split by responsibility. No `#[allow]`.

### P1. Topology layer in `crates/ve-geometry` (pure Rust, no graph)
Files to add: `topology.rs` (edge incidence, adjacency), `topology/boundary.rs` (loops), `validate.rs`.
- Edge map keyed by sorted vertex pair in a `BTreeMap`; each edge holds its incident face-corners in a fixed
  order. Derive face-face and vertex-face adjacency from it. Boundary loops, edge class (boundary, manifold,
  nonmanifold with the count), connected components, isolated vertices, Euler characteristic.
- `validate(&PolyMesh) -> Result<Report, GeometryError>` with the checks in the operator plan: indices in
  range, at least three distinct corners per face, no repeated corner, nonzero face area **relative to f32
  mesh resolution** (never an absolute epsilon; Manifold output contains slivers), consistent winding on
  manifold interior edges, boundary loop closure, expected component and Euler change.
- Every walk takes `&ve_kernel::ExecutionContext` and polls cancellation; counters for work done.
- Tests (`tests/g6_topology.rs`): cube, open grid, two components, isolated vertex, repeated corner,
  zero-area face, reversed winding, edge with three faces, 100,000-face grid timing.
- Robust triangulation (needed by `poly.to_solid`, G6-02): project the face on its Newell normal and ear-clip;
  refuse non-simple or zero-area faces with `poly.triangulation_unsupported`. Replace the fan in
  `PolyMesh::to_kernel_mesh` (it is only right for planar convex faces) or make the fan an explicit mode.

### P2. Operators on `PolyMesh` in `ve-geometry`, easiest first (plan order)
`poly/flip.rs`, `weld.rs`, `dissolve.rs`, `extrude.rs`, `inset.rs`, `bridge.rs`, `bevel.rs`. Each is a pure
function `candidate = op(&PolyMesh, &Selection, &Params, &ExecutionContext)` that edits a **copy**, runs the
validator and the expected-topology check, and returns the new mesh or a typed `poly.*` refusal; the caller
publishes only on success (transactional). Write the refusal cases first; each operator's cases are listed in
the plan table. Propagation per domain exactly as the plan says (corner data is never blended across a seam).
New diagnostic codes go through the generated schema and `docs/modeling/diagnostics.md` in the same change.

### P3. Selections and face tags
Persistent commands never store a face ordinal. Implement the predicates the plan lists (face tag, origin label,
plane or normal query, named set), resolution against the candidate input, `selection.lost` for none and
`selection.ambiguous` for several, deterministic child tags derived from operation id plus source tag. Origin
labels can reuse the kernel origin ids that material regions already use (`ve-modeling/src/regions.rs`,
`Solid::from_mesh_runs`). Write the contract into the plan or `cases.md` before coding it. Cases: G6-14.

### P4. Graph integration (only after P2 passes)
Touch points, learned from G5 and the colour-region work:
- `ve-graph/src/spec.rs` (`PortType::Poly`), `builtin/` new `poly.rs` with specs and `READY` entries,
  `text/coerce.rs`, `value.rs` for any new parameter type (selection predicates), `bun run schema:generate`.
- `ve-modeling`: a `Value` variant or field for the polygon mesh (`cache.rs`), `operations.rs` dispatch,
  `plan/estimate.rs` (faces, vertices, candidate pairs, output; refuse before allocation), `evaluate.rs` and
  `instances/output.rs` for output. **Open polygon outputs have no volume**; they still need positions,
  triangulated indices, normals and `Part.region`.
- `poly.from_solid` and `poly.to_solid` with `LossPolicy` surfaced as a diagnostic.
- Browser side: `src/engine/mesh-upload.ts` and the runtime take parts as they are; verify an open mesh
  renders (G6-12). MCP guide text in `tools/mcp/server.mjs` only for operators whose cases pass.
- Parity corpus lines in `crates/ve-modeling/examples/corpus.rs` for every operator (G6-18), registered
  browser tests in `xtask/src/web.rs` if you add any.

### P5. Evidence
Gallery: the parameterised building facade (windows inset, frames extruded, bevelled cornice) and a box whose
top face is extruded and inset into a tray, six SwiftShader views each (`crates/ve-modeling/tests/gallery/`,
`web/tests/gallery.mjs`). A real-model run if credit exists. Cases G6-01 to G6-18 pass or are recorded limits.

### P6. Close
`docs/modeling/g6-report.md` (implemented, measured, remaining, SwiftShader only), ADR acceptance request,
`cargo xtask check --strict` once with a fresh log and the real `^exit=` line, nothing committed by the agent.

## 2. Work that can run in parallel (does not touch the G6 files)
- **Language and tool-call evaluation harness.** Build a fixed prompt set (the six in
  `engine/target/reports/big-test/programs/`, plus a few simpler ones) and record per model: first-try compile
  rate, repairs, tokens, cost, wall time and a visual rating. Then try, in this order: a trimmed core guide plus
  `get_operator_help(name)` and prompt caching; a `snapshot` tool returning a rendered view as an image and a
  `measure` tool (bounds, volume, thickness, floating parts); `get_example(name)` from our own gallery
  programs. Measure before and after; do not guess. Needs provider credit for strong models; the CLI and
  AgentRouter notes are in `handoff-next-after-g5.md`.
- **Colour regions through the operations that flatten them** (twist, taper, bend, `inst.realize`,
  `xform.attach`): see `handoff-next-after-g5.md` section 3C for the design.
- **Comb-union measurement:** build two interleaved dense meshes and compare the capped union estimate with the
  real triangle count; record whether the estimate can undercount enough to start a heavy boolean.
- **World-generation scoping** (owner asked about generating worlds): terrain and fields (G8), rule-based
  scattering, procedural materials (G10), level of detail and culling, a measured Quest 3 triangle and draw-call
  budget first, logic (L0/L1). Write it as its own handoff before building.

## 3. Traps (new here plus the standing ones)
- Boolean output has slivers and zero-area triangles; thresholds relative to f32 resolution, never absolute.
- Determinism across native and WASI: sorted maps, fixed traversal order, no hash iteration, no parallel sums.
- Budgets count work done; estimates that multiply per level compound, so test chains.
- Open and non-manifold meshes are legal G6 inputs; a Manifold kernel error on them is a bug, not a contract.
- Fan triangulation of concave or non-planar faces is wrong; ear-clip or refuse.
- Vendored C++ (none yet for G6) would need `VENDOR.md`, clang-format, clang-tidy, ASan and UBSan tests.
- Test helpers that unwrap need a file-level `#![expect(clippy::unwrap_used, reason = "...")]`; pedantic
  lints include similar_names, many_single_char_names and cast lints.
- No shell edits of files, ever; all SwiftShader evidence says "software renderer".

## 4. Done means
Owner decisions in section 0 recorded; P1 to P6 complete or each gap recorded as a limit with a case; the facade
renders from six views; `g6-report.md` has measured numbers; strict gate log named in the report; ADRs proposed
for acceptance; nothing committed by the agent.
