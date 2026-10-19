# Handoff: completing G8 (fields)

Date: 2026-10-14. Read first: `handoff-common.md` (rules; use `cargo xtask quick` while working and the cached
`cargo xtask check --strict` once at the end, from a fresh log under `engine/target/reports/`), `handoff-g8-fields.md` (the full G8
brief, decisions 1-8, cases G8-01..G8-20), `docs/adr/0022-bounded-spatial-fields.md` (Proposed), `docs/modeling/g8-report.md`.
This file lists only what is left, in the order to do it, with the facts a review of the first slice found.

## 1. What exists (verified 2026-10-14)

| Layer | Where | State |
|---|---|---|
| Expression and tape | `ve-geometry/src/field/{model.rs,expr.rs,tape/{lower.rs,eval.rs}}`, `field.rs` | `Field` values, lowering to a topological tape, Rust reference evaluator, guarantee propagation; plane and ellipsoid are graph-backed |
| Kernel | `cpp/src/field_mesh.cpp`, `cpp/src/field_tape.cpp`, `cpp/src/field_tape.hpp`, `cpp/tests/field_test.cpp`, `ve-kernel/src/field.rs`, `ffi/field.rs`, `examples/field_bench.rs` | Split C ABI mesher and tape interpreter; typed refusals (invalid parameter, clipping via six-face lattice sample, empty, cancel, cell/tape/ops/padded-voxel limits) |
| Graph | `ve-graph/src/builtin/fields.rs`; `ve-modeling/src/field_ops.rs` | `PortType::Field`; 19 operators registered, including bounded polyline tube and repeat; a field cannot reach a solid socket |
| Tests | `ve-geometry/tests/g8_fields.rs` (8), `ve-modeling/tests/g8_graph.rs` (6), `ve-modeling/tests/g8_persistence.rs`, `cpp/tests/field_test.cpp` | sphere convergence, torus volume and genus, every opcode Rust versus C++ bit parity, ellipsoid guarantee, tube and repeat bounds/parity, native in-sampling cancellation; graph vocabulary, warning behavior, boolean conversion, clipping, preflight caps and Preview scaling; field group/instance save, reload, undo and redo |
| Docs | `g8-report.md`, ADR 0022, `cases.md` (G8-01, 02, 03, 05, 11, 12, 15), `limits.md`, `diagnostics.md` (`field.invalid_parameter`, `field.clipped`, `field.empty`, `budget.field_ops`) | partial |

Enforced limits (ADR 0022): 2,000,000 grid samples, 64,000,000 sample-instruction operations, 4,096 instructions, 2,097,152 padded
voxel slots. P0 numbers: standalone sphere 0.267 s native / 0.430 s WASI at 1M samples, 1.938 / 3.031 s at 8M; with a 64-instruction
tape 1.120 / 1.291 s at 1M and 9.042 / 10.133 s at 8M; WASI linear memory about 89 MiB at 1M, 386 MiB at 8M; production
Rust-to-kernel 1M x 64 ops took 1.506 s, 91,728 triangles. Native and WASI `sin`, `cos`, `exp`, `pow`, `atan2`, `tanh` **differ**
over a million inputs; only `sqrt` matched; none of those are in the tape.

Latest checkpoint: `cargo xtask quick ve-geometry ve-graph ve-kernel ve-modeling` passed (456 tests,
Clippy, formatting and rules); `cargo xtask parity` passed (kernel 20 lines, modeling 37 lines).
`cargo xtask cpp` passed native ASan/UBSan and WASI clang-tidy after bounded repeat. The native field
cancellation case measured 0.7–3.3 ms across runs and asserts under 500 ms. The filtered browser
gallery passed all three shards, including both new pieces and the vase lifecycle sequence. The
The current code tree's full strict gate passed all fresh stages (602 Rust tests and all browser scripts: 32 pool jobs plus 3 serialized timing checks, SwiftShader only; log
`target/reports/g9-fidelity-pass1.log`, `exit=0`). Browser release size and headset behavior are not tested in this checkpoint.

