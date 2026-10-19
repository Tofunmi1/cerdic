# ADR 0019: polygon operators and subdivision backend

- **Status:** Proposed
- **Date:** 2026-10-14
- **Decision owners:** Spatial Lab maintainers
- **Supersedes:** the X3-dependent part of Track 2 decision D2

## Context

G6 needs face-level topology edits over open and closed polygon meshes. G7 needs Catmull-Clark with
boundary rules, creases and face-varying (corner/UV) data. The evaluator is Rust/WASM with a C++
kernel, preflight budgets, typed diagnostics, deterministic native/WASI parity and cooperative
cancellation. Its C++ build disables exceptions and RTTI.

X3 probed OpenSubdiv 3.7.0 (`9dab8a47bfbb1388ec8388fe61f5f916e6123f38`) and PMP 3.0.0
(`f2fb04f4a4188a5c1ab137e83b96e62fa99c639f`). Reproducible commands and measured limits are in
[`experiments.md`](../modeling/experiments.md#x3-opensubdiv-and-pmp-for-g6g7-2026-10-14--answered-with-a-split-choice).

## Decision

1. **G6 topology and edit operators are authored on Spatial Lab's `PolyMesh`.** The engine owns its
   adjacency, input contracts, face selections, attribute propagation, error handling, budgets and
   transaction semantics. PMP remains a reference and candidate isolated remeshing adapter. PMP is
   not linked into the evaluator in its current form: it does not compile with the engine's
   no-exception/no-RTTI C++ flags. Do not let raw PMP face handles or properties become document IDs.
2. **OpenSubdiv CPU Far is the proposed G7 Catmull-Clark backend.** Keep it behind an adapter that
   accepts validated `PolyMesh` topology and returns bounded refined geometry plus defined corner
   data. No GPU backend, threads or optional OpenSubdiv Osd device backend are included. This choice
   is conditional on G7 proving boundary, crease, UV-seam, refinement-budget, cancellation-boundary
   and native/WASI cases.
3. **No library call may bypass preflight or transactions.** Cap input counts, level count, total
   predicted vertices/faces, memory and output size before allocation. Check cancellation before
   and after each bounded phase. Neither probed library supports the engine's cancellation callback;
   one in-progress library call is therefore not interruptible. G7 must keep each call within the
   accepted worst-case budget or replace it with cancellable in-house phases.
4. **Selections are semantic and node-addressed.** Persistent commands use face tags/origin labels
   or deterministic geometric predicates, never array indices. A missing or ambiguous result is a
   typed refusal. Generated faces receive deterministic tags derived from operation ID and source
   semantic tag. Viewport face indices remain transient view state.
5. **Attribute rules are explicit per operator and domain.** Face attributes follow the source face
   to retained caps; generated walls get derived tags and documented inherited values. Point data is
   copied or interpolated only where the operation defines weights. Edge data follows mapped source
   edges. Corner data preserves UV seams by treating each face corner independently; it is never
   averaged across a seam. An operator that cannot define propagation refuses or reports a loss
   diagnostic instead of silently dropping it.

## X3 evidence summary

OpenSubdiv Far refines Catmull-Clark topology and interpolates vertex primvars. Its public topology
descriptor includes boundary/hole metadata, edge and vertex crease inputs, and face-varying channels;
Sdc exposes boundary, crease and face-varying interpolation rules. Its native and WASI smoke builds
passed (the standalone WASI probe needed a probe-only missing `<cmath>` include fix). The same small
probe returned 641 vertices and 600 faces natively and under Node WASI. At 100,000 input triangles,
one native Catmark step produced 300,000 quad faces. In the final ten repetitions OpenSubdiv median
was 16.07 ms (observed range 15.85–21.03); peak process RSS was 47.0 MB. A standalone WASI link added
78,450 raw / 30,061 Brotli bytes over a matching
minimal C++ WASI executable. This is not an evaluator integration delta.

PMP provides subdivision, face flipping, hole fill, triangulation, remeshing, decimation, and
halfedge primitives. Its named API lacks the G6 face operations. The probed PMP source failed the
engine-compatible WASI build on exceptions and RTTI, and no safe C ABI adaptation was tested. Native
Catmull-Clark on the same grid had a ten-run median of 24.95 ms (range 24.65–30.62), with 59.5 MB
process RSS. These
measurements include mesh construction and are smoke tests, not product latency claims. The complete
operator matrix and limitations are recorded in X3.

## Consequences

- G6 can be tested without tying the document format to a third-party halfedge representation.
- The initial G6 implementation is in-house even though PMP contains useful primitives. G6 scope is
  not reduced: every assigned operator still gets an implementation or a typed, documented limit.
- G7 may use OpenSubdiv while the cage and semantic data remain Spatial Lab-owned. OpenSubdiv's
  current build itself is not proof of safe/cancellable evaluator integration.
- PMP remeshing remains an open adapter question. If enabled later, isolate its exception/RTTI and
  attribute behaviour behind a C-compatible, panic-contained boundary and repeat the size/parity
  audit.

## Validation before Accepted

Review X3 and `docs/plans/g6-polygon-operators.md`; all G6 case rows must pass or be explicitly
deferred by a new decision. Before an OpenSubdiv adapter is accepted for G7, add corner UV and crease
reference cases, native/WASI output checks, budget refusals and a documented cancellation maximum.
Run `cargo xtask check --strict` after engine integration. The standalone X3 probe is not an engine
gate and was run on Apple Silicon plus Node WASI only.
