# Experiment reports

One section per experiment (X1 to X9 in the plan). Spike code is throwaway and lives under `spikes/`
(never imported by the engine); the findings live here. Machine for every number: Apple M1, 16 GB,
macOS 26.1, headless Chromium from Playwright 1.64, release builds, warm HTTP cache.

## X6: the engine canvas inside a React shell (2026-10-12) — answered, adopt with two engine fixes

**Question.** Can the engine canvas live as one panel of a React 19 editor (lifecycle, resizing, pointer
coordinates, focus), or does it need an iframe?

**Build.** Spike `spikes/x6-react-shell`: Vite 7 and React 19 from the repository root, `StrictMode` on,
a three-panel layout (a field and buttons, the engine pane, buttons) with a resizable pane. The engine is
the real runtime (the dev build, 62 MB) loaded as `ve_runtime.js` plus its wasm, the wasi shim and the
`snippets/` directory (the WebXR bridge script, which the glue imports and which is easy to forget).

**What the experiment found in the engine (both fixed, both with regression tests):**
1. **Any resize killed the engine.** The runner changed the window's resolution but never sent Bevy's
   `WindowResized` message, which is what makes the camera recompute its target size. The render targets
   kept the old size while the surface took the new one, a validation error followed, and Bevy quit the
   app, permanently. This was true of the engine's own page too (a browser window resize or opening
   devtools would have frozen the viewer), and had gone unnoticed because no test resized a rendering
   page. Fixed in `ve-platform-web` (the message, plus the scale-factor message when it changes);
   `web/tests/resize.mjs` resizes six times including 64 by 48 and fails without the fix.
2. **Hover stayed lit when the pointer left the canvas** for another panel (no event told the engine).
   Fixed: `pointerleave` clears the cursor, and a press captures the pointer so a drag that ends outside
   the canvas still delivers its release and cannot leave a grab held. Case added to `interact.mjs`.

**Results after the fixes:**

| Check | Result |
|---|---|
| Boot under StrictMode (effects run twice) | started exactly once behind a module-level guard; no errors; 1.7 to 2.9 s with the 62 MB dev wasm |
| Canvas sized to its panel | the drawing buffer followed the pane: 360 by 600, then 700 by 600, then 1 by 600 (hidden), then 360 by 600 again; the engine kept running through all of it |
| Pointer coordinates with an offset canvas (panel at x=216) | hover found at the same relative positions in a 360 and a 700 pixel pane (the engine reads the position relative to the canvas) |
| Pointer over another panel | no hover (after fix 2) |
| Panel remount (React unmounts and mounts the pane) | one engine-owned canvas moved into the new mount: still running, same size logic, hover works, `start` called once |
| Canvas out of the document for a second, then back | no errors, still running, size recovered |
| Typing in a React field | "wasd" typed intact; no engine errors (the engine listens for pointer events on the canvas only; keyboard capture stays the shell's job) |
| **Naive pattern** (React renders its own `<canvas id="ve">`, re-created on remount) | the new element is a blank 300 by 150 canvas while the engine keeps rendering into the old detached one: **must not be used** |

**Not measured.** Pointer lock inside the shell (headless Chromium refuses it); the viewer's keyboard
handling next to focused fields; two engines in one page (a second glue instance would be needed; not
required, one engine per page); repeated move cycles for leaks; the release build and a real GPU.

**Decision.** Embed in the React shell (D8 stands; the iframe fallback is not needed). Rules for V0: the
engine canvas is created once outside React and moved into the pane; the engine is booted once per page
load behind a guard; there is no `stop()`, so hide a panel rather than destroy it (add a teardown API
only if a fixture needs it); ship the engine as glue, wasm, shim and the `snippets/` directory.

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

### Production G2 follow-through

The production evaluator and worker now exist separately from the disposable spike; details and
reproducible evidence are in [g2-report.md](g2-report.md). Large transferable results, cooperative
native boolean cancellation, hard timeout/crash recovery and cache/upload bounds are covered by the
production tests. Current release evaluator is 217,936 B stripped Brotli q9. Headset CPU cost and
hardware frame time remain open; production software-browser measurements do not replace those tests.
