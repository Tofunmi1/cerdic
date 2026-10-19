# Handoff: G6 operators (P2), with the three newly assigned operations

Written 2026-10-14, after the topology layer landed and was reviewed. Read `handoff-g6-implementation.md`
first (work packages P1 to P6, pre-flight, traps); this file replaces its P2 section with what is now decided
and what to build next. Authority order: ADR 0019, `docs/plans/g6-polygon-operators.md`, the G6 rows of
`docs/modeling/cases.md`, then this file.

## 1. Where things stand (verified 2026-10-14)
- The P1 foundation has deterministic topology, structural/report validation, bounded ear clipping and
  whole-component face orientation. Current P2 operator progress is recorded below.
- `validate` **reports** instead of rejecting: `ValidationReport` carries `degenerate_faces`,
  `nonmanifold_edges`, `inconsistent_winding_edges`, `unclosed_boundary_edge count`, Euler characteristic.
  Only malformed loops, repeated corners and cancellation are errors. Each operator must look at the report
  for the elements **its own selection touches** and refuse only then.
- Triangulation (`triangulate.rs`): ear clipping for simple faces, accepts non-planarity up to 0.1 % of the
  face extent, refuses grossly warped faces (`GeometryError::TriangulationUnsupported`).
- Why the validator was changed: of 40 real evaluator parts, the hollow lamp shade and the hollow goblet
  (Manifold boolean output) contain sliver faces below f32 resolution and used to fail validation. Those two
  are your regression fixtures for every operator (see section 4).
- **Owner decision (2026-10-14): assign all three** of per-face orientation, dissolve-vertex and
  dissolve-face. The plan rows and cases now record those assignments.

### Progress update (2026-10-14)
- `GeometryError::PolyRefusal` now carries a code and optional `Element`; `Element` identifies a face,
  edge or vertex. `poly.flip_faces` implements the partial-flip winding-count rule and sliver refusal.
- `poly.weld` uses the sorted-cell grid and bounded candidate-pair count. `poly.dissolve_edge`,
  `poly.dissolve_vertex` (interior fan and valence-two boundary), and `poly.dissolve_faces` are
  implemented with typed refusals, attribute remapping and transactional candidates.
- The lamp shade and goblet output meshes are checked in at
  `crates/ve-geometry/tests/fixtures/`; `cargo run -p ve-modeling --example
  export_g6_sliver_fixtures` regenerates them from the saved evaluator programs. The G6-01 fixture
  test exercises every operator implemented so far on both. A successful local edit is found away
  from slivers for flip and dissolve; the global weld may instead correctly refuse with a named face
  if its tolerance collapses a face.
- The focused 100,352-face and cancellation cases pass for edge, vertex and patch dissolution. The
  complete `ve-geometry` tests, clippy with `-D warnings`, `cargo xtask rules`, and `git diff --check`
  passed after the latest core operator changes.
- `poly.extrude_faces` now has a narrow planar connected-patch implementation with typed selection,
  winding, sliver, direction and attribute refusals. Point, face and corner provenance is copied;
  edge-domain attributes refuse because side-edge propagation is not yet defined. It has a closed-cube
  topology/attribute test, cancellation coverage, a 100,489-face case and a real sliver selection case.
  V1 currently requires an outward offset along the selected face normal. Geometric self-intersection
  detection for a long extrusion is not implemented; this operator must not be exposed as safe for
  arbitrary offsets until that gap is closed. Individual-face mode and `poly.grid` remain unassigned.
- `poly.inset_face` now insets one strictly convex planar polygon using adjacent offset-line
  intersections. It validates the inradius clearance, rejects nonconvex/too-large inputs with named
  face refusals, and remaps point/face/corner data by source provenance. Edge-domain attributes refuse.
  Cube/square success, invalid/concavity refusals, both real sliver fixtures, cancellation and a
  100,489-face input are covered. The geometric contract remains the planned single face; no
  multi-face or individual mode has been added.
