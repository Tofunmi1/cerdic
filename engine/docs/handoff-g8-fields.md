# Handoff: G8 organic fields and meshing

Date: 2026-10-14. Read first: `handoff-common.md` (rules, gate speeds: use `cargo xtask quick` while working and the cached
`cargo xtask check --strict` at the end), `docs/plans/track2-modeling.md` (G8 text near line 418, contracts in 3.2, limits in
`docs/modeling/limits.md`), `docs/modeling/g6-report.md` and `g7-report.md` (the patterns to copy: graph operators, typed
refusals, budgets, parity, gallery, MCP). G8 follows G7; nothing in G8 needs any other open gate.

## 1. What G8 is and why it matters

Today every shape is a primitive, a boolean of primitives, a swept or lofted surface or an edited polygon mesh. None of those
make a creature, a rock, a cloud, terrain or any blobby, smooth-blended form. G8 adds **fields**: a bounded expression language
for signed-distance-like functions (primitives, smooth blends, transforms, repeat, deterministic noise), compiled to a tape that
is evaluated in C++ with no per-sample JavaScript, and meshed into an ordinary `Solid` by Manifold's `LevelSet`. After meshing,
the result is a normal solid: it booleans, transforms, instances, takes materials and goes to `poly.from_solid`.

Plan acceptance (do not weaken): sphere and torus analytic checks; blends checked by convergence or reference samples; clipped and
unbounded domains and oversized grids rejected; boundary behaviour checked; a creature fixture with a multi-view silhouette review
and named regions, **without claiming animation-ready topology**. Expose the distance guarantee: blends, deformations and noise may
lose the exact-distance property, and a scalar field is not a distance guarantee.

## 2. State you start from (verified 2026-10-14)

- Manifold is vendored with `Manifold::LevelSet(std::function<double(vec3)> sdf, Box bounds, double edgeLength, double level = 0,
  double tolerance = -1, bool canParallel = true)` (`third_party/manifold/include/manifold/manifold.h:105`, implementation
  `src/sdf.cpp`) and an `ExecutionContext::LevelSet` that polls cancellation and reports progress between the voxel-sampling and
  mesh-extraction phases (`common.h:288`, `src/execution_impl.cpp:131`). Convention: positive inside, `level` 0 is the surface.
- Nothing in the engine calls it yet: no `LevelSet` entry in `cpp/include/ve_kernel.h` or `cpp/src/kernel.cpp`.
- Limits already reserved in `docs/modeling/limits.md`: field grid cells 8,000,000 (32 MiB of f32), field tape instructions 4,096,
  triangles per result 2,000,000 final / 200,000 preview, kernel extent `MAX_EXTENT` 1e6 m.
- Patterns to copy: C ABI plus safe wrapper (`ve-kernel`), geometry-level code (`ve-geometry`), graph operators registered in
  `ve-graph/src/builtin/` with `PortType` values (the G6 work added `Poly` and `Selection`), evaluator in `ve-modeling/src/`, plan
  and estimate in `ve-modeling/src/plan/`, budgets and `Quality` (Preview 200,000 / Final 2,000,000 triangles), warnings channel
  (`EvaluationWarning`, `poly.subdivide.attribute_dropped`), colour regions (ADR 0018, `ve_solid_from_mesh_runs`), gallery fixtures
  (`crates/ve-modeling/tests/gallery/*.txt` plus `gallery.rs`/`web/tests/gallery.mjs`), MCP guide (`tools/mcp/server.mjs`).