Release production-adapter measurements are now recorded in ADR 0022 and `g8-report.md`: three native and Node WASI runs for the sphere
and creature fixtures, including median time, range, WASI linear memory and output triangles. These are local microbenchmarks, not browser or
headset measurements.

## 2. Findings from the review of the first slice (fix or decide first)

1. **Resolved for the current creature fixture.** The body field has 32 tape instructions and bounds 70 × 74 × 58 at Final, or 35 × 37 × 29 at Preview. The sample-instruction work is about 9.6M Final and 1.2M Preview, below the 64M cap. Wall-time remains unmeasured on the production WASI adapter.
2. **Graph preflight is implemented; triangle estimate remains heuristic.** The planner now checks field samples, tape, sample-instruction
   work and padded slots before evaluation. It estimates triangles as primitive count × 10 × the square of the longest grid axis. Measure
   that estimate against gallery fixtures and ensure it does not block valid cases; the exact post-mesh triangle budget remains authoritative.
3. **Preview scaling is implemented.** Preview doubles field edge length (about 8× fewer samples on a cubic grid); tests cover a request
   accepted at Preview but refused at Final and sphere volume within 0.08 m³ of analytic volume. Measure editing latency in the browser.
4. **Resolved for the current vocabulary.** The report lists all 19 registered field operators; plane, ellipsoid, tube and repeat are graph-backed,
   and F5 is within the current caps. Keep the inventory synchronized as operators are added.
5. **Resolved for parity and baseline performance evidence.** Seven field cases are in `crates/ve-modeling/examples/corpus.rs`, and
   `cargo xtask parity` reports identical native/WASI output (37 total lines). Production-adapter native/WASI timing and WASI memory for
   the sphere and creature are recorded in ADR 0022 and `g8-report.md`; accepted-cap timing remains open.
6. **Size and the full gate are unmeasured**: release wasm delta, browser suites and strict gate have not run with fields.
7. Expression data types and tape lowering/reference evaluation are split into `field/model.rs`, `field/expr.rs`, and
   `field/tape/{lower.rs,eval.rs}`. The C++ mesher and interpreter are split into `field_mesh.cpp` and `field_tape.cpp`.

## 3. Work packages, in order

### R1. Reconcile code, report and plan (small, do first)
- **Current pass:** registered `field.plane` and `field.ellipsoid` graph nodes. Unequal ellipsoids use `min(radii) * (1 - length(p / radii))`, a conservative distance lower bound; equal radii are exact. Updated F5 to a 96-cubed Final / 48-cubed Preview target and reconciled the report inventory. Split expression data types and Rust tape lowering/reference evaluation into modules under `field/`.
- **Completed:** separated the C++ interpreter into `field_tape.cpp` with declarations in `field_tape.hpp`; meshing and ABI remain in `field_mesh.cpp`.
- Keep the operator inventory synchronized across `g8-report.md`, `cases.md`, ADR 0022 and `docs/plans/track2-modeling.md`.
- Verify the creature's real tape size and work against the revised F5 target when its fixture is authored.

### R2. Planning, Preview scaling and budgets (findings 2, 3)
- **Implemented:** planning preflight computes predicted samples, instruction count, sample-instruction work and padded voxel slots for each
  `field.to_solid`; typed budget failures occur before geometry evaluation. The triangle estimate is primitive count × 10 × longest grid
  axis squared, and actual post-mesh triangles remain the authoritative budget check. Preview doubles edge length. Tests cover caps and
  Preview volume tolerance.
- **Still open:** measure the triangle heuristic on gallery fixtures and show it does not block valid models. Add a measured Final-time
  warning model (milliseconds per million sample-instructions) to ADR 0022; warn rather than reject.

### R3. Speed (measure first; do only what the numbers justify)
The release production adapter now has three-run native/WASI measurements for a 1,030,301-point sphere and the 32-instruction creature;
see `g8-report.md` and ADR 0022. Native medians were 344.7 ms and 286.9 ms; WASI medians were 355.0 ms and 349.1 ms respectively.
The creature used 22.1 MiB WASI linear memory. This is enough to establish the current baseline; the measurements do not include browser
worker overhead or cancellation.
- Candidates, in order of cost and risk: (a) per-primitive bounding tests in the tape (skip evaluating a child whose bound sphere is
  farther than the current best by more than the blend radius; keep results bit-identical or prove the difference is zero for the
  surface); (b) constant folding and common-subexpression elimination in lowering; (c) coarse-to-fine sampling (evaluate on a coarse grid,
  refine only near the surface) if `LevelSet` allows it or by using a narrow band of our own; (d) none, if Preview scaling already makes
  editing acceptable.