- After extrusion and inset: `cargo test -p ve-geometry`, clippy with `-D warnings`, and `cargo xtask rules`
  pass. This is operator-level progress only; G6 remains open.
- `poly.delete_faces` is assigned and removes selected faces transactionally, leaving an open boundary.
  It refuses deleting every face, creating new isolated vertices, and touching a nonmanifold edge;
  surviving point/face/corner values are retained and edge-domain data refuses. Cube opening,
  isolated-point/all-faces refusals, sliver selection, cancellation and a 100,489-face case pass.
- `poly.bridge_loops` joins two equal-count existing boundary loops using a deterministic minimum-cost
  reversed cyclic alignment (bounded to one million candidate pairs). It preserves point data and maps
  face/corner data from source boundaries. Matching, mismatch, large-mesh and cancellation cases pass.
  It does not yet detect geometric loop/surface intersections, and alignment is ordinal-based rather
  than a semantic selection value; keep it out of graph/MCP exposure until the intersection contract is
  completed. `poly.bevel_edges` remains unimplemented.
- P2 is **not complete**: bridge and bevel remain. `poly.grid` and individual inset/extrude modes from §8.1 are
  also awaiting the owner's answer. Add operator-specific manifold/winding/isolated-element cases for
  the new dissolve operators; run the package gate after the next implementation batch.

## 2. Assignment decision and recorded contracts
The three operations are assigned in `docs/plans/g6-polygon-operators.md`; G6-19 onward are recorded in
`cases.md`, with current typed codes in `docs/modeling/diagnostics.md`. These contracts remain authoritative;
change them only with a written reason:

| Operator | Contract | Typed refusals | Propagation | Cases |
|---|---|---|---|---|
| `poly.flip_faces` | Reverse the corner order of the selected faces. Whole connected components always allowed (same as `flip_component`). A partial selection is allowed only if the number of inconsistent-winding edges after the flip is **not greater** than before (so it can repair a wrongly wound face), or the caller sets `allow_inconsistent=true` (documented, reported in the diagnostic). | `poly.selection_empty`, `poly.selection_invalid`, `poly.flip_increases_inconsistency` (names the first new bad edge) | Face tags stay; corner values reverse with corners; point and edge data unchanged | G6-10 plus new rows |
| `poly.dissolve_vertex` | An interior manifold vertex: merge its incident faces into one loop (the vertex disappears). A boundary vertex of valence 2: drop it and join its two boundary edges. | `poly.vertex_boundary_unsupported` (boundary vertex with more than two edges), `poly.nonmanifold` (pinched vertex, named), `poly.loop_invalid` (result loop not simple or fewer than three corners), `poly.face_degenerate` | Merged face keeps a shared face tag only if all agree, else refuse with `poly.attribute_conflict` (explicit winner param later); corner data copied corner by corner | new rows |
| `poly.dissolve_faces` | Merge a **connected, consistently wound** set of selected faces (connected through shared edges) into one n-gon by removing their interior edges. | `poly.selection_disconnected`, `poly.loop_invalid` (result not one simple loop: holes and pinches unsupported in v1), `poly.nonmanifold`, `poly.winding_conflict`, `poly.attribute_conflict` | As above | new rows |

Add case rows for each (a partial flip that repairs one wrong face; a partial flip that makes it worse is
refused; dissolve a valence-4 interior vertex; refuse a pinched vertex; dissolve a 2x2 patch of quads into
one n-gon; refuse a patch whose union has a hole). Keep the existing G6-01 to G6-18 ids and append G6-19+.

**Decision recorded for this implementation batch:** `poly.delete_faces` is assigned to open boundaries
for later bridging and window openings. `poly.bridge_loops` currently accepts only boundaries already
present on the mesh; it does not auto-delete faces or resample unequal loops. Geometric intersection
checking remains a blocker to graph/MCP exposure.

## 3. Build order and what each operator must do
Work in this order; finish each with its refusal cases first, then the success cases. After each operator run
`cargo fmt --all`, `cargo test -p ve-geometry`, clippy and `cargo xtask rules`. Keep files under 400 lines
and functions under 80 (split by responsibility: one file per operator under `src/poly/`).

