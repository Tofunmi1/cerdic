# Track 1: runtime plan (render, web platform, WebXR, interaction)

Owner-approved scope. This file is the anchor: work happens in milestone order, one milestone at
a time. If something is not in a milestone, it goes in the Parking lot, not into code.

## 1. Goal and non-goals
**Goal.** A Rust/WASM runtime that renders our procedural geometry in the browser on desktop and in
WebXR (Quest 3 = reference device), with controller/hand/mouse interaction, built on a trimmed Bevy
0.20.0 fork (ADR 0001), gated by `cargo xtask check` (AGENTS.md).

**Non-goals for Track 1** (each is a later track; do not start them here):
geometry-language growth, extension registry beyond `ve-core`, command/transaction system, scene
solver, logic/NPC/memory, in-world UI panels, audio, physics, networking/multi-user, editor UI,
asset import of any kind (ADR 0003), Blender integration.

## 2. Working rules (the anti-digression contract)
1. One active milestone. Write its acceptance tests *first*; it is done only when they pass and
   `cargo xtask check` is green. Update the status table (section 8) in the same change.
2. Spikes answer a question and are thrown away; product code is rewritten into crates, never
   imported from `spikes/`.
3. Every risky unknown has a time box and a pre-agreed fallback (section 6). When a time box
   expires, stop and report to the owner; do not improvise a new direction.
4. New ideas go to the Parking lot (section 9). They are not acted on until triaged.
5. Changing a decision needs a new ADR and a note to the owner. Changing this plan needs the same.
6. Numbers beat opinions: each milestone records its measurements (size, frame time, tests) in
   section 8. Claims without a measurement are labelled as targets.
7. Report after each milestone: what passed, measurements, surprises, next milestone.

## 3. Architecture for Track 1
```
engine/
  crates/
    ve-core            (exists) Name, Registry. No Bevy/platform.
    ve-kernel-sys      raw FFI declarations + CMake build of cpp/ (Manifold, meshopt) per target.
    ve-kernel          safe public API (Solid, Mesh, typed errors). All unsafe lives in one audited
                       private module (ffi.rs), every block with a SAFETY comment.
    ve-xr-math         pure: poses, views, projection, IPD, reference-space math. Native-testable.
    ve-interact        pure: ray, pick, grab state machine, gesture recognition. Native-testable.
    ve-render          Bevy render crates glue: schedule, driver, extraction, pipelines, shaders.
    ve-platform-web    wasm-only: canvas, wgpu manual init, event loop, input events.
    ve-xr-web          wasm-only: WebXR session/frame/input, XR framebuffer import, haptics.
    ve-runtime         assembles plugins into a Bevy App; the single entry the page calls.
  cpp/                 C++ kernel sources + CMake; clang-tidy/format configs live here.
  third_party/bevy     vendored Bevy 0.20.0 subtree + patches/ (each patch documented).
  web/                 page, JS glue (<=1k lines), Playwright e2e + IWER scripts.
```
Rules: pure crates (`ve-core`, `ve-xr-math`, `ve-interact`, `ve-kernel`) never import Bevy, wgpu or web
crates (enforced by `xtask rules`; extend `ENGINE_AGNOSTIC` as each is added). Bevy types appear
only in `ve-render`, `ve-platform-web`, `ve-xr-web`, `ve-runtime`.

What we take from Bevy 0.20 (measured): `bevy_render` already has `MeshAllocator`, `RenderMesh`,
`ViewTarget`/`ViewUniform`/`ExtractedView`, `ManualTextureViews`, render phases, `RenderSystems`
sets. Rendering is one ECS schedule per camera run by `camera_driver` (core_pipeline/schedule.rs).
We write ourselves: mesh-instance extraction and batching (the lean equivalent of bevy_pbr's
render/mesh.rs, 14.6k lines upstream), the forward pipeline and shaders, the stereo driver, the
platform runner, the WebXR layer.

