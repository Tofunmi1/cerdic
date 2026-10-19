# Handoff: X3 (OpenSubdiv and PMP probe), then G6 (polygon tools)

Written 2026-10-14. Read `handoff-common.md` first (owner rules, gate mechanics, architecture), then
`handoff-next-after-g5.md` (current state of the tree and the traps learned in G5). This file covers the plan's
next gate: **X3 is complete; G6 starts after maintainer review of its operator table and proposed ADR.** Plan text:
`docs/plans/track2-modeling.md` (G6 around line 384, X3 in the experiments table around line 527, decisions
D1 and D2 near line 577, rule 4 near line 67). Acceptance rows: `docs/modeling/cases.md` (G6 line).

**X3 status (2026-10-14):** experiment, ADR 0019, operator plan and the 18-row G6 case table are now
written. OpenSubdiv's native/WASI Catmark smoke build passed; PMP's no-exception/no-RTTI WASI build
failed as recorded in the experiment. Before starting G6, the owner should review ADR 0019 and the
operator contracts. The full evaluator size delta, large-mesh WASI timing, crease/UV fixtures and
runtime non-manifold behavior remain unmeasured; source inspection is not runtime acceptance.

## 1. What you are building and why
G6 gives the node language a **polygon mesh type with face-level editing**: select faces, extrude, inset, bridge
loops, weld, dissolve, flip orientation, and a restricted bevel. Today every shape is a closed Manifold solid and
the operators work on whole solids, so there is no way to say "inset this face" or "extrude the top of that box".
G6 is the route to hand-editable hard-surface detail and, with G7 (subdivision) and G8 (fields), to organic forms
such as a rabbit. It also unblocks V5 (a polygon edit mode in the viewport, optional).

The plan's contract is: boundary, closed and non-manifold inputs follow a stated contract; attribute domains and
semantic selections are preserved where defined; bevel failures are typed and transactional (the document is
unchanged); connectivity, winding and Euler changes are validated; open meshes need not be solids.
**Never begin G6 with an unassigned operator** (plan text).

## 2. What exists (verified 2026-10-14)
- `crates/ve-geometry` (about 500 lines, G1a): `PolyMesh::new(positions, face_sizes, corners)`, accessors
  (`positions`, `face_sizes`, `corners`, `attributes`, `face_count`, `edge_count`, `domain_len`),
  `set_attribute`/`remove_attribute`; attribute `Domain` is Point, Edge, Face or Corner, with a `Propagation`
  policy; adapters `PolyMesh::from_kernel_mesh`, `to_kernel_mesh(LossPolicy)` (larger faces become **fans, only
  correct for planar convex faces**), `solid_to_polymesh`, `polymesh_to_solid`. Tests in `tests/g1a_kernel.rs`.
- **No adjacency, no editing operators, no graph integration.** There is no PolyMesh port type in
  `ve-graph/src/spec.rs`, no `Value` variant in `ve-modeling/src/cache.rs`, no operator, no selection model, and the
  renderer path (`Part`) has no polygon or open-mesh output. G6 must add all of these.
- Things G5 and the follow-ups give you for free: material regions and Manifold origin ids (a kernel run tells you
  which input a triangle came from, `ve-modeling/src/regions.rs`); `Part.region`; the staged evaluation, budget and
  cancellation protocol; `Labels`-style tagging as a model for semantic face tags.
- Decisions already made: D1 licences are not a selection criterion (OpenSubdiv's Tomorrow Open Source
  Technology Licence 1.0 is allowed; no GPL or AGPL, principles only; ADR 0005). D2 has an X3 amendment
  proposed in ADR 0019: OpenSubdiv CPU Far for G7 behind a validated adapter, G6 authored on PolyMesh, and
  PMP held as a reference/remeshing candidate pending ABI compatibility. Manifold smooth stays a separate
  triangle operation. Neither library is vendored (`third_party/` has bevy, clipper2, manifold,
  meshoptimizer, wgpu-hal); vendoring needs a `VENDOR.md` with upstream, tag, commit, licence and local changes,
  like `third_party/manifold/VENDOR.md`.
- Gate mechanics and rules that bite: 400 lines per `.rs` file, 80 lines per function, no `#[allow]`, pedantic
  clippy (similar_names, many_single_char_names, cast lints, `unwrap` in test helpers needs a file-level
  `#![expect(clippy::unwrap_used, reason = "...")]`), clang-format and clang-tidy on `cpp/` (unchecked `[]` on
  vectors is an error), `cargo xtask rules`, parity native versus WASI.

