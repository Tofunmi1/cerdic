# Handoff: subdivision attributes (UV, float point data, holes)

Status: implemented and validated 2026-10-14. Read `docs/modeling/g7-report.md` for behavior, limits and measurements;
read `docs/adr/0021-subdivision-adapter.md` for the recorded decisions. The original implementation plan follows as history.

Date: 2026-10-14. Read first: `handoff-common.md` (rules, gate speeds), `handoff-g7-subdivision.md` (brief),
`handoff-g7-fixes.md` and `docs/modeling/g7-report.md` (what exists and what was recorded as a limit),
`docs/adr/0021-subdivision-adapter.md` (Proposed). This closes the three gaps G7 left as typed refusals. Until it is
done, any cage that carries UVs, float per-vertex data or hole tags cannot be subdivided.

## 1. Why, and in what order

| Gap | Today | Why it matters | Priority |
|---|---|---|---|
| UV face-varying channel | paired corner `u`/`v` refuse `poly.subdivide.uv_fvar_unsupported` | The procedural-materials work (plan G10) needs UVs on smooth surfaces; subdivision is the main smooth-surface tool | 1 |
| Float point attributes | `F32`/`F64` point attributes refuse `poly.subdivide.point_interpolation_unsupported` | Weights, colours and masks authored on a cage vanish otherwise; later deformers and fields (G8) produce them | 2 |
| Hole tags | a non-zero face `hole` attribute refuses `poly.subdivide.holes_unsupported` | Openings in smooth surfaces; lowest demand | 3 (optional; a recorded limit is acceptable if it does not fit) |

Do them in that order. Items 1 and 2 share one C ABI change (section 3), so design that once.

## 2. State you start from (verified 2026-10-14)

- C ABI (`cpp/include/ve_kernel.h`): `ve_subdivide_catmark(...)` takes 19 arguments, including `hole_faces`/`hole_count`
  that the implementation currently rejects (`prepare_input` returns `VE_ERR_INVALID_ARG` when `hole_count != 0`, and the
  descriptor sets `numHoles = 0`). Output is read with `ve_subdivision_num_*` and one `ve_subdivision_copy`.
- `cpp/src/subdivision.cpp` (521 lines; C++ is not under the 400-line Rust cap but is gated by clang-format, clang-tidy,
  ASan and UBSan): builds a `Far::TopologyDescriptor` in `make_refiner`, refines uniformly, interpolates positions with
  `Far::PrimvarRefiner::Interpolate` per level in `interpolate_positions` (polls `ve_context_cancelled` between
  levels), derives vertex provenance in `set_vertex_origins` and cage-face provenance in `set_face_parents`.
- Rust: `ve-kernel/src/ffi/subdivision.rs` (unsafe call and copy), `ve-kernel/src/subdivision.rs` (validation,
  `SubdivisionMesh`), `ve-geometry/src/subdivide.rs` (adapter; `check_supported_attributes` raises the refusal codes,
  `propagate_attributes` copies face attributes by provenance and reports dropped integer point attributes),
  `ve-modeling/src/poly_ops/subdivision.rs` (graph operator), diagnostics in `docs/modeling/diagnostics.md`.
- Vendored OpenSubdiv 3.7.0 includes Far, Vtr and Sdc (`third_party/opensubdiv/VENDOR.md`); face-varying and primvar
  refinement are in Far, so nothing new needs vendoring. Check the size delta anyway.
- Release sizes after the node: evaluator 1,954,650 raw / 550,502 Brotli-9; runtime 34,513,972 / 4,120,575.
  Near-cap refine (1.98M output faces) took 311 ms native / 710 ms WASI with about 514 MB native and about 400 MiB WASI
  memory. Extra channels add to that memory; section 4 sets the limit.
- Attribute model (`ve-geometry/src/attr.rs`): scalar attributes, one value per element, in domains Point, Face,
  Corner, Edge, CurvePoint, Instance; types `F32`, `F64`, `I32`, `U32`. UVs are two corner attributes named `u` and `v`.

## 3. P1. One ABI change that carries both (design first, then build)

Replace the long argument list with an input struct and add channel output, instead of growing the 19-argument call.

- `ve_subdivide_input` (plain C struct, versioned size field): the existing cage and sharpness fields, `hole_faces`,
  plus
  - point channels: `const float* point_values`, `size_t point_channels` (interleaved: vertex-major, `channels` floats per
    vertex);
  - one face-varying channel: `const float* fvar_values` (unique values, `fvar_width` floats each, normally 2),
    `size_t fvar_value_count`, `const uint32_t* fvar_corner_values` (one value index per cage corner).
  Keep the old function as a thin wrapper until nothing calls it, then delete it.
