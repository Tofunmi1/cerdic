# AAA gaps and programmable GPU compute (plan, drafted 2026-11-07)

Owner brief: fix what is broken, close the gaps that stop AAA games, and add vork-style programmable GPU work (agents write compute programs; water, VFX, procedural everything).
Rules that hold for every item: all content and tools run in the browser; everything is reachable through the language (nodes, scripts), never a bespoke generator; no pulled models; unsupported features are ours to build; an item is done only when a test passes and the result was looked at in the browser (pictures) or measured. Sizes are S, M, L; there is no schedule pressure, scope is never cut for time.

## Findings this plan rests on (all seen on 2026-11-07)
- Skinned humans placed by the game away from the origin distort: with `blend="dual"` part of the body (right hip and arm in the kit test) is pulled back toward the origin; with `blend="linear"` the body lands about twice as far out with the same band. Bone part poses are correct (every bone copy sat at the spawn point); the fault is in how skin joints or the skin mesh take the placement (`game_models.rs`, `skinning.rs`, `skinning_gpu.rs`, `animation/part_2.rs`). Reproduced with the repo's own `autorig_human` kit, so not scene-specific.
- `scene.sky` with `light_shafts` above 0 (and haze) blacks the sky; `gi=true` did not light an exterior.
- No compute pipelines anywhere in `crates` (render WGSL only: terrain, water, exposure). Particles, fluids (APIC), texture bakes are CPU.
- Limits hit: textures max 1024 and the viewport texture memory fills, 64 copies per boolean array, 32 skinned parts per scene, 100k triangles for a refined deformation, `xform.mirror` keeps the original (a footgun).
- No interior test exists in `web/tests`; the studio room hides only when a game script drives the camera; the static viewer does not play clips or show particles.

## P0: fix what is broken (first)
| # | Item | Size | Done when |
|---|---|---|---|
| 0.1 | Characters without the skinned-placement bug (owner decision 2026-11-07: content comes from the language and models, Rust is kept for GPU compute) | M | DONE for the heroine: `web/tests/scenes/characters/hero_cut.txt` + `hero_armour.txt` cut the real `body.human` (via `poly.to_solid` and boolean boxes at the joints) into 15 rigid segments with joint-cover spheres, her clips ported with explicit pivots, hair, armour, sword and cape as rigid parts; spawned at x=3 in play she is undistorted (pictures reviewed). Still to do: front and clip pictures of every clip, the boss the same way, a segment-cut macro in a character library. The skinned-placement bug itself stays a known engine bug (both blends, reproduced with the autorig kit), unfixed by decision. |
| 0.2 | `light_shafts` black sky, exterior GI | M | Sky renders with shafts on; a window-shaft scene shows visible shafts; an exterior lit only by `gi=true` is lit. |
| 0.3 | Limits: texture size 2048 and 4096 with per-part memory accounting and streaming of unused sizes; boolean arrays above 64 batched inside the kernel; noise deformation budget by region not whole solid; skinned parts per scene raised or merged automatically | L | Each limit has a test and the `limits` guide page is regenerated from code. |
| 0.4 | Interior test: the Greek hall as a permanent scene with `web/tests/interior.mjs` | S | Script checks lit floor under a skylight, camera inside walls, nav on the floor, triangle budget, eye review. |
| 0.5 | Language footguns: `xform.mirror` gets `keep="both"|"mirror"|"right"`; macro messages for arithmetic inside strings and for `output` of a call; `help_field` mentions them | S | Tests for each message; the shotgun/AK/Emberfall files lose their workarounds. |
| 0.6 | Harness: clip playback and particles in the static viewer, a `play` helper that starts a world and a camera path, a pose-picture script | M | `show-scene` shows animated parts at a given time and particles; `web/tests/README` lists the harness. |

## P1: characters and animation (after 0.1)
1. Rebuild Emberfall's heroine and boss on the fixed placement (hero.txt and boss.txt exist, clean wiring): clips checked by pose pictures, hair that does not cover the eye, cloth cape with `cloth.sim`. (M)
2. Face: eye, brow and mouth morph targets from the body pack (blend shapes through `body.human` sliders or a morph node), blinking and talking drive. (L)
3. Hair: cards or ribbon hair with a shader, skinned, with `rig.secondary` physics. (L)
4. Foot planting and slope IK in play (`rig.ik`, `rig.gait` on skinned humans), additive layers and aim offsets in `anim.machine`. (L)
5. Ragdoll and hit reactions on a skinned human through `phys.ragdoll`. (M)
6. The fight: finish the cathedral set, effects and the `bossfight` library from the Wukong scripts. (L)