1. `poly.flip_faces` (extends `flip.rs` or a sibling file).
2. `poly.weld`: deterministic distance clusters found with a **sorted-cell spatial grid** (cell size from the
   tolerance), union-find with stable minimum-index representative, rewrite faces. Count the pairs actually
   tested against a budget; refuse before work when it cannot be met. Never merge corner (UV) data across charts.
3. `poly.dissolve_edge`, then `poly.dissolve_vertex`, then `poly.dissolve_faces` (share a loop-merge helper).
4. `poly.extrude_faces`, `poly.inset_face`.
5. `poly.delete_faces` (only if assigned), then `poly.bridge_loops`, then `poly.bevel_edges`.

Every operator follows the same shape:
```
candidate = op(&PolyMesh, &Selection, &Params, &ExecutionContext) -> Result<PolyMesh, GeometryError>
```
- Work on a **copy**; on any failure the input is untouched (transactional).
- Run `validate` on the input and on the candidate; compare the **report** against what the operator promises
  (expected Euler change, component count, no new non-manifold or inconsistent edges, no new degenerate faces).
  Refuse only for conditions that touch the selection or that the operator itself would introduce.
- Poll `ExecutionContext` in every loop; keep work counters.
- Propagate attributes per domain exactly as the plan table says; corner data is never blended across a seam.

**Make errors name the element.** `GeometryError::PolyRefusal(&'static str)` carries only a code. The plan
requires refusals to name the offending face, edge or vertex. Change it to carry the code plus the element
(for example `PolyRefusal { code, element: Option<Element> }` with `Element::{Face(usize), Edge(u32, u32),
Vertex(u32)}`), update `flip.rs` and its tests, and have the graph layer later map a face ordinal to its
semantic tag for the diagnostic. Do this before the second operator so every operator uses it.

## 4. Fixtures and tests every operator needs
- **Slivers:** keep two real meshes with sliver faces as checked-in test data: the hollow lamp shade and the
  hollow goblet. Export them from the evaluator (programs are in `engine/target/reports/big-test/programs/`,
  git-ignored; the Rust loader pattern is in `crates/ve-modeling/tests/gallery.rs`) into a small text format
  under `crates/ve-geometry/tests/fixtures/`, with a README stating they are our own procedural output. A
  test per operator: an edit away from the slivers succeeds; an edit whose selection touches a sliver is
  refused with the sliver named.
- Closed, open, disconnected, non-manifold (edge with three faces), inconsistent-winding and isolated-vertex
  inputs for every operator (G6-13 style): each follows its stated contract and names the element.
- A 100,000-face input for every operator: finishes within the stated budget or refuses before work, and a
  cancellation test (see `g6_16_*` in `g6_topology.rs`).
- Determinism: output identical run to run; native versus WASI parity comes when the operators reach the
  evaluator corpus (P4), but write them with sorted maps and fixed traversal now.

## 5. Not in this stretch
Graph integration (P4), semantic selections (P3), gallery (P5) and the report (P6) follow after the operators
pass; **section 8 lists everything that remains in G6, in order, so nothing is lost.** Do not wire anything
into `ve-graph` or `ve-modeling` yet, except where an error type change forces it. Do not commit; the owner
commits the combined tree. SwiftShader is the only renderer so far.

## 6. Parallel work that does not touch these files
Language and tool-call evaluation harness; colour regions through deformers; the comb-union measurement; a
world-generation scoping handoff. Details are in `handoff-g6-implementation.md` section 2 and
`handoff-next-after-g5.md` section 3.

## 8. Status 2026-10-14 (read this first)