- Compiler flags for the C++ already include `-ffp-contract=off` and `-fexcess-precision=standard` (Manifold's own flags); keep that.
- Render and size context: release wasm is 34.5 MB raw / 4.12 MB Brotli runtime and 1.95 MB / 0.55 MB evaluator; a field module should
  cost far less than OpenSubdiv did (record the real delta).

## 3. Decisions to take early (write ADR 0022; owner decides, recommendations in brackets)

1. **Socket type.** [A new `PortType::Field` (a pure expression value, `Field3<f64>`), distinct from per-element attribute fields and
   from solids, as the plan says: "a spatial SDF and a per-element field are different socket kinds".] Only `field.to_solid` produces
   geometry; fields compose with each other and nothing else.
2. **Tape.** [A flat register-machine or stack tape of at most 4,096 instructions compiled in Rust from the graph, executed by a
   small C++ interpreter inside the kernel with no allocation per sample; constants in a side table; every instruction is IEEE
   `+ - * / sqrt min max abs floor` or a small set of our own deterministic functions.] No JavaScript crossing per sample.
3. **Deterministic math (critical for native versus WASI parity).** `sin`, `cos`, `exp`, `pow`, `atan2`, `tanh` from `libm` can differ in
   the last bits between native (Apple libm) and `wasm32-wasip1` (wasi-libc), which would make meshes differ and fail the parity
   gate. [Implement any needed transcendental in our own C++ with polynomial or table code using only exact IEEE operations, and keep
   noise to an integer hash plus polynomial smoothing (`smoothstep`-style), which is exact. Measure native versus WASI bit equality
   for each function you keep; if you must use a libm function, prove it first (P0).]
4. **Distance guarantee as a typed property.** [Every field carries `Exact`, `LowerBound` (Lipschitz at most 1, safe for offset/shell
   within a stated margin) or `None`. Primitives are exact; `min`/`max` combine to at most `LowerBound`; smooth blends, noise displacement,
   twist and bend are `None`; transforms keep or scale the guarantee. Show it in the inspector hover and in diagnostics. `field.offset`
   and `field.shell` on a non-exact field work but emit a warning `field.distance_inexact` through the warnings channel.]
5. **Bounds and resolution.** [`field.to_solid` takes an explicit `min`/`max` box and an `edge_length` (or `resolution`); there is no
   default box. Cells = product of axis cell counts, limited to 8,000,000. Preview uses a coarser edge length than Final (stated
   factor, for example 2), so cell counts and triangle budgets differ by tier like the other operators.]
6. **Clipped domains.** [Refuse when the surface touches the bounds: sample the six faces of the grid (an O(n squared) pass, cheap)
   and refuse with `field.clipped` and the first offending point if any boundary sample is inside; document that features thinner than
   the grid can slip between samples.] Also refuse an unbounded or empty result with typed codes (`field.empty`, `field.unbounded`).
7. **Colour regions.** The creature needs named regions (head, body, legs). [Each field primitive may carry a region label; a second tape
   returns the index of the nearest or dominant primitive; after meshing, each triangle gets the label sampled at its centroid, and the
   result goes through `ve_solid_from_mesh_runs` so ADR 0018 colour regions apply. State that region boundaries are as coarse as the
   grid.] If this proves large, ship G8 without it, record the limit, and name regions by splitting into separate solids as a fallback.
8. **Preview versus final and caching.** Field evaluation is expensive; check that the evaluator's cache keys include the tape and the
   bounds and resolution so unchanged fields are not re-meshed.

## 4. Work packages, in order

### P0. A measurement spike first (one day, results go in chat and in the ADR)
Because the whole design hangs on cost and determinism, measure before building the language:
- Wire a throwaway C++ call to `ExecutionContext::LevelSet` with a hard-coded sphere and a hard-coded 64-instruction tape; record,
  native release and WASI, for 1M and 8M cells: time, peak memory, triangle counts, and whether the result is manifold.
- Per-sample cost of a 64 and a 4,096-instruction tape, to set an instruction-times-cells budget (an "ops" cap) next to the 8M cell
  cap. A 4,096-instruction tape over 8M cells is about 3e10 operations and cannot be allowed; the budget must be measured.
- Native versus WASI bit equality of `sin`, `cos`, `exp`, `pow`, `atan2`, `tanh`, `sqrt` over a million inputs.
- What `LevelSet` does when the shape crosses the bounds (open boundary, non-manifold, or clipped closed?) and how it uses `tolerance`.
- Whether cancellation latency is acceptable (the context polls between phases; how long is the longest phase at the caps?).
Deliver the numbers; only then fix the caps and the tape design.

### P1. Tape, evaluator and reference (Rust, then C++)
- `ve-geometry/src/field/` (new module split into files under 400 lines): tape types, expression tree, compiler (tree to tape,
  constant folding, instruction and register limits, guarantee propagation), and a **pure Rust reference evaluator** used by tests
  and to check the C++ one.
- C++ interpreter in new `cpp/src/field.cpp`, with the C ABI in `cpp/include/ve_kernel.h` (a tape struct passed by pointer and
  length, bounds, edge length, `ve_context`, status), calling `ExecutionContext::LevelSet`. Keep RAII, no exceptions across the ABI,
  `// SAFETY:` comments on the Rust FFI, clang-format/clang-tidy/ASan/UBSan clean.
- Safe wrapper in `ve-kernel` (`Solid::from_field` or `field_to_solid`), typed errors for cancel, budget and non-manifold results.
- Equality test: Rust reference and C++ interpreter give identical samples (bit for bit) on a grid for every instruction.

### P2. Field vocabulary (graph)
Register the operators in `ve-graph/src/builtin/` (a new `fields.rs` to stay under the cap), with `PortType::Field`, parameter ranges,
defaults, help text, and the regenerated schema (`bun run schema:generate`). Suggested set (finish the core, cut nothing the plan names):
- Primitives: `field.sphere`, `field.box` (rounded), `field.torus`, `field.capsule`, `field.cylinder`, `field.plane`, `field.ellipsoid`
  (bound-only, mark the guarantee), `field.segment` or `field.tube` (distance to a polyline or Bezier, bounded sample count).
- Combine: `field.union`, `field.intersect`, `field.subtract`, and the smooth variants with a `blend` radius (guarantee `None`);
  `field.blend_k` style choice documented in the ADR (polynomial smooth min is exact IEEE and deterministic).
- Transforms: `field.move`, `field.rotate`, `field.scale` (uniform keeps the guarantee, non-uniform degrades it), `field.mirror`,
  `field.repeat` (bounded count only, statically bounded expansion as the plan requires), `field.offset`, `field.shell`.
- Deformation and noise: `field.twist`, `field.bend` (need deterministic sin/cos, decision 3), `field.noise` (value or gradient noise from
  an integer hash, seed and frequency, `octaves` at most a stated bound) used as `field.displace(field, noise, amount)`.
- Output: `field.to_solid(field, min, max, edge_length)` returns a `Solid`; add material and regions through the existing
  `solid.material`.
Every operator declares version, units, defaults, ranges, the guarantee it keeps, determinism and its cost in tape instructions.

### P3. Evaluator, plan and budgets
- `ve-modeling/src/field_ops/` (new): compile the field subgraph feeding a `field.to_solid` into one tape, run it through the kernel,
  convert to a `Value` with the usual `Part` metadata, honour `Quality` (coarser edge length for Preview), and cache by tape hash.
- Plan and estimate (`plan/estimate.rs`): predicted cells and predicted operations before running; refuse with limit and observed counts
  (`budget.field_cells`, `budget.field_tape`, `budget.field_ops`); a triangle-count estimate from cells (state the bound you chose and
  how conservative it is, measured in P0) and a hard refusal after meshing if the real count passes the geometry budget.
- Cancellation: the kernel polls between phases; also poll inside the tape loop on a coarse stride (not per sample).
- Diagnostics: stable codes in `docs/modeling/diagnostics.md` for every refusal and warning, each with node, observed value, limit.

### P4. Tests (names suggested; write them first for the numeric ones)
- Analytic: sphere radius r meshes to volume within a stated error of `4/3 pi r^3` and area within a stated error of `4 pi r^2`, with the
  error shrinking as the edge length halves (check the rate, not just one point); torus likewise; bounding boxes.
- Convergence and reference samples for each blend; smooth union of two spheres matches the reference distance samples; the guarantee
  property is correct for each operator (property test with a random point grid against the exact distance where it is exact).
- Determinism: same tape gives byte-identical meshes in two runs; **native versus WASI identical** on new corpus lines (add 5 or more to
  `crates/ve-modeling/examples/corpus.rs`: sphere, torus, smooth blend, noise with a seed, a repeat); different noise seeds differ.
- Refusals: oversized grid (cell cap), too long a tape, clipped domain (`field.clipped`), empty result, unbounded input, repeat count
  beyond bound, non-finite parameters, cancellation (before and during), budget after meshing.
- Graph: save, reload, undo, redo, group and instance a field subgraph; a `field.to_solid` result feeds `solid.union`, `poly.from_solid`,
  `solid.material`; stale selections are not involved here.
- Rust reference versus C++ interpreter equality for every instruction (P1).

### P5. Fixtures and the creature
- Gallery: a field primitives sheet, a smooth-blend study, a noise rock, and **the creature** (D7): body from smooth-blended ellipsoids,
  head, four limbs as capsules or tubes, eyes as small emissive spheres (a separate part so they can glow), skin detail from low-amplitude
  noise, and named regions for colour. All procedural, no imported geometry. Six fixed views (`web/tests/gallery.mjs`) and a silhouette
  review recorded in the report; claim no rig or animation-ready topology.
- Show a follow-up edit on the creature through the text path (change one limb length and one blend radius) and that colours and regions
  survive it, per the plan's follow-up edit rule.

### P6. Page, MCP, docs
- MCP guide block in `tools/mcp/server.mjs` and a block in `web/tests/mcp.mjs` (a model builds a blob, a rock and a creature from
  text, edits it by ID, and a clipped or oversized request is refused with the typed code). Keep the guide short (token cost matters).
- Inspector: show the guarantee for field nodes; optionally a bounds box overlay and parameter handles for `field.to_solid` bounds
  (see `parameterHandles` in `web/index.html`). Optional, only after the core passes.
- Reports and docs: `docs/modeling/g8-report.md` (what was built, each case with its test name, numbers from P0, sizes, timings,
  limits), ADR 0022 (decisions above, Proposed), `cases.md` (G8-01 onwards), `diagnostics.md`, `limits.md` (final caps with the measured
  basis), `docs/plans/track2-modeling.md` status, and update the plan table where it differs from what shipped.

## 5. Cases to cover (refine with the owner; keep IDs stable)

| ID | Case | Pass condition |
|---|---|---|
| G8-01 | Sphere analytic | volume and area within stated error, error halves with edge length |
| G8-02 | Torus analytic | same, plus genus 1 |
| G8-03 | Box and capsule | exact extents; closed manifold solid |
| G8-04 | Union/intersect/subtract | match the corresponding Manifold boolean within tolerance |
| G8-05 | Smooth blend | reference distance samples match; guarantee `None` reported |
| G8-06 | Transforms and mirror | extents and symmetry; scale degrades the guarantee as declared |
| G8-07 | Bounded repeat | count and spacing; refuses beyond the bound |
| G8-08 | Noise | same seed identical, other seed different; amplitude within the stated bound; native equals WASI |
| G8-09 | Twist/bend | deterministic sin/cos; native equals WASI; guarantee `None` |
| G8-10 | Offset/shell | exact on an exact field; warning `field.distance_inexact` on an inexact one |
| G8-11 | Clipped domain | refuses `field.clipped` naming a point |
| G8-12 | Oversized grid | refuses before sampling with limit and observed cells |
| G8-13 | Long tape | refuses beyond 4,096 instructions with the count |
| G8-14 | Cancellation | pre-cancelled returns `eval.cancelled`; cancel during meshing stops; latency recorded |
| G8-15 | Result usable | booleans, transforms, instancing, `poly.from_solid`, materials accept the result |
| G8-16 | Save/reload/undo/group/instance | stable IDs and identical output |
| G8-17 | Regions | named regions colour the result; survive a follow-up edit |
| G8-18 | Creature | closed solid, six-view silhouette review, follow-up edit succeeds |
| G8-19 | Budget after meshing | a field that passes the pre-estimate but exceeds the triangle budget is refused with the real count |
| G8-20 | Native versus WASI | at least five corpus lines identical byte for byte |

## 6. Traps

- **Do not use libm transcendentals without proving native/WASI equality** (decision 3); the parity gate will catch it late.
- Manifold works in `double`, the kernel boundary is `f32`: convert once in the adapter and say where precision is lost. The tape uses
  `f64`; reject non-finite parameters and clamp nothing silently.
- A field is not a solid: do not let a `Field` reach a solid-only socket; the type check in `ve-graph` must refuse it with a clear code.
- Meshing cost is cells times tape length; both caps are needed, and the tier difference (Preview coarser) must be tested at the caps.
- `LevelSet` output triangle counts grow with surface area over edge length squared; the 2,000,000-triangle Final budget is easy to hit.
- Keep Rust files under 400 lines, functions under 80, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy. Several of the
  test files from G7 are at the cap; put new tests in new files. C++ is gated by clang-format, clang-tidy, ASan and UBSan.
- Determinism: no hash maps with random order or threads in the tape compiler; use sorted structures. Noise uses an integer hash only.
- Colour regions through booleans and moves already work; through `LevelSet` they do not (it has no per-triangle labels), hence decision 7.
- Another agent may be editing the tree; if something unrelated breaks, re-run before assuming it is yours. No commits, no attribution
  lines, edit tools only (formatters and `bun run schema:generate` are fine). Do not reuse the router key pasted in chat earlier.

## 7. Done means

`field.*` operators and `field.to_solid` exist and are reachable from text, MCP and the page; sphere and torus pass the analytic checks
with a measured convergence rate; blends are checked by reference samples; clipped, unbounded, oversized, over-long and over-budget cases
refuse with typed codes; native and WASI agree byte for byte on at least five corpus lines; cancellation works and its latency is recorded;
the creature fixture has a six-view silhouette review, named regions and a follow-up edit that survives; every field node reports its
distance guarantee; caps and the ops budget rest on P0 measurements; the report, ADR 0022, cases, diagnostics and limits agree with the
code; release wasm sizes and timings are recorded; `cargo xtask quick` is clean while working and `cargo xtask check --strict` passes from
a fresh log with the real `exit=` line. Report honestly: SwiftShader only, no headset, no real-model run.
