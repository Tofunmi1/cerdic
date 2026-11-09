// What the MCP server tells a model about the language, in layers so a model reads only what it needs.
//
//   get_guide()                short core: the syntax, the workflow, and the list of topics (about 3 KB)
//   get_guide(topic="physics") one topic with its examples, taken from the full guide text
//   list_operators()           the families and the operator names in each (about 2 KB)
//   list_operators(family=..)  one family: each operator's signature and one line of help
//   describe_operator(id)      every parameter of one operator, with unit, range, default and help
//
// Before this, one get_guide call returned 31 KB and one list_operators call 100 KB, in every session.

/** The topics cut out of the full guide text: where each starts (the first words of its paragraph) and what it covers. */
export const SECTIONS = [
  { topic: 'shapes', start: 'Smooth shapes: a few', about: 'profiles, solid primitives, moving/mirroring/arraying, rounding, kits (a vase, a table)' },
  { topic: 'curves', start: 'Curved paths (initial bounded subset)', about: 'curves, sweeps along a path, lofts between profiles' },
  { topic: 'deformers', start: 'Bounded mesh deformations refine', about: 'twist, taper, bend, shear, wave, noise, lattice' },
  { topic: 'physics', start: 'Physics (play mode):', about: 'bodies, joints, collisions, gravity, a walking viewer' },
  { topic: 'world', start: 'Landscapes: terrain.heightfield', about: 'terrain, placing things on it, water, sky' },
  { topic: 'polygons', start: 'Polygon editing (faces, edges, vertices)', about: 'editing meshes: select, inset, extrude, bevel, subdivide, UVs' },
  { topic: 'reuse', start: 'Groups (build once, use many times)', about: 'groups, rigid alignment, assembly sockets, shared placements (instances)' },
  { topic: 'appearance', start: 'Colour and finish: solid.material', about: 'colour, opacity, procedural textures' },
  { topic: 'animation', start: 'Animation: anim.track@1', about: 'keyframed motion, waves, colour/glow/visible tracks' },
  { topic: 'scripts', start: 'Scripts (behaviour you write)', about: 'behaviour in JavaScript: the script tool, the script.js node, the world tool' },
  { topic: 'rigging', start: 'Bendy things (skinning)', about: 'bones, skinning, inverse kinematics' },
  { topic: 'assemblies', start: 'Assemblies: anim.parent@1', about: 'parts that follow other parts, glow' },
  { topic: 'behaviours', start: 'Behaviours: behavior.on@1', about: 'reacting to clicks and proximity, flags and logic, grabbing, clips, timers, sockets' },
];

import { MATERIALS } from './materials.mjs';
import { COMPACT } from './compact-guide.mjs';

