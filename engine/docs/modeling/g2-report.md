# G2 worker evidence (2026-10-12)

Status: G2 implemented; final `cargo xtask check --strict` passed with no skipped steps, including
226 native tests and the worker browser acceptance. Separate advisories and root release build passed.
V0 is committed as `c49d512`.
The logs and JSON below are generated artifacts under `target/`; rerun the commands to reproduce them.

## Architecture

`ve-modeling` is a pure, stepped evaluator. `ve-evaluator` exposes it to a dedicated worker, using
the same C++ kernel and WASI shim as the renderer module. Neither threads nor SharedArrayBuffer are
required. The window stages commands in a cloned Rust `Session`; history, conflicts and IDs use the
existing command system. The committed document and scene change together after render assets are
prepared. Failed candidate edits leave the committed document and geometry intact.

One active job and one latest pending job bound the queue below the eight-job cap. Pending commands
accumulate in Rust, so replacing an evaluation request does not lose independent edits. Every result
is checked against document identity, revision, request, quality, evaluation hash and worker generation.
Preview/final are different cache keys. Selection metadata names the authored node and revision.

The cache holds exported immutable meshes, never kernel handles, with a 128 MiB LRU budget including
charged metadata. New values enter it only after a whole job succeeds. All graph/parameter validation
and predicted geometry/storage checks precede kernel allocation. Boolean growth estimates are
conservative estimates, not mathematical upper bounds; actual mesh/storage checks and hard worker
termination cover unexpected growth. A single backend operation can allocate internally before the
post-check. No universal backend allocation bound is claimed.

`ExecutionContext` cancellation is exposed for binary booleans and batch union. A native test cancels
an active 32-sphere batch tree from another thread and observes `Cancelled`; C++ sanitizer tests cover
pre-cancellation, invalid batch lengths and fresh-context recovery. A single-threaded browser worker
cannot receive messages during one synchronous WASM call: it cooperates between DAG steps, with
termination as the hard-limit fallback. Failed or partially cancelled jobs publish no staged cache.

Uploads pack at most 512 triangles per chunk (36,864 B) and stage one chunk per frame. Render asset
readiness gates the final swap, scheduled before picking. The Bevy byte limiter is also configured to
8 MiB. CPU asset creation is measured separately from transfer delivery and whole upload duration.
Projection has a 60-second deadline and typed errors; runtime calls cannot leave the host silently
waiting forever. Headset GPU/frame timings are not established by these checks.

## Acceptance and evidence

- Native evaluator tests: primitive/boolean versus direct kernel, budget rejection before execution,
  reduced-cap LRU eviction, 100 cache reuses, and cancelled staged-cache rejection.
- `cargo xtask parity`: kernel corpus plus production evaluator corpus, identical native/WASI mesh
  positions, topology and volume. The boolean fixture additionally checks analytic volume 0.5 m³.
- `web/tests/evaluation.mjs`: actual evaluator worker and renderer; rapid edits, stale revision
  conflicts, late preview/generation rejection, ten-second timeout, crash recovery, visible failure
  status, retained scene, bounded cache/live meshes after 100 jobs, large transfer, staged upload,
  frame pacing/input during the blocked worker, and reload during a job. Included in the strict gate.
- Logs: `target/g2-check.log`, `target/g2-parity.log`, `target/g2-advisories.log`.
- Browser metrics/screenshot: `target/reports/g2-evaluation.json`, `g2-editor.png`.

The G2-01 list originally required F1, whose profile operators are introduced by G3. G2 uses the
existing primitive/boolean fixture as the handoff's G2 section requires; F1 parity belongs to G3.
Cache eviction is exercised with the same production algorithm under a 1,024 B cap, alongside browser
assertions on the normal 128 MiB cap, without allocating 128 MiB merely to trigger a test.

The reviewed editor screenshot shows generated boxes on the blue background; eleven overlapping
outputs read as one cube. This is a worker/projection fixture, not the G3 modeling gallery. Initial
comparison props disappear on the first authored projection; camera state remains independent.

## Measurements and limits

Release `bun run build` passed. Brotli q9 measurements strip custom sections in memory:

| Module | Raw shipping WASM | Stripped WASM | Stripped Brotli |
|---|---:|---:|---:|
| Runtime | 33,933,707 B | 19,097,455 B | 3,398,760 B |
| Evaluator | 843,335 B | 683,806 B | 217,936 B |

Runtime delta: +28,509 B from V0, +40,623 B from G1b (3,358,137 B). Evaluator is a separate new
payload, +34,814 B over X1's disposable experiment. Shipping copies retain custom sections;
no wasm-opt has been applied. These are the final release measurements including budget diagnostics.

The final strict browser run measured:

| Workload | Observation |
|---|---|
| 100 warm regenerations | 4,480 B cache; 11 hits, zero operators re-executed; 11 live meshes |
| Large final result | 2,361,960 B transferred; 173.1 ms worker work; 44,892,160 B WASM memory |
| Large transfer delivery | 20.5 ms from worker send to window handler, including scheduling |
| Large staged upload | 267 live chunks; 13,373.8 ms total under SwiftShader; 36,864 B max chunk |
| Asset creation | 0.3 ms observed maximum; this is CPU staging, not measured GPU time |
| Recovery | 902.7 ms through first scene upload; 9.8 ms most recent worker startup |
| Window frames during blocked worker | 195 intervals; median 50 ms, p95 83.4 ms, maximum 133.4 ms; text input remained usable |

Recovery includes rendering/upload; it is not comparable to X1's worker-only 46–70 ms restart metric.
One chunk per frame is deliberately conservative and adds latency and draw calls. Future batching
can use the same bounds and readiness checks. Browser timing uses software rendering and is not a
Quest or real-GPU performance result. The final strict run records frame-gap distribution during the
blocked worker; no 60 fps claim is inferred from headless scheduling. This software-rendered run
does not meet a 60 fps target and is not presented as such. Audit B02 is closed for the verified
worker/cancellation paths; headset CPU competition remains a hardware follow-up.

Actual headset worker CPU competition, GPU upload cost, XR frame time, Safari, Quest Browser and
headed pointer lock remain untested. No hardware limitation is hidden by the emulated XR tests.

## Validation finding

The first strict run found a pre-existing sphere picking property-test failure for a 0.1 m sphere
roughly 33 m from the ray origin. Subtracting squared f32 distances lost precision. Picking now forms
the perpendicular vector using f64 intermediates and converts the returned distance to the existing
f32 API. The minimized proptest input is retained as a regression seed.
