# Adopting six open-source libraries (noise, animation, terrain, effects, physics, navigation)

Status: plan written 2026-10-31. Nothing implemented yet. Update the status table at the bottom as work lands.

Sources (all MIT or zlib-style; read from shallow clones kept outside the repo, never vendored whole):
FastNoise2, ozz-animation, Terrain3D, Effekseer, Jolt Physics, Recast/Detour.
The aim is the same as every other engine task: an LLM builds anything through tool calls and our node language. Each item below
is only finished when it is reachable from the language, has a recipe or guide entry, and an LLM-driven run uses it.

## Rules that apply to every workstream

- **Port, do not vendor.** We rewrite the algorithms in Rust inside existing crates (few-crates rule: modules, not new crates, unless a
  workstream says otherwise). Each ported file starts with a header naming the source file and its licence. `lib/NOTICE` gets one
  entry per library (copyright line, licence name, what we ported). Jolt is vendored only if its spike (W5) says we need its code compiled.
- **Deterministic.** Gameplay-visible results use integer hashes and `+ - * /` with `sqrt` in `f64`/`f32`, no platform libm trig, no
  hash-map iteration order. Visual-only GPU code may differ across devices; it must never feed back into game logic.
- **Tests first, never weakened.** Analytic or property tests written before the port; when a test proves wrong, fix it and say so.
  Golden values come from our own implementation after an independent check (range, symmetry, continuity, known identities), not from
  the reference code's output, so we test the maths and not a copy.
- **Language first.** Each feature gets node(s) in `ve-graph`, near-miss forgiveness, a `get_guide` entry, and where useful a recipe.
- **Everything procedural, in the browser.** No imported assets. WASM and WebGL2 must keep working; WebGPU paths need a WebGL2 fallback.
- **Process.** Codex jobs at medium effort, one crate per job at a time, handoff file `docs/handoff-codex-<name>.md`. I run browser tests.
  Nothing is committed or staged until the owner says so.

## W1. Noise (FastNoise2) — start here

Today: `ve-geometry/src/field/noise.rs` (3D smoothstep value noise, fractal and ridged) and `ve-world/src/terrain/generate.rs` (2D gradient
noise with 8 directions, one warp). Two unrelated implementations.

Build one noise module used by both (put it in the lowest crate both depend on; check the dependency direction first):

- Basis: value, Perlin (improved gradients, 2D/3D/4D), OpenSimplex2 and OpenSimplex2S, cellular (distance function euclidean, manhattan,
  hybrid; return value, distance, second distance, difference, lookup).
- Fractals over any basis: fBm, ridged, billow, with lacunarity, gain, weighted strength, octave count.
- Domain warp: gradient, simplex, fractal progressive and independent.
- Combinators: blend, add, multiply, min/max (smooth), remap, domain scale/offset/rotate, seed offset, terrace.
- Fourth dimension is time, so water, clouds and fire can animate.
- Compatibility: the current noise stays selectable (`kind=value_legacy`) and remains the default, so existing worlds do not change.
- Graph: extend the field `noise` node and `terrain` noise with `kind`, `fractal`, `warp`, `cell_return`; a `noise.sample` field for scatter.
- GPU: `noise.wgsl` with the same algorithms (hash by integer ops) included in terrain, water and cloud shaders; naga validation test;
  CPU/GPU parity sampled by a native wgpu test only if the machine allows, otherwise by comparing against a CPU reimplementation of the WGSL integer hash.
- Acceptance: property tests (range, determinism, continuity, zero mean, no axis artefacts), terrain with each kind rendered and looked at, an
  LLM run that asks for "rolling hills with ridged mountains and a cellular crater field" and needs no more calls than today.

## W2. Animation (ozz-animation)

Today: `ve-animstate` (state machine, weights), `ve-modeling/src/ik.rs` (cyclic coordinate descent, no pole, no limits), dual-quaternion
skinning. Missing: two-bone IK, aim IK, per-joint masks, additive layers, animation events, clip compression.

- Two-bone IK (analytic): start, mid, end, target, pole vector, twist, soften, weight. Replaces CCD for 3-joint limbs; `rig.ik solver=two_bone`.
- Aim IK: forward and up vectors, offset, pole, twist, weight (heads, turrets, eyes).
- CCD gains pole, per-joint angle limits and weight, for tentacles and long chains.
- Blending: per-joint weight masks (`anim.mask`: upper body, legs, by bone name or subtree), additive clips with a reference pose (`anim.additive`),
  layer weights; matches ozz `BlendingJob`. Hooked into the existing machine so a recipe can mix walk with an upper-body wave.
