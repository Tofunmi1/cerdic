# Handoff: the finish line for G8

Date: 2026-10-14. Read first: `handoff-common.md` (rules; `cargo xtask quick` while working, the cached
`cargo xtask check --strict` once at the end from a fresh log under `engine/target/reports/`), `handoff-g8-fields.md` (brief, decisions,
cases G8-01..G8-20), `handoff-g8-remaining.md` (the full package list R1..R10, kept by the previous agent),
`docs/adr/0022-bounded-spatial-fields.md`, `docs/modeling/g8-report.md`. This file does not replace them; it records where G8 stands after
the latest pass, what a review and a render test found, and the shortest honest path to done. If this file and the report disagree, the
code and a fresh run decide, and the report gets fixed.

## 1. Status after the latest pass (verified 2026-10-14)

Done and checked before this finish pass (456 tests passed in `cargo xtask quick ve-geometry ve-graph ve-kernel ve-modeling`;
`cargo xtask parity` matched 37 modeling corpus lines native versus WASI including seven field cases; the gallery three-shard
browser run passed):
- Registered field operators: sphere, box, torus, capsule, cylinder, ellipsoid, plane, tube, union, smooth_union, intersect, subtract, move,
  scale (uniform), mirror, repeat (bounded), offset, shell, and `field.to_solid`. The expression and tape are split into `model.rs`, `expr.rs`
  and `tape/{lower,eval}.rs`.
- Cancellation during sampling: the C++ callback checks every 4,096 samples, returns a constant once cancelled and the adapter discards the
  result as `Cancelled`; native latency 0.7 to 3.3 ms on a 64-instruction field (asserted under 500 ms).
- Fixtures: `field_repeat_tube` and `field_creature` (body and head from ellipsoids, tube legs and tail, smooth unions, separately meshed
  eyes), rendered from six views.

Completed in this pass: creature body spacing is 0.026 m; the six-view Preview render no longer shows the prior sawtooth limb faceting.
`gallery.rs` now checks the actual Preview mesh stays under 200,000 triangles and compares it with Final (12% total-volume tolerance,
0.06 m per bounds face). Interactive browser evaluation requests Preview by default; there is no idle upgrade to Final. Final is available
only when a caller explicitly requests it from the evaluator. The fixture lattices are 70 × 74 × 58 (300,440 body samples) for Final and
35 × 37 × 29 (37,555 body samples) for Preview. The triangle heuristic estimates 123,210 body and 12,500 eye triangles in Preview,
135,710 total; actual mesh triangles are bounded by the regression test but not printed as a measurement.

Not done (G8 is open): noise, displacement, twist, bend; cancellation latency at accepted caps; Preview edit latency; MCP and inspector
support for fields; release wasm size. The current tree now passes the full strict gate (fresh log `target/reports/g9-fidelity-pass1.log`);
G8 acceptance remains open. Three-run native/WASI production-adapter time and memory are recorded
in ADR 0022 and the G8 report. Field group save/reload/instance edit/undo/redo is covered by `g8_persistence.rs`.

## 2. Findings from today (act on these first)

1. **Resolved:** `field_creature.txt` now meshes its body at `edge_length=0.026`. The Preview render uses 0.052 m spacing and the six-view
   browser check passes without the previous sawtooth limb surface. `gallery.rs` checks the 200,000 triangle Preview cap, 12% volume
   tolerance and 0.06 m bounds tolerance against Final.
2. **Resolved for this fixture; general heuristic remains approximate.** Preview scaling and field lattice/tape/work/padded-slot preflight
   already exist. The creature Preview lattices are 35 × 37 × 29 points for the body and 25 × 11 × 9 for the eye field. The planner's
   triangle estimates are 123,210 and 12,500 respectively, below the Preview cap; actual post-mesh triangles remain authoritative.
3. **Resolved by documenting current behavior.** `EvaluationHost.request()` defaults to `preview`, the page has no idle Final request,
   and Final is used only when a caller explicitly requests it. The interactive page therefore remains Preview quality until a future
   explicit Final/export workflow is built.
4. **Resolved:** `limits.md` F5 now describes the fixture's actual bounds, 0.026 m Final / 0.052 m Preview spacing and sample counts.

## 3. Remaining work, in order

### F1. Fix the creature and its evidence (complete)
- The creature uses 0.026 m body spacing and 0.025 m eye spacing. Six-view SwiftShader browser rendering passed after the change; the
  silhouette review records the prior limb sawtooth and its removal in Preview.
- `g8_creature_preview_stays_within_triangle_budget` enforces the 200,000 triangle Preview limit and compares Preview to Final for total
  volume and bounds.
- Optionally give the creature named colour regions once F5 exists (belly, back, legs), and a follow-up edit demonstration (limb length and
  blend radius) with the result still valid.

