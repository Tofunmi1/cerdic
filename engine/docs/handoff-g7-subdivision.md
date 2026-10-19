# Handoff: G7 subdivision surfaces (Catmull-Clark through OpenSubdiv)

Written 2026-10-14, after G6 was implemented and its strict gate passed. Read `handoff-common.md` (owner
rules, gate mechanics), `handoff-next-after-g5.md` (traps), then `docs/modeling/g6-report.md` and
`docs/adr/0019-polygon-and-subdivision-backend.md` and `0020-polygon-selection-and-graph-integration.md`
(what G7 builds on). Plan text: `docs/plans/track2-modeling.md` (G7 near line 392). Acceptance:
`docs/modeling/cases.md` (the G7 line). X3 evidence: `docs/modeling/experiments.md` (X3) and
`spikes/x3-polygon/`.

## 1. What G7 is and why it matters
G7 makes smooth shapes from coarse polygon cages: a cube becomes a rounded form, a blocked-out body becomes a
character. With G6 (edit the cage) and G8 (organic fields) it is the route to things like a rabbit. The plan's
contract: Catmull-Clark with supported boundary and crease rules and face-varying (UV) data, using the backend X3
selected (OpenSubdiv CPU Far, behind an adapter); the **cage is kept** and edits stay on it; preview and final
levels; extraordinary vertices, creases, boundaries and UV seams match reference cases within tolerance;
refinement growth is preflighted and cancellable. Manifold's smooth and refine remain a separate triangle
operation and are not a substitute.

## 2. State you start from (verified 2026-10-14)
- G6 is complete: fifteen `poly.*` nodes, `Poly` and `Selection` port types, validated `PolyMesh` topology
  (`ve-geometry`: `Topology`, `validate`), ear-clip triangulation, the node and MCP integration, gallery,
  parity. `cargo xtask check --strict` passed (`target/g6-strict-01.log`, 549 tests).
- ADR 0019 (Proposed) chooses **OpenSubdiv CPU Far, behind an adapter that takes validated `PolyMesh`
  topology and returns bounded refined geometry plus defined corner data; no GPU backend, threads or Osd device
  backend**. The choice is *conditional on G7 proving* boundary, crease, UV-seam, refinement-budget,
  cancellation-boundary and native/WASI cases.
- X3 measured (spike only, not evaluator integration): OpenSubdiv 3.7.0 (`9dab8a47bfbb1388ec8388fe61f5f916e6123f38`)
  built natively and for `wasm32-wasip1`; the small probe returned 641 vertices and 600 faces on both; one
  Catmark step of a 100,000-triangle grid made 300,000 quads in a median 16.07 ms native (includes mesh setup);
  the standalone WASI link added 78,450 raw / 30,061 Brotli bytes over a bare C++ executable.
- **Not verified, and G7's job to verify:** crease and UV behaviour against reference values, large-mesh timing
  under WASI, the real size delta inside the evaluator, behaviour on non-manifold input, and cancellation.
- Licence: OpenSubdiv's Tomorrow Open Source Technology License 1.0 is allowed (ADR 0005). Vendor with a
  `VENDOR.md` (upstream, tag, commit, licence file, local changes) as `third_party/manifold/VENDOR.md` does.
- The X3 spike needed one patch for WASI (`spikes/x3-polygon/opensubdiv-wasi-compat.patch`: a missing `<cmath>`
  include in `opensubdiv/bfr/tessellation.cpp`). If you vendor only `sdc`, `vtr` and `far`, the Bfr file is not
  built and the patch may not be needed; check.

## 3. Design decisions you must make early (write them into an ADR, number 0021)
1. **How creases and corners reach the subdivider.** `PolyMesh` attributes are *scalar, one value per element*
   (`AttrData` is a flat vector; there are no vector types) and edge-domain attributes have no documented
   element order; also most G6 operators **refuse** edge attributes (`poly.attribute_conflict`) because new
   edges have no source value. So do not store creases as an edge attribute. Recommended: `poly.subdivide`
   takes **optional selection inputs**: `creases` (a `Selection` of edges) with a `crease` sharpness
   parameter, and `corners` (a `Selection` of vertices) with a `corner` sharpness parameter. Selections are
   already rule-based, stable and checked against the mesh (ADR 0020). If several sharpness values are
   needed, allow more than one crease input, or add a `poly.crease` node that carries a sharpness with its
   selection.
