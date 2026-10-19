# Handoff: fixing what the G7 review found

Date: 2026-10-14. Read first: `handoff-common.md` (rules, how the repo works; it now describes the two gate speeds),
`handoff-g7-subdivision.md` (brief), `handoff-g7-remaining.md` (previous package list), `docs/modeling/g7-report.md`,
`docs/adr/0021-subdivision-adapter.md`. This file is the list of defects and gaps a review found in the G7 work and
how to close them. The review ran `cargo xtask quick` (320 tests passed), probed the graph operator through the
text path, read the FFI and adapter code, and measured release sizes and timings.

## 0. How to work (changed this week)

- While working use `cargo xtask quick [crate...]` (fmt, clippy and tests for changed crates and their dependents,
  then rules). Use the full `cargo xtask check --strict` only at the end of a package, from a fresh log kept under
  `engine/target/` (not `/tmp`), and wait for the real `^exit=` line. Every stage prints its time.
- File changes only with Write/Edit; formatters and `bun run schema:generate` are fine. No commits (the owner commits),
  no co-author or tool-attribution text anywhere. Do not reuse the router key that was pasted into chat earlier.
- `.rs` files stay under 400 lines, functions under 80, no `#[allow]` (use `#[expect]` with a reason), pedantic clippy.
  Several files are near the caps; split by responsibility, do not append. `ve-kernel/tests/subdivision.rs` is at 399.
- The tree has ~200 uncommitted paths and another agent may be editing at the same time (a half-finished edit made a
  worker crash during the review, then cleared). If something unrelated breaks, re-run before assuming it is yours.

## 1. What the review found, worst first

| # | Finding | Evidence |
|---|---|---|
| F-A | The report overstated polygon editing after subdivision | Report now says curved quads convert to solids and subdivision is the v1 final editing step. Text graph tests cover `poly.extrude_faces` → `poly.patch_nonplanar` and `poly.bevel_edges` → `poly.bevel_network_unsupported` at levels 1 and 2. A general compatibility matrix for every other G6 editor remains unclaimed. |
| F-B | Float point, UV and hole propagation were absent | Float point and paired UV remain typed limits with reasons and refusal tests. A non-zero face `hole` U32 tag now refuses with `poly.subdivide.holes_unsupported` and has a regression test; no hole propagation is claimed. |
| F-C | The report contradicted itself and was stale | Report title, duplicate test entries, contradictory crease status and post-subdivision claim are corrected. Fresh strict count, release sizes and persistent gate-log path are recorded below. |
| F-D | Reference tests did not use parent correspondence | Refined vertex comparisons now use `vertex_origin_kind/index`; semi-sharp edge and corner parent points match the one-level blend reference at 0, 0.5, 1, 2 and 10. |
| F-E | Near-cap WASI and memory evidence was missing | All four benchmark cases now run under Node WASI. The ~1.98M-face cases measured 409/710 ms and 419,364,864 bytes WASM linear memory; native peak RSS was 514,441,216 bytes. Corpus parity is 29 lines (five sharpness values, open boundary and warning payload). |
| F-F | Warning delivery and failure cleanup needed coverage | Warnings aggregate from multiple nodes and de-duplicate; failed evaluation clears stale warnings; Problems panel typed report selection remains covered. The corpus tests the production warning constructor/wire shape because the language cannot currently author point attributes. |

## Current package status (2026-10-14)

The substantive review fixes above are in the tree and the full strict gate passes. F2/F3 are deliberately recorded as
explicit limits with specific reasons: the kernel C ABI has no bounded scalar primvar channel, while UV FVar
needs corner-to-value indices and remapping into refined corner order. F4 is explicitly refused and tested until
hole-child topology and provenance are defined. The v1 edit-order decision is implemented in operator help and
the MCP guide. Do not say every G6 editor has been matrix-tested: only the conversions, selection, extrusion,
bevel and persistence paths exercised by named G7 tests have evidence. This is an explicit v1 policy: edit the
cage before subdivision; other post-subdivision operators are outside the tested compatibility contract.

Final validation: `cargo xtask check --strict` passed with 572 Rust test cases and `all gates passed`.
The persistent log is `engine/target/reports/g7-fixes-strict.log` and ends in `exit=0`. The release browser build
was refreshed afterward; `cargo xtask rules` is the required final check for the documentation updates.

Good, keep as is: FFI wrapper (inputs validated, owned handle freed in `Drop`, buffers sized from queried counts),
selection checks (`selection.stale`, `poly.selection_kind`), parse-time `levels` range, budget refusal
(`budget.triangles`, estimate about 1.5 times the true count on a quad grid, so conservative), stacking
`poly.subdivide` twice, sphere at level 3, open grid output, fractional sharpness.

### Measurements the review added (release, native, Apple silicon, 2026-10-14)

- `cargo run --release -p ve-kernel --example subdivision_bench`, Node WASI runs, and `/usr/bin/time -l` results are
  recorded in `docs/modeling/g7-report.md`; the near-cap WASI memory peak is about 400 MiB, so the caps remain.