**Everything in the lists below is now done**, except the items in `docs/modeling/g6-report.md` under
"Limits and what is not done" and the owner decisions. The operators, selections, graph integration, MCP
guide, gallery, parity corpus, ADR 0020, report and a passing strict gate (`target/g6-strict-01.log`) exist.
The sections below are kept as the record of what was planned; do not redo them. What is left for G6 is the
owner's: accept ADR 0019 and ADR 0020, commit the tree, and decide about the added scope. After that the
next gates are G7 (OpenSubdiv adapter behind validated `PolyMesh` topology) and, optionally, V5 (polygon edit
mode in the viewport).

## 8a. (history) Everything that remains in G6 (the complete list)

### 8.1 Two gaps found while checking the facade fixture (decide first; both are small)
The plan's G6 fixture is a parameterised building facade (windows inset, frames extruded, bevelled cornice).
With the operators as specified it cannot be built, because:
1. **There is no source of polygon meshes except `poly.from_solid`**, and a Manifold box is triangulated, so
   a wall face is two triangles, not a quad grid of window cells. The chain `from_solid`, select the face by
   normal, `dissolve_faces` (merge the two triangles into a quad) works for one quad, but a facade needs a
   grid of cells. Recommended: add a small primitive **`poly.grid@1(width, height, columns, rows)`** (quad
   grid, open boundary, deterministic ids) and, if wanted, `poly.box_quads`.
2. **`poly.inset_face` and `poly.extrude_faces` act on one face or one connected patch.** A facade insets many
   separate cells. Recommended: an `individual=true` mode that applies the operator to each selected face
   independently (each must satisfy the single-face contract, otherwise the whole command refuses naming the
   face), as modelling tools do.
Both need the owner's yes because they add scope to the plan table; if refused, the facade fixture must be
redefined and the reason recorded.

### 8.2 Case status (G6-01 to G6-18 from `cases.md`, plus the new rows)
| Case | Status now | What remains |
|---|---|---|
| G6-01 solid to polygon and back, slivers | Adapters and validator exist; sliver reporting tested | `poly.from_solid` and `poly.to_solid` graph operators (P4); round-trip volume and winding test; sliver fixtures |
| G6-02 concave and non-planar triangulation | Done in `ve-geometry` (3 tests) | Make `poly.to_solid` and `to_kernel_mesh` use it everywhere (no fan); surface the diagnostic |
| G6-03 extrude top face | open | `poly.extrude_faces` |
| G6-04, G6-05 inset refusals | open | `poly.inset_face` |
| G6-06, G6-07 bridge | open | `poly.bridge_loops` (and the delete-faces decision) |
| G6-08 weld | open | `poly.weld` with spatial grid |
| G6-09 dissolve edge | open | `poly.dissolve_edge` |
| G6-10 flip | Whole-component flip done and tested | `poly.flip_faces` (per-face, new contract) |
| G6-11 bevel | open | `poly.bevel_edges` |
| G6-12 open mesh output | open | polygon `Value` and render path with no volume (P4) |
| G6-13 non-manifold, winding, touching shells | Validator and topology report them (tests) | Each operator's own contract and refusal naming the element |
| G6-14 semantic selection | open | P3 selections and tags |
| G6-15 corner (UV) attributes through operators | open | per-operator propagation tests |
| G6-16 100,000 faces and cancellation | Topology and triangulation covered | every operator: preflight, budget, cancellation tests |
| G6-17 persistence, undo, group, instance | open | P4 plus tests like `g5_persistence.rs` |
| G6-18 native versus WASI | open | corpus lines for every operator (P4) |
| G6-19+ (new) | not written | per-face flip repairs one wrong face and refuses to make it worse; dissolve a valence-4 vertex; refuse a pinched vertex; dissolve a 2x2 patch into one n-gon; refuse a patch with a hole or disconnected; delete-faces opens a boundary and refuses an isolated vertex (if assigned); `poly.grid` and individual-mode rows (if assigned); sliver-fixture rows |