2. **UVs (face-varying data).** A UV is two scalar corner-domain attributes today (name them, for example,
   `uv_u` and `uv_v`; decide and document the convention). OpenSubdiv face-varying channels need, per channel,
   a list of unique values and an index per corner; build them by welding corners that share a vertex and equal
   values, so a UV seam stays a seam. Corner data is never averaged across a seam (the plan and ADR 0019 say so).
3. **Other attributes.** Face attributes (`region`, `role`, tags): each child face copies its parent's value
   (discrete, never blended). Point attributes: float ones interpolate through the refiner; integer ones cannot
   be defined for new vertices, so they are dropped and *reported* (not silently), or the node refuses; pick one
   and document it. Edge attributes refuse, as in G6.
4. **Levels, preview and final.** The plan wants preview and final levels. `operations::execute` does not
   currently receive the job's `Quality`; either plumb it through (small, touches `operations.rs`, `evaluate.rs`)
   or give the node two parameters (`preview_levels`, `levels`) and have the planner pick by quality. Cap total
   output against the quality triangle budget (Preview 200,000, Final 2,000,000): a level multiplies faces by
   about four, so predict quads as `sum(face sizes)` at level one and times four after, triangles as twice the
   quads, and refuse before refining.
5. **Rendering curved quads (the trap).** Subdivided quads are *not planar*. `PolyMesh` triangulation refuses
   faces more than 0.1 percent of their extent out of plane (test `g6_02_nonplanar_polygon_is_refused_...` holds
   a 0.2-warped quad). A subdivided mesh would therefore fail to draw or convert to a solid. Add an explicit
   curved-quad triangulation mode: split a quad along its **shorter diagonal** (deterministic), keep refusing
   non-planar faces with more than four corners, and select the mode for subdivision results (all of whose faces
   are quads). This changes the G6-02 contract for quads, so it needs the ADR. Also render with smooth shading:
   the page already computes corner normals with a 40 degree crease (`src/engine/mesh-upload.ts`); optionally
   evaluate true limit normals from the refiner.
6. **Cancellation.** Neither library supports the engine's cancellation callback; one library call cannot be
   interrupted. Keep each call within a stated worst case: refine topology once (measure it), then interpolate
   **one level at a time** (`PrimvarRefiner` per level) and poll `ExecutionContext` between levels. Record the
   longest uninterruptible call you measure in the report.
7. **Adaptive versus uniform.** Start with uniform refinement (predictable growth). Adaptive feature refinement
   is out of scope unless you can bound its output the same way.

## 4. Work packages
Each package ends with tests, `cargo fmt --all`, clippy with `-D warnings`, `cargo xtask rules`; one strict gate
at the end. Keep `.rs` files under 400 lines and functions under 80; no `#[allow]`; native and WASI output must
agree (sorted maps, fixed orders).

### P1. Vendor and build OpenSubdiv
- Vendor only what is needed (`opensubdiv/sdc`, `vtr`, `far`; not `osd`, `bfr`, examples, regression, docs) into
  `third_party/opensubdiv/` with `VENDOR.md`. Add it to `cpp/CMakeLists.txt` as a static library built with the
  kernel's flags (`-fno-exceptions`, no RTTI; `crates/ve-kernel-sys/build.rs` is the one place that drives CMake
  for native and `wasm32-wasip1`). Disable ptex, OpenMP, TBB, GPU backends (the X3 README lists the flags).
- The C++ gate applies: clang-format, clang-tidy (unchecked `[]` on vectors is an error; use `std::span`,
  `.data()` ranges, `std::copy_n`), ASan and UBSan native tests (`cpp/tests/`). Third-party headers are excluded
  from tidy by the existing configuration; check it covers the new directory.
