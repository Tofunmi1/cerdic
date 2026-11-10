# Plan: finish the animation pipeline and the rest of Blender

Tracking file. Tick items as they land; add the date and the evidence (test name or report) next to each.
Last updated 2026-10-17. Done so far: W1 timers, W2 sockets, W3 clips and sequences, W4 animated copies, W5 glTF export of
glow/colour/visibility, W7 IK, W8 skinning depth (dual quaternion, painted weights; evidence: 7 `rig::` unit tests,
`rig_skin` 8 tests, corpus 81 lines native == wasi, `g10-skin` twist pinch ratio linear 0.25 vs dual 0.94, Sonnet 2/2 on
`twisting-forearm` and `painted-knee`), W11 deformers, W10 curves (partly), W13 `inst.drop`, W16 terrain, W19 physics
bodies. Deferred by choice (design needs per-copy bone poses and a new animatable property): W9 skinned crowds, W8b shape
keys, W6 headset panel. Next: W20 joints, collide triggers and scene gravity, then W14 poly backlog, material opacity
(water, glass), W12 organic (largely covered by the SDF fields: smooth union, tube, capsule), W15 to W18.

**Done bar for every item** (same as `AGENTS.md`): reachable from the typed graph (spec, guide text, tool schema),
a Rust or unit test, a corpus line when the evaluator output changes (native == `wasm32-wasip1`), a focused
browser script when something is visible, an MCP real-model prompt when a model is meant to author it, and an ADR
when a decision changes. Blender code: vendored with `VENDOR.md`, `VE patch` markers only for our edits, native and
WASM bit hash equal, trap stubs audited, size recorded.

**Visual fidelity findings (2026-10-18), the next focus.** Measured on the village demos: (1) the texture language already makes real
PBR surfaces (the `bricks` node with a `height` channel gives mortar grooves and bevels through normals); (2) a program that has a
`height` channel AND `roughness = colorize(...)` hangs the bake (cause unknown); use `brightness_contrast` for roughness instead; (3) a
box's six faces share one map in a 3 x 2 grid, so each gets 170 x 256 pixels at the fixed 512 x 512 layer size
(`ve_render::LAYER_SIZE`, 64 layers), which blurs big surfaces; (4) the string cap of 256 bytes silently limited texture programs
(fixed 2026-10-18). To do: renderer layers larger than 512 with a memory budget (fewer, bigger layers, or size classes per part),
UV tiling for large surfaces, per-face (not atlas) maps, the high-to-low baker (W17), a material kit, and the bake-hang fix.

Legend: `[x]` done and verified, `[~]` built but not fully verified, `[ ]` not started.

## Work order for everything still open (added 2026-10-17)

Each batch ends with the full browser sweep run on its own (no builds in parallel), the Rust and TS tests, the parity
corpus and an updated tick list below. Design choices are fixed here so work does not stall on them.

**Batch 1: finish the animation pipeline (small to medium, all inside what exists)**
1. W1 Timer trigger (`trigger="timer"`, `after` seconds from entering play, optional `every`): fixed-step counter, no
   new state model. Test: a door opens by itself after 2 s; replay gives the same step.
2. W2 Socket triggers (`attach`, `detach`): wire socket snapping into the viewer first (the snap hook exists, nothing
   fills it), then fire on a part settling into or leaving a named socket frame. Test: a key snaps into a lock and
   opens a door; carrying it away closes it.
3. W3 Named clips and sequences: `anim.track` gets `clip` (a name); `behavior.on` gets `clip`, `speed`, `delay`; a
   clip may `then` start another. Reverses the earlier "not now" because skeleton actions (walk, wave, idle) need it.
   Test: a rigged arm with `wave` and `reach` clips chosen by two buttons; a sequence runs A then B.
4. W4 Instance animation: placed sets carry per-instance phase offsets (`anim.track` on a prototype plays out of
   step by `phase_step`); instance parts may be parents/children only as a set. Test: a field of reeds sways out of step.
5. W5 glTF export of glow, visibility and colour: `KHR_animation_pointer` (emissive strength, baseColorFactor) and
   scale-to-zero for visibility, kept only if the Khronos validator accepts the file; otherwise documented as viewer-only.
