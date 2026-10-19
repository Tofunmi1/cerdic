# Handoff: what remains in G7 (subdivision surfaces)

Date: 2026-10-14. Read first: `handoff-g7-subdivision.md` (the full G7 brief and traps),
`docs/adr/0021-subdivision-adapter.md` (Proposed), `docs/modeling/g7-report.md` (what was measured),
`docs/adr/0019` and `0020`. This file only lists what is left and the order to do it in.

**Status note:** this was an earlier worklist. For completed review fixes and current validation, use
[`handoff-g7-fixes.md`](handoff-g7-fixes.md) and [`g7-report.md`](modeling/g7-report.md).

## 1. Where G7 stands

The initial graph vertical slice is implemented and reachable from text, MCP and the page. The remaining work
is chiefly attribute fidelity, stronger numerical evidence, and persistence coverage beyond save/reload.

| Layer | File | State |
|---|---|---|
| Vendored library | `third_party/opensubdiv/` (3.7.0 Far/Vtr/Sdc, commit 9dab8a47) | builds native, `wasm32-wasip1`, `wasm32-unknown-unknown` |
| C ABI + kernel | `cpp/src/subdivision.cpp`, `cpp/include/ve_kernel.h` (`ve_subdivide_catmark`, `ve_subdivision_*`) | uniform Catmull-Clark, boundary rules, edge creases, corner sharpness, cancellation polls, growth preflight, provenance |
| Safe wrapper | `crates/ve-kernel/src/subdivision.rs`, `ffi/subdivision.rs` | done |
| Adapter | `crates/ve-geometry/src/subdivide.rs` | new `PolyMesh`, face attributes/regions propagated, integer point attrs reported by adapter; unsupported interpolation/edge attributes typed refusal |
| Graph | `crates/ve-graph/src/builtin/poly.rs`, `crates/ve-modeling/src/poly_ops/subdivision.rs` | `poly.subdivide@1` with level/boundary and crease/corner selections; triangle budget preflight |
| Browser/MCP | `web/`, `tools/mcp/server.mjs` | text, gallery and MCP examples/tests cover a bevelled block |
| Tests | kernel, geometry, modeling and C++ suites | cube, growth, boundary, crease/corner, extraordinary valence 3/5, gallery, save/reload, parity and cancellation |
| Size | evaluator/runtime | measured after graph node; see `g7-report.md` |

Fresh `cargo xtask check --strict` run completed with 572 Rust test cases, printed `all gates passed`, and
returned `EXIT=0` from a Bash wrapper. The editor acceptance initially exposed a worker crash from reading
`warnings` before its declaration; the source was fixed, `web/dist` and `public/engine` were rebuilt, and editor
acceptance passed in the full run. See `g7-report.md` for details.

### Case status (from the G7 brief)

| Case | Status |
|---|---|
| G7-01 cube one level | covered (kernel test) |
| G7-02 levels 1-4 | four sequential levels checked for bounded, non-increasing extents; cage output is also asserted unchanged |
| G7-03 extraordinary vertices | valence 3/5 output matches an independent one-level Catmull-Clark reference; exact infinite-level limit position is a documented extension |
| G7-04 boundary | covered for edges and edges-plus-corners |
| G7-05 crease | Per-parent edge points match the one-level semi-sharp blend reference at 0/0.5/1/2/10; multilevel decay remains future coverage |
| G7-06 corner sharpness | Per-parent corner points match the one-level reference at 0/0.5/1/2/10; multilevel decay remains future coverage |
| G7-07 UV seam | documented unsupported limit; paired `u`/`v` corner attributes refuse with `poly.subdivide.uv_fvar_unsupported` |
| G7-08 attributes | face attrs propagate; dropped integer point attrs reach the visible Problems report; other unsupported domains have exact refusal codes; non-zero `hole` tags explicitly refuse with a regression test |
| G7-09 growth preflight | kernel and graph quality preflight implemented; two-million cap remains conservative pending cap-sized latency measurement |
| G7-10 cancellation | pre-cancel is tested and interpolation polls between levels; deterministic asynchronous between-level cancellation is untested, while `RefineUniform` itself remains uninterruptible |
| G7-11 cage kept, edits upstream | graph emits cage and refined outputs; the cage retains its original triangle count and evaluates unchanged |
| G7-12 curved quads render and convert | deterministic shorter-diagonal mode and fold refusal implemented; gallery conversion covered |
| G7-13 non-manifold, winding | non-manifold and reversed-face winding refusals assert the typed edge diagnostics |
| G7-14 native vs WASI | parity corpus and actual Node WASI subdivision test run; UV corpus is excluded because FVar is explicitly refused |
| G7-15 big cage and high level timings | ~100k level1 and small level4 measured on native/WASI; cap-sized uninterruptible timing is explicitly a limit and cap remains conservative |
| G7-16 save/reload/undo/group/instance | subdivided graph save/reload and undo/redo tested; grouped subdivided block instances survive reload and shared parameter history |

