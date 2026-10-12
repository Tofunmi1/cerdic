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
| M6 Shadows/tiers | done | **Tiers:** `QualityTier` low/medium/high fixes MSAA (1/2/4), shadow map size (off/1024/2048), filter (1 tap / 3x3), point-light cap (2/4/8) and headset resolution scale (0.8/1.0/1.0); page uses `?quality=` or `__ve.setQuality`, cameras follow the table, the XR layer is created at the tier's scale (IWER ignores the scale, so that part is untested until the headset). Tested: each tier boots with its settings, low and high really render differently, 24 live switches leave wasm memory flat (0 KiB) and the image identical to the golden. **Shadows:** one orthographic sun map fitted to a 10 m sphere around the viewer (covers both eyes), position snapped to whole texels so edges do not shimmer, depth-only pass before the camera schedules, hardware comparison sampler with normal-offset and slope bias, optional 3x3 PCF. Unit-tested fitting/snapping/degenerate sun; browser-tested: shadows only darken (0 brighter pixels vs the shadowless tier), the new `shadows` scene golden and the XR golden (the bitten cube's shadow shows the light coming through its holes, identical in both eyes) reviewed by eye. 73 tests. Not done: cascades (single map, 10 m), point-light shadows, soft-shadow penumbra by distance; revisit with the headset numbers (M9). A shadow map costs one extra scene draw per frame, which is why low has none |
| M8a spike (early) | done | Spike `spikes/xr-framebuffer`: with wgpu-hal patch 1 (`texture_from_web_framebuffer`, see third_party/PATCHES.md) wgpu rendered a clear into a WebGL framebuffer created outside wgpu; pixel read back exactly, framebuffer complete, no GL error. Still to prove on a real XR layer: depth (wgpu's external-framebuffer pass supports one colour attachment; if the layer's own depth cannot be used, render to our own colour+depth textures and blit per eye, the planned fallback) |
| M7 Stereo core | done | `ve-xr-math` (15 tests: asymmetric reverse-z projection, GL-projection recovery, eye placement, side-by-side layout) and a stereo render path: a `StereoViews` component on a `Camera3d` yields two view slots; ONE render pass draws both eyes (viewport per eye) into one MSAA target with one resolve, sharing instance data. Desktop mock headset (`stereo` scene) verified by a golden image plus a parallax check (halves similar but not identical). 53 tests total. Deviation: no frustum culling exists yet, so "shared culling" is deferred to the performance work (M9/M12). **Lesson:** on GL the MSAA resolve is a blit that obeys the scissor test, so a leftover per-eye scissor rectangle resolved only the last eye; viewports alone confine rendering, so stereo uses no scissor |
| M8 WebXR layer | done | **M8b done:** JS bridge (`ve-xr-web/js/xr.js`, plain values only) + `ve-xr-web` crate (pure frame logic unit-tested natively; session glue wasm-only) + `XrPlugin`/`XrCamera`/`XrRig`/`XrInput`. The XR session loop drives the app (the window rAF loop yields via `set_externally_driven`). Verified against IWER's emulated Quest 3 in Chromium: session enters, eye separation equals the device IPD exactly (0.063 m), head pose follows the rig, a 0.5 m head step moves the camera exactly 0.5 m, controllers are read, ending the session returns to normal rendering; golden stereo image reviewed. Findings: Chromium has a native `navigator.xr` so IWER needs `forceInstall`; IWER is mono by default (`stereoEnabled`); IWER's layer framebuffer is `null` (draw to the canvas). **M8c done:** real-headset output path. The layer's opaque non-sRGB framebuffer is imported through the patched wgpu-hal and set as the headset camera's target; non-sRGB outputs render offscreen (MSAA, resolve) and a present pass copies into them with a hand-written sRGB encode (wgpu's GL external-framebuffer pass cannot be an MSAA resolve target). `xr-layer.mjs` hands IWER's layer a JS-made RGBA8 framebuffer and reads it back: 0% pixel difference from the window-path golden, right way up. Found and patched (PATCHES.md #2): Bevy requested an sRGB view format that WebGL2 cannot provide. **Known waste, for M12:** Bevy still allocates its own main textures per view that we never use (at Quest eye-buffer size that is tens of MB each plus a 4x MSAA copy) and our MSAA, offscreen and depth targets add to it; measure on the headset and trim. **M8d done:** controller input. `XrInput` carries grip/aim poses, analogue buttons and axes per hand; the XR scene's wands follow each grip through the rig and swell with the trigger; status JSON exposes controllers. Tested in IWER: tracked, the wand moves exactly 0.4 m when the hand moves 0.4 m, trigger value reaches the app. Select/squeeze *events*, hand tracking and the interaction model are M11. **M8 is complete;** next M6 (software), then M9 on hardware |
| M9 Quest gate | prepared; **hardware run pending (owner + headset)** | Everything that does not need the device is built and tested. `ve-core`: `FrameStats` (rolling p50/p95/max, dropped frames = over 1.5x the refresh interval) and `Governor` (steps one tier down fast when p95 exceeds the budget, up slowly after 8 s of 25% headroom, backoff after a failed climb; property-tested for range, step size, cooldown spacing, garbage input, and a borderline-cost simulation that settles on the affordable level with <= 8 changes in 40k frames). Runtime: real frame times in the status JSON (`perf`), optional governor driving the tiers (`?governor=1`), window refresh override (`?hz=`), headset refresh from `session.frameRate`. Benchmark scenes from section 7: `?scene=bench1|2|3` = 45/155/305 draws, 48k/202k/404k triangles (every prop its own mesh asset so nothing batches). Workflow: `node tools/serve-lan.mjs` (HTTPS with a generated self-signed cert for Wi-Fi, or `--http` + `adb reverse` over USB; localhost counts as secure) then open `?scene=bench1&bench=1&seconds=60`, press Enter VR, wait: the page ends the session, shows the numbers on screen and posts a JSON report that the server saves under `target/reports/`. Tested end to end under IWER (report contents, server safety, certificate). **To do on the headset:** run S1/S2/S3 at each tier, record numbers, decide stay / multiview (M10) / fallback per section 6; also unverified until then: the tier's framebuffer scale and the unused-main-texture memory waste |
| M10 Multiview | conditional | |
| M11 Interaction | done (hand tracking untested) | **M11a:** `ve-interact`, an engine-agnostic crate (38 tests incl. properties): ray picking, one pointer abstraction for mouse, controllers, hands and gaze, a hover/select/grab state machine with baseline-relative grabs (no jumps when holders change), two-hand move/scale/rotate about the object's own centre, a socket-snap hook on release, pinch recognition with hysteresis. **M11b:** runtime glue (`ve-runtime/src/interact.rs`) and `ve-xr-web::controller_pointer`. Objects marked `Interactable` hover (emissive glow), can be clicked, dragged and wheel-pushed with the mouse, and grabbed with the controller squeeze (thumbstick pushes/pulls the held object); two hands scale and turn. Each controller draws a laser that stops at what it hits. Status JSON has an `interact` section (hover, held, object transforms, last 24 events). Browser tests: `interact.mjs` (mouse hover/click/drag/drop/wheel) and `xr-interact.mjs` (IWER controller hover, grab, carry, release, two-hand scale 3x). XR golden re-reviewed: lasers and hover glow are the only change. **Not covered:** hand-tracking pinch input (the recogniser is unit-tested; `XrInput` does not carry joints yet) and gaze; both need a headset to judge. |
| H1 Audit hardening | done (headset-only items open) | From the 2026-10-11 external audit (`docs/research-handoff.md`); each fix has a regression. **R01 governor:** it now judges overload from the 95th-percentile *interval* against the refresh period with a 10% jitter margin, and treats a step up as a trial that backs off if it fails (a display-paced loop cannot show spare capacity); waits are elapsed milliseconds, pauses over 1 s restart the judgement, a governor request the page overrode times out. Tests: paced 72 Hz recovery, jitter, pause, missed refreshes. **R02 XR resolution:** the layer is allocated at the largest tier scale and a tier change requests a per-view viewport scale each frame; the engine draws into the viewports it is given. Emulated runtime test: low = exactly 0.8 of high, repeated toggles return to the same size; status shows requested scale and whether the runtime supports it. *Whether a real Quest grants it is still a headset question.* **R03 session lifecycle:** a failure after the session was granted ends it; the request claims `Requesting` before awaiting; test covers rejected reference space, double entry, simultaneous entry, repeated enter/exit. **R04 normals:** cofactor normal matrix in the shader; instances that are mirrored, collapsed or non-finite are not drawn (documented, tested). **R05:** cylinders stand along Y, converted once in the C++ kernel; bounds, centred, cone and volume tests. **R06:** lights and shadows are fitted to the point between the XR eyes, not the desktop camera. **R07:** explicit order (XR frame, wands and interaction, status); a controller that loses tracking or disappears is treated as gone, so grabs release once and cannot be resumed by reappearing. **R08:** `scene` now reports instances/mesh assets/triangles, and a new `render` section reports draws and triangles submitted per pass (counted at the draw calls, both eyes); estimated missed refresh periods; the benchmark page makes one report per session (timing from presentation start, final figures kept by the engine), tests check two sessions are independent and that counts reconcile. **R10:** `Shape::from_local_bounds` and `Pose::from_matrix` refuse collapsed, mirrored, sheared and non-finite transforms; `q` and `-q` compare equal. **B01:** kernel dimensions are bounded (`MAX_EXTENT`) and `simplify` rejects non-finite positions. **B03:** the C++ build watches the whole vendored trees. **B04:** `cargo xtask check` says when a gate was skipped and `--strict` fails on a skip; `cargo xtask advisories` checks RustSec. **B06:** the LAN server answers malformed URLs with 400, caps retained reports and never overwrites one. **Not done:** R09 culling and extraction cost (measure on the headset first), stable input-source ids beyond handedness, instanced-mesh benchmark workload, B02 worker/cancellation (belongs to the language/document follow-on), B05 release-size measurement, B07 certificate renewal, and the `npm` versus Bun choice in `xtask/src/web.rs`. |
| M12 Hardening | pending | |

## 9. Parking lot (ideas not acted on; triage with the owner)
- Language growth: moved to `track2-modeling.md` (draft, awaiting owner approval): attach/socket snapping, creature and grammar nodes, procedural materials, critique loop.
- Extension registry beyond `ve-core`: pack format, schema DSL, tool-schema generation.
- Command/transaction/undo system; scene solver; logic/NPC/memory; in-world UI; audio; physics.
- Register the geom-lang MCP server in Claude Code (owner approval needed).
- CI workflow file once a remote exists (owner decision).

## 10. Definition of done (every milestone)
`cargo xtask check` green; acceptance tests present and passing; measurements recorded in section 8;
ADR written if a decision changed; no `spikes/` imports; owner report sent.