6. W6 In-headset panel: a wrist panel (Play/Stop, a "playing" cue, timeline scrub with the ray) drawn in the XR view.

**Batch 2: rigging depth**
7. W7 IK (`rig.ik`): chain of bone parts plus a target part, solved every fixed step. Design: our own deterministic CCD
   solver (only `+ - * /` and `sqrt`, so native == wasm by construction) first; Blender's `iksolver` stays vendored and is
   compared against ours as an oracle on reachable targets, and swapped in only if its output is bit-identical.
   Test: a two-bone arm reaches a moving target; an unreachable target stretches straight at it; joint limits hold.
8. W8 Skinning quality (ADR 0033): DONE 2026-10-17: dual-quaternion blending (`blend="dual_quaternion"`), weight painting
   (`paint` stamps on `rig.skin`), cap raised to 131,072 vertices after measuring, GPU skinning decided against until a
   real frame time asks for it. Open (W8b): shape keys (`rig.morph`).
9. W9 Skinned instance sets (a crowd) after W4.

**Batch 3: the rest of Blender (each is a node family with caps, estimate, cancellation, corpus line, test)**
10. W10 Curves (C1): PARTLY DONE 2026-10-17: `curve.spline` (centripetal Catmull-Rom through points), `curve.helix`,
    `curve.fillet` (rounded corners), `curve.resample`, `curve.reverse`, and `solid.sweep` `scale_end` (taper) and `twist`
    (evidence: `curve_tools` 6 tests, corpus 89 lines native == wasi, Sonnet 3/3 on `curve-tools`). Open: NURBS, trim,
    mesh to curve, `curve_fit_nd` for freehand sketch points, per-point radius.
11. W11 Deformers (C2): DONE 2026-10-17: shear, wave, noise (smooth value noise, seeded), lattice (Bernstein
    free-form deformation with sparse control offsets) join twist/taper/bend on solids; same refinement, orientation and
    fold checks (evidence: 5 map unit tests, `g5_deformers_more` 5 tests, corpus, Sonnet 4/4 on `deform-tools`).
    Refinement passes rise from 2 to 7 for the new maps (triangle cap 100,000). Open: deformers on `poly` meshes.
12. W12 Organic (C3): skin modifier (skeleton graph to quad mesh), metaballs, subdivision smoothing presets.
13. W13 Geometry queries (C4, C10): PARTLY DONE 2026-10-17: `inst.drop` (vertical raycast onto a surface solid,
    bucketed grid, `offset`/`align`/`outside`; evidence: `terrain` tests, corpus 92 lines, Sonnet 3/3 on `terrain-tools`).
    Open: BVH nearest, shrinkwrap, mesh fairing, Poisson distribution on faces, general (non-vertical) raycast.
14. W14 Poly operator backlog (C11): bridge, loop cut, connect, triangulate, beautify, join triangles, weld, spin,
    screw, dual mesh, rotate edges, fill grid/hole, offset loops.
15. W15 Exact booleans (C5) as a second path, compared with Manifold on non-manifold input.
16. W16 Terrain, water, sky (C6, C7): PARTLY DONE 2026-10-17: `terrain.heightfield` (hills/ridged, island edge, lake
    beds; ADR 0034; browser `g10-terrain` shows trees rooted on hills). `water.surface` and `scene.sky` are implemented
    as bounded procedural solids and a per-eye analytic background with a sun and seeded clouds (ADR 0038; Rust and
    parity tests added; water/sky browser pixel checks pass, including an IWER Quest 3 two-eye sky capture; scenery demo shows 6,944 terrain triangles, 12 dropped trees,
    translucent water and a live sky; native/WASI corpus matches at 31 kernel and 100 modeling lines). Sonnet
    `water-tools` passed 3/3. `sky-tools` passed clear-noon; the dusk and night prompts were interrupted by the Claude
    session usage limit before evaluation completed. Transparency for water is supported by
    `solid.material.opacity` (ADR 0036; native/WASI, browser pixel test, glTF validator, MCP prompts 3/3).
    Open: richer physical atmosphere/Nishita fit, ocean FFT, erosion, and headset GPU/visual review.
