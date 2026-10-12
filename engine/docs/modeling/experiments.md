# Experiment reports

One section per experiment (X1 to X9 in the plan). Spike code is throwaway and lives under `spikes/`
(never imported by the engine); the findings live here. Machine for every number: Apple M1, 16 GB,
macOS 26.1, headless Chromium from Playwright 1.64, release builds, warm HTTP cache.

## X1: evaluator-only WASM in a worker (2026-10-12) — answered, adopt

**Question.** Can the geometry evaluator (kernel plus graph, no renderer) run in a worker built from the
same source, with the real C++ toolchain, and be terminated and restarted when a job sticks?

**Build.** Spike `spikes/x1-worker`: a `cdylib` of `ve-kernel` and `ve-graph` with `wasm-bindgen`
(`--target web`), the same wasi shim as the runtime, Rust 1.97.1, no new dependency, no threads, no
SharedArrayBuffer. The glue needs every `wasi_snapshot_preview1` import repointed to the shim (the
first attempt replaced only the first and the worker failed to load: a build-script detail, not a
design problem).

| Measurement | Result |
|---|---|
| Module size | 688,783 B raw; 569,498 B with custom sections stripped; **183,122 B brotli** (the full runtime is 3,358,137 B brotli, so the evaluator is 5.5 percent of it) |
| Cold start in a fresh worker (fetch, compile, instantiate; warm cache) | 31 to 70 ms total, 12 to 28 ms of it instantiation (4 runs) |
| Jobs (sphere unions through the kernel) | 2 spheres: 5 to 9 ms; 6 spheres: 36 to 56 ms; 10 spheres: 100 to 150 ms; 14 spheres: 152 to 195 ms |
| Transfer of the result (up to 90 KB) | under 1 ms over the job time (the result is moved, not copied) |
| Command path (`ve-graph` batch of 200 nodes) | 13 to 14 ms |
| Memory after 20 more jobs | 12.4 MB before and after: no growth once warm |
| Main thread during a 186 ms job | 60 frames per second kept, longest frame gap 17 ms (the same as idle) |
| Stuck job (`spin`) terminated from the page | worker terminated, the pending call settled as `terminated`; terminate, new worker, init and a first job in **46 to 70 ms**; five restarts, **identical results every time**; main thread longest gap 17 ms throughout |
| Deliberate trap (`unreachable`) | reported as `RuntimeError: unreachable`; the same worker answered the next call (not proof that the instance is safe: policy is still to restart after any trap) |

**Not measured.** Result transfers above 0.1 MB; the cost on a Quest (a worker competes for CPU cores
with the XR frame loop, unmeasured); cooperative cancellation (`ExecutionContext` is not in the C ABI
yet, so only termination was tested); jobs that take seconds; Safari and Quest Browser behaviour.

**Decision.** Two entry points from one source: a separate evaluator module for the worker (183 KB
brotli) rather than loading the 3.4 MB runtime twice. Termination plus restart is a sound recovery
path (under 100 ms). ADR 0006 is accepted with this evidence. G2 must still measure the large-transfer
case, add the ABI for cooperative cancellation, and test on the headset.