## P2: programmable GPU compute (the vork-style work)
Architecture: a `gpu.program@1` node (WGSL source, typed uniforms, named buffers with element type and count, a dispatch size, an order among passes). Its outputs bind to one of: a particle buffer drawn as instanced sprites or ribbons, a storage texture used by `solid.texture`, a height field used by water and terrain, or a vertex buffer. WebGPU only; WebGL2 keeps the CPU path with a lower cap (the quality tier decides). GPU output never feeds gameplay (the rule already stated for particles). Programs are validated at evaluation time with naga (in the evaluator worker) and errors carry source line, column and a suggestion. Determinism: fixed step, seeded hash, no wall-clock; a CPU reference exists for small counts so tests can compare.
| # | Item | Size | Done when |
|---|---|---|---|
| G1 | Spike: a compute pass in Bevy's render graph updates a particle buffer (1M particles) drawn as instanced sprites in the browser | L | Frame time measured on desktop WebGPU; the orrery plasma runs at 1M particles. |
| G2 | The node: spec, typed inputs, buffer declarations, validation errors with spans, limits (buffers, workgroups, memory), caps and estimate | L | Operator manifest, guide page, cases.md entries, tests for every refusal. |
| G3 | Texture and height-field outputs (star surface, nebula, storm sea, wave field) | M | A compute texture shows on a part and animates; export keeps the baked CPU fallback. |
| G4 | GPU liquid: port the APIC solver (grid transfer, pressure solve) to compute, surface reconstruction (screen-space or GPU mesh), foam and spray | L | Bowl-of-water scene (the owner's picture) at 100k+ particles with a rim and a spill; compared against the CPU solver on a small case. |
| G5 | Orrery v2: GPU plasma, star, sea and nebula; water demo scene | M | Both scenes in the gallery with pictures. |
| G6 | Make it LLM-friendly: program templates (particles, advection, SPH, reaction-diffusion, ocean FFT), a `help_field` entry, a limits page, an error-feedback loop test with a fresh model | M | A fresh model writes a working compute effect from the guide alone. |

## P2.0: the per-character-copy simulation contract (foundation for G1 to G6; decided 2026-11-07)
Finding: six systems read bone poses by global part name (CPU skin, GPU skin, live cloth, fluids, ragdoll, IK/gait/secondary chains) and `game_models.rs` teaches each, one by one, to work on a game entity's renamed copy (`name#id`). Skins and chains are done, live cloth was added 2026-11-07 (built, one open issue: the original un-placed cloth is not hidden), physics bodies and joints and fluids are not. The physics world is built once when play starts, so a copy spawned later has no bodies.
Contract: every simulation family (physics bodies and joints, cloth, secondary chains, particles, later GPU programs) is copied per game entity with its own state, its inputs bound through the copy's own names (bone poses, colliders), and its outputs (mesh points, poses, particles) written to the copy; the game wires this once.
| # | Step | Done when |
|---|---|---|
| C1 | `ve_physics::World::remove` (body, collider, joints, springs; ids of other bodies stay) | DONE 2026-11-07, test `a_removed_body_leaves_with_its_joints_and_the_other_ids_stay`, 16 world tests pass. |
| C2 | The runtime physics adds bodies and joints for a copy when it spawns and removes them when it leaves (the world is no longer built only once) | DONE 2026-11-07 (`physics/part_5.rs`; `buoyancy.txt`: three copies, remove one, the rest continue). |
| C3 | `game_models` copies `Body`, joints (their `to` renamed to the copy) and ragdoll rigs per entity | DONE for bodies and joints (`Body::copy_for`); ragdoll rigs not copied yet. |
| C4 | Kinematic bodies follow the copy's animated bone each step (anchor for hair, ropes, capes made of linked bodies) | DONE 2026-11-07 and browser-checked (`web/tests/scenes/sim/kinematic.txt`: a paddle on a keyframed track carries a kinematic body and shoves a ball 6 m). Finding fixed on the way: the evaluator dropped any `phys.body` on a part with tracks or a parent (`phys.body_dropped`); a kinematic body now keeps them (dynamic and static bodies on animated parts are still dropped; test in `ve-modeling/tests/phys_body.rs`). |
| C5 | Cloth: hide the originals, one solver per copy (code written 2026-11-07), pictures | A placed character's cape drapes and moves. |
| C6 | Fluids and particle emitters through the same contract | A splash on a placed character's hand. |
| C7 | MCP: `sim_status` (bodies, joints, cloths, particles per entity), `sim_step` (advance N seconds and report), pose and contact queries | A model builds a bridge with ropes and cloth through tools only and reads its state. |

## P3: rendering
Probe or traced global illumination that lights exteriors and interiors, decals (`fx.decal`), SSR and volumetric fog on WebGPU with a probe fallback, temporal anti-aliasing and upscaling, occlusion culling, character LOD and imposters, texture streaming. Each needs a measured before and after on a fixed scene (`forest_v3` style draw counts and frame time) and an eye review.

## P4: game systems
Behaviour trees or utility AI as nodes, dialogue and quest graphs, a cutscene timeline (camera, animation, audio cues) with an authoring node, real fonts and localisation, audio mixing and spatial quality, input rebinding and accessibility options, profiling overlay, save slots and settings, a shipping and packaging step. Each is a node or library reachable from the language with a demo scene.

## P5: content pipeline and scale
Hot reload of worlds in the studio, the persistent evaluation cache (vork idea), id and depth snapshot channels for tests (vork idea), asset libraries written in the language (props, weapons, characters as macros), batch generation with the eval harness. The goal is hours of content from the language, not demo scenes.

## Engine Rust budget (owner decision 2026-11-07)
New Rust is for GPU compute only (P2). Everything else is done in the language, JavaScript libraries and models: characters (cut bodies), the cathedral and effects, AI, dialogue, UI, cutscenes. The renderer and runtime bugs the language cannot reach (skinned placement, `light_shafts` sky, texture budget) are listed, not fixed, until the owner says otherwise; each has a language-side workaround or none needed yet.

## Order
P0.1 first (it blocks characters). Then in parallel: P0.2 to P0.6, and P2 G1 (independent of characters). P1 follows 0.1; P2 G2 to G6 in order; P3 and P4 as slots open; P5 continues throughout.

## Risks
- G1 touches Bevy's render graph and a bundle rebuild takes about 7 minutes; keep each spike small.
- 0.1 may be two bugs (a stale joint buffer and a doubled placement); the first step is to instrument rather than guess.
- WebGL2 has no compute; effects that need it must say so in their guide text and fall back or refuse cleanly.

## Needs from the owner
Which first demo to prove G1 on (orrery plasma or the water bowl); whether 2048 and 4096 textures are acceptable on the Quest-class profile.