- Any optimisation must keep native and WASI byte-identical and must not change guarantees; add a test that the optimised tape and the
  reference give the same samples.

### R4. Vocabulary: bounded repeat, noise, displacement, twist, bend, tube (decision 3 of `handoff-g8-fields.md`)
- **No libm.** Implement every transcendental you need in our own code using only exact IEEE operations (polynomial or table, exact range
  reduction by `floor`), **in both** the C++ interpreter and the Rust reference evaluator, from one specification, and test that they
  agree bit for bit on a million inputs and that native and WASI agree. Do not call `f64::sin` or `std::sin` in either evaluator.
- **Implemented:** `field.repeat` (bounded count per axis, max 16; folded coordinates keep it a single tape instruction). Rust/C++ sample parity and native/WASI adapter parity are covered.
- `field.noise`: value or gradient noise from an integer hash (seed, frequency, `octaves` at most 4, lacunarity and gain as parameters)
  with polynomial smoothing; `field.displace(field, noise, amount)` adds scaled noise to the field and sets the guarantee `None`.
  Same seed gives identical samples; a different seed differs; amplitude is bounded by the stated envelope.
- `field.twist` and `field.bend` (angle per metre and axis; need the deterministic sin/cos; guarantee `None`).
- **Implemented:** `field.tube` accepts a capped ordered polyline and radius, lowers to exact segment-distance instructions and hard unions, and has native/C++ parity and an analytic capsule-volume test. Two-point tubes are Exact; multi-segment tubes use a conservative LowerBound.
- **Implemented:** `field.offset` and `field.shell` emit `field.distance_inexact` through `EvaluationWarning` for non-Exact inputs; exact sphere offset remains warning-free. The code is listed in `diagnostics.md` and covered in `g8_graph.rs` (case G8-10).

### R5. Regions (decision 7; deferred and documented)
- G8 does not implement per-triangle field regions. The current `LevelSet` path has no primitive-label output, so supporting regions needs a
  second label evaluation and C ABI propagation through the mesh adapter. ADR 0022 and case G8-17 record the limitation. Separate
  `field.to_solid` outputs are the supported workaround; do not claim named field regions.

### R6. Cancellation (case G8-14)
- **Implementation and native regression test verified.** The C++ sample callback checks cancellation every 4,096 points; once observed, it returns a constant and the adapter discards the result as `Cancelled`. `cargo xtask cpp` passes; the native 64-instruction cancellation case measured 0.7–3.3 ms across runs and asserts under 500 ms. Still measure cancel-to-return latency at accepted caps and for the creature, natively and under WASI.

### R7. Corpus, persistence and the browser path
- **Current pass:** seven production Rust-to-kernel field corpus cases cover sphere, torus, ellipsoid, smooth blend, creature-like capsule blend, polyline tube and bounded repeat. `cargo xtask parity` reports identical full native/WASI output (37 lines).
- **Still open:** deterministic noise. `g8_persistence.rs` covers field subgraph grouping/instancing, canonical reload, stable IDs, and undo/redo output. WASI release time/memory measurements are recorded separately; parity output itself remains equality-only.
- The minimum field corpus and production-adapter parity requirement are met. Extend the corpus when repeat/noise are implemented and record the added case's WASI time and memory.
- G8-16 core save/reload/group/instance/stable-ID/undo/redo evidence is in `g8_persistence.rs`; additional field-to-solid chaining through
  transforms, booleans, `poly.from_solid` and `solid.material` is not yet covered in one persistence case.
- Browser: repeat and creature fixtures pass through the real worker and all six fixed SwiftShader views; gallery shard times were
  140–159 s. The worker protocol completes without timeout. Preview edit latency for larger field graphs is not measured.