/** Topics written for this guide rather than cut out of the full text. */
export const EXTRA_TOPICS = {
  compact: COMPACT,
  recipes: {
    about: 'start from a working scene and save reusable groups as recipes',
    text: `Start with a working skeleton: call start_from with no arguments to list templates, then start_from(template="rolling-course") to get a complete document. Edit the counts, dimensions, colours, names or rules to make it a different thing. This is the fastest way to a good result. Use apply=true to send a template through apply_text when an editor page is connected.

Recipes are model-written groups saved across server sessions. Save a group once you have built it twice: recipes(action="save", name="crate_stack", text="group crate_stack ... end", about="two crates stacked"). A recipe is one group named exactly like the recipe; expose the values you want to vary. list shows exposed parameter names, show returns the source and a call line to paste into your program, and delete removes it. Paste the shown group source ahead of the program that calls it, like a kit.

Use run for multi-step work and return only the useful result. Build, check, and read the delta in one call:
  const built = await tools.apply_text({text: program});
  const checked = await tools.check({});
  return {built, checked};

Loop over positions, applying one pipe-delimited row at each point:
  for (const [i, p] of positions.entries()) {
    await tools.apply_text({text: "piece_" + i + " = solid.box(size=[0.2, 0.2, 0.2]) | xform.move(by=[" + p.join(", ") + "]) | output piece_" + i});
  }
  return await tools.scene_overview({});

Apply edit operations, read the delta with diff, and return only changed lines:
  const edited = await tools.edit({ops: [{op: "set", node: "wall", params: {size: [4, 2, 0.2]}}]});
  const changed = await tools.diff({since: edited.revision - 1});
  return changed.lines;`
  },
  world_api: {
    about: 'runtime world tool methods and their parameters',
    text: `The world tool reads and changes the running world. Pass method and params; an entity is its id from world.parts or world.find, or an authored part name. Answers arrive on the next frame.

- registry.list {}: list readable components and which are writable.
- world.parts {}: list authored parts by name and entity id.
- world.find {name}: find a part by name.
- world.list_components {entity}: list components on an entity.
- world.get_components {entity, components:[names]}: read selected components; use the returned JSON shape when writing.
- world.insert_components {entity, components:{name:value}}: insert or replace components. Also world.mutate_components has the same parameters. Writes Transform, Visibility and light components. Animation tracks or a parent overwrite a part's Transform each frame, so move static parts.
- world.query {components:[names], has:[names], name, limit}: query entities, returning selected components and optionally filtering by required components or name.
- world.spawn_entity {components, shape, position}: create a visible runtime entity. shape may be {kind:"box", size:[x,y,z]}, {kind:"sphere", radius}, or {kind:"cylinder", radius, height}; shape also accepts color:[r,g,b], emissive, metallic, roughness and opacity.
- world.despawn_entity {entity}: remove an entity spawned by this tool or an authored part.
- world.bounds {entity}: return its world-space {center, size}; Transform is only an offset from where the part was built.
- animation.set_float {part, name, value}, animation.set_bool {part, name, value}, animation.trigger {part, name}: drive an animation state machine.
- water.disturb {part?, x, z, radius?, amount?, splash?}: disturb sim-enabled water under the point; default radius 0.5 m, amount 0.04 m. A positive amount above 0.03 m also emits a splash unless splash is false (default true). Omitted part targets every sim water part under the point.
- water.status {}: return {parts: [{name, resolution, cell, steps, time, max_abs, volume, wet_cells}]} for simulated water. Resolution is the longest grid side, cell is spacing in metres, steps counts fixed steps since construction, time is simulated seconds, max_abs is peak absolute displacement in metres, volume is signed displaced cubic metres, wet_cells counts water cells. Empty parts when no water simulates.
- water.height {x, z}: return {height}, simulated displacement plus procedural wave height in metres.
- sim.status {}: every running simulation program (sim.program nodes, one per part and per copy of a character): {sims: [{part, mode, rate, elements, steps, params, states, bones, min, max, fingerprint, error}]}. mode is "particles" (tetrahedra) or "mesh" (drives the vertices of its input mesh); min and max are the box the points fill; fingerprint is a hash of the whole state (equal fingerprints mean equal simulations, so use it to check that a change did or did not alter the result); error is null or the message that stopped the program (a kernel wrote another element, an index outside an array, too much work).
- sim.step {part?, steps}: advance the programs of the part (all when omitted) by that many fixed steps now (1/rate seconds each, at most 10,000), feeding them the current bone poses, and answer like sim.status. Use it to run a simulation to a state and then measure it, without waiting for frames.
- sim.set_param {part?, name, values}: set a param of a running program (values: a number or an array of numbers); it keeps the value until sim.reset. sim.reset {part?}: start the program again from its initial state.
- fx.burst {name, position, scale}: emit one-shot particles (explode, sparks, smoke, dust, fire, splash, magic, steam, embers).
- physics.raycast {origin:[x,y,z], direction:[x,y,z], max_distance=100, mask?}: play mode only. The first physics body a ray meets: {hit:false} or {hit:true, part, node, point, normal, distance}. For aiming, line of sight, hitscan shots and "is the floor under me". Only parts with phys.body (and limp ragdoll bones) can be hit.
- physics.overlap {center:[x,y,z], radius}: play mode only. {parts:[{part, node}]} that have a physics body inside the sphere (an explosion's victims, a proximity check).
- To make a ragdoll start from play (a button, a thrown ball, a timer), put behavior.on(trigger="select"|"collide"|"timer"|..., action="ragdoll", target="<the phys.ragdoll name>") on any part: no clip is needed.
- physics.ragdoll {name, impulse?:[x,y,z]}: play mode only. Makes the character whose phys.ragdoll is called name go limp from its current pose, with an optional shove on its root bone (a shot, a blast: try [0,0,-40] for a hard push); it falls and tumbles on every phys.body and stands again when play stops. {bones:n}.
- ground.sample {x, z}: sample ground at a point.
- ground.profile {from:[x,z], to:[x,z], samples}: sample a line across declared game terrain, reporting height, normal, surface, decel and bounce; height is null over a void.
- world.batch {ops:[{method, params}]}: run up to 512 world calls in one request.

Component reflection is cheap for edits and inspection, but use scripts for animating thousands of things every frame. The world must be on screen and running.`
  },
  materials: {
    about: 'ready-made procedural surfaces (bark, concrete, rust, asphalt, dirt, brick) to paste into solid.texture',
    text: `Flat colour reads as a toy. These surface programs are tested (each bakes in the viewer) and use a texture expression language you can also edit. Use one by pasting its
program into solid.texture@1 after a solid.material, with the tile it lists (metres per repeat). tile is required for merged, scattered or procedural meshes: they carry no UVs, so the
texture is projected in world space. Example:
  trunk_m = solid.material@1(trunk, color=[0.3, 0.2, 0.14], roughness=0.95)
  trunk_t = solid.texture@1(trunk_m, program="<a program below>", tile=1.2, size="512")
A program that fails to bake is reported by check as texture.bake_failed with the node and the reason (texture_nodes lists the node catalog). Mirrored coordinates keep tiles seamless,
so a close look shows mirror symmetry; vary the tile between neighbouring parts to hide it. Each distinct program costs one texture layer; the viewer has about 32.
${Object.entries(MATERIALS).map(([name, m]) => `
${name} (${m.about}; tile=${m.tile}):
${m.program}`).join('\n')}`,
  },
  characters: {
    about: 'make_character: a rigged, animated character or creature in one call',
    text: `make_character builds a whole animated character. It writes the graph for you (a smooth body from tapered bones, a rig, skinning,
walk / run / idle clips, toon shading), so you do not build limbs one node at a time.

  make_character(plan="biped", sliders={size: 1.7, head: 1.2, legs: 1.1, color: [0.95, 0.8, 0.7]})
  make_character(plan="quadruped", sliders={size: 2.4, horns: true, wings: true}, name="dragon")
  make_character(plan="serpent", sliders={size: 3, tail: 1.3})

plan: biped, quadruped or serpent. sliders: size in metres (a serpent's is its length), head, limbs, torso, legs, neck, tail as
multipliers 0.5 to 2, color [r, g, b], horns and wings for a quadruped. name keeps several characters apart (up to 13 lowercase
letters and digits); at = [x, y, z] places one. Call it again with another name for more characters.

For a body no plan covers, give creature = { name, color, bones: [...], blobs: [...] }. A bone is { name, parent, head, tail, radius:
[at head, at tail], role, pair, phase }. The creature stands on y = 0 and faces +z. role (leg, arm, spine, neck, head, tail, wing, free)
picks how it moves. pair=true means you wrote the +x side and the left and right copies are made (name_l, name_r). A bone's parent must
come earlier in the list; at most 32 rig bones (a pair counts twice), chains at most 8 deep. blobs are rounded lumps with no bone
(a head, a chest): { name, center, radii }. Errors name the bone or slider to change.

Afterwards: frame_view, and solid.style on the part named after the character to change its look (see topic styles).`,
  },
  styles: {
    about: 'looks (anime, painterly, ink...), cel shading, and textures that follow the shape',
    text: `Looks. solid.style@1(solid, style="anime"|"realistic"|"painterly"|"flat"|"retro"|"ink", outline_scale=1) sets shading, outline and
texture finishing in one word and keeps colour, roughness and textures. realistic removes an earlier style. solid.toon@1 gives
exact control of the cel bands (bands, feather, shadow colour, rim, spec, outline). Put either after solid.material / solid.texture.

Non-repeating textures: solid.texture@1(..., tile=2, no_repeat=true, no_repeat_scale=1) blends three shifted samples to hide visible repeats on large surfaces. no_repeat defaults to false; no_repeat_scale is grid cells per tile (0.25..8, default 1), with larger values making smaller patches. Low quality uses a single sample.

Textures that follow the shape. A solid.texture program can use surface nodes, which read the part's own geometry:
  surface_convex(gain)   1 on sharp outer edges: edge wear, chipped paint
  surface_cavity(gain)   1 in creases and dents: dirt, grime
  surface_up(power)      1 on tops, 0 on walls: moss, snow, dust
  surface_ao(strength)   ambient occlusion; surface_curvature(gain) signed; surface_height(low, high) height above the ground
Example (stone with pale worn edges, dark creases and moss on top):
  solid.texture@1(block, program="ao = surface_ao(strength=0.35); stone = colorize(ao, gradient=[0:#4a4038, 1:#a8978a]); moss = colorize(ao, gradient=[0:#2e4a1c, 1:#78a83e]); wear = colorize(ao, gradient=[0:#a79b8a, 1:#efe6d4]); up = surface_up(power=3); edge = surface_convex(gain=7); a1 = blend(moss, stone, up, blend_type=normal, amount=0.9); albedo = blend(wear, a1, edge, blend_type=normal, amount=0.85)")
IMPORTANT: blend(top, bottom, mask) draws the FIRST colour where the mask is 1. blend(moss, stone, up) is moss on the tops.
Use 3D noise nodes (tex3d_fbm ...) with surface nodes: a 2D noise shows seams between the part's UV charts.`,
  },
  fields: {
    about: 'functions of position (field.expr, field.mask_from) that decide where things are planted',
    text: `A pattern is a function of position, written as a short program. field.expr@1(code="...") makes one; scatter.place@1 takes it as its
third input (where) and multiplies it into the planting density: 1 plants as usual, 0 plants nothing, between thins out.

  w = field.expr@1(code="out = step(8.0, p.y) * (1.0 - step(18.0, p.y))")      only between 8 and 18 m high
  f = scatter.place@1(tree, ground, w, radius=3, seed=2, max_slope=35)

Inside code: p (world position, vec3), n (surface normal), uv, the numbers pi and tau, vector types vec2/vec3/vec4, and the results of other
patterns linked as inputs a, b, c, d (field.expr@1(a_node, b_node, code="out = a * (1.0 - b)")). Functions: abs sign floor ceil fract sqrt exp log pow
sin cos atan2 min max clamp mix step smoothstep length distance select(condition, a, b), the % operator, and noise: perlin, fbm, ridged. The
program must assign out; statements are separated by ";" (a = p.x * 2.0; out = a + 1.0). Keep it under about 256 operations.
Patterns are sampled per planting cell, so the thinning follows the ground, not the grid.

Variation per copy: inst.vary@1(set, scale=p1, yaw=p2, lift=p3) gives every placement its own size, turn about up (radians) and height from
patterns sampled where it stands (each optional). Noise makes natural differences:
  s = field.expr@1(code="out = 0.7 + 0.6 * (0.5 + 0.5 * perlin(p * 0.37, 3.0))")
  y = field.expr@1(code="out = 6.2831853 * fract(perlin(p * 1.9, 5.0) * 7.0)")
  trees = inst.vary@1(forest, scale=s, yaw=y)

field.mask_from@1(tile, layer="flat") turns a terrain channel (made by terrain.mask) into a pattern, the same layer scatter.place's layer
parameter reads. A pattern is not geometry: link it into a node that takes one; it cannot be a scene output.
Mistakes come back as expr.error (with the position in code), pattern.not_float (a pattern must give a number) or terrain.unknown_layer.`,
  },
  plants: {
    about: 'trees (flora.tree, seasons), ground kits (wildflower, fern, mushroom, log, stump, reed, flowering_bush, rock), variants with scatter.place mix and pick, and a forest recipe',
    text: `Plants are kits written in the language (list_operators ids=["kit.oak"] returns the source): oak, pine, grass_tuft. Put the kit source ahead of the
program and call it like a group: oak1 = oak(seed=3, leaf_color=[0.2, 0.42, 0.12]). They stand on y = 0 at about 10 m (oak), 9.4 m (pine), 0.45 m (grass).
Grass uses solid.foliage(grass=true): roots blend toward ground_color by ground_blend (0..1), darken near the soil and tilt their normals up.
Blades stay full size to fade_near metres and shrink to zero at fade_far; translucency (0..1) adds sunlight through their tips, and exposure_boost defaults to 1.
The grass_tuft kit enables this by default (blend 0.6, near 25, far 70, translucency 0.5); color still sets the blade colour. Set ground_color to your terrain's soil colour.
Low quality skips translucency and root occlusion. These are viewer effects; the mesh keeps its original triangles.
Plant them with scatter.place, name where each grows with patterns, and give each tree its own size with inst.vary. A mixed wood:

  t = terrain.tile@1(size=[128, 128], cell=1, seed=4, height=20, scale=90)
  g = terrain.material@1(t)
  oak1 = oak()
  pine1 = pine()
  zone = field.expr@1(code="out = step(0.0, perlin(p * 0.03, 9.0))")            1 where oaks grow
  other = field.expr@1(code="out = 1.0 - step(0.0, perlin(p * 0.03, 9.0))")     1 where pines grow
  oaks = scatter.place@1(oak1, t, zone, radius=11, seed=2, max_slope=30)
  pines = scatter.place@1(pine1, t, other, radius=8, seed=5, max_slope=30)
  size = field.expr@1(code="out = 0.7 + 0.6 * (0.5 + 0.5 * perlin(p * 0.21, 3.0))")
  oaks_v = inst.vary@1(oaks, scale=size)
  pines_v = inst.vary@1(pines, scale=size)
  output ground = g
  output oaks = oaks_v
  output pines = pines_v

Each species is its own output (an instance set has one shape). Spacing is radius in metres: 11 for broad oaks, 8 for pines, 1.5 for grass.
DETAIL: oak(edge_length=...) trades smoothness for triangles: 0.35 (default) 7,400, 0.5 4,100, 0.8 1,600, 1.2 800. pine(ragged=0.0) is a clean stack of cones, 280 triangles
instead of 2,700. Use the fine ones for the handful near the player and the lean ones for a wood: build two kits (oak1 = oak(), oak_far = oak(edge_length=1.2)), scatter
them with complementary where patterns (one near the player's start, one elsewhere), and keep inside the budget.
Terrain detail uses non-repeating sampling by default: terrain.material@1(t, no_repeat=false) turns it off for comparison. no_repeat_scale sets grid cells per detail tile (0.25..8, default 1); larger values make smaller patches. Low quality uses a single sample regardless of the flag.

BUILDINGS AND LANDMARKS BY HAND: terrain.flatten@1(t, at=[30, 40], radius=5, blend=6) levels a pad (the ground keeps its height at the centre, or
to="height", height=12 sets one) and terrain.place@1(hut, pad, at=[[30, 40], [62, 56]], yaw_scale=[[0, 1], [-35, 1]], sink=0.1) stands copies of any
shape on the ground at the x,z you list, so you never need the height. Flatten after terrain.erode (erosion rebuilds the ground and would undo the pad) and before terrain.mask, and pass the flattened tile as the
ground of terrain.place. Ropes, washing lines and chains: curve.hang@1(from=[...], to=[...], slack=0.05) then curve.tube.
REAL TREES AND SEASONS: flora.tree@1(kind="oak"|"pine"|"birch"|"willow"|"shrub"|"palm", height=9, seed=3, lod=1) grows a real tree (species bark, curved limbs, root flare,
lobed or needle leaves). season="autumn" turns the default green leaves orange and red with yellow among them, "spring" a fresh pale green, "winter" strips the leaves; a
colour you give in leaves= always wins over the season. lod 1 is half the triangles of lod 0 and is the right level for a wood.
VARIATION: an instance set has ONE shape, so a mixed planting is several scatter.place nodes that SHARE one planting: give them the same ground, radius, density and seed, the
same mix="0.45, 0.35, 0.2" and each its own pick (0, 1, 2). Every plant goes to exactly one of them (mix is the weights), so variants never overlap and their shares follow the
weights. Make the variants differ by seed, height, season or colour: oak_a = flora.tree(seed=3), oak_b = flora.tree(seed=5, leaves=[0.17, 0.4, 0.1]); oaks_a = scatter.place(oak_a,
land, radius=11, seed=2, mix="1, 1", pick=0); oaks_b = scatter.place(oak_b, land, radius=11, seed=2, mix="1, 1", pick=1). A grove in its own colours is a separate scatter with its
own where pattern (grove = field.expr(code="out = smoothstep(0.1, 0.4, perlin(p * 0.018, 21.0))") and season="autumn"). terrain.biomes@1(land) gives the layers biome_forest, biome_meadow
and biome_rock to plant by (layer="biome_forest").
GROUND KITS (put the kit source ahead, call like a group): wildflower (petal_color, centre_color, petals), fern (color, seed), mushroom (cap_color, stem_color, spot_color), log, stump,
reed (color, head_color), flowering_bush (flower_color, seed), rock (seed). Plant each in several colourings with mix and pick: a meadow is wildflower() in pink, gold, violet and white,
mix="3, 3, 2, 2", pick 0 to 3, radius 1.1. Ferns and mushrooms go under trees (layer="biome_forest"), rocks beside slopes (max_slope=50, sink=0.12, align=0.5, scale=[0.6, 2.2]), reeds
at low wet ground, logs and stumps sparsely (radius 14 to 16). The whole forest is web/tests/scenes/flora/forest.txt.
COST: an oak is about 7,400 triangles, a pine 2,700, a tuft 42, and every copy draws them all. The wood above (44 oaks, 84 pines, 4,600 tufts on
128 x 128 m) draws about 780,000 triangles, 2.6 times the provisional Quest budget of 300,000. For a headset-sized scene use radius 18 (oaks), 14
(pines) and 3 (grass): 15 oaks, 21 pines, 1,160 tufts, about 250,000 in all with the ground. Always run scene_overview or check after planting; it counts every copy.`,
  },
  game: {
    about: 'a game as nodes: variables, kinds, rules, goals; read it with game_describe and play it with playtest',
    text: `A game is data. Write it as game.* nodes and join them with game.collect@1; output the joined node as "rules".

  score = game.var@1(name="score", value=0)
  hero  = game.kind@1(name="player", size=0.5, speed=4, tags=["hero"])
  coin  = game.kind@1(name="coin", size=0.4, tags=["pickup"])
  p0    = game.spawn@1(kind="player", at=[0, 0, 0])
  c0    = game.spawn@1(kind="coin", at=[3, 0, 2], count=8, spread=6)
  move  = game.input@1(name="move", axis="x,z", target="tag:hero")
  pick  = game.rule@1(on="touch", a="tag:hero", b="tag:pickup", do="add:score:1;destroy:b")
  win   = game.goal@1(outcome="win", when="score>=8")
  all   = game.collect@1(score, hero, coin, p0, c0, move, pick, win, name="coins")
  output rules = all

Write the game in this order and check each part: (1) core loop in one sentence, (2) kinds with tags (what exists), (3) spawns (where and how many; game.spawn@1(places, kind="lantern") makes one entity per placement of a set, standing exactly where each was placed, so scatter.place on
hilly ground puts pickups on the hill: spots = scatter.place@1(marker, ground, radius=26, seed=9, max_slope=15), then game.spawn@1(spots, kind="lantern"); at most 2,048),
(4) rules (on= touch, tick every=seconds, var:<name>; a= and b= select by tag:<tag> or kind:<name>; when= a condition on variables;
do= actions joined by ";": add:var:n, set:var:n, destroy:a|b, lose, win, effect:sfx:name@b), (5) goals (win/lose when a condition holds),
(6) feedback (effects): effect:sfx:coin@b plays a procedural sound where entity b stood (coin, jump, hit, explosion, laser, whoosh, click, powerup,
splash, dooropen, doorclose and more; check names any that do not exist, with the real list in its fix). Kinds draw as scene parts: game.kind(part="coin_mesh")
shows every entity of that kind with the scene output named coin_mesh (check reports a part that does not exist). Show progress with game.hud@1(text="Lanterns {score} / 4", anchor="gaze", size=0.005): a panel of plain geometry in front of the viewer whose {names} are
variables (anchors gaze, left, right, top, bottom; size is the edge of one font pixel in metres, 0.005 gives 3.5 cm letters; at most 8 panels of 64 characters).
The player is the hero:
game.input(name="walk", follow="viewer", target="tag:hero") puts the entity tagged hero where the viewer stands, so walking into a pickup touches it. Kits (a key-and-door pair, a coin kit) are game.collect nodes you can join into the final collect.

A key and a door is three rules and a variable (a rule with when= only fires when its condition holds; a door that opens is a destroyed entity):
  has_key = game.var@1(name="has_key", value=0)
  take = game.rule@1(on="touch", a="tag:hero", b="tag:key", do="add:has_key:1;destroy:b;effect:sfx:coin@b")
  open = game.rule@1(on="touch", a="tag:hero", b="tag:gate", when="has_key>=1", do="destroy:b;effect:sfx:dooropen@b")
  leave = game.rule@1(on="touch", a="tag:hero", b="tag:exit", do="win")
Kinds touch as spheres of size metres. To make a kind an obstacle give it solid=[w, h, d] (a box in metres centred on the entity): game.kind@1(name="door", size=2.0,
tags=["gate"], part="door_mesh", solid=[0.5, 2.6, 7.6]). The hero (the player) cannot walk into a live solid and is held on the side it came from, even if the viewer jumps across it;
destroying the entity (the open rule) removes the obstacle. Without solid a door is only a picture that disappears and a bot walks straight through. The player starts at the
hero's spawn, facing the first other entity.

HANDS (what makes it a VR game, not a walking game): give each hand an entity and follow a controller, and mark carryable things grab=true:
  gl = game.kind@1(name="glove_l", size=0.09, tags=["hand", "hand_l"], part="glove_mesh")
  gr = game.kind@1(name="glove_r", size=0.09, tags=["hand", "hand_r"], part="glove_mesh")
  fuse = game.kind@1(name="fuse", size=0.12, tags=["item"], part="fuse_mesh", grab=true)
  hl = game.input@1(name="left", follow="left_hand", target="tag:hand_l")      hr = same with "right_hand" and "tag:hand_r"
Squeezing a grip within reach (hand size + item size) picks up the nearest grab=true thing and carries it until the grip opens; pulling a
trigger raises a use event on what is in reach. Events: grab, release, use (a = the hand, b = the thing), plus touch as ever, and a carried
thing touches what it is brought to. A socket is a rule on touch: game.rule@1(on="touch", a="kind:fuse", b="kind:port1",
do="add:fuses:1;destroy:a;destroy:b;spawn:fuse_in@1.5,1.3,2.8;move:kind:door1:0,2.6,0;effect:sfx:dooropen@b"); a lever is a rule on use:
game.rule@1(on="use", a="tag:hand", b="kind:lever", do="destroy:b;spawn:lever_dn@x,y,z;destroy:kind:cage_gate"). Actions you can chain: add, set,
destroy, spawn:kind@x,y,z, move:selector:dx,dy,dz (a door slides up: move:kind:door1:0,2.6,0), setvel, setprop, timer:name:seconds, emit, win,
lose, effect:sfx. Centre a door's mesh on its own origin and spawn it at half its height, because its solid is centred on the entity. In
a browser without a headset the right hand floats in front of the view: hold G to grip and T to pull the trigger. A finished example with
four fuses, two doors, a lever, a cage and a reactor is web/tests/worlds/blackout.txt.

LEVELS ARE REAL LANDSCAPES, NOT BOXES. A game level outdoors is a terrain, with real plants, built from the engine's own tools; boxes for ground and spheres on sticks for trees are
a failure, not a shortcut. Ground: terrain.tile@1 (one editable tile with a height grid; use cell=0.5 or finer for anything a ball rolls on), shaped with terrain.erode, levelled
where it must be flat with terrain.flatten@1 (a pad for a tee, a green, a house; to="height" with a LOWER height digs a hollow such as a bunker or a pond bed, and the shoulder
blends back into the land), dressed with terrain.material (grass, sand, rock by slope and height), water with river.carve and river.water. Plants: flora.tree@1 species
(oak, pine, birch, willow ...) planted with scatter.place@1 over the tile (radius, seed, max_slope; use a terrain.mask zone to keep them off the playing area), grass tufts and
rocks the same way; props such as fences come from the kits (kit.fence ...; list_operators shows them). Whatever the player or a ball moves on is named in game.terrain: the
OUTPUT names of the terrain tile parts. Surfaces that change how a ball rolls (a green, a bunker) are footprint parts: a thin flat disc or pad (solid.cylinder, a few millimetres tall, a
colour of its own) set exactly on the ground with terrain.place (sink=0, at=[[x, z]]) and named in game.surface(parts=[...]); the hollow itself is the terrain.flatten. Hand-held
tools and props are modelled as the real thing: a golf club has a tapered shaft (a loft or sweep that narrows toward the head, about 14 mm to 9 mm), a rubber grip with bands, a
hosel where the shaft meets the head, and a head with a real shape (an iron: a thin blade with a sloped face and a sole; a putter: a blade or mallet with a flat face and a sight
line), bevelled edges (poly.bevel_network) and a metal or painted material; look at it close up with snapshot before you call it done. Check the whole level by flying a view
over it and by looking at it from where the player stands.

THROWING: a kind with fall=true is pulled down by gravity and rests one radius above the ground (game.ground@1(y=0.25) moves the ground, default 0); bounce=0..0.95
is how much speed it keeps off the floor. A grabbed one does not fall; let go of it while the hand swings and it leaves with the hand's velocity, so it can be thrown:
  rock = game.kind@1(name="crate", size=0.25, tags=["item"], part="crate_mesh", grab=true, fall=true, bounce=0.3)
  hit  = game.rule@1(on="touch", a="tag:item", b="tag:creature", do="stun:b:4;destroy:a;effect:particles:explode@b;effect:sfx:boom@b")
PLAYABLE HERO: game.controller@1(kind="player", speed=5, accel=40, decel=50, jump=8, gravity=24, extra_jumps=1, relative="camera", turn=720) makes a kind walk, jump (variable
height, coyote time, jump buffer, double jump) and stand on the ground and on the tops of kinds with solid=[w,h,d] (it steps up ledges lower than step=0.3). mode="walk" (3D),
"platformer" (a side-scroller: lock="z" keeps it in a plane, gravity and jump as above) or "top_down" (no gravity, no jump). The page feeds move.x/move.z from WASD or the stick, a
jump press and a jump axis from Space/C or the pad's A, and camera.yaw from where the player looks; declare game.input@1(name="move", axis="x,z", target="tag:hero") and
game.input@1(name="jump", key="press", target="tag:hero"). The hero gets props grounded and speed for animation rules. VIEW: game.camera@1(name="behind", mode="third", distance=5,
height=1.6, shoulder=0.5, smoothing=0.08) follows tag:hero (mouse orbits it); modes first, third, top_down, side (dead zone, bounds="x0,x1,z0,z1"), iso, fixed (at="x,y,z", watches the
target). The first camera starts; game.rule@1(on="start", do="effect:camera:above:1.5") blends to the camera named above in 1.5 s (no seconds = a cut). A rule's every= is in seconds (on="tick" every=8 fires every eight seconds), so two camera rules with every= make the view switch back and forth for good; switch on an event instead. Example: web/tests/scenes/walker/walker.txt.
FALLING: game.controller@1(fall_limit=-8) with game.ground@1(y=-30) puts the hero back on the last safe ground when it falls below -8 m and raises the event fell: game.rule@1(on="fell", do="add:lives:-1"). Animation: a controlled hero tells its animation machine speed (number), grounded and jumping (flags) and the triggers jump and land, with no rules.
CONTROLS: game.input@1(name="jump", key="press", target="tag:hero", keys="Space,KeyC", pad="button0", label="Jump") names the keys (KeyboardEvent codes) and gamepad controls (button<N>, stick:left, stick:right, axis<N>+/-) of an input; an axis input needs a positive and a negative key per axis: axis="x,z" keys="KeyD,KeyA,KeyW,KeyS" pad="stick:left". The player can rebind keys; inputs named move and jump get WASD/stick and Space/C/A when you name none. Enter and the d-pad/A work menus on their own.
FLOW: game.state@1(name="title", frozen=true) declares states (the first is where the game starts; a frozen one stops everything except presses: title, paused, game_over, win; the states named win and game_over are entered by the goals). Limit a rule to one with game.rule@1(on="input:pause", state="playing", do="go:paused"); every rule on an event sees the state the event began in, so one key can pause in playing and resume in paused. Actions: go:STATE, restart or restart:STATE (rebuild the game and enter the state), level:NAME (game.level@1(name="cave") + game.spawn@1(level="cave"): swaps the level's entities and puts heroes at its start), checkpoint and respawn. Entering a state raises state:NAME, loading a level level:NAME. The page keeps save slots, undo and settings.
SCREEN UI: game.ui@1 draws the game's menus and HUD over the view with a procedural font (no assets). type is text, bar, panel, button, stack, grid or minimap. A stack places its children (parent="menu"): game.ui@1(name="menu", type="stack", anchor="c", align="center", gap=0.03, pad=0.05, fill="rgba(8,12,20,0.8)", show="title") then game.ui@1(name="play", type="button", parent="menu", text="Play", size="0.3,0.075", do="go:playing"). Text binds variables and keys: text="Score {score}   {key:jump} to jump". Bars: type="bar", value="hp" (a variable), max=100 or max_var="hp_max", size="0.3,0.03". Letter height is letter=0.04. show="paused,title" limits an element to flow states. Buttons raise ui:NAME; arrows and the d-pad move the focus, Enter and A press it, a click presses; tip="..." shows under the focused button. Minimap: type="minimap", size="0.2,0.2", radius=30, follow="tag:hero", tags="coin,enemy". Write colours as rgba(r,g,b,a) or quoted "#rrggbb". Example: web/tests/scenes/platformer/platformer.txt.
SHOOTING: game.health@1(kind="turret", hp=30, iframes=0.3, knock=3) gives a kind hit points (props health, max_health); game.projectile@1(kind="bolt", speed=26, life=1.5, damage=10, pierce=0, splash=0, homing=0, gravity=0) makes a kind a shot that flies, hurts the first victim, stops at solid walls and raises hit and impact; game.weapon@1(name="blaster", mode="projectile", shot="bolt", input="fire", reload_input="reload", owner="tag:hero", ammo=16, reload=1.2, cooldown=0.2, spread=1.5, aim="camera", auto=true) fires it (mode="hitscan" with range and damage is an instant ray). aim: heading, camera (through the crosshair of game.camera@1), or a selector such as tag:enemy (toward the nearest). An enemy weapon has input="none" and a rule fires it: game.rule@1(on="tick", every=2.5, do="fire:spit"). Rule actions hurt:SELECTOR:AMOUNT (optionally @SOURCE), heal:SELECTOR:AMOUNT, fire:WEAPON, reload:WEAPON; events hurt, killed (the entity is removed after the rules hear it), fired, empty, reloaded, hit, impact. The magazine is the owner's prop ammo_NAME, so a bar or text can show it: game.ui@1(type="bar", value="hero.health", max_var="hero.max_health") and text="Ammo {hero.ammo_blaster}". Example: web/tests/scenes/arena/arena.txt.
PARTY COMBAT: game.brain@1(kind="knight", foes="tag:enemy", leader="tag:hero", speed=4.6, follow=3, engage=20, weapon="sword") makes a kind follow its leader and fight on its own (companions and enemies); keep=9 stands a ranged fighter off. game.weapon@1(mode="melee", range=2.4, arc=110, damage=95, foes="tag:enemy", aim="heading"|"tag:enemy") swings at everything in the arc; foes keeps allies from hurting each other (also for shots and rays). game.targeting@1(leader="tag:hero", foes="tag:enemy", range=30) keeps the hero's target (prop target; the foe's prop targeted is 1) and the rule action target:next cycles it. game.ability@1(name="smash", owner="tag:hero", input="power1", cooldown=12, range=5, needs_target=true, do="hurt:b:320") runs actions with a = the owner, b = its target, and counts the owner's prop cd_smash down; refused:smash is raised when it cannot work. Actions: blast:CENTRE:RADIUS:AMOUNT[@FOES] (blast:a:4.5:170@tag:enemy), dash:a:11:0.25 and iframes:a:0.45 (a dodge), heal:tag:party:260. Screen elements: game.ui@1(type="slot", value="vriden.cd_smash", max_var="vriden.cdmax_smash", icon="sword", key="{key:power1}") an ability square with a cooldown, type="portrait" (text="V", fill=colour), type="floaters" (damage numbers, +healing and Dodge! over the heads of what was hurt: one per game), and when="targeted.health>0" shows a target frame only while something is targeted ({targeted.health}, value="targeted.health"). A game with many nodes is gathered in groups: game.collect@1(...) of kinds, spawns, weapons and the controller together (they are checked against each other), another of screens, and an outer collect of the groups. Example: web/tests/scenes/party/party.txt.
CREATURES: a kind with an agent patrols, notices the hero, chases and can be stunned: game.agent@1(kind="scuttler", patrol=[[-3, 12], [3, 12], [3, 18]], speed=1.4,
chase_speed=2.6, sight=6, lose=10). It walks on what game.nav@1(parts=["floor", "ramp"], cell=0.25, radius=0.35) names (the output names of the walkable parts; with no nav
line it walks straight through walls). Events a rule can use: sight, lost, stun, recover, catch (a = the creature, b = the hero): game.rule@1(on="catch",
do="add:lives:-1;stun:a:3;effect:anim:alert@a"), game.rule@1(on="stun", do="effect:anim:stagger@a"). stun:SELECTOR:SECONDS freezes creatures for that long. A
creature kind draws as a model when its part is an animated hierarchy (parts that anim.parent each other, plus an anim.machine): every creature is its own copy
with its own walk cycle, turned to face where it goes, and effect:anim aimed at a creature reaches only that one. A skinned character (body.human with body.skin, body.outfit, rig.skin
eyes) is a creature too: the kind's part is the skeleton root, which carries the anim.machine and one track per clip the machine names (a small bob is enough); the bones
chain to it with anim.parent, but never anim.parent a skinned part itself (rig.skin_not_rigid: its skin is dropped). The skinned parts join the copy through the bones they follow. effect:particles:NAME@b bursts at b
(explode, sparks, smoke, dust, fire, splash, magic, steam, embers).

ROLLING THINGS AND HAND-HELD TOOLS (golf, billiards, bowling, hockey, tennis ... any game where something rolls on the ground and a hand hits it): the ground a thing
rolls on is built from the parts you name: game.terrain@1(parts=["course", "green"], cell=0.2) (the OUTPUT names of the authored parts that are walkable ground: their top
surface is the ground, anywhere no part covers is void and a rolling thing over it is out). game.surface@1(name="sand", parts=["bunker"], decel=8, bounce=0.1) says what a
rolling thing does on the footprint of those parts: decel is how fast it slows in m/s2 (a fast green 0.55, short grass 1.0, rough 3, sand 8; a billiard cloth about 0.3), bounce
how much of a bounce it keeps; later surfaces win where parts overlap, and anywhere else is decel 1.0. A rolling thing: game.kind@1(name="ball", size=0.03, roll=true,
bounce=0.3, part="ball_mesh"): it falls, bounces, rolls, runs down slopes and rests. A hand-held tool (club, cue, bat, racquet, mallet): game.kind@1(name="putter", size=0.05,
tags=["hand_r"], swing=true, reach=0.9, loft=0, power=1.0, part="putter_mesh") with game.input@1(name="tool", follow="right_hand", target="tag:hand_r"): its head is reach
metres out of the hand along the hand's -Z axis, and a swing whose head passes through a rolling thing hits it with the head's speed (times power, at most 45 m/s) along
the swing, tilted up by loft degrees (flat 0, a wedge-like 50). Model the tool so its grip is at the part's origin and its shaft and head run out along -Z (the part is drawn
turned with the hand). On a desktop the right hand floats in front of the view and the tool points where you look: aim by looking at the ball, swing by turning the view
quickly. A pocket (a hole, a net, a cup, a pocket of a table): game.kind@1(name="cup", size=0.07, pocket=true) spawned where it is: a rolling thing over it slower than
1.6 m/s raises pocket (a = the pocket, b = the thing): game.rule@1(on="pocket", do="add:holes:1;destroy:b;effect:sfx:powerup@a"); a swing raises swing:
game.rule@1(on="swing", do="add:strokes:1"); a rolling thing that leaves the ground raises out (a = the thing). To send the player somewhere (the next tee, the
other end of the table): effect:message:goto_X_Y_Z_LX_LZ (feet at X, Y, Z looking at LX, LZ), and spawn the next ball with spawn:ball@x,y,z. A game of this kind
needs a score or count in the HUD and something to do after each pocket; keep a variable for the stage (add:hole:1) and give each stage its own pocket position.

Check it, do not trust it: game_describe prints what the document defines; playtest(policy="seek:hero:pickup") walks a bot to the nearest pickup and
reports won, lost or playing, the final variables and never_fired (rules no play reached: usually a wrong tag). playtest(policy="idle")
should lose by a clock goal, or run out the steps. A game that no policy wins, or a rule in never_fired, is a design bug to fix before building more.
For a hand-held tool striking a rolling thing, call playtest({policy: "aim:ball:cup1,cup2,cup3"}): it reads the live positions and ground profile,
swings the right hand frame by frame, waits for rest or stage progress, and adapts the speed for up to 14 shots per target. It reports status,
strokes per target, shots taken, the last shot plan and whether the game was won; this policy runs in the viewer and can take several minutes.
Errors name the node that wrote the bad item (game.rule, game.kind, game.duplicate, budget.kinds ...).`,
  },
};