- Events: track triggering with correct wrap-around and edge detection (`anim.event name time`), delivered to the game as world events
  (footsteps, hit frames, spawn points).
- Offline: keyframe decimation with hierarchical tolerance (distance measured at a child joint) for generated clips; motion extraction
  (root motion from a clip) later.
- Tests: analytic reach and elbow plane, pole flip stability, aim convergence, mask sums, additive identity, event firing across the loop boundary.
- Acceptance: pegasus and a biped reach for a target with correct knees and elbows, an upper-body-only action over a walk, footstep events driving sound.

## W3. Terrain look (Terrain3D techniques)

Today: `forward_terrain.wgsl` / `terrain.wesl` blend four procedural layers by slope and height with drift and stochastic sampling.
Two copies of `TerrainParams` (ve-render and ve-bevy) must stay in sync; `ve-bevy/tests/terrain_shader.rs` guards the Bevy side.

- Macro variation: two low-frequency noise tints multiplied into albedo, strongest on flat ground, so far terrain stops repeating.
- Dual scaling: second texture/noise scale blended in with distance.
- Height-aware layer blending: layer weights sharpened by a per-layer procedural height so rock shows through grass, sand fills hollows.
- Steep-slope projection: past 45 degrees sample on a wall-aligned plane (no stretched streaks on cliffs).
- Auto-shader parameters named like Terrain3D's (`slope`, `height_reduction`, `base`, `overlay`) so the LLM has few knobs.
- Geomorphing for the quadtree LOD (`docs/plans/terrain-lod-streaming.md`) and per-layer displacement as an option.
- Foliage LOD: grass/tree impostor switch tied to the same camera bands (octahedral impostors already exist).
- Tests: shader composes and validates (extend `terrain_shader.rs`), new params keep both struct copies identical, look.mjs captures at ground height
  (cliff, meadow, far view) compared by eye before and after.

## W4. Effects system (Effekseer design, replaces `ve-particles`)

Today: `ve-particles`, 1,100 lines of CPU billboard simulation (five spawn shapes, one ground plane, turbulence). A toy.