## 3. X3: probe method, outcome and remaining evidence
**Question.** Can OpenSubdiv (CPU) and PMP provide the G6 and G7 operators and rules inside our constraints
(panic-free Rust/WASM evaluator, cooperative cancellation, preflight budgets, deterministic output across native
and WASI), and which operators does neither provide?

**Protocol and result.** The isolated probe is `spikes/x3-polygon/`; pins, licenses, CMake configuration,
and exact rerun commands are in its README. The measured capability matrix, native timings, WASI build
results, limited binary-size comparison, API/source audit and all unmeasured items are in the X3 section
of [`experiments.md`](modeling/experiments.md#x3-opensubdiv-and-pmp-for-g6g7-2026-10-14--answered-with-a-split-choice).
OpenSubdiv is proposed for G7 Catmull-Clark behind a validated adapter. G6 editing operators are built
on `PolyMesh`; PMP is not evaluator-compatible with the current no-exception/no-RTTI toolchain.

The measurements are intentionally incomplete where the protocol asked for more than this probe could
establish: there is no full evaluator link delta, no large-mesh WASI timing, no runtime non-manifold
matrix, no crease/UV numerical suite, no small-mesh peak-memory profile, and no exhaustive reachable
panic-path audit. The measured standalone C++ WASI delta must not be reported as the evaluator delta.

**Deliverables.** (1) X3 report in `docs/modeling/experiments.md` (question, build, numbers, decision and
unmeasured items). (2) **ADR 0019: the polygon and subdivision backend.** (3) Operator plans with algorithms,
input contracts, proposed failure codes and cases in `docs/plans/g6-polygon-operators.md`. (4) The G6 case table
in `cases.md`. These research deliverables are complete; G6 implementation starts after owner review.

## 4. G6: build order after X3
Each step ends with tests, clippy, rules and, at the end, a strict gate. Add an ADR before changing any decision.

1. **Topology layer in `ve-geometry`** (no graph yet). Derived, rebuildable adjacency from `PolyMesh`: edge map
   with both faces, face-face, vertex-face, boundary loops, per-edge manifold classification, component count,
   Euler characteristic. Deterministic iteration (use `BTreeMap`; native and WASI must agree byte for byte).
   Validation functions the operators call before and after every edit: indices in range, face size at least 3,
   no repeated corner within a face, consistent winding across shared edges, and the Euler change expected from
   the operator. Cap sizes and make every loop cancellable through `ExecutionContext`.
2. **Graph integration.** A `PolyMesh` port type and `Value` variant; operators to convert (`poly.from_solid`,
   `poly.to_solid` with `LossPolicy` surfaced as a diagnostic, because fan triangulation of non-planar or
   concave faces is wrong); output of an open or non-solid polygon mesh as a renderable part with no volume
   (positions, triangulated indices, normals, `Part.region`-style per-material parts still apply); preflight
   triangle and memory estimates; operator registry entries, text form, MCP guide text, generated TypeScript
   (`bun run schema:generate`), parity corpus lines.
3. **Selections.** Faces are chosen by a semantic rule, not an index, so edits survive upstream changes:
   by normal direction or plane, by face-domain tag attribute, by origin (a face that came from input A of a
   boolean, using the kernel origin ids), or by a named set. Define what happens when a selection matches nothing
   (`selection.lost`, existing diagnostic) or several things (`selection.ambiguous`). Write the contract in an ADR
   or in the G6 section of `cases.md` before coding it.
4. **Operators, easiest first**, each with typed diagnostics and a case row: orientation flip; weld by distance;
   dissolve (edge, vertex, face) with refusal when it would create a non-manifold or collapse a face;
   face extrusion (offset, scale, along normal or direction); inset (uniform distance, refusal when it would
   self-overlap or flip a face); bridge between two boundary loops (equal length, or documented
   correspondence rule, orientation check); restricted bevel (edges with exactly two faces, bounded width and
   segments, typed refusal for the unsupported cases, transactional: failure leaves the document unchanged).
   Attribute propagation follows the `Propagation` policy per domain (corner data like UVs must not be
   blended across seams).
5. **Gallery and evidence.** The plan's G6 fixture is a **parameterised building facade** (D7): windows inset,
   frames extruded, a bevelled cornice. Add it to the gallery (`crates/ve-modeling/tests/gallery/`,
   `web/tests/gallery.mjs`) with six SwiftShader views, and a case showing a box with its top face extruded and
   inset into a tray. Include a real-model run if credit exists.
6. **Close.** A G6 report (`docs/modeling/g6-report.md`: implemented, measured, remaining), ADR acceptance
   request to the owner, `cargo xtask check --strict` once, results recorded honestly (SwiftShader only).

## 5. Draft G6 cases (refine in step 3 of section 3)
| ID | Case | Expected |
|---|---|---|
| G6-01 | Closed cube to `PolyMesh` and back | Same volume; no attribute loss reported when none; face and corner counts preserved |
| G6-02 | Face with more than 3 corners that is concave or non-planar converted to triangles | Typed diagnostic, not a silent fan |
| G6-03 | Extrude the top face of a box by 0.2 | Euler characteristic unchanged, volume equals analytic, winding consistent, side faces created |
| G6-04 | Inset a quad face by a distance larger than half its width | Typed refusal naming the face; document unchanged |
| G6-05 | Inset a face with a hole boundary or on an open boundary | Follows the stated boundary contract |
| G6-06 | Bridge two equal-length boundary loops with opposite orientation | Closed tube; wrong orientation refused or flipped by a documented rule |
| G6-07 | Bridge loops of different length | Typed refusal or documented correspondence; no silent repair |
| G6-08 | Weld by distance on a mesh with duplicate vertices | Vertex count drops; no face collapses to zero area; corner attributes preserved |
| G6-09 | Dissolve an edge between two coplanar faces | Face merged; non-manifold result refused |
| G6-10 | Flip orientation of selected faces | Volume sign and normals consistent; shared edges stay consistently wound |
| G6-11 | Bevel a manifold edge, then a non-manifold edge, then a width too large | First succeeds with Euler change as specified; the others fail typed and transactionally |
| G6-12 | Open mesh (no solid) through extrude and output | Renders as polygons; no volume claimed; no kernel error |
| G6-13 | Non-manifold input (edge with three faces) | Follows the contract: refusal with the edge named, or defined behaviour, per operator |
| G6-14 | Selection by origin: faces that came from the first operand of a union | Same faces after upstream parameter changes; `selection.lost` when the operand is gone |
| G6-15 | Corner (UV) attribute through extrude and inset | Seams preserved, never blended across a seam |
| G6-16 | 100,000-face mesh through each operator | Within the preview budget or refused before work starts; cancellation stops it |
| G6-17 | Save, reload, undo, redo, group, instance with polygon operators | Stable IDs and identical output, as for G5-11 |
| G6-18 | Native and WASI corpus | Byte-identical output lines for every operator |

## 6. Traps (new for this track, plus the standing ones)
- Boolean output (Manifold) contains slivers and occasional zero-area triangles; never assume a clean polygon
  mesh when converting a solid. Use thresholds relative to the mesh's f32 resolution, not absolute numbers.
- `to_kernel_mesh` fans larger faces; that is wrong for concave or non-planar faces. Triangulate properly
  (ear clipping or constrained triangulation) and test it, or refuse.
- A budget must count work actually done, not candidates in a bucket (G5 lesson: the pair budget refused valid
  tubes). Estimates that multiply per level compound; keep them bounded and test chains.
- Determinism across native and WASI: no `HashMap` iteration order, no parallel reductions, no float
  accumulation order differences. Use sorted maps and fixed orders.
- A kernel run can be empty; derive a run's end from the next run or the total.
- Anything that makes several parts per node must set `Part.region`; the runtime keys surfaces by (node, region).
- Vendored C++ is gated: clang-format, clang-tidy, ASan and UBSan native tests, licence file, VENDOR.md.
- Do not claim headset or physical-GPU results; everything so far is SwiftShader.

## 7. Decisions for the owner (do not decide for them)
1. Commit and ADR acceptance state of the G5 tree (0017, 0018) before G6 adds more to the same tree.
2. Review the proposed D2 amendment in ADR 0019: G6 operators are authored on `PolyMesh`, OpenSubdiv CPU Far
   is the proposed G7 backend, and PMP remains a reference/remeshing candidate until its WASI ABI conflict
   is resolved. The proposal is the default for implementation; maintainers may accept or revise it.
3. Whether G6 operators appear in the MCP guide before they have real-model evidence (default: only operators
   whose cases pass, as for G5).
4. Optional: V5 polygon edit mode in the viewport after G6, or the extensibility and export tracks first
   (see `handoff-next-after-g5.md`, section 3F).

## 8. Done means
X3: the section, ADR 0019, the operator plan and the G6 case table exist and the owner has seen them.
G6: every case in the table passes or is recorded as a stated limit; the facade gallery renders from six views;
`g6-report.md` exists with measured time and size; ADR(s) proposed for acceptance; `cargo xtask check --strict`
passed with the log named in the report; nothing committed by the agent.