### F2. Preview scaling, planning and the Final upgrade (complete for current UI behavior)
- `Quality::Preview` doubles field edge length; `Quality::Final` uses authored spacing. The Preview/Final fixture test enforces 12% total
  volume and 0.06 m bounds tolerances.
- Graph planning preflights grid samples, tape instructions, sample-instruction work and padded slots before evaluation. Its triangle
  estimate is heuristic; the creature passes its Preview budget at the estimate, and the evaluated mesh still gets an authoritative cap.
- The browser evaluation host requests Preview by default and does not automatically request Final when idle. That behavior is now stated
  in the report; building a Final/export UI is outside this finish pass.

### F3. Noise, displacement, twist, bend (the vocabulary the plan still names)
Follow R4 in `handoff-g8-remaining.md`. The rule that matters: **no libm**. Implement `sin`, `cos` (and anything else) in our own code from
exact IEEE operations, once, as a specification used by both the C++ interpreter and the Rust reference evaluator, and prove native versus WASI
bit equality with a million-input test. Noise is an integer hash with polynomial smoothing. Guarantees for these are `None`. The
`field.distance_inexact` warning for offset or shell on an inexact field is implemented and tested; remaining operators and deterministic math
are still open.

### F4. Regions or a recorded limit (limit documented)
- Named field regions are deferred for G8. `LevelSet` currently returns geometry without per-triangle primitive labels, and no label-tape
  or propagation ABI exists. ADR 0022, the G8 report and case G8-17 state this limit. Separate `field.to_solid` outputs are the workaround;
  the creature keeps eye details separate and does not claim named field regions.

### F5. Persistence, WASI evidence and cancellation at the caps
- **G8-16 implemented:** `g8_persistence.rs` tests a grouped field subgraph, two instances, canonical save/reload with identical meshes,
  a stable-ID instance override, and undo/redo restoring mesh output.
- Still needed: exercise more field chains through booleans/materials and persistence of transformed field expressions.
- **Production measurements recorded:** `examples/field_measure.rs` and `tools/run-wasi-measure.mjs` run the release modeling adapter for a
  1,030,301-point sphere and the creature on native and Node WASI. Three-run medians and linear memory are in ADR 0022 and `g8-report.md`.
- Still needed: cancellation latency at accepted caps, especially under WASI; the current synchronous Node WASI harness has no external
  cancellation source during one blocking geometry call.
- Keep the parity corpus lines; add lines for each new operator (noise, twist, bend, repeat, tube) as they land.

### F6. MCP and inspector
Follow R9: a short MCP guide block and a test where a model builds a blob and a creature, edits by ID, and an oversized or clipped request is
refused with its code; the inspector shows each field's distance guarantee, taken from the Rust evaluator (an evaluation note per node), not
recomputed in JavaScript.

### F7. Evidence, docs and the gate
- Release wasm size delta against the G7 figures (evaluator 1,954,650 raw / 550,502 Brotli-9; runtime 34,513,972 / 4,120,575); timings for the
  new cases; update `g8-report.md` (state exactly what exists with test names, the creature before and after, the Preview behaviour),
  ADR 0022 (decisions taken, measured basis; stays Proposed), `cases.md` (G8-01..G8-20 with status), `diagnostics.md`, `limits.md`.
- `cargo xtask check --strict` from a fresh log under `engine/target/reports/`, real `exit=` line. Say which stages were cached.

## 4. Traps

- Fixture resolution is a quality decision with a budget, not a free parameter: every finer value costs samples, operations and triangles, and
  the Preview tier refuses anything over 200,000 triangles.
- Rust reference evaluator and C++ interpreter are two implementations of one specification; every new instruction needs the bit-equality test.
- The report must not claim more than a test proves; the previous summaries said "faceted joins" for what was whole-limb faceting.
- Rust files under 400 lines, functions under 80, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy; C++ gated by clang-format,
  clang-tidy, ASan, UBSan. New tests go in new files.
- Another agent may edit the tree; if something unrelated breaks, re-run before assuming it is yours. No commits, no attribution lines, edit tools
  only (formatters and `bun run schema:generate` are fine). Do not reuse the router key pasted in chat earlier.

## 5. Done means

The creature renders smooth from all six views within the Preview budget and the report says what changed; Preview scales field resolution and
the plan refuses over-budget fields before running; the page's Final behaviour is stated and implemented or documented; noise, displace, twist and
bend exist with our own math proven bit-equal native versus WASI; regions work or are recorded as a limit; persistence, instancing and WASI
evidence (time, memory, cancellation latency at the caps) are recorded; MCP builds and edits fields and the inspector shows each guarantee; sizes
and timings are recorded; report, ADR 0022, cases, diagnostics and limits agree with the code; `cargo xtask quick` is clean while working and the
strict gate passes from a fresh log with the real `exit=` line.