Design (Effekseer's node tree, with our fields as the force source):

- **Effect = tree of nodes.** Each node: spawn (rate, bursts, delay, duration, loop), lifetime, transform channels (position, rotation, scale) as fixed,
  random range, easing, curve or path, colour and alpha over life, UV animation (flipbook, scroll), kill rules, LOD, and a renderer:
  sprite, ribbon/trail, ring, mesh instances, track.
- **Forces:** force, wind, vortex, turbulence (our curl noise from W1), drag, gravity, attractor, and a field force that samples any `field` node (SDF push).
- **Collision:** against heightfield terrain, SDF fields, and the physics world's colliders (bounce, stick, kill, spawn child).
- **Sub-emitters:** on birth, death, collision.
- **Simulation:** a deterministic CPU reference (small counts, test oracle, gameplay particles) and a GPU compute path (Bevy/wgpu) for large counts.
  WebGL2 has no compute shaders, so there the CPU path runs with a lower cap; the quality tier decides. GPU output never feeds gameplay.
- **Rendering:** sprite (soft particles with depth fade, lit option, additive/alpha), ribbons built from a per-particle position ring buffer, ring, instanced meshes.
- **Language:** `fx.effect`, `fx.emitter`, `fx.force`, `fx.collide`, `fx.ribbon`, `fx.mesh`, `fx.on` (events), plus recipes: fire, smoke, sparks, dust, rain,
  splash, magic, muzzle flash, explosion. Existing `particles` nodes keep working and compile to the new spec.
- Crate: evolve `ve-particles` in place; rename only if the owner asks.
- Phases: (1) spec, validation and CPU sim with tests; (2) language nodes and recipes; (3) ribbons and trails; (4) GPU compute and instanced render;
  (5) collisions and sub-emitters; (6) soft/lit particles and LOD. Measure particle counts and frame time on desktop and Quest; record in the plan.
- Acceptance: an LLM run builds campfire, torch trail, dust trail behind a vehicle, and water splash using only recipes plus a few parameters.

## W5. Physics (Jolt features, on Rapier first)

Today: `ve-physics` wraps Rapier behind our interface (ADR 0035). Five joint kinds, a ground-probing walker, a contact list. Ray casts exist only
inside the walker; games cannot cast rays, shapes, or query overlaps.

- Queries: ray cast, shape cast (sphere, capsule, box), overlap tests, exposed in the world API and the graph (`physics.raycast`, `physics.overlap`)
  with layers and filters. Needed for aiming, line of sight, vehicle wheels, effects collisions.
- Joint limits and motors (hinge, slider, ball cone) where Rapier supports them; check and test.
- Ragdoll: built from `rig.bone` chains (capsule per bone, cone/hinge limits from Jolt's ragdoll settings), blend from animation to ragdoll and back.
- Vehicles: Jolt's model written on Rapier forces (raycast wheels, suspension, tyre friction curves, engine torque curve, transmission, differentials, anti-roll bars);
  `vehicle` node with wheel list and a `drive` input. Tracked vehicles later.
- Character: virtual character controller (step up, slope limits, moving platforms, push) improving the walker.
- Spike (gate, measured, no commitment): compile Jolt with our C++ kernel to wasm32, record size and determinism against Rapier on a fixed scene.
  Adopt it only if Rapier cannot carry vehicles, ragdolls and soft bodies acceptably.
- Soft bodies and cloth: after the spike decision.
- Acceptance: a drivable car on terrain, a ragdoll collapse, a raycast-aimed shot, all authored through nodes.

## W6. Navigation (Recast/Detour)

Today: `ve-nav` single-level walkable grid, A*, funnel, ORCA-lite crowd, dynamic overlay. Its own doc names multi-level as the next step.

- Stage A, spans: replace per-cell lists with Recast-style spans (heightfield rasterisation, walkable filters for ledge, low clearance, step), region
  partitioning and layer extraction, so stairs and upper floors work. Public `NavMesh` API stays stable.
- Stage B, polygons (optional): contours, polygon mesh, off-mesh links (jumps, ladders, doors) replacing the grid path graph where it pays off.
- Stage C, dynamic: tile cache with obstacle add/remove rebuilding only touched tiles; connects to the existing `DynamicSpace`.
- Stage D, crowd: path corridor and sampled-velocity obstacle avoidance (Detour) in place of ORCA-lite; proximity grid for neighbours.
- Tests: two-storey building path via stairs, blocked door via obstacle then reopened, agent clearance under a low ceiling, determinism across runs.
- Acceptance: NPCs walk through a generated multi-floor building and avoid each other and a moved obstacle.

## Order and dependencies

1. W1 noise (small, everything else uses it: terrain, water, clouds, effects turbulence).
2. W2 animation (independent; parallel with W1 because different crates).
3. W3 terrain look (after W1 gives noise in shaders).
4. W4 effects (needs W1 curl noise; biggest; phases above).
5. W5 physics queries and ragdoll, then vehicles; Jolt spike last.
6. W6 navigation spans when the city/building work needs upper floors; sooner if a game demands it.

Parallel Codex jobs only across different crates (`ve-geometry`/`ve-world`, `ve-animstate`/`ve-modeling`, `ve-bevy`/`ve-render`, `ve-particles`, `ve-physics`, `ve-nav`).

## Open decisions for the owner

- Vendoring: default is port-only with NOTICE entries; say if you want the reference trees copied into `lib/` (about 400 MB for these six; disk is tight).
- Rename `ve-particles` to `ve-fx`, or keep the name.
- GPU effects on WebGL2: accept CPU fallback with lower caps (proposed), or require WebGPU.
- Run the Jolt spike now or after W4.

## Status

| Workstream | State |
|---|---|
| W1 noise | 2026-10-31: `ve_world::noise` done (value, Perlin, simplex, cellular; fBm/ridged/billow; gradient/simplex warp; 2D-4D; 19 property tests) and `terrain.tile basis=` wired (classic default keeps existing worlds; 4 tests + 1 language test). Simplex is one classic-simplex basis; OpenSimplex2/2S and SuperSimplex were NOT ported. Pending: WGSL noise (do when W3/W4 need it), field `noise` node kinds (the C++ field tape must match, so a C++ port is needed), `noise.sample` for scatter, guide entry. |
| W2 animation | 2026-10-31: `ve_modeling::ik_limb` (two-bone analytic with pole/twist/soften/weight; aim) + `rig.ik solver="two_bone"` + new `rig.aim` node, wired to the runtime and checked live (worlds `two_bone_arm.txt`, `aim_head.txt`); 16+5 tests. Deviations: soften uses a rational curve (ozz's quartic overshoots the target); blends use nlerp. 2026-10-31 (later): overlay LAYERS and clip EVENTS on the state machine: program statements `layer NAME clip= weight=PARAM|N mask=a,b additive once speed= fade=` and `event CLIP NAME at=SECONDS` (`ve-animstate`: `crossed`, `apply_additive`, `apply_override`, layer clocks and smoothed weights, events gated by state weight; 10 tests), the runtime applies masked overlays per part (`animation/machine.rs::layered_pose`) and reports events to scripts as `anim.<name>` (`world_events_since`); checked live (`tools/animation-layers-live-test.mjs`, world `layers_test.txt`: footsteps counted, wave takes the arm over and lets go; the ADDITIVE mode is covered by the native maths tests only). 2026-11-04: CCD per-joint turn limit (`rig.ik limit=`, `ik::solve_limited`, 4 tests), keyframe decimation (`anim.simplify`: tolerance measured at the bone's tip, 6 tests), motion measurement (`value.motion`: travel, path, duration, speed, lift of a clip, 4 tests) and animation events as game-rule triggers (`on="host:anim.<name>"`, live check `web/tests/e6-anim-events.mjs`) are done. Pending: CCD pole, a live check of the ADDITIVE layer mode, root motion that moves the entity (only measured so far). |
| W3 terrain look | 2026-10-31: shared `forward_terrain.wgsl` gets macro variation (two slow tints, scaled by the existing `variation` and by flatness) and noise-broken rock/grass transitions (also scaled by `variation`, so 0 keeps the crisp old look); Bevy `terrain.wesl` fades fine relief with distance (60 to 200 m) and skips its six extra lookups beyond it. Checked by eye on `worlds/terrain_look.txt` (mid and far views) and the shader compose/validate tests. Not needed: Terrain3D's steep-slope projection (our detail noise is 3D, so cliffs do not stretch). Not done: geomorphed LOD seams, per-layer displacement, a distance-based second detail scale. |
| W4 effects | 2026-10-31, phases 1-3 built, live check pending: `ve_particles::fx` (CPU reference sim: spawn clocks with delay/duration/loop/bursts, forces gravity/wind/drag/vortex/attractor/curl-noise turbulence, collisions against a `World` trait (bounce/stick/kill), sub-emitters on birth/death/collide, trails, placement, caps, validation; 18 tests) + `fx::draw` (soft sprites by vertex colour, depth-sorted for alpha; ribbons; 5 tests) + language `fx.emitter/force/collide/trail/sub` (8 tests; effect rides on the part) + runtime/bridge/`effects.rs` + `SurfaceFinish::particle` (Bevy path only) + guide topic `effects`. 2026-10-31 (later), all checked live: LIT CUBE particles (`render="cube"`, `spin`; `draw::cubes`, normals on every batch), particles COLLIDE WITH THE SCENE'S PHYSICS BODIES in play mode as well as the ground plane (`ve_physics::World::nearest_surface`, 5 tests; `tools/fx-physics-live-test.mjs`: chips rest at 1.03 m on a 1 m crate, fall to the plane when physics is off), and a `fx.status` world call (alive counts and the box they fill). Not done: GPU compute path (big counts still run on the CPU), field/SDF forces and collisions (only planes and physics bodies), soft-particle depth fade, the old `ve-particles` showcase still separate. |
| W5 physics | 2026-10-31. Correction to the plan: `ve-physics` ALREADY had ray cast, shape cast, sphere overlap, sensors and hinge/slider limits and motors (`world/query.rs`, `joint.rs`); they are just not reachable from the runtime world API or the language. Built natively in `ve-physics`: ball-joint swing/twist limits (`JointDesc.swing/twist`), `ragdoll::build` (capsule bones, ball/hinge joints with limits, self-collision off; 7 tests incl. settling, no tearing, limits hold, determinism), `World::raycast_excluding/body_mass/point_velocity/apply_impulse_at`, sleeping bodies report zero velocity. Vehicles (`vehicle.rs`, `tests/vehicle.rs`): drive, brake, steer, land and determinism tests pass; the rest-height test still fails because `World::velocity` reads a steady -0.07 m/s while the pose does not move (cause not found; first suspect is how Rapier orders velocity and position in a step versus an impulse applied before it). PARKED by the owner: vehicles come last. WIRED 2026-10-31 and checked live (`tools/ragdoll-live-test.mjs`, world `ragdoll_test.txt`): world API `physics.raycast`, `physics.overlap`, `physics.ragdoll` (play mode only) and the `phys.ragdoll` node (resolved against `rig.bone` segments and `anim.parent`; the runtime builds the bodies from the character's CURRENT animated pose, drives the bone parts from physics in the animation pass, and stops on Stop). A scene with a ragdoll now counts as playable in the page. Not done: a game-rule effect kind (`effect:ragdoll@a`) so a creature's death can trigger it from rules, ragdoll-to-animation blend back, Jolt spike. |
| W6 navigation | not started |