- Refreshed release browser artifacts (`cargo xtask web`): evaluator 1,954,650 raw / 550,502 Brotli-9
  (+144,547 / +39,152 versus G6); runtime 34,513,972 / 4,120,575 (+3,829 / +853 versus G6).

### Probe programs (re-run these after your fixes)

Evaluate through the text path with a test or a small throwaway example (delete it afterwards). Expected results are
today's behaviour; the right-hand column is the target.

```
a: box -> poly.from_solid -> poly.merge_coplanar -> poly.subdivide(levels=2) -> poly.to_solid      OK 192 tri, vol 0.3495
b: the same, then poly.subdivide(levels=2) again, then to_solid                                    OK 3072 tri
c: ... subdivide(levels=1); top = poly.select(s, element="face", by="box", box_min=[-1,0.3,-1], box_max=[1,1,1]);
   poly.extrude_faces(s, top, offset=[0,0.1,0]); to_solid                                          FAILS patch_nonplanar -> decide (F1)
d: poly.grid(width=2,height=2,columns=512,rows=512) -> subdivide(levels=4)                         budget.triangles, observed 201,326,592
e: crease selection made on the triangulated mesh passed to subdivide of the merged cage            selection.stale
f: poly.grid(columns=8,rows=8) -> subdivide(levels=2, boundary="edges") as a Poly output           OK 2048 tri
g: a face selection passed as creases                                                              poly.selection_kind
h: levels=5                                                                                        param.range at parse time
i: solid.sphere(segments=24) -> from_solid -> subdivide(levels=3) -> to_solid                      OK 27648 tri, vol 0.4899
j: quad cube, creases=sharp edges, sharpness=1.5, levels=3 -> to_solid                             OK 768 tri, vol 0.8740
k: ... subdivide(1); edges = select(s, element="edge", by="sharp", angle=0.3); bevel_edges(width=0.02)
                                                                                                   FAILS bevel_network_unsupported
```

## 2. Work packages, in order

### F1. Decide and document editing after subdivision (F-A)
1. Build the matrix: run every `poly.*` operator on a level-1 and a level-2 subdivided quad cube (and an open grid) and
   record pass, refuse (with code) or wrong result. Put it in `limits.md` and the G7 report. Add a test per operator
   that either succeeds with a valid closed solid or refuses with the stable code (so a future change cannot break
   it silently).
2. The failures seen are G6 v1 limits (planar patch for `extrude_faces`; disjoint edges for `bevel_edges`), not
   defects in the triangulation mode. Say so plainly; strike "follows the mesh into polygon edits" unless the matrix
   supports it for the operators that do work.
3. Owner decision, state a recommendation: extend extrude (individual mode) to curved quads by extruding along the
   Newell normal, or leave subdivision as a final step. Recommendation: leave it final, document it, and put the
   order "edit the cage first, subdivide last" in the operator help text and the MCP guide. Only implement the
   extension if the owner asks.

### F2. Float point attribute interpolation through OpenSubdiv (F-B, G7-08)
- Add to the C ABI an entry that refines per-vertex float primvars with `Far::PrimvarRefiner::Interpolate` (vertex
  interpolation, uses the same crease and boundary rules as positions), per level, into the final level. Pass
  interleaved floats (N channels). Poll cancellation between levels as positions do. Keep memory bounded: total
  floats = vertices times channels; refuse past a stated cap with a typed code and put the cap in the preflight.
- Safe wrapper in `ve-kernel/src/subdivision.rs`, adapter in `ve-geometry/src/subdivide.rs` (new module, the file is
  at 205 lines): scalar `F32` point attributes become interpolated `F32` point attributes on the result, keyed by the
  same name. `F64`: decide (recommendation: interpolate in f32 and keep f64 type only if the value round-trips; otherwise
  refuse with a code; write it in ADR 0021).
- Replace `poly.subdivide.point_interpolation_unsupported` with this behaviour; keep a refusal for anything still
  unsupported. Tests: a linear attribute on a flat grid stays linear; a constant attribute stays constant; a crease
  keeps values on the crease edge; native and WASI outputs are identical.

### F3. UV face-varying channel (F-B, G7-07)
- Paired corner attributes `u` and `v` (the G6/G7 convention) become one face-varying channel. Use OpenSubdiv's
  face-varying topology (`CreateFVarChannel`) and `InterpolateFaceVarying`, with a recorded boundary mode (start with
  `FVAR_LINEAR_BOUNDARIES` or the mode that keeps seams sharp; justify the choice in the ADR with a test of a seam).
- Output as corner attributes `u`, `v` on the refined faces, in the refined corner order (match `corners` from the
  kernel). Seam rule: two corners at one vertex with different UVs stay different; continuous UVs stay continuous.
- Tests: G7-07 (seam survives, continuous stays continuous), a unit-square grid with identity UVs stays identity after
  subdivision, native versus WASI identical. Extend the parity corpus with a seam mesh.
- Other corner and edge attributes keep their typed refusals unless you decide otherwise.

