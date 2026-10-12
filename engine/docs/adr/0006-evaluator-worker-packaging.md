# 0006: Geometry evaluation runs in a worker
- Status: proposed (confirmed or revised by experiment X1 before G2)
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
- Packaging (one module reused by both entries, or two) is decided by X1 from measured size,
  startup and transfer cost.

## Consequences
ADR 0001 stays accepted: this is one codebase with two entry points, not a second engine. G2
implements the worker, limits and recovery before expensive modeling operations are added. Audit
item B02 is closed by G2.