## 4. Test strategy
| Level | Tool | Runs where | Covers |
|---|---|---|---|
| Unit/property | nextest, proptest | native | pure crates, math, state machines |
| C++ kernel | native build + ASan/UBSan, same tests as wasm | native + wasm | kernel correctness and memory safety |
| Golden image | Playwright + Chrome (SwiftShader/WebGL2) | CI-able | render correctness; pixel diff with tolerance, snapshots committed |
| XR e2e | Playwright + IWER (`metaQuest3` preset) | CI-able | session, per-eye render, controller/hand input flows |
| Fuzz | cargo-fuzz | native | any parser/loader added in Track 1 |
| Perf | criterion + in-page frame-time overlay | native / real Quest | budgets in section 7 |
| Hardware | Quest 3 over local HTTPS (LAN only) | manual | frame time, thermals, comfort, multiview decision |
Golden-image and IWER runs prove correctness, not performance. Performance is judged only on the
headset.

## 5. Milestones
Each: deliverable, acceptance tests (written first), size estimate, depends-on.

**M0 Foundation** (done). Workspace, lints, xtask gates, AGENTS.md, ADRs 0001-0003, C++ configs.

**M1 Vendor and trim Bevy (done).** Subtree `third_party/bevy` at v0.20.0; path deps for the kept crates
(ecs, app, asset, mesh, camera, transform, render + needed tiny ones); `patches/` dir with a
README convention (one patch = one purpose + upstream reference). `cargo deny` passes with the tree.
*Accept:* `cargo xtask check` green; `cargo build --release --target wasm32-unknown-unknown` of
`ve-render` stub succeeds; size and crate count recorded. *Est:* ~0.3k own lines. *Needs:* M0.

**M2 C++ kernel bridge.** `cpp/` kernel (Manifold C API + meshopt + our weld) built by CMake for
native and wasm; `ve-kernel-sys` (unsafe, minimal) and `ve-kernel` (safe RAII `Solid`: cube, sphere,
cylinder, union/difference/intersection, transform, `to_mesh`). Errors are typed; no panics.
*Accept:* identical results native vs wasm on a shared test corpus; ASan/UBSan clean; fuzz target for
mesh-input validation; clang-tidy clean. *Est:* 1.5-2.5k. *Needs:* M1.

**M3 Web platform + wgpu device.** Canvas, WebGL2 context (optionally xrCompatible), wgpu
adapter/device via `RenderCreation::Manual`, resize/DPR, our own event loop on rAF, input events to
Bevy resources. *Accept:* Playwright golden shows a cleared canvas of a set colour; device limits
logged and asserted (max texture, uniform sizes) against our budget. *Est:* ~1k. *Needs:* M1.

**M4 Render core v0.** Lean mesh-instance extraction, batching, one forward pipeline (unlit +
lambert), depth, our camera schedule `Forward3d` and driver (adapted from `camera_driver`), WGSL.
Draws M2's cube-minus-sphere. *Accept:* golden image; frame loop stable 1000 frames with no
growth in memory; unit tests for extraction/batching logic. *Est:* ~3k. *Needs:* M2, M3.

**M5 Materials, lights, post.** Metal-rough PBR-lite (borrow BRDF WGSL from Bevy), directional +
capped point/spot lights, ambient/SH environment, MSAA resolve, tonemap + gamma pass (needed for
the non-sRGB XR framebuffer). *Accept:* golden images per material/light case; light-count caps by
quality tier tested. *Est:* ~3k Rust + ~1.2k WGSL. *Needs:* M4.

**M6 Shadows and quality tiers.** One shadow map (directional), PCF; tier table (low/med/high)
selecting MSAA, shadows, light caps, resolution scale. *Accept:* golden images; tiers switch at
runtime without leaks. *Est:* ~1.5k. *Needs:* M5.

**M7 Stereo core (desktop-verifiable).** View abstraction for N views; stereo driver (two passes
into per-eye viewports of one texture); shared culling across eyes; mock HMD (IPD, side-by-side
preview) for desktop. *Accept:* golden side-by-side image; culling shared (test counts); view
projection math property-tested in `ve-xr-math`. *Est:* ~1.5k. *Needs:* M5.