### F4. Hole tags (F-B)
- The C ABI already accepts `hole_faces`; `PolyMesh` has no representation. Recommendation: a reserved face attribute
  (for example `U32` named `hole`, non-zero marks a hole) read by the adapter and passed through; confirm how
  OpenSubdiv treats the children of a hole face (check the pinned 3.7.0 source) and test it. If that proves larger than
  a package, record it as a limit with the reason and a rejected-input test, and do not claim hole support.

### F5. Real tests for creases and extraordinary vertices (F-D)
- Add an in-test reference for edge and corner sharpness at one level (edge point blend `lerp(smooth, sharp, s)` for
  `0 <= s <= 1`, full sharp rule at `s >= 1`; vertex rules likewise) and compare **positions per vertex** for
  sharpness 0, 0.5, 1, 2, 10. For multilevel (Chaikin-like decay: each level reduces sharpness by one) compare level
  two against the reference applied twice, or state in the report that only one level is referenced.
- Rewrite the valence 3 and 5 test to match each refined vertex with its own parent through
  `vertex_origin_kind` and `vertex_origin_index` and assert the position, not nearest-neighbour.
- Rename any test whose name overstates what it checks. Keep `tests/subdivision.rs` under the 400-line cap
  (split it into `subdivision_creases.rs`, for example).

### F6. Evidence and the numbers (F-C, F-E)
- Run the benchmark example under WASI (`cargo build --release --target wasm32-wasip1 -p ve-kernel --example
  subdivision_bench`, then `node tools/run-wasi.mjs <wasm>`), record all four cases next to the native ones, and add
  peak memory for the near-cap cases (native `/usr/bin/time -l`, and the wasm memory size). Keep the 2,000,000 caps if
  the WASI time is acceptable; if not, change them and say why with numbers.
- Add parity lines to `crates/ve-modeling/examples/corpus.rs`: crease sweep, open-boundary grid, UV seam (after F3),
  and an attribute-dropped warning case. Check `cargo xtask parity` shows them identical.
- Release sizes have been refreshed and recorded in `docs/modeling/g7-report.md`.
- Test the warnings channel end to end: evaluator to worker to page to Problems panel for `attribute_dropped`, and a
  second node's warnings not being lost when two nodes warn. Make sure a failed evaluation clears stale warnings.

### F7. Make the report true
- Rewrite `docs/modeling/g7-report.md`: correct title, one entry per test run with the right counts, remove the
  contradictory sentences, move the review's timings and sizes in, state the editing-after-subdivision matrix (F1),
  and list what is implemented (float interpolation, UV, holes) or recorded as a limit, each with its test name.
- Update `docs/adr/0021-subdivision-adapter.md` (policies taken or still open, the f64 decision, UV boundary mode,
  hole representation), `cases.md` (G7-01 to G7-16 with status), `diagnostics.md` (new and removed codes),
  `limits.md`, `docs/plans/track2-modeling.md`. ADRs 0019 and 0021 stay Proposed; the owner accepts them.
- Keep the gate log under `engine/target/reports/` and cite that path.

## 3. Owner decisions (recommendation in brackets; build to it behind "Proposed" if unavailable)

1. Editing after subdivision. [Applied Proposed default: final step only; operator help/MCP say "edit the cage, subdivide last".]
2. `F64` point attributes. [Applied Proposed default: refuse the f32 kernel conversion rather than silently lose precision.]
3. Holes. [Applied Proposed default: non-zero `hole` face tags return a typed tested refusal until child topology and provenance semantics are designed.]

## 4. Traps

- Do not claim "done" for F2 to F4 on the strength of a refusal test; the refusal is today's behaviour.
- UV order: refined corner order is the kernel's `corners`; do not reuse the cage's corner indices.
- Determinism: no hash maps or threads in the adapter; use sorted structures (`BTreeMap`). Native and WASI must match
  byte for byte on the corpus lines.
- `PolyMesh` triangulation mode (`CurvedQuads`) is set only by `subdivide`; mesh values that pass through other
  operators must keep it, and `poly.to_solid` of a fold at or above 90 degrees must still refuse (existing test:
  `curved_quad_uses_the_shorter_diagonal_deterministically`).
- The C++ gate: the subdivision translation unit already disables two analyzer checks for false positives inside
  OpenSubdiv headers; do not widen that to project code. `cpp/src/subdivision.cpp` is 521 lines; split if it grows.
- Colour regions through subdivision are mapped by face label to triangle spans; if you add attributes, keep
  `region` as a face attribute copied through face provenance, and keep the two-colour test.

## 5. Done means

Every finding in section 1 is fixed or recorded as a limit with a test; probes a to k behave as stated (c and k per
the owner's decision); float point and UV subdivide work with parity and WASI evidence, or are explicitly recorded as
limits with the reason; creases and valence tests compare per-vertex positions; the report, ADR, cases, diagnostics
and limits agree with each other and with the code; release sizes and WASI timings are recorded; `cargo xtask quick`
is clean while working and `cargo xtask check --strict` passes from a fresh log under `engine/target/` with the real
`exit=` line. Report honestly: SwiftShader only, no headset, no real-model run.