## 2. Decisions recorded in the implementation

ADR 0021 remains Proposed pending owner review. The implementation follows the recommendations below; these are
not maintainer acceptance of the ADR.

1. **Curved quads.** Triangulate four-sided subdivision faces along the shorter diagonal; larger non-planar faces
   keep the refusal.] This amends the G6-02 contract, so ADR 0021 needs an explicit amendment and `poly.to_solid`
   behaviour must be stated (does a curved result become a closed solid, and with what fold bound).
2. **Integer point attributes.** Drop them and report their names in `Subdivided::dropped_attributes`.
3. **Demo piece.** A subdivided, bevelled block (poly tools into subdivision); a low-poly head or rabbit only if the
   owner wants one, built procedurally. No imported models.] Separately: whether to spend provider credit on a real
   model run (the router key is exhausted; do not reuse the pasted key).

## 3. Work packages, in order

### R1. Curved-quad triangulation (implemented; regression coverage exists)
- `crates/ve-geometry/src/triangulate.rs` has `MAX_PLANAR_DEVIATION_RATIO = 1e-3`; faces beyond it fail as
  `non-planar polygon`. The per-mesh mode splits a quad along its shorter diagonal, with deterministic tie-break.
  Keep this policy and the refusal for n-gons above 4.
- Re-check consumers: the kernel conversion in `ve-modeling/src/poly_ops/convert.rs` (`poly.to_solid`), the
  `poly.self_intersection` check, the normals/smoothing path in the page (`SMOOTH_TRIANGLE_LIMIT`), and the
  watertightness proof. A folded quad (diagonal choice makes a fold past a stated bound) must still refuse.
- Tests: G7-12, plus a property test that both diagonals give a closed manifold for a cube-derived quad.

### R2. Attributes, UVs and holes (unsupported paths now explicit and tested)
- Current contract: face attributes copy to children; integer point attributes are omitted with a surfaced typed
  warning listing each name; float point attributes refuse until OpenSubdiv primvar interpolation is wired;
  edge/corner attributes refuse with domain-specific codes.
- UVs: paired scalar corner attributes (`u`, `v`) refuse as `poly.subdivide.uv_fvar_unsupported`. FVar seam and
  continuity interpolation is a future feature; do not flatten or discard it silently. Record and test the
  selected OpenSubdiv FVar boundary rule when implementing it.
- Holes: a non-zero U32 face attribute named `hole` is recognized and refused as
  `poly.subdivide.holes_unsupported`. The adapter has no first-class hole topology or child-face filtering;
  open boundary cages are a separate supported case.
- Face attributes are copied to children. Integer point drops produce `poly.subdivide.attribute_dropped` evaluation
  warnings, carried through the worker and displayed/selectable in the Problems panel. Float point, edge, corner,
  and paired UV attributes refuse with distinct stable codes. Tagged hole input is explicitly refused and tested;
  hole propagation cannot be claimed.
- Tests assert the refusal codes and the browser panel displays/selects a typed report. Implement FVar, point
  primvars, or hole tags only with native/WASI parity and seam/continuity tests; do not silently degrade them.

### R3. The graph operator `poly.subdivide@1` (implemented; close remaining coverage gaps)
- Follow the G6 pattern: spec in `ve-graph/src/builtin/poly.rs` (or a new `builtin/subdivide.rs` to stay under the
  400-line cap), evaluator in `ve-modeling/src/poly_ops/` (new file), registration, help text, generated schema
  (`bun run schema:generate`), `Value` plumbing as for the other poly operators.
- Parameters: `levels` 1-4 (integer, default 1), `boundary` ("edges" or "edges_and_corners"), optional `creases`
  input as a `Selection` of edges with `sharpness`, optional corner selection (decision from the G7 brief:
  creases via optional selection inputs, not edge attributes). Output a `Poly`; it must feed `poly.to_solid`,
  `poly.*` tools and be a valid graph output.
- Preflight in `plan.rs`/estimate: exact growth (faces times 4^n; use the recurrence the kernel already has), refuse
  with limit and observed counts when over the quality budget (Preview 200,000 / Final 2,000,000 triangles after
  triangulation, not faces; remember quads become two triangles).