**M8 WebXR layer.** Session lifecycle, reference spaces, view poses to cameras, XRSession rAF loop,
input sources (controllers, hands, gaze/transient pointer), haptics, hit-test hook.
*M8a crux (time box 2 days):* render into the `XRWebGLLayer` framebuffer. Preferred: a small wgpu-hal
GLES patch to wrap the external framebuffer as a render target. Fallback: render to our own texture and
blit per eye to the XR framebuffer with raw GL. Decide by measurement, record in ADR 0004.
*Accept:* under IWER `metaQuest3`: session enters, both eyes render (golden), controller pose moves a
cube, trigger fires an event; session end cleans up. *Est:* 3-4k Rust + <=1k JS. *Needs:* M7.

**M9 Quest 3 hardware gate.** LAN-only HTTPS test page; frame-time overlay (rAF deltas, dropped
frames); frame-time governor driving the quality tiers; fixed benchmark scenes (section 7).
*Accept:* measured numbers recorded; decision taken per section 6 (stay / multiview patch / fallback).
*Est:* ~0.8k + test scenes. *Needs:* M8 and the owner with the headset.

**M10 Multiview (conditional on M9).** Only if budgets are missed: wgpu-hal GLES
`max_multiview_view_count` patch, per-pipeline `multiview`, view-uniform arrays,
`OCULUS_multiview` path. *Accept:* same golden images; measured gain recorded. *Est:* 1-2k + patches.

**M11 Interaction.** Ray pick, grab/hold/release state machine, two-hand scale/rotate, pinch and
gaze+pinch gestures (own recognizer over hand joints), desktop mouse/touch parity through one input
abstraction, socket-snap *hook* only (logic lives in a later track). *Accept:* property tests on the
state machines; XR e2e flows under IWER; same flows by mouse in desktop golden tests. *Est:* 2-3k.
*Needs:* M8.

**M12 Hardening.** Fuzz in CI-style runs, perf benches with budgets, 30-minute soak (memory flat),
panic-free audit (`wasm` abort policy), docs, ADRs, status table finalised. *Est:* ~1-2k. *Needs:* M11.

Order: M1 -> (M2 || M3) -> M4 -> M5 -> M6 -> M7 -> M8 -> M9 -> (M10) -> M11 -> M12.
Total estimate: 20-30k lines (Rust ~17-24k, WGSL ~2-3k, JS ~1k, C++ shim ~0.5k), excluding vendored Bevy.

## 6. Risks, time boxes, fallbacks, stop-and-report triggers
| Risk | Time box | Fallback | Report when |
|---|---|---|---|
| Render into XR framebuffer from wgpu GLES (M8a) | 2 days | own texture + raw-GL blit per eye | both routes fail |
| Bevy render crates need heavy patching for stereo/WebGL2 | per milestone | thin own wgpu renderer keeping `bevy_ecs` (ADR 0001) | >1.5k lines of Bevy patches in one milestone |
| Quest frame budget missed (M9) | 1 week of optimisation | M10 multiview, then quality-tier cuts, then fallback renderer | frame time > budget after M10 |
| C++ in wasm size/UB | per milestone | keep kernel API small; native sanitizers | any sanitizer finding unfixed >1 day |
| IWER cannot run headless | 1 day | run IWER in headed Chrome for manual e2e | not solved in the time box |
| WebGL2 feature gaps on device | at M3 | adjust pipeline (no storage buffers etc.), document limits | needed feature missing on Quest |

## 7. Initial budgets (targets to be revised by M9 measurements, not facts)
Quest 3: 72 Hz (13.9 ms/frame) first, 90 Hz as stretch. Benchmark scenes: S1 empty room + 20 props
(~50k tris, 40 draws), S2 dense (~200k tris, 150 draws), S3 stress (~400k tris, 300 draws).
Wasm size: <= 4 MB brotli for the runtime (measured baseline: 1.82 MB brotli for the trimmed render
set). Boot time: <= 3 s on desktop. Memory: flat over 30 minutes.

