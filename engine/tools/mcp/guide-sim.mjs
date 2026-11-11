// The "sim" guide topic: simulations written in SimL and run live by the sim.program node. The recipes are real, tested scenes:
// web/tests/scenes/sim/murmuration.txt (a flock), cape.txt (cloth on a moving bone), cape_game.txt (the cloth per copy of a character).

export const SIM = {
  about: 'simulate anything: cloth, hair, ropes, flocks, swarms, crowds, soft bodies, growth. A program in SimL (state arrays, kernels, a schedule) run live by sim.program; tools sim.status, sim.step, sim.set_param',
  text: `Simulations. sim.program@1(in?, source, library, values, seed, rate, size) runs a program you write, live. There is no cloth node, flock node or hair node:
those are programs. The same text gives the same result on every machine.

A program is lines (or \`;\` separated). In a node's source write newlines as \\n.
  sim name
  param gravity = vec3(0.0, -9.8, 0.0)        a number or vector you can change later (values="gravity=0,-3,0", sim.set_param)
  input mesh cloth                            a mesh given on the node's \`in\` (a poly.grid for a cape, a strip for hair, a line for a rope)
  input bone anchor                           the pose of the part with this output name (in a game: this copy's own part)
  state pos: vec3[cloth]                      an array with one value per element; sizes: a number, a param, or a mesh input (one per vertex)
  state vel: vec3[256]                        types: f32 vec2 vec3 vec4 bool
  kernel init each i in pos { ... }           runs once at the start (a kernel named init), for each element i
  kernel step each i in pos { ... }           runs for every element, every step
  kernel once_a once { ... }                  runs once per step (set up or measure)
  schedule step; repeat 8 { solve }; finish   the kernels to run each step, in order; repeat N for solver passes (1 to 1000)
  output particles pos                        draw a vec3 array as small tetrahedra (size=, up to 4,000), or
  output mesh pos                             move the vertices of the \`in\` mesh (the array is sized by the mesh input)

Inside a kernel: \`let x = ...\`, \`x = ...\`, \`+= -= *= /=\`, \`if c { } else { }\`, \`for k in 0..8 { }\` (constant bounds, at most 4096 turns),
state is read as pos[j] and written ONLY as pos[i] (your own element) or pos[i].y = ...
Values and operators: numbers, true/false, vec2/vec3/vec4 (vec3(1.0) fills all three), swizzles (p.xz, v.y), + - * / %, < <= > >= == !=, && || !.
Functions: abs sign floor ceil fract sqrt exp log sin cos min max pow atan2 clamp mix step smoothstep length distance dot cross normalize
select(cond, a, b), hash(a, b) and rand(a, b) (0 to 1, deterministic from the seed). Builtins: i (element), n (count of the kernel's array), dt, t, step, seed.
Neighbours: for j in neighbors(pos, p, radius) cap 24 { ... } visits the elements within radius of p, in index order, at most cap (cap is required).
Meshes: rest(m, i) where vertex i started; for j, len in adjacent(m, i) { ... } each vertex joined to i by an edge and the edge's rest length.
Bones: bone_point(anchor, p) carries a point from the bone's rest pose by its current pose; bone_vector(anchor, d) a direction.
Colliders: sdf_capsule(p, a, b, r) signed distance to a capsule (negative inside); closest_segment(p, a, b) the nearest point of the segment.
Springs: pull(p, q, rest) is the move that gives the distance between p and q its rest length (add half of it to each end, all of it to a pinned neighbour).
Ready-made: library="bridge" is 41 planks hung between two posts (set cut to a link number with sim.set_param and the bridge snaps there and falls in two
halves: web/tests/scenes/sim/bridge.txt); the same chain text makes ropes, chains and hair strands.

THE ONE RULE: a kernel reads the state as it was when the kernel started (plus its own writes to its own element) and writes only its own element.
So elements never race and the order they run in does not matter. A spring between two particles is therefore written from each end: each particle loops
over its adjacent vertices and moves itself (see the library cloth). A write to another element is refused at run time with the line.

Library: sim.program(in, library="cloth", values="pin_height=1.45; collider_radius=0.2; collider_top=0,1.1,-0.08; collider_bottom=0,0.7,-0.08") is a cape:
points above pin_height ride the bone named anchor; a capsule along the bone named body pushes the cloth out; params gravity damping relax pin_height collider_radius.
Name your bone parts anchor and body (rig.bone) or copy the plain-text program and rename the inputs.

Physics, both ways. \`input bone crate\` reads any part, including one a phys.body moves (play mode): bone_point(crate, p) is where the body carries p now. \`output push crate push\`
(push is a vec3 array) changes the speed of the dynamic body of the part crate by the SUM of the array each step, in metres per second: buoyancy, wind, a magnet,
a spring to somewhere. A program with only a push output still needs an \`output particles\` or \`output mesh\` to be drawn; draw a small probe marker. In a game each copy of a
model has its own body (crate#id) and its own program instance reading and pushing that body; a copy that leaves the game takes its body with it. Recipe:
web/tests/scenes/sim/buoyancy.txt (crates settle half under the water line; lift = 20 * depth fraction * dt gives a bobbing equilibrium with phys.body linear_damping 1.2).

Characters: put the simulation in the model of a game kind (its bones in the kind's hierarchy). Every copy the game spawns runs its OWN instance on its own
bones (a cape per soldier). Check with the world tool: sim.status lists one entry per part and per copy; sim.step {steps} runs them now; the fingerprint field
changes when anything in the state changes, so "did my edit change the simulation" is one comparison.

Errors name the line, the column and the fix: an undefined name (did you mean ...), a type mismatch, a neighbors loop without cap, a mesh program with no \`in\`,
more than 4,000 particles, a kernel that ran over 100,000 statements for one element. Start small (a few particles and one rule), run sim.step, read sim.status.

Recipe, a flock (web/tests/scenes/sim/murmuration.txt): state pos and vel, a flock kernel that loops over neighbours adding cohesion, alignment and separation to
an acceleration, a move kernel, schedule flock; move, output particles pos. Recipe, rain: pos[i].y -= speed * dt, and when it is below 0 put it back at the top using
rand(i, step) for its new x and z.`,
};