- Errors: stable diagnostic codes in `docs/modeling/diagnostics.md` (selection stale/lost, attribute drop warning,
  budget, curved-fold, non-manifold, winding). Cancellation must reach the kernel through the existing
  `ExecutionContext`.
- Tests: G7-09, G7-11, G7-13 and G7-16 (`g7_graph.rs`, `g7_persistence.rs`); the graph emits the source cage
  alongside the independently refined mesh.

### R4. Colour regions through subdivision (implemented)
- Face-region labels are copied through face provenance, then converted to triangle runs for region materials.
  The two-colour union regression is in `crates/ve-modeling/tests/material_regions.rs`.

### R5. Parity and robustness evidence (implemented with explicit measurement limits)
- Modeling parity and actual Node WASI test execution are covered. Add a UV seam corpus case after FVar attributes
  exist.
- Release timings cover a ~100k-face cage at level 1 and a small level-4 cage. The cap-sized uninterruptible
  `RefineUniform` call remains explicitly unmeasured; retain conservative limits until a dedicated run (G7-10, G7-15).
- Four-level bounds and independent extraordinary-vertex reference checks now pass. Crease sweeps and winding
  refusal also have tests. The maximum-case uninterruptible duration remains unmeasured; retain the conservative
  two-million caps until that benchmark is run.

### R6. Page, MCP and gallery (implemented; preserve these checks)
- Gallery piece (decision 3) in `crates/ve-modeling/tests/gallery/*.txt`, listed in `gallery.rs` and
  `web/tests/gallery.mjs`; make sure the six-view screenshots are visible and smooth (flat shading versus
  corner-normal smoothing at the crease angle: a smooth subdivided surface should read as smooth).
- MCP guide and browser test include a polygon-plus-subdivision example; keep the guide concise.
- Modeling parity corpus has four subdivision cases; add UV coverage when that feature is implemented.

### R7. Reports, ADRs and docs (updated; maintain as gaps close)
- The report, cases, diagnostics, limits and plan reflect current implementation. Refresh strict-gate counts and
  record remaining limits after substantial changes.
- ADR 0021 records implementation policies and stays Proposed pending owner review. UV mode remains open until
  face-varying support lands. Do not change ADR 0019/0021 statuses.
- Keep the OpenSubdiv license and vendor record current. Run `cargo deny` as part of the strict gate.

## 4. Traps specific to what remains

- **Curved quads** use the mesh triangulation mode; retain conversion and fold-refusal regression tests so later
  edits do not switch back to the planar-only policy.
- File caps: `.rs` 400 lines, functions 80 lines, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy.
  `ve-geometry/src/subdivide.rs` is at 94 lines and `subdivision.rs` at 160; new attribute code goes in a new
  module, not appended. C++ (`subdivision.cpp` is 521 lines) is gated by clang-format/clang-tidy/ASan/UBSan, not
  the Rust caps, but split it if it keeps growing.
- clang-tidy: the subdivision translation unit excludes two analyzer checks for false positives inside OpenSubdiv
  headers. Do not widen that exclusion to project code.
- Cancellation: topology refinement is one uninterruptible library call. Preflight exact growth so it cannot run
  long on a refusable input; measure the worst allowed case.
- Determinism: output vertex and face order must be identical native versus WASI. OpenSubdiv's iteration order is
  deterministic, but any hash map or threading in the adapter is not allowed; use sorted structures (BTreeMap).
- Provenance is the only link back to the cage. Keep it through save, reload, undo and instancing (G7-16).
- Work in the owner's dirty tree: roughly 163 paths are uncommitted. Do not commit, reset or stash. Use only
  Write/Edit for file changes (formatters and `bun run schema:generate` are fine). No co-author or tool-attribution
  lines in any commit message or PR text.
- A concurrent clippy failure exists in `crates/ve-kernel/tests/subdivision.rs` line ~226 (`float_cmp` on an
  array compare, found 2026-10-14) if the gate was run before it was fixed; the strict gate in the G7 report
  passed afterwards. Re-run the gate to be sure.

## 5. Done means

`poly.subdivide@1` exists, is reachable from text, MCP and the page, saves/reloads, and renders smooth;
cases G7-01 to G7-16 pass or are recorded as limits with reasons; native and WASI outputs are identical for the
new corpus lines and the WASI subdivision test has actually run; budgets rest on measured release numbers or
clearly documented conservative caps pending a maximum-case timing;
`g7-report.md`, ADR 0021, cases, diagnostics, limits and the plan are updated; `cargo xtask check --strict` passes
from a fresh log with the real `exit=` marker; the owner's three decisions are recorded as taken or still open.
Report honestly: SwiftShader-only measurements, no headset, no real-model run unless credit is provided.