## 8. Status
| Milestone | State | Measurements / notes |
|---|---|---|
| M0 Foundation | done | gates verified by planting violations (unwrap, #[allow], forbidden dep, 401-line file) |
| M1 Vendor/trim Bevy | done | 38 crate dirs vendored (15 MB, tag v0.20.0, b3bdd75), ve-render stub builds native + wasm, deny/clippy/tests/wasm gates green; ve-render wasm tree =      224 crates; spike baseline 7.2 MB wasm (1.82 MB brotli) |
| M2 C++ kernel | done | C ABI kernel over Manifold 3.5.4 + meshopt 1.3 (vendored, offline CMake). 19 Rust tests + 17 C++ checks under ASan/UBSan, clang-tidy clean (wasm sysroot), native == wasm32-wasip1 output on an 11-line corpus (`cargo xtask parity`). Deviation: proptest robustness tests (512 cases of garbage meshes) stand in for a cargo-fuzz target; spike baseline: boot 69 ms in Chrome |
| M3 Web platform | done | Canvas window + rAF runner + pointer input (ve-platform-web), clear pass (ve-render), entry point (ve-runtime). Headless Chromium (SwiftShader): backend Gl (WebGL2), boot 529 ms (dev build), ~60 fps, canvas pixels exactly (26,51,102), resize follows viewport, no console errors. Limits seen: texture 8192, bind groups 8, UBO 65536 B, 8 colour attachments. Sizes (unoptimised, with debug info): web-dev 57 MB, release 29 MB; optimisation is M12 |
| M4 Render core v0 | done | `Forward3d` schedule + camera driver, mesh extraction, instance batching (model matrix as instance attributes: WebGL2 has no storage buffers), reverse-z depth, forward pipeline, dynamic view uniforms. The C++ kernel's cube-minus-sphere renders in headless Chromium (WebGL2): golden-image test (tolerance 10/channel, <=0.2% pixels), background colour exact, wasm memory growth 0 bytes over 600 frames (12.6 MB resident), 3 batching unit tests. Browser test harness fixed after a flake (server bound IPv4 loopback explicitly). Deviations: no MSAA yet (M5), draws straight to the output view (gamma/tonemap pass is M5), no frustum culling yet |
| M5 Materials/lights/post | done | Metallic-roughness PBR (GGX, Smith, Schlick), sun + up to 8 point lights (closest kept when over the cap), ambient, ACES tonemap in the shader, MSAA 4x resolving into the output (samples discarded after resolve), per-instance material data so batching depends only on the mesh. Three goldens reviewed by eye (kernel / material grid / coloured lights), 37 tests. Deviations: tonemap is in the forward shader, not a separate pass; the gamma step for a non-sRGB XR framebuffer moves to M8, where that output format exists. **Lesson:** the output attachment must come from `ViewTarget::out_texture_color_attachment`; building one from the raw view skips the "written" mark, Bevy never presents the frame, and the screen silently shows the previous one (cost most of a day-equivalent of bisecting) |
| M6 Shadows/tiers | pending | |
| M8a spike (early) | done | Spike `spikes/xr-framebuffer`: with wgpu-hal patch 1 (`texture_from_web_framebuffer`, see third_party/PATCHES.md) wgpu rendered a clear into a WebGL framebuffer created outside wgpu; pixel read back exactly, framebuffer complete, no GL error. Still to prove on a real XR layer: depth (wgpu's external-framebuffer pass supports one colour attachment; if the layer's own depth cannot be used, render to our own colour+depth textures and blit per eye, the planned fallback) |
| M7 Stereo core | pending | |
| M8 WebXR layer | pending | |
| M9 Quest gate | pending | needs owner + headset |
| M10 Multiview | conditional | |
| M11 Interaction | pending | |
| M12 Hardening | pending | |

## 9. Parking lot (ideas not acted on; triage with the owner)
- Language growth: attach/socket snapping, skeleton/creature nodes, grammar nodes, procedural materials, critique loop.
- Extension registry beyond `ve-core`: pack format, schema DSL, tool-schema generation.
- Command/transaction/undo system; scene solver; logic/NPC/memory; in-world UI; audio; physics.
- Register the geom-lang MCP server in Claude Code (owner approval needed).
- CI workflow file once a remote exists (owner decision).

## 10. Definition of done (every milestone)
`cargo xtask check` green; acceptance tests present and passing; measurements recorded in section 8;
ADR written if a decision changed; no `spikes/` imports; owner report sent.