- Measure the real evaluator size delta (`cargo xtask web`, Brotli quality 9, against `g6-report.md`: evaluator
  1,810,103 raw / 511,350 Brotli, runtime 34,510,143 / 4,119,722).

### P2. C ABI and safe wrapper
- `cpp/include/ve_kernel.h` and `cpp/src/` gain a small, handle-based API (pattern: `ve_solid`): build a refiner
  from face sizes, corner indices, optional crease pairs and weights, optional corner (vertex) weights, optional
  hole faces, and face-varying channels (values plus value indices per corner); options (Catmark scheme, vertex
  boundary rule, face-varying linear rule, creasing method); refine uniformly to N levels; copy out positions,
  face sizes, corners, per-channel values and indices, and **provenance** (each output face's parent face at
  level zero, each output vertex's origin) so attributes can be propagated. Verify every class and method name
  against the vendored 3.7.0 headers before relying on this outline: `Far::TopologyDescriptor`,
  `Far::TopologyRefinerFactory`, `Far::TopologyRefiner::RefineUniform`, `Far::PrimvarRefiner` (`Interpolate`,
  `InterpolateFaceVarying`), `Far::TopologyLevel`.
- Errors map to `ve_status` (invalid argument, non-manifold, too large, buffer too small); no exception crosses
  the boundary; counts are checked before any allocation.
- `ve-kernel-sys` declarations, `ve-kernel/src/ffi/` unsafe layer with `// SAFETY:` comments, a safe
  `ve_kernel` function. Tests in `crates/ve-kernel/tests/` (pattern: `origin_runs.rs`).

### P3. Adapter in `ve-geometry`
`ve_geometry::subdivide(mesh, options, context) -> Result<Subdivided, GeometryError>` (new file under `src/`,
e.g. `subdivide.rs` plus `subdivide/provenance.rs`): validates the cage (`validate`; refuse non-manifold edges
and inconsistent winding that touch faces, with `poly.*`-style typed codes naming the element), builds the
descriptor (UV welding, creases, corners), preflights output size against a budget, calls the kernel, then
rebuilds a `PolyMesh` and propagates attributes per decision 3 of section 3. Keep the cage untouched (it is a
new mesh). Polls cancellation between levels.

### P4. Graph integration
`poly.subdivide@1` (ports `in`, optional `creases` and `corners` selections; parameters per section 3), and
whatever curved-quad support you add. Touch points are the same as in G6: `ve-graph` `PortType` use and
`builtin/poly.rs` (keep functions under 80 lines; split spec functions), `ve-modeling/src/poly_ops/edit.rs`
(new file `poly_ops/subdivide.rs`), `plan/estimate.rs` (predicted faces, exact for uniform levels),
`poly_ops/convert.rs` triangulation mode, `docs/modeling/diagnostics.md`, `bun run schema:generate`, MCP guide
text in `tools/mcp/server.mjs` (only after the cases pass), parity corpus lines in
`crates/ve-modeling/examples/corpus.rs`.

### P5. Cases to write first (G7-01 onwards; refine with the owner)
| ID | Case | Expected |
|---|---|---|
| G7-01 | One Catmark level of a closed cube | 26 vertices, 24 quads, Euler 2, positions equal an independent in-test implementation of the Catmull-Clark rules (face point, edge point, vertex point) within 1e-6 |
| G7-02 | Levels 1 to 4 of a cube | Face counts 6 x 4^n; each level's surface stays inside the previous bounding box; successive level differences shrink; closed and consistently wound |
| G7-03 | A cage with extraordinary vertices (valence 3 and 5) | Matches the reference rules at the extraordinary vertices; no cracks |
| G7-04 | Open grid with boundary | The boundary rule requested (edge only, or edge and corner) is honoured; boundary stays a polyline of the expected shape |
| G7-05 | Creased edge (sharpness 0, 1, 2, 10) | Sharpness 0 equals no crease; 10 keeps a straight crease edge; intermediate values in between; reference positions from the in-test rules |
| G7-06 | Corner (vertex) sharpness | The corner vertex stays in place at high sharpness |
| G7-07 | UV seam | Two corners with different UVs at one vertex stay different after refinement; no UV is averaged across the seam; continuous UVs stay continuous |
| G7-08 | Attributes | Face attributes copied to children; float point attributes interpolate; integer point attributes dropped and reported (or refused) per the ADR; edge attributes refused |
| G7-09 | Growth preflight | A cage and level predicted over the quality budget refuses before the library is called, with limit and observed counts |
| G7-10 | Cancellation | Pre-cancelled call returns `eval.cancelled`; cancel between levels stops; the longest uninterruptible call is measured and recorded |
| G7-11 | Cage kept, edits upstream | Editing the cage (G6 operators) and re-subdividing gives the new surface; the cage's own value is unchanged |
| G7-12 | Curved quads render and convert | A subdivided cube draws and converts to a closed solid through `poly.to_solid`; a quad folded more than a stated bound still refuses |
| G7-13 | Non-manifold and winding-conflict cages | Typed refusal naming the element; no library call |
| G7-14 | Native versus WASI | Identical output lines for cube, creased box, grid with boundary and a UV-seam mesh |
| G7-15 | 100,000-face cage at one level, and a small cage at the highest allowed level | Completes under the stated budget; timings recorded (release, native and WASI) |
| G7-16 | Save, reload, undo, redo, group, instance | Stable IDs and identical output, as for G6-17 |

### P6. Evidence and close
Gallery pieces in `crates/ve-modeling/tests/gallery/` and `web/tests/gallery.mjs` (six SwiftShader views each):
a smooth cube-to-blob, a creased box (hard edges softened elsewhere), a blocked-out character head or a
low-poly rabbit body subdivided (this is the demo). A real-model run on a subdivision prompt if credit exists.
`docs/modeling/g7-report.md` (implemented, measured size delta and timings, longest uninterruptible call,
limits, SwiftShader only), ADR 0019 acceptance request plus ADR 0021, Track 2 plan status, one
`cargo xtask check --strict` with a fresh log and the real `^exit=` line. Nothing committed by the agent.

## 5. Traps (from G5 and G6, plus the new ones)
- Manifold output and any evaluator mesh can contain slivers; thresholds relative to f32 resolution.
- Subdivided quads are non-planar: do not push them through the ear-clip path (section 3, item 5).
- Edge attributes are scalar and unordered, and G6 operators refuse them: keep creases out of attributes.
- `PolyMesh` attributes are scalars: a UV is two corner attributes; there is no vector type.
- A budget must count work actually done; estimates that multiply per level must be exact for uniform
  refinement (faces times four per level), so do not over-estimate and refuse valid work.
- Determinism: no hash-map iteration, fixed traversal order; native and WASI must match to the last printed digit
  (the G6 corpus already did with trigonometry; subdivision rules are rational arithmetic, so it should hold).
- Vendored C++ is gated hard: clang-format, clang-tidy, ASan, UBSan, `VENDOR.md`, no exceptions, no RTTI.
- The build flag set matters for size: build only Sdc, Vtr and Far, optimise for size on wasm
  (`MinSizeRel` is already the wasm profile in `build.rs`).
- No shell-based file edits, ever. Do not publish or commit.

## 6. Decisions for the owner (do not decide for them)
1. Accept or amend ADR 0019 (its OpenSubdiv choice is conditional on G7's evidence), and ADR 0020.
2. Commit the combined tree (about 165 paths, G5 through G6) before G7 adds more.
3. The curved-quad triangulation rule (shorter diagonal for four-sided faces) changes a G6 contract: approve it
   in the G7 ADR.
4. Whether integer point attributes are dropped and reported or make the node refuse.
5. Whether a low-poly rabbit (or head) is the G7 demo piece, and whether to spend router credit on a model run.

## 7. Done means
OpenSubdiv is vendored and builds natively and for WASI under the kernel's flags with the C++ gate passing; the
adapter and `poly.subdivide` exist with cases G7-01 to G7-16 passing or recorded as limits; native and WASI
output agree; the measured evaluator size delta, timings and longest uninterruptible call are in
`g7-report.md`; gallery pieces render from six views; ADRs proposed for acceptance; the strict gate passed with
its log named; nothing committed by the agent.