17. W17 Baking (C8): AO/curvature/UV-space maps from our evaluators; lightmap feasibility spike only.
18. Render quality passes: ACES filmic mapping, viewer exposure/grading and analytic sky reflections already run in the forward renderer. Opt-in geometric specular AA, exposure, SSAO and bloom controls are wired through the viewer (ADR 0040). Current low-tier room-corner browser probe finds a 0-pixel viewport delta when AO or bloom is toggled, despite runtime flags changing; fix the post-pass presentation path before claiming these effects complete. Open: AO corner/open-floor margins, emissive halo, specular-AA stability, stereo seams, per-effect timing and headset timing.
18. W18 Sculpt and remesh (C9): brush math behind our mesh type, quad remesh feasibility.

**Batch 4: physics (Rapier)**
19. W19 Rapier crate wiring (D2): DONE 2026-10-17 (ADR 0035): `ve-physics` crate over Rapier 0.22 (enhanced determinism),
    `phys.body` node (static/dynamic/kinematic; auto/convex/box/sphere/capsule/cylinder/mesh), play-mode world in the viewer,
    pick up and throw, Play button for physics scenes. Evidence: 6 `ve-physics` tests, example corpus native == wasi, `phys_body`
    2 tests, ve-modeling corpus 95 lines native == wasi, browser `g10-physics` (crate falls 358 px and rests at y=0.199,
    held crate hangs in the hand and drops on release, two plays give identical per-second snapshots), Sonnet 3/3 on
    `physics-tools`. Still open here: `phys.joint`.
20. W20 Joints, collide triggers, character controller: PARTLY DONE 2026-10-17 (ADR 0037): `phys.joint` (hinge with limits and
    motor, slider, ball, fixed, spring; contacts between joined bodies off; 8 solver iterations) and `behavior.on`
    `trigger="collide"` (optionally `accepts` one part). Evidence: 5 joint tests in `ve-physics` (pendulum, door limits, motor,
    slider, weld, spring, a 10 s weighted pendulum stays in plane), example corpus native == wasi, ve-modeling corpus 99 lines
    native == wasi, `phys_body` 3 tests, browser `g10-demos` (30-body mechanisms demo: pendulum in plane, windmill turns,
    plank bridge holds a ball, flag ring set by the bell), Sonnet 4/4 on `joints-tools`. Open: joints from assembly sockets,
    a grab that drives joints, scene gravity, a character controller, sensors.
21. W21 (only if needed) Bullet for soft bodies as a separate node family.

**Batch 5: evidence**
22. W22 Stress evaluation of 40 to 60 fresh prompts (typed nodes vs generated code), real-GPU and headset frame
    times, brotli sizes, then the owner's strict gate.

Order of execution (W6, the headset panel, moved after W7 and W8 because IK and skinning depth matter more and the panel
needs a real headset to judge): W1, W5, W3, W2, W4 (all done), W7, W8, W9, W6, then Batch 3 in the order W10, W11, W13, W14, W12, W15,
W16, W17, W18, then Batch 4, then Batch 5. Anything blocked by an owner decision is skipped and listed, not guessed.

Status of each W item is tracked in the sections below (A for animation, B for rigging, C for Blender, D for physics, E
for evidence); a W item is ticked when its row there is ticked.

## A. Animation core (ours, rigid parts)

- [x] A1 Keyframed tracks `anim.track` (position, rotation, scale, pivot, loop, linear/smooth/step). ADR 0026.
- [x] A2 glTF export of tracks (validated, last-rotation check). `g10-animation`.
- [x] A3 Behaviours: click and proximity start a part's clip; fixed 1/60 s step; play mode vs edit mode; Play
      button. ADR 0027. `g10-behavior`.
- [x] A4 Targets and signal listeners: a switch runs another part's clip. `g10-behavior`, `switch-tools` (3/3).
- [x] A5 Parent-child assemblies `anim.parent` (chains to 8, cycle/missing/too-deep warnings, glTF nesting).
      ADR 0028. `g10-hierarchy`.
- [x] A6 Glow tracks (emissive and the light it emits, light follows the part). `g10-hierarchy` lamp.
- [x] A7 `anim.wave` (sine/triangle, written as a looping track), `visible` tracks, `ease_in`/`ease_out`.
      Rust tests; browser: wave bobs 96 px, a visible track hides a block (`g10-hierarchy`). 2026-10-17.