### 8.3 P3: selections and face tags (design first, then code)
Persistent commands never carry a face ordinal. **Recommended design (needs an ADR, number 0020, proposed):**
a dedicated selection node, for example `poly.select_faces@1(mesh, by="normal", direction=[0,1,0],
angle=5deg)` or `by="tag"`, `by="origin"`, `by="plane"`, `by="named"`, producing a **Selection value** that
operator nodes take on a `selection` input port (new `PortType::Selection`). This keeps text one node per line,
gives each selection its own stable node id, and lets undo, groups and instances treat it like any node.
Rules to write down: resolution happens against the candidate input before the edit; no match gives
`selection.lost`; several matches where one is required gives `selection.ambiguous`; generated faces get
deterministic child tags derived from operation id and source tag; origin predicates reuse the kernel origin
ids (`ve-modeling/src/regions.rs`, `Solid::from_mesh_runs`) so "faces that came from operand A" survives
upstream edits. Viewport face picking is V5 (optional) and out of scope; the viewport would convert a picked
ordinal to a semantic tag before building a command.

### 8.4 P4: graph integration (only after the operators pass)
- `ve-graph`: `PortType::Poly` and `PortType::Selection` in `spec.rs`; operator specs in a new
  `builtin/poly.rs` (keep each file under 400 lines) with `READY` entries only for operators whose cases pass;
  parameter types for tolerances, distances, modes; text-form coercion; `bun run schema:generate`.
- Diagnostics: register every `poly.*` code and the selection codes in the generated schema and in
  `docs/modeling/diagnostics.md`; the graph layer maps `PolyRefusal` plus element to a diagnostic with the
  node, the semantic tag (never the ordinal) and a suggestion.
- `ve-modeling`: a `Value` field or variant for the polygon mesh (`cache.rs`, include it in `bytes()`),
  dispatch in `operations.rs`, preflight in `plan/estimate.rs` for faces, vertices, corners, weld pairs
  actually tested and output faces (refuse before allocation; add chain tests so estimates do not compound),
  limits in `docs/modeling/limits.md`, cancellation through `ExecutionContext`.
- Output of an open or non-solid polygon mesh: a `Part` with triangulated indices (use the new triangulation),
  normals, **no claimed volume**, `Part.region` still applies; confirm `src/engine/mesh-upload.ts` and the
  runtime draw it (G6-12) and that the viewer does not assume a closed solid.
- `poly.from_solid` / `poly.to_solid` with `LossPolicy` surfaced as a diagnostic.
- MCP guide text in `tools/mcp/server.mjs`: only operators whose cases pass, each with one example.
- Parity corpus lines in `crates/ve-modeling/examples/corpus.rs` for every operator (G6-18); any new browser
  tests registered in `xtask/src/web.rs`.

### 8.5 P5: evidence
Gallery pieces with six SwiftShader views each in `crates/ve-modeling/tests/gallery/` and
`web/tests/gallery.mjs`: the parameterised building facade (subject to 8.1), a box whose top face is extruded
and inset into a tray, and a bevelled-edge block. A real-model run of a polygon-editing prompt if credit
exists. Add the gallery cases to `cases.md`.

### 8.6 P6: close
`docs/modeling/g6-report.md` (implemented, measured time and memory at 100,000 faces, remaining limits,
SwiftShader only), ADR 0019 acceptance request plus ADR 0020 (selections) and any ADR for added scope,
Track 2 plan G6 status updated, `cargo xtask check --strict` once with a fresh log and the real `^exit=` line,
nothing committed by the agent. G7 (OpenSubdiv adapter), V5 (polygon edit mode) and the deferred wider
contracts (non-planar extrude, bevel edge networks, nonconvex inset, bridge with resampling) are **not** part
of G6; record them as follow-on work in the report.

## 7. Done means for this stretch
The three operators are assigned in the plan and cases (and the delete-faces question is answered); every
operator in the build order exists in `ve-geometry` with refusal and success cases, sliver fixtures, a
100,000-face case and a cancellation case; refusals name their element; `cargo test -p ve-geometry`, clippy
and `cargo xtask rules` pass; nothing is committed. Then continue with P3 and P4 (section 8). **G6 itself is
done only when section 8 is complete.**