- Output accessors on `ve_subdivision`: `ve_subdivision_point_channels(...)` copy (vertex-major, final level only),
  `ve_subdivision_fvar_value_count`, `ve_subdivision_copy_fvar(values, corner_value_indices)` (one index per refined
  corner, in the refined `corners` order that `ve_subdivision_copy` already returns).
- Implementation, in new `cpp/src/subdivision_channels.cpp` (keep `subdivision.cpp` from growing):
  - Point channels: a `Far::PrimvarRefiner::Interpolate` per level over an N-float struct (same loop and cancellation poll
    as positions; a templated or runtime-width primvar type with `Clear` and `AddWithWeight`).
  - FVar: set `descriptor.numFVarChannels = 1` and `fvarChannels` with the unique-value count and per-corner indices;
    set the face-varying boundary mode with `Sdc::Options::SetFVarLinearInterpolation`; refine with
    `PrimvarRefiner::InterpolateFaceVarying(level, src, dst, channel)`; read the refined per-corner value indices with
    `TopologyLevel::GetFaceFVarValues(face, channel)` for each refined face, in the refined corner order.
- Rust side: safe wrapper types in `ve-kernel/src/subdivision.rs` (new `SubdivisionInput`/channel types so the 11-argument
  functions go away; clippy `too_many_arguments` is already strained), the unsafe copy in `ffi/subdivision.rs`, with
  `// SAFETY:` comments stating capacities equal queried counts.
- Bounds: refuse before refinement with a typed code if `vertices_total_over_levels * channels` or the unique UV count
  exceeds a cap (section 4). Poll cancellation between levels for every channel. Keep the exact growth preflight
  (`growth_preflight`) as the source of truth for counts.

## 4. P2. Float point attributes

Behaviour: each scalar `F32` point attribute becomes an interpolated `F32` point attribute with the same name on the refined
mesh, using the same boundary and crease rules as positions. Interpolate all float point attributes in one pass as
channels of one interleaved array (deterministic channel order: attribute name order, `BTreeMap` order as elsewhere).

Decisions to record in ADR 0021, with recommendations:
- `F64`. [Keep refusing with a distinct code (`poly.subdivide.f64_unsupported`) rather than silently narrowing. Revisit only if
  authored data needs it.] Do not store an `F64` attribute that went through f32 as if it were exact.
- Cap on channels. [At most 8 point channels; total interleaved floats at most 32 million (128 MB); measure the near-cap case
  with 1 and 8 channels and write the numbers down.] If the measured WASI memory is too high, lower the cap, do not skip the
  measurement.
- Integer point attributes stay dropped and reported (existing policy).

Tests (names suggested): linear attribute on a flat grid stays exactly linear; constant stays constant; values on a crease
edge keep their ends; two attributes are independent and keep their names; an attribute plus a crease plus a boundary rule;
F64 refuses; cap refusal gives limit and observed counts; cancellation between levels with channels; native versus WASI
identical (add corpus lines in `crates/ve-modeling/examples/corpus.rs`); ASan/UBSan C++ test for the channel path in
`cpp/tests/subdivision_test.cpp`.

## 5. P3. UV face-varying channel

Behaviour: when the cage has corner attributes `u` and `v` (both `F32`, same length as `corners`), the result has corner attributes
`u` and `v` on the refined faces, with seams preserved.

- Welding (in Rust, deterministic): build the unique `(u, v)` table by exact bit equality in order of first appearance, and the
  per-corner value index. Do not reuse cage corner indices after refinement; the refined corner order comes from the kernel.
- Boundary mode: decide and record with a test. Recommendation: linear boundaries (`FVAR_LINEAR_BOUNDARIES`), so texture
  seams and borders do not shrink; verify it against a unit-square UV grid.
- A cage that has `u` without `v` (or the reverse), different types, or a wrong length refuses with a specific code
  (`poly.subdivide.uv_incomplete`), not the generic corner-attribute code. Other corner attributes and all edge attributes keep
  their typed refusals unless you decide otherwise and write it down.
- Reconcile with `PolyMesh` rules: corner attributes must be validated against `corners.len()`; keep the refined mesh valid.

Tests: G7-07 (a seam, two corners at one vertex with different UVs, stay different; a continuous mesh stays continuous); an
identity-UV grid keeps UV equal to position after any level; a cube with per-face UVs keeps all six islands separate; seam plus
crease; native versus WASI identical (corpus line); the `u`-without-`v` refusal; a UV mesh through `poly.to_solid` and the
graph with `poly.subdivide` and save/reload.

## 6. P4. Hole tags (optional; resolved as a documented limit)

