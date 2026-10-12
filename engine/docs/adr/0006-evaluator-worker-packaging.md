# 0006: Geometry evaluation runs in a worker
- Status: accepted 2026-10-12 after experiment X1 (evidence in `docs/modeling/experiments.md`)
- Date: 2026-10-12

## Context
ADR 0001 chose one engine in one WASM module. Heavy modeling operations (booleans, subdivision,
field meshing) can run for seconds; a synchronous WASM call on the window thread would stall input and
the XR lifecycle, and a stuck call cannot be cancelled from inside. Manifold's `ExecutionContext`
gives cooperative cancellation only for some operations.

## Decision
- Evaluation runs in a dedicated worker built from the same Rust and C++ source as the engine, with a
  worker entry and no renderer. It evaluates immutable candidates and returns transferable buffers.
- The window owns the document revision and history, supervises the worker, and terminates and
  restarts it for stuck or crashed jobs. Cooperative cancellation is used where verified.
- No threads, SharedArrayBuffer or isolation headers are introduced.
- Jobs and results carry document ID, revision, request ID, worker generation, quality and dependency
  hash; stale results are discarded.
- Packaging: **two modules from one source.** X1 measured the evaluator-only module at 183 KB brotli
  against 3.36 MB for the full runtime, a cold start of 31 to 70 ms, and a stuck job terminated and
  restarted in 46 to 70 ms with identical results, so a separate small module beats loading the
  runtime twice. Still open for G2: large transfers, cooperative cancellation in the C ABI, and the
  CPU cost of a worker on a headset.

## Consequences
ADR 0001 stays accepted: this is one codebase with two entry points, not a second engine. G2
implements the worker, limits and recovery before expensive modeling operations are added. Audit
item B02 is closed by G2.