- [x] A8 Corpus lines for hierarchy, glow, wave, visible, easing: native == wasi, 53 lines. 2026-10-17.
- [x] A9 MCP guide + `assembly-tools.json` (robot arm, car wheels, bobbing lantern, windmill): Sonnet 4/4 (the
      windmill fused hub and blades into one solid, accepted). 2026-10-17.
- [x] A10 Animated colour track (`color`, keys `t:r,g,b`): modeling test, browser (red to blue, `g10-hierarchy`),
      corpus line. 2026-10-17.
- [x] A11 Named clips (ADR 0031): `clip` on `anim.track`/`anim.wave`, one running state per clip, poses combine,
      `behavior.on clip=`, a sender picks a clip of its target (`target="arm_out:wave"`), one glTF animation per clip.
      Evidence: `anim_tracks` clip test, `each_clip_is_sampled_at_its_own_time`, `g10-clips` (two clips run separately and
      combine, exported as two animations), `clips-tools` Sonnet 2/2 (run alone). W3, 2026-10-17. Reverses the earlier
      "not now" decision.
- [x] A12 Sequences: `then="b; c"` chains up to 8 clips, each starting when the one before reaches its end running
      forward, and the next click replays it. `g10-clips` (b starts only after a, c after b, replay). Not done:
      backward chaining, clip speed/delay (use timers), blending between clips.
- [x] A13 State and conditions: named flags (`sets`, `needs`, `watch`, trigger `flag`, action `follow`),
      deterministic in the fixed step. ADR 0029. `g10-logic` (locked door + key, lamp follows switch, AND gate),
      `logic-tools` Sonnet 3/3, corpus 58 lines native == wasi. 2026-10-17.
- [~] A14 More triggers. Done: `grab`, `release`, `hold` (play-mode grabbing, flags follow the hand, put back on
      stop; `g10-grab`, `carry-the-key` Sonnet pass) and `timer` (`after`, `every`; fixed-step, pure `timer_due` unit
      test, `g10-logic` door opens by itself after 2 s and a 0.5 s blinker flips its flag both ways; W1, 2026-10-17).
      Sockets (W2, 2026-10-17): `trigger="socket"` with `accepts` and `radius`; a grabbable part let go inside the
      radius snaps onto the socket and the socket fires like `hold` (sets and clip), picking it out reverses them
      (`g10-socket`: let go 43 px off, it ends 11.5 px from the lock centre by perspective, the flag sets and clears; model
      test, corpus 73 lines native == wasi). Position only, no rotation matching yet. Open: collide (after physics, W20).
- [x] A15 Animation on placed instance sets (W4, 2026-10-17): a set carries its tracks; every copy plays them in its
      own frame; `phase_step` (seconds per copy index) and `phase_jitter` (a fixed integer-mix offset per copy, identical
      in Rust and TypeScript, checked against each other) put a field out of step. glTF: copies of sets up to 256 get
      their own channels (a node with a matrix cannot be animated), larger sets stay static. Evidence: `anim_tracks`
      set test, `jitter_matches_the_typescript_copy_the_exporter_uses`, `g10-field` (five reeds lean alike without a
      phase, fan out -113 to +121 px with one; export valid with 15 channels), `field-tools` Sonnet 2/2, corpus 74 lines
      native == wasi. Only motion tracks apply to copies (not glow/colour/visibility/behaviours).
- [x] A16 Playback UI (page): timeline with play/pause, scrubber, speed, current time, for shared-clock
      animation (`viewer_animation_speed`, `clock` in `viewer_behaviors`). `g10-hierarchy`. 2026-10-17.
- [~] A17 Play mode in the headset: the Y/B button toggles it (`g10-xr-play`, IWER Quest 3); controller trigger
      already clicks. Open: an in-headset panel (Play/Stop and a visible "playing" cue), timeline for the headset.
- [~] A18 glTF export of `glow` (emissive strength), `color` (baseColorFactor) and `visible` (KHR_node_visibility), all
      through KHR_animation_pointer: the Khronos validator reports 0 errors and 0 warnings (`tests/look.test.ts`, 4
      tests; an animated part gets its own material, a twin with the same look is untouched). W5, 2026-10-17.
      Not verified: playback in a third-party viewer. Open: per-clip animations (with W3).
