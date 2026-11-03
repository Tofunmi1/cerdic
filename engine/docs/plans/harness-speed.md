# Harness speed (2026-11-03)

Status: measured, first fix landed, the rest proposed. The harness is everything between a model's tool call and a frame it can look at: parse, evaluate (worker wasm), upload, texture bake, light probes, render, snapshot.

## 1. Measured (Chromium on the owner's Mac, `worlds/paris_dev.txt`, 380 x 320 m, 65 nodes, 24 outputs, 460k stored triangles)

Cold load, before and after the first fix (`tools/profile/load-timeline.mjs`):

| Stage | Before | After |
| --- | --- | --- |
| Page ready (wasm from localhost, 116 MB dev build) | 1.4 s | 1.2 s |
| Evaluation in the worker | 36 s | 3.0 s |
| World visible (Final committed) | 42.1 s | 8.8 s |
| Light probes done | 92.4 s | 53.3 s |

Inside the old 38.7 s evaluation (phase marks in a copy of the worker): geometry steps 2.6 s, data extraction 0.01 s, **floating-parts check (`eval_attachments`) 36.1 s**. The same world evaluates natively in 1.1 s (`eval_text`), so the wasm geometry (3.0 s) is about 2.7 times native.

Edits, after the fix (`tools/profile/edit-loop.mjs`, parameter-set commands on the loaded world):

| Edit | Evaluation | Visible after | Light probes done after |
| --- | --- | --- | --- |
| sky `sun_elevation` | 0.65 s (64 of 65 nodes cached) | 4.2 s | 27.3 s |
| one building part `detail` | 2.6 s (62 cached) | 6.0 s | 28.8 s |
| a texture colour | 0.64 s (65 cached) | 4.0 s | 26.8 s |

So evaluation is cached and cheap on edits, yet every edit costs 4 to 6 s to show and about 25 s for the light probes. An edit that changes no geometry still re-uploads everything and re-bakes every probe.

## 2. Landed

- `ve-measure::attachments_within` (tests first, `tests/attach.rs`, 3 new): nearest boxes first, stop at the first neighbour within tolerance, a work budget (`ATTACHMENT_WORK_BUDGET`, 600,000 steps) and a tree-against-tree distance query (`bvh/proximity.rs`) instead of one query per vertex. `attachments_json` uses it. The floating-parts check on the Paris world: 36.1 s to 0.9 s, still returning a real answer (`floating: []`).
- Texture compiler int/float fix (see `game-world-system.md` section 12).

## 3. Proposed, ranked by seconds saved per edit

1. **Upload only what changed.** An edit that leaves a part's geometry identical (same hash) should skip its re-upload and texture bake. Target: 4 to 6 s to under 1 s for sky, material and texture edits. The projection layer already carries a document hash per result; add a per-part hash and diff.
2. **Light probes: skip, draft, cache.**
   - Skip the re-bake when geometry and sun are unchanged (a texture colour edit): 27 s to 0.
   - A draft tier (fewer rays and bounces, coarser spacing) while a model iterates, the full bake when the world sits idle: 27 s to about 3 s per look.
   - Cache a finished bake by content hash (IndexedDB), so a reload or a hosted demo shows lit scenes at once.
   - Longer term, bake on the GPU (WebGPU compute) instead of the main thread.
3. **A fast check inside the page.** The product is the browser with the chat: the model's tool calls are plain function calls in the page, with no server, no native process and no MCP hop. The evaluator already runs in a worker, so add an evaluate-only mode that returns geometry stats, budget, diagnostics, per-node timing and game validation without uploading, texture baking, light probes or rendering (about 3 s on the Paris world, against 8.8 s to a visible frame and 53 s to lit). The model uses it for most steps and asks for a snapshot only to judge looks. (`eval_text` stays a developer measuring tool, never part of the product.)
4. **Smaller, faster startup for the hosted demo.** The runtime wasm is 116 MB in the dev profile (17 MB gzipped). Measure a release build with `wasm-opt`, brotli and streaming compile; trim `bevy_ecs debug`.
5. **Wasm geometry at 2.7 times native.** Try SIMD, `wasm-opt -O3`, and moving the occlusion bake (about 1 s native) to a parallel pass.
6. **Dev loop.** The runtime and evaluator rebuild by hand (cargo, bindgen, an import patch after every bindgen): one script, one command.
7. **Texture bake on edit.** Bake only programs whose source or inputs changed (the cache key is the program text).

## 4. The in-browser chat loop (everything runs in the page)

Once the engine takes 4 to 9 s an edit, the model's own latency (several seconds a tool call) dominates, so the harness has to ask for fewer, bigger calls and overlap the waiting:
- **Fewer round trips:** the `batch` and `run` tools do many edits, a check and a state delta in one call; every mutating call already returns a delta; dry runs answer without evaluating.
- **Overlap:** while the model writes its next call, the page can evaluate the last edit ahead of time, and a small pool of evaluator workers can try two or three variants a model proposes (for example three sun angles) in parallel, returning one contact sheet.
- **Cheaper turns:** the guide, operator list and tool schemas are a stable prefix for prompt caching (keep them byte-identical between turns); snapshots default to the small contact sheet and `image=false` comparisons; long results are summarised with a `detail` request.
- **Honest progress:** the page shows the stage it is in (evaluating, uploading, baking, lighting), so a model or a person never waits blind, and the model can look at a draft frame before the light probes finish.
- **Local only:** no tool depends on a server; the model provider call is the only network use, from the browser.

## 5. Method

Measure before changing: the timeline script prints each stage's wall time, the phase marks come from a copy of `eval-worker.js` with `performance.now()` around each call, and `eval_text` gives native per-node time. Every fix lands with a test and a before-and-after number here.