/** Where `text` starts: the index of the first of `starts`' first words, or -1. */
function find(text, start) {
  return text.indexOf(start);
}

/**
 * Cuts the full guide text into its topics. A topic runs from its start phrase to the next topic's. Returns the sections found
 * and the start phrases that were not found (the guide text and `SECTIONS` have drifted apart).
 */
export function splitGuide(full) {
  const found = [];
  const missing = [];
  for (const section of SECTIONS) {
    const at = find(full, section.start);
    if (at < 0) missing.push(section.topic);
    else found.push({ ...section, at });
  }
  found.sort((a, b) => a.at - b.at);
  const workflow = full.indexOf('How to work: call get_text');
  const topics = {};
  found.forEach((section, i) => {
    const end = i + 1 < found.length ? found[i + 1].at : (workflow > section.at ? workflow : full.length);
    topics[section.topic] = full.slice(section.at, end).trim();
  });
  if (topics.world) topics.world += `\n\nBuild editable ground with terrain.tile, shape it with terrain.erode and terrain.flatten, and dress it with terrain.material.
For a lake on a landscape use water.fill@1: it floods the connected hollow below level, starting at world x,z at (default the tile centre).
The result is a flat open sheet with depth tint and shore foam; shore sets the wet bank width in metres. It has no side walls.
Give the terrain and the water separate outputs. Dig a hollow, then fill it:
  tile = terrain.tile@1(size=[128, 128], cell=1, height=0, seed=3)
  pit = tile | terrain.flatten@1(at=[64, 64], radius=12, blend=12, to="height", height=-1)
  lake = pit | water.fill@1(at=[64, 64], level=-0.3, shore=3, clarity=2.5, ripple=0.8, foam=0.4)
  output ground = pit
  output lake = lake
Only the seeded basin fills. A level above the surrounding ground floods to the tile border. flow defaults to 0; direction and seed pick the wave pattern.
For rivers use river.carve followed by river.water. water.surface is a closed standalone water body; use water.fill on a landscape.
Use terrain.place for props on the ground, scatter.place for plants, scene.sky for the sky and scene.environment for ambient light.`;
  return { topics, missing };
}

