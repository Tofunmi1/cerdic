# Simulation programs: simulate anything from the language (spec draft, 2026-11-07)

Status (2026-11-07): owner decisions taken (section 9). M0 is built and tested: SimL (`crates/ve-world/src/sim*`: lexer, typed parser, CPU interpreter, 10 tests), the `sim.program@1` node (ve-graph spec, `ve-modeling/src/sim_program.rs`, 3 tests), the runtime host (`ve-runtime/src/sim_live.rs`, per-instance state with `Shared::copy`), the TypeScript bridge, and the demo `web/tests/scenes/sim/murmuration.txt`. Browser verification, the spatial hash, the 400-line split of `parser.rs`/`run.rs`, scopes/pose inputs (M1) and the GPU backend are open. Replaces the idea of one bespoke node per simulation (cloth, fluid, secondary chains, particles). Companion to `aaa-gaps-and-gpu-compute.md` (P2).

## 1. Goal and principles
- Any simulation a model can describe (cloth, hair, ropes, soft bodies, fluids, fire, smoke, crowds, flocking, erosion, reaction-diffusion, growth, mass-spring structures, anything with state that evolves by rules) is a *program* written in the language, not an engine feature.
- A program is data: state declarations, kernels, a schedule, bindings. It lives in the document (a node), is written in text or created with typed commands (same document, see `llm-pipeline-and-harness.md`), and is reachable from MCP.
- Deterministic everywhere (fixed step, seeded hash, no clock), cancellable, bounded (state bytes, steps, neighbours), and refused with errors that carry source line, column and a suggestion.
- One source, two executors: a CPU interpreter (the deterministic reference and the WebGL2 fallback) and WebGPU compute (scale). A program never says which; the quality tier and caps decide.
- Per-instance by construction: every simulation belongs to one scope (the world, or one game entity's copy) and resolves its inputs through that scope's names. There is no per-feature work to support placed characters.
- Existing bespoke solvers (XPBD cloth in `ve-world`, APIC and PBF water, `rig.secondary`) become reference implementations and the first library programs; they are retired only after the library version matches them.

## 2. Concepts
- **System**: one `sim.program@1` node: a name, a source text, a step rate, a seed, a domain, caps.
- **State**: named typed arrays declared by the program: `pos: vec3[N]`, `cell: f32[X*Y*Z]`, `alive: bool[N]`. The size is a count or a grid shape (from a parameter or from an input mesh/volume). State persists between steps; `reset` restores the initial values.
- **Kernel**: a function run for each element of one array (`each i in pos`), each edge, each cell, each pair from a neighbour query, or once. It reads and writes state, parameters, inputs and builtins.
- **Schedule**: the order of kernels per step, with `repeat k { ... }` for solver iterations and `if` on parameters. Kernels in one stage see the previous stage's writes (barriers between stages); within a stage, writes to the same element are the program's responsibility (the language offers `atomic_add` for scatters).
- **Inputs**: named bindings supplied by the document or the game: poses of named bones (resolved in the instance scope), distance fields and volumes (colliders, wind, terrain), meshes (rest shape, topology), textures, scalar parameters, other systems' outputs (coupling), the physics bodies of the scope (read their poses, apply impulses).
- **Outputs**: what the state drives, by kind: `mesh` (point positions of a poly, normals recomputed), `particles` (a draw with size, colour and trail from state), `texture` (a storage texture used by `solid.texture`), `field` (a volume for other nodes), `poses` (bone matrices for anim.parent and skins), `impulses` (back into physics), `events` (counters and flags a game script can read).
- **Scope**: world, or entity copy `name#id`; decides the state instance and how input names resolve.

## 3. The kernel language (SimL)
SimL is the field-expression language (`field.expr`: vec2/vec3/vec4, swizzles, `+ - * / %`, comparisons, `select`, `abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length distance dot cross normalize`, `ramp`, `perlin fbm ridged voronoi`) extended with statements and state access. It stays small enough for a model to hold in its head and to compile to both executors.
- **Types**: `f32 i32 bool vec2 vec3 vec4 mat3 mat4 quat`; fixed-size arrays only; no recursion, no dynamic allocation, no pointers.
- **Declarations**: `state name: type[count]`, `param name = value` (settable at run time), `input kind name`, `const`.
- **Statements**: `let`, assignment, compound assignment, `if/else`, bounded `for k in 0..K` (K a constant), `each` kernel headers, `repeat k { stages }`.
- **Builtins**: `i` (element), `n` (count), `dt`, `t`, `step`, `seed`, `hash(i, k)` and `rand(i, k)` (deterministic, counter-based, same on CPU and GPU), `neighbors(point, radius)` loop (uniform-grid hash rebuilt per stage, a per-kernel cap on visited neighbours), `field(name).dist(p)` and `.grad(p)`, `grid(name).sample(p)`, `pose(bone)` (a mat4), `edges` and `tris` of an input mesh, `atomic_add(array[i], v)`, reductions `sum min max` over an array.
- **Constraint helpers** (sugar over plain kernels, so a model can also write them by hand): `distance(a, b, rest, compliance)`, `bend(a, b, c, d, compliance)`, `pin(i, target)`, `collide_sdf(i, field, thickness, friction)`, `volume(tet, rest, compliance)`.
- **Errors**: a read of an undeclared name, a kernel that writes state it did not declare, a loop without a constant bound, a neighbour loop without a cap, a binding that does not resolve in the scope: all refused at evaluation with line, column and the fix.

## 4. Example kernels (illustrative syntax, not final)
Cloth (position-based): predict, then iterate constraints and collisions, then finalise velocity.
```
sim cape
param gravity = vec3(0, -9.8, 0), damping = 0.99, compliance = 0.0
input mesh rest            # rest shape and topology (edges, pins by vertex group)
input field body           # distance field of the character, in the scope
input poses chest          # bone matrix the pins ride
state pos: vec3[rest.points], prev: vec3[rest.points], vel: vec3[rest.points]
kernel predict   each i in pos     { vel[i] = (vel[i] + gravity * dt) * damping; prev[i] = pos[i]; pos[i] += vel[i] * dt }
kernel pins      each i in pos     { if rest.pinned[i] { pos[i] = pose(chest) * rest.pos[i] } }
kernel stretch   each e in rest.edges { distance(pos[e.a], pos[e.b], rest.length[e], compliance) }
kernel collide   each i in pos     { collide_sdf(i, body, 0.01, 0.5) }
kernel finish    each i in pos     { vel[i] = (pos[i] - prev[i]) / dt }
schedule predict; repeat 6 { pins; stretch; collide }; finish
output mesh rest.points = pos
```
Flocking: `each i in pos { for j in neighbors(pos[i], 2.0) cap 24 { ... separation, alignment, cohesion ... } }`, output `particles`. Fluid: particle-grid transfer with `atomic_add` into a `grid` state, a pressure solve `repeat`, output `particles` and a `field` for a surface. Fire: advected scalar grid plus emitters, output `texture`.

## 5. Document and tool surface
- Node: `sim.program@1(source, seed, rate, substeps, domain, caps)` with variadic named inputs (`in.<name>`) and named outputs; typed in the registry like any operator, so the text language, `apply_commands`, the generated TypeScript API and `help_field` all see it.
- A simulation library of programs (cloth, hair strand, rope, soft body, SPH, APIC, smoke, flock, sandpile, reaction-diffusion) ships as templates and as macros (groups), and as `start_from` templates.
- MCP (see the hybrid plan): `sim_status` (programs, scopes, state sizes, step counts, backend), `sim_step` (advance N steps or seconds, deterministic), `sim_reset`, `sim_set_param`, `observe` (JSON by path into state or outputs). The `world` and `script` tools are subsumed over time.
- A game script can read outputs (`events`, counts) and set parameters; it cannot write state arrays.

## 6. Execution
- **CPU reference**: SimL compiles to the same tape the field language uses (extended with state loads/stores, loops, neighbour iteration). Single-threaded, fixed element order, f32 with a fixed reduction order; it is the oracle and the fallback (lower caps).
- **GPU**: SimL compiles to WGSL: one compute dispatch per kernel stage, storage buffers per state array with ping-pong where a kernel reads what it writes, a spatial hash built by a sort/scan pass for `neighbors`, workgroup size chosen from the cap table. Same builtins, same `hash`. Runs in WebGPU only; there is no compute in WebGL2.
- **Scheduling**: systems of one scope run in declaration order inside the frame's fixed steps; a system can depend on another's output (coupling) and the dependency order is checked for cycles.
- **Readback rule**: GPU output feeds rendering only; anything a script or the game logic depends on comes from the CPU reference or from declared `events` read back with a one-frame delay. This is the existing rule for particles.
- **Budgets**: state bytes, elements, kernel statements, neighbour cap, steps per frame, dispatches per frame; the estimate is part of evaluation (`check` reports it) and an over-budget program is refused, not slowed.

## 7. Instances and coupling (the contract)
- A system declared under a game kind is instantiated per entity copy; its input names resolve through the copy's name suffix (`chest` becomes `chest#7`), its outputs write to that copy's parts. A system declared in the world is a singleton.
- Instance creation, removal and teleport are lifecycle events the runtime handles once for all systems (state allocated and freed, teleports do not whip the state).
- Physics: a program reads body and bone poses and can apply impulses; rigid bodies and joints of a scope are added and removed with the copy (per-copy physics, step C2 to C4 of the plan). Cloth, hair, ropes and fluids that touch bodies do so through fields and impulses declared as inputs and outputs, not through special cases.

## 8. Delivery (each ends in a scene that is looked at, plus numbers where numbers exist)
- **M0**: parser and validator for SimL, state/kernel/schedule model, CPU interpreter, `sim.program` node with `particles` output. Demo: a swarm and a pendulum chain.
- **M1**: scopes and instances, `poses` and `mesh` outputs, pose and field inputs, lifecycle (spawn, remove, teleport). Demo: a placed character's hair and cape as programs.
- **M2**: neighbour search, constraint helpers, `impulses` to physics. Demo: cloth on a placed character, a rope bridge from linked programs and bodies.
- **M3**: the GPU backend, caps and the quality tier. Demo: a million-particle plasma around the orrery.
- **M4**: grids, `atomic_add`, `texture` and `field` outputs. Demo: GPU water in a bowl (the owner's reference picture), smoke, reaction-diffusion texture.
- **M5**: the library as templates and macros, MCP `sim_*` tools, the old bespoke solvers retired where the library version matches.

## 9. Decisions (owner, 2026-11-07)
1. SimL extends the field-expression language: same number types, swizzles, functions and platform independent maths (`expr::math`), statements added. Taken as recommended.
2. A kernel writes only its own element; scatters go through atomics (M4). Taken as recommended. The CPU reference enforces it at run time, so a parallel executor needs no race handling.
3. CPU bit-identical on every platform, GPU within a tolerance. Taken as recommended.
4. The first GPU demo is not the orrery plasma or the water bowl: it is a murmuration (a large flock with neighbour interaction), which stresses the neighbour search that fluid and cloth do not. The CPU version of the same program ships first (`scenes/sim/murmuration.txt`).
5. Texture and buffer memory policy for the Quest-class tier: dropped, not needed.

## 11. Status after the owner's "do 1 to 5" (2026-11-07)
| Item | State |
|---|---|
| Spatial hash for `neighbors` | DONE. `sim/grid.rs`; same visit order as testing every element; a 4,000-bird flock runs; test compares the grid with a Rust brute-force count. |
| M1 inputs, mesh output, per-copy | DONE and seen in the browser. `input mesh`, `input bone`, `rest`, `adjacent`, `bone_point`/`bone_vector`, `sdf_capsule`/`closest_segment`, `output mesh`; `sim.program` takes an `in` mesh, `library`, `values`; `cape.txt` (cloth on a moving bone with a capsule collider) and `cape_game.txt` (two copies of a character kind, each with its own cloth instance reading `anchor#id`/`body#id`; the prototype does not step). |
| M2 cloth as a library program | DONE: `sim/library.rs` CLOTH (Jacobi position-based, every point gathers its own edges). Tests: hangs from pins, bounded stretch, follows the anchor bone, capsule pushes it out, bit-repeatable. Also DONE (second pass): the constraint helper `pull(p, q, rest)`, the BRIDGE library program (41 planks between two pinned posts; `sim.set_param cut=<link>` snaps it and the halves swing down; test, scene `bridge.txt`, browser-checked through the world API). Physics coupling DONE (third pass, browser-checked with `buoyancy.txt`): `ve_physics::World::push`; the runtime physics world now adds and removes bodies as parts appear and go (`physics/part_5.rs` `sync_bodies`: a removed character takes its body and joints with it, hidden model prototypes never have bodies), kinematic bodies follow their part's transform each step (C4), `game_models` gives every spawned copy its own `Body` (renamed node, part name and joint targets; `Body::copy_for`), SimL reads a physics-moved part through `input bone` and pushes a body with `output push BODY array`. Three crates spawned from one kind fall, are lifted by three program instances (each reading and pushing its own `crate#id`), settle at the water line, and removing one removes only its body. This closes plan steps C2 to C4; the plan table is updated. NOT done: retiring `cloth.sim`/`fluid.sim` (see section 12). |
| M3 GPU backend | MOSTLY. `sim/wgsl.rs` writes one compute shader per kernel with the CPU's snapshot rule (live + snapshot buffers) and now covers mesh inputs (one word buffer per mesh: rest points, CSR neighbours, rest lengths), bone poses (one buffer of 16 f32s per input), `rest`, `adjacent`, `bone_point`/`bone_vector`; every shader validates in naga; `web/tests/sim-gpu.mjs` runs five programs on a real WebGPU device against the CPU reference: rand/hash init bit-identical, shift exact, flock (1,024 birds, neighbour loop) within 4e-5 after 100 steps, bridge within 6e-5, cloth on a mesh with a moving bone within 2e-6 after 240 steps; 100,000 particles for 240 steps in about 90 ms on the Apple GPU. NOT done: a Bevy render-graph pass that draws GPU buffers (the G1 spike; without it the node still draws at most 4,000 tetrahedra or a mesh of 20,000 points, so the GPU executor has no visible product use yet), a GPU neighbour grid (the flock shader tests every element), atomics. |
| MCP tools | DONE through the world API: `sim.status`, `sim.step`, `sim.set_param`, `sim.reset` (per part and per copy, fingerprint of the state), documented in the world guide and a new `sim` guide topic; verified against the two-copy scene. |

## 12. What is left, and how to do it
1. **Draw GPU results (G1).** The GPU executor works (section 11) but nothing draws its buffers, so the node is still capped at 4,000 tetrahedra or a 20,000-point mesh. Design: one render-world resource per GPU simulation holding the storage buffers of `GpuPlan`; a compute node ahead of the main pass that runs the schedule (the `sim-gpu.mjs` runner is the reference for bindings, snapshot copies and per-step clock); a Bevy `Material` whose vertex stage pulls the particle position from the `particles` storage buffer by instance index (a mesh of one tetrahedron drawn `count` times), so no CPU copy; the CPU `Sim` stays the oracle and the WebGL2 fallback. The state of a GPU simulation never feeds gameplay (the existing particle rule), so `sim.status` reports it as `backend: gpu` with counts only. Cost: the Bevy material and render-graph node are the work; every check is a 7 minute bundle rebuild.
2. **GPU neighbour grid and atomics (M4).** The flock shader tests every element, so it is quadratic. A grid needs a counting sort (clear counts, atomic count, prefix sum, scatter) before each kernel that uses `neighbors`, and the CPU's rule (the first `cap` neighbours in index order) needs each cell's list sorted by element number, because atomic scatter order is not stable. WGSL has `atomic<u32>`; the language addition is `atomic_add` on a declared state array for scatters (grids, histograms, fluid transfer). The JS harness compares against the CPU, so this can be built and checked without a bundle rebuild.
3. **Retire `cloth.sim` and `fluid.sim`.** The library cloth must first match what `cloth.sim` offers: wind and drag, a floor, friction, stretch and bend stiffness, pins by selection and by bone, up to 16 collider capsules by bone name (today the library has one capsule and `pin_height`). Port those as params and kernels, add a parity test per feature against `ve_world::cloth`, switch the evaluator so `cloth.sim` builds a `sim.program` with the library text and its params (the node and its text stay as the front door), then delete `cloth_live.rs` and the cloth fields. Fluids (PBF and APIC) need the grid transfer (item 2) first. The generic live carrier (one field on a part instead of one per feature) comes with this: `sim` already is that field; cloth and fluid move onto it.

## 10. Built in M0 (what exists, what it taught)
- Language: `sim`, `param`, `state name: type[count|param]`, `kernel name each i in arr {}` / `once {}`, `schedule a; repeat 6 { b; c }`, `output particles|values arr`; `let`, assignment and compound assignment, `if/else if/else`, `for k in a..b` (constant bounds, 4096 turns), `for j in neighbors(arr, centre, radius) cap N`; `hash`, `rand`, the shared functions. Every refusal has line, column and a fix; names get "did you mean" by edit distance.
- Semantics: a kernel reads a snapshot of the arrays it writes, plus its own element; a kernel that writes another element is refused at run time. Test `a_kernel_reads_the_state_as_it_was_when_it_started` pins this (an in-place order would give a different answer).
- Budgets: 1,048,576 elements per array, 64 MiB state, 100,000 statements per element per kernel, 64 KiB source; the node draws at most 4,000 particles.
- Found: neighbour search is brute force O(n^2) in the CPU reference, so flocks are held to about 300 birds until the spatial hash (visit order stays index order, so results do not change).
- Found: a text-language string cannot hold a raw newline, so a program in a node is written with `\n` escapes or `;` separators (both parse). A multi-line way to write long sources (a block string or a file reference) is wanted.
- Found: each live thing (cloth, fluid, now sim) is a field threaded through about 20 files of evaluator and runtime. The substrate should replace that with one generic `live` carrier; cloth and fluid then move onto it.