Today `PolyMesh` has no hole domain. A reserved face attribute named `hole` (`U32`; non-zero is a hole) is already detected and
refused by the adapter. Steps:
1. Read the pinned source (`third_party/opensubdiv/opensubdiv/vtr` and `far`) to confirm how hole faces are tagged and
   inherited by child faces, and whether children appear in the output topology. Record the answer; do not assume.
2. If children of a hole face are present and tagged: pass the cage `hole` face indices through the descriptor (`numHoles`,
   `holeIndices`; remove the `hole_count != 0` rejection), tag refined faces from provenance, and decide whether the output
   mesh keeps them as faces carrying `hole = 1` (recommended: keep them and let `poly.to_solid` and the renderer refuse or skip
   them with a clear code) or filters them out. Update `set_face_parents` accordingly.
3. If it needs a larger contract (an output filter that shifts face indices and so every selection fingerprint), stop and
   record it as a limit with the reason and a test that the refusal stays stable. Do not claim hole support.

Resolution: pinned OpenSubdiv 3.7.0 marks base faces through `setBaseFaceHole` / `TopologyLevel::setFaceHole`; the refiner path
used here has no child-face hole inheritance contract or output filtering contract. Keep the stable
`poly.subdivide.holes_unsupported` refusal and its regression test. No hole support is claimed.

## 7. Evidence and docs (close the package)

- `cargo xtask quick ve-kernel ve-geometry ve-modeling` while working; `cargo xtask check --strict` once at the end from a
  fresh log under `engine/target/reports/`, waiting for `exit=`.
- Re-measure release wasm sizes and the near-cap timings and memory (use `subdivision_bench`, add cases with UV and with 1 and
  8 channels; run natively and under `node tools/run-wasi.mjs`). Record `+X raw / +Y Brotli-9` against the figures in section 2.
- Update `docs/modeling/g7-report.md` (move each gap from "limits" to "implemented" with its test name, or keep it with the
  reason), ADR 0021 (f64, channel caps, UV boundary mode, hole decision; stays Proposed), `cases.md` (G7-07, G7-08),
  `diagnostics.md` (added and removed codes), `limits.md`, `docs/modeling/mcp-test-provider.md` and the MCP guide in
  `tools/mcp/server.mjs` (one short sentence: UVs and float point data are carried through `poly.subdivide`).
- Added `uv_subdivided_grid.txt` to the gallery: `poly.grid` authors normalized UV corners, then `poly.subdivide` runs them
  through the browser evaluator and viewport. The geometry test verifies identity UVs after two levels, and native/WASI parity
  includes UV islands and two point channels.

Implementation evidence (2026-10-14): the unified size-versioned C ABI and `subdivision_channels.cpp` are in place; point
channels use one OpenSubdiv primvar refiner per level, UVs use its face-varying refiner with `FVAR_LINEAR_BOUNDARIES`, and the
adapter welds UV values by exact float bits in first-seen order. Eight channels / 32 million refined floats remain the limits.
Near-cap eight-channel WASI memory was 563,347,456 bytes; near-cap UV memory was 650,444,800 bytes. See
`docs/modeling/g7-report.md` for per-case times and native RSS.

## 8. Traps

- Determinism: no hash maps or threads in the adapter. Value tables use first-appearance order or `BTreeMap`; native and WASI
  must match byte for byte on every new corpus line.
- Unsafe: every new `unsafe` block keeps a `// SAFETY:` comment; the wrapper owns the handle (`Drop` frees it) and sizes buffers
  from the counts the C side reports.
- Do not widen the clang-tidy exclusions in `cpp/CMakeLists.txt`; they exist only for false positives inside OpenSubdiv headers.
- Rust files stay under 400 lines (`ve-kernel/tests/subdivision.rs` is at the cap; new tests go in a new file), functions under 80,
  no `#[allow]`, pedantic clippy.
- A refusal test is not an implementation test. Keep the old refusal tests only for the cases that still refuse.
- Another agent may be editing the same tree; if something unrelated breaks, re-run before assuming it is yours. No commits, no
  attribution lines, edit tools only, do not reuse the router key pasted in chat earlier.

## 9. Done means

A cage with `u`/`v` corner attributes and one with float point attributes subdivides, keeps their names, seams and crease behaviour,
and matches between native and WASI on corpus lines; caps and memory are measured and recorded; holes remain a tested limit with
the source-based reason; ADR, report, cases, diagnostics, limits and the MCP guide agree with the code; release sizes and timings
are updated; `cargo xtask check --strict --no-cache` passes from a fresh log with the real `exit=` line. Report honestly:
SwiftShader only, no headset, no provider-backed real-model run.