/** The topic list a model sees at the end of the core guide: name, what it covers, and how big it is. */
export function topicIndex(topics) {
  const all = { ...Object.fromEntries(SECTIONS.map(s => [s.topic, { about: s.about, text: topics[s.topic] ?? '' }])), ...EXTRA_TOPICS };
  return Object.entries(all).map(([name, t]) => `  ${name.padEnd(11)} ${t.about} (${Math.ceil((t.text?.length ?? 0) / 1000)} KB)`).join('\n');
}

export function coreGuide(topics) {
  return `The document is a graph of operators, written one node per line:

  name = family.op@1(input_node, ..., key=value, ...)
  output part_name = node_name        (or: output node_name)

Inputs are earlier node names, in port order (or port=name). Values are numbers, "text", true/false and [lists]. Lengths are metres
(suffixes mm and cm work); angles are degrees unless you write rad (angle=90deg, 1.57rad). Y is up. Lines starting with # are comments.
Pipes and loops make a long build short; see get_guide(topic="compact") for examples and limits.

Example (a vase with a 3 mm minimum wall that the engine enforces):
  outer = profile.polygon@1(points=[[0,0],[0.06,0],[0.11,0.12],[0.07,0.26],[0.05,0.3],[0,0.3]])
  inner = profile.polygon@1(points=[[0,0.01],[0.056,0.01],[0.104,0.12],[0.064,0.26],[0.04,0.31],[0,0.31]])
  wall = check.min_wall@1(outer, inner, min=3mm)
  body = solid.revolve@1(outer, angle=360deg, segments=64)
  cavity = solid.revolve@1(inner, angle=360deg, segments=64)
  vase = solid.difference@1(body, cavity)
  output vase

How to work:
1. Find operators: list_operators() shows the families and their names; list_operators(family="solid") gives each operator's signature
   (inputs, parameters with defaults) and one line of help; describe_operator(id) gives every parameter's unit, range and help.
   texture_nodes searches the procedural texture catalog. Use only operators that exist.
2. Read how-tos with get_guide(topic=...): worked examples for each area (topics below). Read only the ones you need.
3. get_text shows the document. apply_text ADDS the nodes of a program (it never replaces; it may read nodes already there by their
   label, and "output name = node" re-points an output). To change an existing node use apply_commands (set_param ...) with ids from
   get_nodes. check_text validates a program without applying it. An edit is atomic: a refused one changes nothing, and its diagnostic
   (code, line, column, suggestion) says what to change. Read it and fix that line.
4. Check what you made: measure gives volumes, sizes and triangle counts: use it instead of estimating. Then frame_view so the person
   watching sees it.
For people and creatures use make_character (topic characters): one call builds a rigged, animated body.

Topics for get_guide(topic):
${topicIndex(topics)}`;
}

export { oneLine, signature, operatorIndex, describeOperator } from './guide-operators.mjs';