### R8. Creature and fixtures (case G8-18, D7)
- `field_repeat_tube` and `field_creature` fixtures are authored and pass native evaluation plus six fixed browser views. The creature uses smooth-blended ellipsoids, tube limbs and tail, and separately meshed eyes. At 0.026 m authored body spacing, the Preview render removes the earlier sawtooth limb faceting. Noise skin, named field regions (deferred in ADR 0022), emissive material and follow-up text edits are not covered. Silhouette review is recorded in `g8-report.md`. Procedural only; no imported geometry; no animation-ready claim.
- Add the pieces to the Showcase page list if the owner wants them there (the showcase scene lives in
  `ve-runtime/src/bench_scene/showcase.rs`).

### R9. MCP and inspector (finding of the first slice: not started)
- MCP guide block in `tools/mcp/server.mjs` (short; field operators, `field.to_solid` bounds and `edge_length`, the guarantee idea, the typed
  refusals) and a block in `web/tests/mcp.mjs`: a model builds a blob, a rock and a creature from text, edits by ID, and an oversized or clipped
  request is refused with its code. Count tokens added to the guide and keep it small.
- Inspector: show the distance guarantee for field nodes. The guarantee is computed in Rust; surface it from the evaluator to the page
  (an evaluation note per node, like warnings) rather than recomputing it in JavaScript, so the two cannot drift. Optional afterwards: a
  bounds box overlay and parameter handles for `field.to_solid` (see `parameterHandles` in `web/index.html`).

### R10. Evidence, docs and the gate
- Release wasm size delta (`cargo xtask web`, Brotli-9 for runtime and evaluator against the G7 figures: evaluator 1,954,650 raw / 550,502,
  runtime 34,513,972 / 4,120,575). Timings for the cases above, native and WASI, and the memory at the caps.
- Update `g8-report.md` (rewrite so it states exactly what exists, with test names), ADR 0022 (decisions taken and the measured basis;
  stays Proposed), `cases.md` (G8-01..G8-20 with status), `diagnostics.md`, `limits.md`, the plan status. Fix any wording that claims
  more than a test proves.
- `cargo xtask check --strict` from a fresh log with the real `exit=` line (stages are cached by content; a first run after these changes
  runs everything). Report honestly: SwiftShader only, no headset, no real-model run.

## 4. Traps

- Determinism: the tape compiler must not iterate a hash map or use threads; the vendored Manifold build has `MANIFOLD_PAR` off, keep it
  that way. Noise uses an integer hash and polynomial smoothing only.
- The Rust reference evaluator and the C++ interpreter are two implementations of one spec; every new instruction needs the bit-equality
  test between them before it is used by anything else.
- Manifold works in `double` and the kernel boundary is `f32`; convert once in the adapter and say where precision is lost.
- Clipping check is lattice-only; thin features can pass. Keep that stated wherever `field.clipped` is documented.
- Rust files under 400 lines, functions under 80, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy; C++ is gated by
  clang-format, clang-tidy, ASan, UBSan. New tests go in new files.
- Another agent may be editing the tree; if something unrelated breaks, re-run before assuming it is yours. No commits, no attribution
  lines, edit tools only (formatters and `bun run schema:generate` are fine). Do not reuse the router key pasted in chat earlier.

## 5. Done means

Every item in section 2 is fixed or recorded with a reason; the operator set is complete (plane, ellipsoid, tube, bounded repeat, noise,
displace, twist, bend) with our own deterministic math proven bit-equal native versus WASI; the plan refuses over-budget fields before
running, Preview scales resolution, and the post-mesh triangle budget is enforced; cancellation during sampling works and its latency is
recorded; regions work or are recorded as a limit; at least five field corpus lines are identical native versus WASI on the production
adapter; the creature gallery has a six-view silhouette review and a follow-up edit; MCP builds and edits fields and refuses bad
requests; the inspector shows each field's guarantee; sizes and timings are recorded; report, ADR 0022, cases, diagnostics, limits and plan
agree with the code; `cargo xtask quick` is clean while working and `cargo xtask check --strict` passes from a fresh log with the real
`exit=` line.