- [x] A19 Audit limits and fuzz: deterministic junk-input tests for the key, flag and node parsers
      (`anim_fuzz`: 5000 flag strings, 400 junk programs; accepted keys stay within 2 to 64). 2026-10-17.

## B. Blender animation pieces (take the algorithms, not the stack)

Design rule: our typed `Clip`, `Skeleton` and `Skin` model on top; Blender's action/NLA/constraint/depsgraph/RNA
stack stays out. New ADR before B2.

- [x] B1 Blender curves for `anim.track`: `interpolation="bezier"` (Blender's automatic clamped F-curve handles, one
      smooth curve through all keys, flat ends) and `back`/`bounce`/`elastic` (BLI_easing). Decision: port the maths
      to Rust and TypeScript and keep Blender as the oracle, instead of vendoring the F-curve evaluator (about 320
      files, 1.3 MB wasm for one function). Oracles: Bezier matches 16 values printed by Blender's own
      `evaluate_fcurve` (< 2e-3), easing matches `BLI_easing` through the kernel (`ve_blender_ease`, 600 samples,
      < 2e-5), TS matches the same Blender table; corpus 62 lines native == wasi. 2026-10-17.
      Not done: user-editable handles (`bezier` uses the automatic ones), per-key interpolation, Blender's
      "continuous acceleration" smoothing mode.
- [x] B2 Skeleton and pose model (ADR 0030): a bone is a part (tracks, parents and behaviours already animate it);
      `rig.bone` records its rest segment; no inverse-bind matrices needed because everything is authored at rest.
      2026-10-17.
- [x] B3 Skinning, linear blending: `rig.skin` nodes and the distance weights (deterministic, integer falloff), the
      shared `deform` function with analytic tests (`rig` unit tests), page transfer, viewer CPU skinning from the bones'
      poses, glTF skin export (validated, armature node when bones are separate roots). Evidence: `rig_skin` (6),
      `skin.test.ts` (2, Khronos validator), `g10-skin` (straight 78x197 px, bent 226x194 px, one piece, top fixed,
      export valid), `rig-tools` Sonnet 3/3 (arm, tail, tentacle), corpus 66 lines native == wasi including the weights.
      2026-10-17. Open: dual-quaternion blending, per-vertex weight painting, skinning of instance sets, skinned
      parts over 65,536 vertices (CPU budget), GPU skinning if frame time needs it.
- [ ] B4 Automatic weights from Blender (`meshlaplacian` heat diffusion) behind the same `rig.skin` node; until then
      distance-based weights (ADR 0030).
- [x] B5 Inverse kinematics (W7, ADR 0032, 2026-10-17): `rig.ik` makes a part the target of a chain of bone parts; our own
      deterministic cyclic coordinate descent in `f64` (`+ - * /` and `sqrt` only), resolved against the bones after
      evaluation (missing/unparented/gapped chains are dropped with warnings). Evidence: 6 native solver tests (reach
      1e-3, rigid bones, out of reach stretches straight, goal on the chain's own line folds it, moved and rotated parents,
      deterministic), 3 evaluator tests, `g10-ik` (arm tip 9 and 15 px from the ball in reach; out of reach it points
      -39.1 deg at a ball at -39.2 deg), `ik-tools` Sonnet 2/2, corpus native == wasi. Blender's `iksolver` remains
      vendored and unbuilt; swap it in behind the same node only if bit-identical. Open: joint limits, pole targets, target
      orientation, chains from other chains' bones.
- [ ] B6 Rig examples a model can author: arm, leg + foot, tail, simple biped, spider; real-model prompts.
- [~] B7 Shape keys / morph targets: `rig.morph@1` blends an equal-topology edited target at a named weight, chains up to
      16 keys, and exports glTF POSITION and NORMAL deltas with source remapping through welded seams and material regions.
      Native Rust and browser builds pass. The `wasm32-unknown-unknown` runtime and evaluator compile, wasm-bindgen emits
      both browser modules, and evaluator morph accessors are present. No headset evaluation has been done yet.
- [ ] B8 Procedural character helpers (skin modifier bodies, see C3) feeding B2 to B4.

## C. Blender modeling still missing

- [ ] C1 Curves: Bezier/NURBS/Catmull-Rom evaluation, curve to mesh with profile (sweep, pipes, rails, rope),
      mesh to curve, resample/trim/fillet, `curve_fit_nd` for freehand points. Nodes `curve.*`.
- [ ] C2 Deformers: lattice, simple deform (bend, twist, taper, stretch), wave, warp.
- [ ] C3 Organic: skin modifier (skeleton graph to quad mesh), metaballs (`mball_tessellate`), subdivision-based
      smoothing presets.
- [ ] C4 Geometry queries: BVH tree and kd-tree raycast/nearest as graph fields, shrinkwrap, mesh fairing.
- [ ] C5 Exact boolean (Blender `mesh_boolean`) as a second path for non-manifold input; compare with Manifold.
- [ ] C6 Terrain and water: noise displacement, ocean FFT (`intern/ocean`), erosion-like passes.
- [ ] C7 Sky: Nishita sky model for procedural skies without HDR images.
- [ ] C8 Baking: AO/curvature/UV-space maps from our evaluators; Cycles lightmap bake feasibility spike (server
      path allowed only if too heavy, per-job, owner decision).
- [ ] C9 Sculpt and remesh: brush math behind our mesh type; quad remesh (`quadriflow`), voxel remesh feasibility.
- [ ] C10 Points and instancing from Blender: Poisson/random distribute on faces, realize instances.
- [ ] C11 Other poly operators in the original plan not yet exposed: bridge loops, loop cut, connect, triangulate,
      beautify, join triangles, weld, spin/screw, dual mesh, rotate edges, fill grid/hole, offset loops.

## D. Physics (Rapier; Bullet only for soft bodies later)

- [x] D1 Spike: Rapier 0.22 native vs `wasm32-wasip1` with `enhanced-determinism`: a 24-box tower for 600 steps
      gives the same hash on both (`497270e1cc8db8d8`, positions sampled every 100 steps); wasm 1.0 MB raw
      (not yet compressed); 600 steps take 22 ms native and 50 ms in wasm. Scratch crate only (not in the repo).
      Still to check: joints, many bodies, a slower machine, browser (wasm32-unknown-unknown) build. 2026-10-17.
- [ ] D2 Thin interface of our own (create body, step, read poses, raycast); typed nodes `phys.body`, `phys.joint`.
- [ ] D3 Joints from the assembly model (hinge, slider, ball, fixed) with limits and motors; grab drives joints.
- [ ] D4 Collide/contact triggers for behaviours (A14).
- [ ] D5 Character controller for walking a scene.
- [ ] D6 (only if needed) Bullet for cloth, ropes and soft bodies as a separate node family.

## E. Quality and evidence

- [ ] E1 Fresh stress evaluation of 40-60 varied prompts (furnished rooms, machines, vehicles, characters, working
      logic), written after the features, scored by measurement; compare typed nodes against generated code
      (the owed G9c comparison).
- [ ] E2 Frame-time measurement of the textured/animated viewer path on a real GPU and on a headset.
- [ ] E3 Browser suite wired into one runnable list (`g10-animation`, `g10-behavior`, `g10-hierarchy`, ...).
- [ ] E4 Size budget: brotli size per Blender component after B and C land.
- [ ] E5 Strict gate run by the owner before any commit or publication.

## Owner items

- [ ] Confirm ADRs 0017 to 0030 and the `engine/xr` goldens.
- [ ] Commit plan for the large uncommitted tree (new vendored trees: blender, eigen, mikktspace, fmt, material-maker).
- [ ] Read the hackathon Terms and Conditions and the submission cutoff.
- [ ] Rotate any pasted keys.
- [ ] Physics choice (Rapier recommended) and CAD kernel choice.

## Suggested order

A7 to A9 (finish what is built), A10, A13 (state), A11 and A12 (clips, sequences), A16 and A17 (UI and headset),
then B1 to B5 (Blender F-curves, skeleton, skinning, IK) with D1 in parallel, then A14/A15 and D2 to D4, then
C1 to C4, C10, C11, then C5 to C9, with E1 after each block.
