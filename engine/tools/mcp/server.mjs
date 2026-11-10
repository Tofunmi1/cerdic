// A test MCP server for the engine: lets any MCP client (Claude Code, for one) build and edit a
// document through the page that is running the engine. No model, no API key: the client's model is
// the provider. It speaks MCP over stdio and relays each tool call to the editor page over a local
// WebSocket; the page runs it through the normal commands (text in, typed commands underneath).
//
//   claude mcp add spatial-engine -- bun engine/tools/mcp/server.mjs
//   open http://127.0.0.1:5173/?mcp=1     (the page connects to ws://127.0.0.1:8765)
//
// For local testing only: it binds to 127.0.0.1, takes one page at a time and has no authentication.
// stdout carries only MCP messages; everything else goes to stderr.
import { mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';
import { coreWithMenu, splitGuide, topicText } from './guide-topics.mjs';
import { EXTRA_TOPICS } from './guide.mjs';
import { TOOL_TOPICS } from './tool-reference.mjs';
import { validateToolArguments } from '../../web/tool-arguments.js';
import { CREATURES } from './guide-creatures.mjs';
import { EFFECTS } from './guide-effects.mjs';
import { SIM } from './guide-sim.mjs';
import { fullSpecs, indexText, select } from './operator-index.mjs';
import { helpField } from './help-field.mjs';
import { LIMITS_TOPIC } from './guide-limits.mjs';
import { callBatch } from './batch.mjs';
import { buildImport } from './import-file.mjs';
import { callRun } from './run.mjs';
import { callRecipes, callStartFrom } from './reuse.mjs';
import { createHints } from './hints.mjs';

const hints = createHints();
hints.reset();
const pendingTips = [];

const PORT = Number(process.env.VE_MCP_PORT ?? 8765);
const TIMEOUT_MS = 90000;
const GUIDE_ARG = process.argv.indexOf('--guide');
let page = null;
let counter = 0;
const pending = new Map();

const log = (...args) => console.error('[spatial-engine mcp]', ...args);

// (not started for `--guide`, which only prints text)
const bridge = GUIDE_ARG >= 0 ? null : Bun.serve({
  hostname: '127.0.0.1',
  port: PORT,
  fetch(request, server) {
    return server.upgrade(request) ? undefined : new Response('spatial-engine MCP bridge', { status: 426 });
  },
  websocket: {
    open(ws) { page?.close(); page = ws; log('editor page connected'); },
    close(ws) { if (page === ws) { page = null; log('editor page disconnected'); } },
    message(_, raw) {
      let message;
      try { message = JSON.parse(String(raw)); } catch { return; }
      const waiting = pending.get(message.id);
      if (!waiting) return;
      pending.delete(message.id);
      clearTimeout(waiting.timer);
      waiting.resolve(message);
    },
  },
});
if (bridge) log(`bridge listening on ws://127.0.0.1:${PORT}`);

function callPage(method, params) {
  if (!page) {
    return Promise.resolve({ error: `No editor page is connected. Open the engine editor with ?mcp=1 (for example http://127.0.0.1:5173/?mcp=1) and try again.` });
  }
  const id = ++counter;
  // A live aiming policy waits for each physical shot to settle (up to 40 s).
  const timeout = method === 'playtest' && String(params?.policy).startsWith('aim:')
    ? 60000 + String(params.policy).split(',').length * 14 * 45000 : TIMEOUT_MS;
  return new Promise((resolve) => {
    const timer = setTimeout(() => { pending.delete(id); resolve({ error: `The page did not answer ${method} within ${timeout / 1000} s.` }); }, timeout);
    pending.set(id, { resolve, timer });
    page.send(JSON.stringify({ id, method, params }));
  });
}

// The kits come from the engine's own registry (crates/ve-modeling/src/kits.rs), so a kit added there reaches the model
// without a second list to keep in step: each entry is `name`, `doc` and the `source` file it includes.
const KITS_RS = await Bun.file(new URL('../../crates/ve-modeling/src/kits.rs', import.meta.url)).text();
const KIT_ENTRY = /name: "([a-z_0-9]+)",\s*doc: "((?:[^"\\]|\\.)*)",\s*source: include_str!\("\.\.\/kits\/([a-z_0-9]+)\.txt"\)/g;
const KITS = await Promise.all([...KITS_RS.matchAll(KIT_ENTRY)].map(async ([, name, doc, file]) => ({
  name,
  doc: JSON.parse(`"${doc}"`),
  source: await Bun.file(new URL(`../../crates/ve-modeling/kits/${file}.txt`, import.meta.url)).text(),
})));

const GUIDE = `The document is a graph of operators, written one node per line:

  name = family.op@1(input_node, ..., key=value, ...)
  output part_name = node_name        (or: output node_name)

Inputs are earlier node names, positional in port order (or port=name). Parameters are key=value.
Lengths are metres (suffixes mm and cm work); angles are degrees unless you write rad (for example
angle=90deg or 1.57rad). Numbers, "text", true/false and [lists] are the only values. Lines starting
with # are comments. Constants and arithmetic: "let w = 1.5" defines one, and {w * 2} (or {w / 2 + 0.1}) in
any value is replaced by its number; units go after the brace ({w * 1000}mm). Functions: min, max, abs,
sqrt, floor, ceil, round, clamp, sin, cos, radians; constants pi and tau. Text constants hold a string you
reuse: let mirror = "u = 1.0 - abs(2.0 * p.x - 1.0);" and then \${mirror} inside any quoted value is replaced by it
(a number constant works there too, \${grain}); a text constant may use earlier ones and carry escaped quotes, so a
texture program or a long list is written once and parameterised. Loops repeat lines: "for i in 0..8" ... "end" runs the
lines between once for each i = 0 to 7, and "for [x, z] in [[3, 1], [4, 2]]" takes a row of numbers each time. Inside, {i * 0.5}
makes a number and {i} in a name makes a different node each time (post_{i} = ...; output post_{i} = post_{i}). Loops hold node,
let and output lines, nest three deep, work inside a group, and may emit up to 2,000 statements, and a group cannot be declared
inside a loop. A loop can also walk text: for [name, x] in [["alpha", 1], ["beta", 2]] binds name to quoted text; {name} inside a
node name is the text itself (letters, digits, underscore only; f_{name}), anywhere else it is that text as a string
(frame.add(b, name={name})), and \${name} inside a string is spliced (name="hand_\${name}"). To build a list or a keys string with a loop
write a join: let pts = join(i in 0..24, "[{0.4 * cos(i * 0.5)}, {i * 0.06}, {0.4 * sin(i * 0.5)}]") then points=[\${pts}]; let keys =
join(i in 0..5, "{i * 0.5}:0,{i * 90},0", "; ") then keys="\${keys}". The header is a loop header (a range, a [list], or [name, x] rows),
the second argument is the template ({expression} is a number, \${name} a constant), the optional third is the separator (default ", ").
\${name} is spliced as written inside or outside a string; a join is limited to 65,536 characters and errors name the iteration.
Choices: "if i < 2" ... "else" ... "end" reads only the branch that holds (else is optional). A condition compares numbers (< <= > >= ==
!=), compares text with == or != (side == "l"), joins with && and ||, negates with !, and may use parentheses; a lone number is true when
it is not zero. It is decided from constants and loop variables while the text is read, never from the model. A let inside a branch stays
inside it; a group cannot be declared inside an if.
Macros: "def limb(tag, name, px)" ... "end" writes a block once; the line limb("arm_l", "upperarm01_l", 0.19) reads it again with the
arguments bound (numbers, expressions, "text", or constants; "n\${i}" builds a text). In the body, {name} in a node name is the text itself, {name}
elsewhere is that text quoted, \${name} is raw (an input: anim.track@1(\${prev}, ...)), and "output {out} = ..." takes a name. A body may hold
nodes, let, for, if and calls of other macros, and sees the constants above the call; it may not hold group, expose, result or a def. A
macro never calls itself (even through another). A def stands at the top level and before its first call. Statements still count towards
the 2,000 cap. Errors name the macro and its arguments ("in iteration macro=limb, tag=\\"arm_l\\"") and point at the body line. Errors name the iteration ("in iteration i=3"). A batch holds up to 10,000 commands (a node costs
one, each input link one, each output one). Without MCP: bun tools/mcp/server.mjs --guide [topic], cargo run -q -p
ve-modeling --example ops -- [family|id|--search word] for the operators, bun tools/texture-nodes.ts [names] for the
texture nodes, and cargo run -q -p ve-modeling --example eval_text -- program.txt to check a program (it prints the
commands, triangles, texture programs and lights it costs). Values: value.number,
value.math and value.size make numbers that you link into any number parameter by its name (a vector
component as size.y), for example r = value.number@1(value=0.5), d = value.math@1(op="mul", a=r, b=2),
ball = solid.sphere@1(radius=d), box = solid.box@1(size=[1, 1, 1], size.y=h). value.size@1(part,
measure="height") measures a part (width, height, depth, longest, shortest, volume, area), so one part
can be sized from another. value.point@1(part, at="head", axis="y") reads a place on a part as a number: at is head or tail
for a bone (body.bone, rig.bone) or min, center, max of any part's bounding box, axis is x, y or z; link it one component at a time
(hy = value.point@1(head_bone, at="head", axis="y"), hat = xform.move@1(cone, by=[0, 0, 0.03], by.y=hy)) so a hat or a sword is
placed from the body's own bones and follows when its sliders change. value.math adds an offset. Only operators from
list_operators (a one-line-each index; pass ids=["solid.box@1"] for full parameter docs) can be used. Y is up. Profiles live in the XY
plane: solid.revolve spins one (x >= 0) about Y, solid.extrude stands one up from y=0.

Example (a vase, with a 3 mm minimum wall that the engine enforces):
  outer = profile.polygon@1(points=[[0,0],[0.06,0],[0.11,0.12],[0.07,0.26],[0.05,0.3],[0,0.3]])
  inner = profile.polygon@1(points=[[0,0.01],[0.056,0.01],[0.104,0.12],[0.064,0.26],[0.04,0.31],[0,0.31]])
  wall = check.min_wall@1(outer, inner, min=3mm)
  body = solid.revolve@1(outer, angle=360deg, segments=64)
  cavity = solid.revolve@1(inner, angle=360deg, segments=64)
  vase = solid.difference@1(body, cavity)
  output vase

Smooth shapes: a few well-chosen profile points are enough if you round them with
profile.smooth@1(profile, iterations=3, keep_axis=true), which turns the corners into a smooth curve
(keep_axis=true for a profile you will revolve, so its base stays flat). For a hollow vessel, revolve
an outer and an inner profile and subtract; let the inner profile rise above the outer rim to open
the top, and add check.min_wall@1 to keep the wall thick enough. Lay out real dimensions in metres.

Furniture and machines: solid.box, solid.cylinder, solid.cone, solid.torus and solid.capsule are
centred on the origin (cone, cylinder and capsule stand along Y); profile.rect and profile.circle
make profiles to extrude. Place a part with xform.move, then repeat it: xform.mirror@1(solid,
axis="x") adds the mirror image across the plane through the origin (put the part off-centre first),
xform.array@1(solid, count_x=3, count_y=1, count_z=2, spacing=[0.2, 0, 0.2]) makes a grid,
xform.radial@1(solid, count=16, radius=0.1) places copies evenly around the Y axis (each moved out
along X first), and xform.repeat@1(solid, count=12, step=[0, 0.18, 0], rotate_step=[0, 15, 0]) makes a
staircase or spiral (copy i is turned i times rotate_step, then moved i times step). Copies are
fused into one solid, so make neighbours overlap a little where they must join. A table:
  leg = solid.box@1(size=[0.05, 0.72, 0.05])
  leg_corner = xform.move@1(leg, by=[0.55, 0.36, 0.30])
  legs_pair = xform.mirror@1(leg_corner, axis="x")
  legs = xform.mirror@1(legs_pair, axis="z")
  top = solid.box@1(size=[1.2, 0.04, 0.7])
  top_raised = xform.move@1(top, by=[0, 0.72, 0])
  table = solid.union@1(legs, top_raised)
  output table

Round a planar outline before extrusion with profile.fillet@1(profile, radius=0.02). For a small
convex primitive, solid.round@1(solid, radius=0.01, segments=12) rounds all edges while preserving
outside bounds. It accepts at most 20,000 input and 250,000 output triangles; radius must be less
than half the smallest bounds dimension. This is not a per-edge bevel or general concave filleting.

Ready-made modeling kits (${KITS.length}): ${KITS.map(kit => kit.name).join(', ')}. list_operators shows each with its size and
parameters; list_operators ids=["kit.door"] returns the group source. Paste that source once at the top of
your program, then instantiate it with item = door() (parameters as key=value) and route the instance to an
output. Kits are text groups, so their construction stays editable in the same graph.

Curved paths (initial bounded subset): curve.polyline@1(points=[[x,y,z], ...], closed=true) makes a
periodic path; omit closed or set it to false for an open path. A repeated final point equal to the
first is removed automatically for a closed polyline.
curve.hang@1(from=[x,y,z], to=[x,y,z], slack=0.05, points=32) makes a catenary rope between two tie points.
curve.bezier@1(points=[[p0x,p0y,p0z], [p1x,p1y,p1z], [p2x,p2y,p2z], [p3x,p3y,p3z]], steps=24)
makes a sampled cubic path; with closed=true its final endpoint must equal the first. curve.circle@1(center=[x,y,z], radius=r, segments=48) makes a closed
circle in the XZ plane. Sweep one convex closed profile loop along an open or closed path:
  path = curve.bezier@1(points=[[0,0,0],[0,0.4,0],[0.4,0.6,0],[0.4,1,0]], steps=32)
  section = profile.circle@1(radius=0.025, segments=24)
  pipe = solid.sweep@1(curve=path, profile=section)
  output pipe
  ring_path = curve.circle@1(center=[2,0,0], radius=1, segments=48)
  ring = solid.sweep@1(curve=ring_path, profile=section)
  output ring
Closed-path sweep currently has a sampled periodic seam check; arbitrary profile holes and concave
end-cap outlines are not supported.
More curve tools (all give a curve that solid.sweep or another curve tool takes):
  curve.spline@1(points=[[x,y,z], ...], steps=8, closed=false)  a smooth curve THROUGH every point (winding pipe, rope, rail, organic outline)
  curve.helix@1(center=[0,0,0], radius=0.5, end_radius=0.5, pitch=0.2, turns=3, segments_per_turn=16)  spring, coil, corkscrew, spiral rail; end_radius different gives a cone spiral
  curve.fillet@1(curve=route, distance=0.2, steps=6)  rounds the sharp corners of a polyline route (pipe runs, handrails)
  curve.resample@1(curve=path, count=32)  evenly spaced points;  curve.reverse@1(curve=path)  swaps start and end
solid.sweep also takes scale_end (size of the profile at the far end relative to the start, open curves: scale_end=0.1
makes a horn, tentacle or tail that tapers to a point) and twist (radians turned along the path; shows on a square or
other non-round profile: a twisted rope or ribbon). Example, a spring and a tapering horn:
  coil = curve.helix@1(radius=0.3, pitch=0.12, turns=6)   wire = profile.circle@1(radius=0.02, segments=12)   spring = solid.sweep@1(curve=coil, profile=wire)
  arc = curve.spline@1(points=[[0,0,0],[0.3,0.4,0],[0.8,0.6,0]], steps=10)   base = profile.circle@1(radius=0.1, segments=16)   horn = solid.sweep@1(curve=arc, profile=base, scale_end=0.1)
A sweep refuses a tube that would pass through itself or bend tighter than its radius (curve.sweep_curvature,
curve.sweep_self_intersection): use a thinner profile, a gentler or longer route, or fillet the corners first.
Loft at least two single convex profile loops with the same point count using even spacing:
  bottom = profile.circle@1(radius=0.2, segments=32)
  waist = profile.circle@1(radius=0.1, segments=32)
  body = solid.loft@1(bottom, waist, spacing=0.2)
By default a loft is straight between sections (cones and cylinders). For a smooth vase belly or bottle
neck add steps=6 (1 to 8): each point then follows a smooth cubic curve through its sections, which
still pass exactly through your profiles:
  body = solid.loft@1(bottom, belly, neck, rim, spacing=0.1, steps=6)
A blend that would turn a section non-convex is refused; lower steps or ease the change in radius.
Lofts currently cap only matching convex sections; holes, changing point counts and arbitrary section
correspondence are refused.
For a hollow vessel, build the outer loft and a smaller inner loft, move the inner loft up to leave a
floor, extend it above the outer rim, then subtract it with solid.difference@1. For a hollow pipe,
sweep a smaller profile on a path that extends past both ends of the outer pipe, then subtract it.
These compositions are supported by focused cases; keep a visible floor and positive wall thickness.
Bounded mesh deformations refine input triangles according to a sampled edge chord tolerance, with at
most two uniform refinement passes (16x triangles):
  twisted = xform.twist@1(body, angle=0.4, tolerance=0.01)
  tapered = xform.taper@1(body, top_scale=0.7, tolerance=0.01)
  bent = xform.bend@1(body, angle=0.3, tolerance=0.01)
Twist and bend angles are radians; taper scale applies at the top. Tolerance is metres and defaults to
0.01. An unmet tolerance returns geometry.deformation_tolerance instead of silently exceeding the
bound. Bend refuses a radius too small for the solid's width, and all of them use the geometry triangle
budget. More deformers, same rules (all act along the solid's height Y within its bounding box):
  leaning = xform.shear@1(body, x_shift=0.3, z_shift=0)       (top slides sideways: a leaning tower)
  flag = xform.wave@1(panel, amplitude=0.08, wavelength=0.5, direction="z", tolerance=0.005)   (a rippling flag or wavy wall)
  rock = xform.noise@1(blob, amplitude=0.06, scale=0.4, seed=3)   (smooth lumpy displacement; same seed, same shape)
  squashed = xform.lattice@1(body, divisions=3, offsets="1 2 1 0 -0.2 0; 1 0 1 0 0.1 0")   (free-form: move lattice control
  points "i j k dx dy dz"; i, j, k run 0 to divisions-1 along X, Y, Z from the minimum corner; bulges, squashes, bottles)
Physics (play mode): phys.body@1(part, kind="static"|"dynamic"|"kinematic", shape="auto", density=1, friction=0.6,
restitution=0, linear_damping=0, angular_damping=0.05) makes a part a physical object. kind="static" never moves (floors,
walls, terrain: mark them or props fall through); "dynamic" falls under gravity and collides (balls, crates, props, a
tower of blocks); "kinematic" is moved by hand and pushes dynamic bodies. In play mode (the Play button, or in a headset
the B or Y button) dynamic parts fall and collide, and any non-static prop can be picked up with the mouse or a
controller and thrown; Stop puts everything back where it was authored. Shapes: auto/convex fit the part's vertices;
box, sphere, capsule, cylinder fit its bounding box (the most stable: use them for crates, balls, pillars); mesh is the
exact surface for STATIC bodies only (terrain, floors, buildings). Place bodies in their starting pose (a ball 2 m up
falls 2 m). restitution=0.8 makes a bouncy ball; density sets weight (heavier pushes lighter); friction 0 is ice. A part
with its own anim.track, anim.parent or skin cannot be a body. A floor must be a static body or everything falls through:
  floor = solid.box@1(size=[10, 0.2, 10])   slab = xform.move@1(floor, by=[0, -0.1, 0])   ground = phys.body@1(slab, kind="static", shape="box")   output ground = ground
  ball = solid.sphere@1(radius=0.2)   up = xform.move@1(ball, by=[0, 2, 0])   bouncy = phys.body@1(up, shape="sphere", restitution=0.7)   output ball = bouncy
Joints (mechanisms): phys.joint@1(body_part, kind="hinge"|"slider"|"ball"|"fixed"|"spring", to="other_output_name", anchor=[x,y,z], axis=[x,y,z], ...)
goes AFTER phys.body on the part that moves and connects it to another part (give its OUTPUT name in to=; that part needs phys.body too)
or to the world (leave to empty). anchor is the pivot in world coordinates at the starting pose. hinge turns about axis
([0,1,0] vertical like a door, [0,0,1] horizontal like a swing); limited=true with low/high in degrees stops it (a door that opens 100
degrees); slider moves along axis (low/high in metres); ball turns freely (rope links, shoulders); fixed welds two parts (a weight on a rod);
spring needs anchor on this part, anchor_b on the other, rest_length, stiffness (5 gentle, 50 stiff), damping. motor_force above 0 with
motor_speed (degrees per second) makes a hinge drive itself: a windmill, a wheel, a fan. Whatever a joint hangs from must be kind="static"
or it swings too. Keep linked parts' masses comparable (density): a heavy weight on a very light rod or plank stretches the joint; give links
density 10 to 30 against a weight of 1 to 5. Pendulum: rod = phys.body, hinge to the ceiling bar (anchor at its top end, axis [0,0,1]), bob
= phys.body with a fixed joint to the rod. Do not place the swinging part inside the bar it hangs from: they will collide.
  door = phys.joint@1(door_body, kind="hinge", to="door_post", anchor=[-0.95, 0.9, -3], axis=[0, 1, 0], limited=true, low=-110, high=110, damping=1.5)
Collisions can drive logic: behavior.on@1(ball_body, trigger="collide", accepts="bell", sets="ring") fires when this body starts touching the part
named in accepts (leave it out for any body) and sets a flag; another part reacts with trigger="flag", watch="ring", action="follow" (a lamp's
glow track). The part with the collide trigger must be a body and must NOT carry its own anim.track (use flags or a signal target for the visible reaction).
Scene physics: phys.world@1(body_part, gravity=[0,-9.81,0], walker=false, height=1.7, radius=0.3, speed=3, jump=4.5) goes after phys.body on ANY ONE body
(the floor is natural) and sets gravity for the whole scene ([0,-1.62,0] Moon, [0,0,0] weightless, [0,-24,0] heavy planet). walker=true makes play
mode a walk-through: instead of flying, the viewer becomes a person who falls under gravity, stands on static bodies, is stopped by walls, steps
over kerbs up to 30 cm and jumps with Space (WASD walks, Shift runs, the mouse looks). The walker starts where the camera is, so put the camera
near the floor; make every floor, wall, stair and ramp a static phys.body with shape box (or mesh for terrain), or the walker falls through
it. Doorways must be wider than twice radius. Only one phys.world per scene counts. Walking is for the desktop viewer; a headset keeps its own movement.
  floor_world = phys.world@1(floor_body, gravity=[0, -9.81, 0], walker=true, jump=5)   output ground = floor_world
Physics is deterministic: the same scene plays out the same way every time. Phys bodies do not export to glTF (the
exported scene is the starting pose).

Landscapes: terrain.heightfield@1(size=[40,40], height=6, scale=12, octaves=4, roughness=0.5, seed=1, resolution=96,
base=0.5, style="hills"|"ridged", edge_falloff=0, flat_below=0) is a seeded fractal ground, one closed solid slab
centred on the origin (ground between y=0 and height, flat base below). edge_falloff=0.6 slopes the land to sea level at
the edges (an island); flat_below=0.3 flattens the lowest 30% into lake beds, which a thin blue box at that level fills.
Colour it with solid.material like any solid. resolution is grid cells per side (64 for a 20 m field, 128 for large).
Put things on it with inst.drop@1(placements, surface, offset=0, align=0, outside="remove"): each copy slides straight
down onto the highest ground under its origin (build the prototype with its base at y=0; offset=-0.05 buries a trunk's
foot; align=1 tilts each to the slope). Scatter inside the ground's footprint first:
  land = terrain.heightfield@1(size=[30, 30], height=4, scale=10, seed=2, resolution=96)   grass = solid.material@1(land, color=[0.25, 0.5, 0.2])
  trunk = solid.cylinder@1(radius=0.1, height=1, segments=8)   crowd = inst.scatter@1(trunk, count=80, width=28, depth=28, min_spacing=1, seed=4)
  trees = inst.drop@1(crowd, land, offset=-0.05)   output ground = grass   output forest = trees
Noise amplitude must stay well below the feature scale or the shape folds: a fold is refused with
geometry.deformation_fold, which means lower the amplitude or raise the scale.
Use bake to bake normal and ambient-occlusion maps from a high part onto a low part; unwrap it first with poly.unwrap_uv.

Water and atmosphere: water.surface@1(size=[x,z], level=0, depth=1, color=[0.08,0.34,0.5], opacity=0.6,
wave_height=0.05, wave_length=3, speed=1, seed=2, resolution=64) creates a bounded closed slab with a seeded
rest surface and animated viewer ripple. Its exported mesh stays static and it is not a physics body. Place the water
slab above the flat_below bed of terrain.heightfield; use output names for both so the lake and shoreline remain
editable. scene.sky@1(sun_elevation=35, sun_azimuth=0, sun_intensity=3, turbidity=3, ground_color=[0.37,0.29,0.2],
clouds=true, cloud_cover=0.35, seed=2) creates a procedural gradient background, sun and optional seeded clouds.
The sky contributes a shadow-casting sun and ambient hemisphere. If scene.environment is also authored, its colours
and ambient strength win while scene.sky continues to draw the background. Example:
  land = terrain.heightfield@1(size=[30,30], height=4, scale=10, flat_below=0.2, seed=3)
  lake = water.surface@1(size=[12,9], level=0.85, depth=0.6, opacity=0.6, seed=4)
  sky = scene.sky@1(sun_elevation=15, turbidity=5, clouds=true, cloud_cover=0.3)
  output ground = land   output lake = lake   output sky = sky

Polygon editing (faces, edges, vertices): poly.from_solid turns a solid into an editable mesh of its
triangles (poly.grid makes a flat quad grid), poly.merge_coplanar joins coplanar triangles back into flat
faces, poly.select chooses elements by a rule, the edit operators take the mesh and the selection, and
poly.to_solid turns a closed result back into a solid (an open mesh can be an output or take a material
directly). A box with a raised panel on top:
  box = solid.box@1(size=[1, 1, 1])
  mesh = poly.from_solid@1(box)
  flat = poly.merge_coplanar@1(mesh)
  top = poly.select@1(flat, by="normal", direction=[0, 1, 0], angle=0.05, expect="one")
  inset = poly.inset_faces@1(flat, top, distance=0.1)
  cap = poly.select@1(inset, by="attribute", attribute="role", value=1)
  raised = poly.extrude_faces@1(inset, cap, offset=[0, 0.2, 0])
  solid = poly.to_solid@1(raised)
  output solid
Make subdivision the last polygon-editing step in v1. Edit the cage first: tools such as planar-patch extrusion
and bevel may refuse on the curved quads or edge networks created by subdivision. Keep the cage output if you
need to return to editing. Smooth an editable cage with poly.subdivide (levels 1 to 4; boundary is edges or edges_and_corners):
  cage = poly.from_solid@1(box)
  smooth = poly.subdivide@1(cage, levels=2, boundary="edges_and_corners")
  output smooth
Whole-mesh cleanup (each takes a polygon mesh and acts on all of it): poly.recalc_normals (direction outward or inward; fixes inside-out
faces after a script or boolean), poly.smooth (factor, iterations; evens out a lumpy surface, also shrinks open borders a little),
poly.fill_holes (max_sides; closes boundary loops up to that size), poly.beautify (method area or angle; flips triangle-pair diagonals by
Blender's measure), poly.planar_faces (iterations, factor; flattens bent quads), poly.join_triangles (face_angle, shape_angle in radians;
turns triangles back into quads), poly.dissolve_degenerate (distance; removes tiny edges and sliver faces).
Tools behind a selection: poly.split_edges (selection of edges, element=edge; disconnects the faces along them so each side has its own
vertices, a hard seam), poly.rotate_edges (edges; direction; spins an edge between two faces to the other diagonal), poly.connect_vertices
(selection of vertices, element=vertex, at least two on one face; adds edges between them and splits the face). Example, a cube whose faces
can move apart: seams = poly.select@1(cube, element="edge", by="sharp"); apart = poly.split_edges@1(cube, seams).
The deformers xform.bend, twist, taper, shear, wave, noise and lattice also take a polygon mesh: every vertex is moved as it is (no
refinement), so give the mesh enough vertices for a strong bend (a denser poly.grid, poly.subdivide_edges); faces and UVs are kept. A flag:
panel = poly.grid@1(width=1.2, height=2, columns=12, rows=24); curved = xform.bend@1(panel, angle=80) (angles in degrees, bend at most 90).
Arrays: inst.along_curve@1(shape, path, count=22, align=true) puts shared copies evenly by length along a curve (open: one at each end;
closed: all the way round), each turned so its +X axis follows the curve and +Y stays up, so build the shape lengthwise along X (a fence
from a curve.expr path); inst.array@1(shape, count_x=5, spacing=[1.3, 0, 0], relative=true) reads the spacing as multiples of the shape's
own size (1.3 leaves 0.3 of its width between copies).
Sculpting and retopology: poly.sculpt@1(mesh, strokes="mask 0 1 0 0.3 1; draw 0 1 0 0.3 0.08; smooth 0 0.9 0 0.4 0.5; grab 0 1 0 0.3 0 0.1 0", symmetry="x", falloff="surface")
applies brush strokes in order (brush, centre x y z, radius, strength; grab takes a move vector instead): draw, inflate, smooth, flatten,
pinch, grab, mask and unmask. Masks persist in the \`sculpt_mask\` point attribute and protect later geometry strokes. Surface falloff follows
connected mesh edges to avoid pulling through the back of a head or shell; sphere falloff keeps ordinary 3D distance. Both fade to nothing
at the radius; at most 64 strokes; give it enough vertices (poly.subdivide, a denser grid). The centre must be near the surface.
poly.remesh_quads@1(mesh, faces=700) rebuilds a smooth organic mesh (a field mesh, a boolean result) as even
quads; it tries several seeds and refuses with the reason on hard-surface shapes with sharp edges (a box with a hole cut in it), so keep
those as they are. UVs: poly.uv_transform@1(mesh, offset_u, offset_v, scale, rotate, pivot) moves or scales UVs, all of them or only the
faces of a poly.select selection (an island); poly.uv_layout@1(mesh, size=1) makes the UV layout a flat mesh to look at (u right, v up).
curve.fit@1(points=[[x, y, z], ...], tolerance=0.02, corner_angle, closed, steps) fits smooth Beziers through sketched points, keeping
corners sharper than corner_angle; feed it to curve.tube.
Volumes (clay-like booleans that cannot fail on a bad mesh): volume.from_mesh@1(closed_mesh, voxel_size=0.05, band=6) samples a closed solid
or polygon mesh into a grid; volume.combine@1(a, b, operation="union" or "subtract" or "intersect", blend=0.1) joins grids of the same
voxel_size (blend melts them together over that many metres); volume.smooth@1(v, iterations, factor) rounds it; volume.offset@1(v,
distance) grows or shrinks it (at most about 0.9 times band times voxel_size); volume.to_mesh@1(v) makes the closed quad mesh, which is the
only way to output a volume. A blob: va = volume.from_mesh@1(a, voxel_size=0.04); vb = volume.from_mesh@1(b, voxel_size=0.04); u =
volume.combine@1(va, vb, blend=0.2); m = volume.to_mesh@1(u).
Cloth: cloth.drape@1(panel, pins, with=body, seconds=3) drops a flat polygon panel (poly.grid, moved above the body) onto a closed mesh or volume
and keeps the settled shape (a tunic, a sheet on a table); raise bend for folds, lower density for light cloth. cloth.sim@1(garment, pins,
pin_bone="name", collide_bones="a,b", wind=[2,0,0], drag=0.5) makes a garment live cloth that moves while the scene runs (a cape behind a running
figure, a banner): pins is a poly.select element=vertex selection held to the pin bone, collide_bones are rig.bone parts it cannot pass through;
the garment must have no tracks of its own, up to 6000 points.
Hair, liquids, breaking: the macro kits autorig_human and hair_cards (list_operators ids=["kit.autorig_human"]; paste the source once at the top).
autorig_human: body = body.human@1(); human_rig("body", 1, ""); human_skin("body", "person", 1, 0.86, 0.66, 0.52) makes a rigged, skinned
character (every bone its own part and output named like the bone); human_ik("l") adds reach targets; to animate one bone name it in the last
argument of human_rig and write its three lines yourself. hair_cards("hair", "head", 0, 1.62, 0, 0.1, 0.35, 28, 0.045, 3) makes strips of hair
that swing from the head bone. anim.retarget@1(target_bone, source_bone, scale=1.5, mirror=true, clips="walk") copies a bone's clips onto another
rig (scaled, left to right). fluid.sim@1(min, max, spacing=0.05, bounds_min, bounds_max, viscosity=0.02) is a live liquid, 1000 to 8000 particles,
its surface rebuilt every frame (put solid.material after it); collide_bones lets a moving bone stir it. poly.fracture@1(shape, pieces=8, piece=k,
seed, voxel_size, gap) is piece k of a shape broken along a Voronoi pattern: loop over k, give each piece phys.body kind=dynamic shape=convex.
Edge-loop tools (select the loop's edges with poly.select element=edge, for example by="height" for a ring at one height): poly.space_loops
(factor, interpolation; even spacing along the ring), poly.relax_loops (iterations, interpolation, even_spacing; smooths it) and
poly.circularize (factor, radius 0 to fit one, angle, flatten, regular; an oval hole becomes round). A tube mouth made round:
top = poly.select@1(tube, element="edge", by="height", axis="y", height=1); round = poly.circularize@1(tube, top, factor=1).
poly.program runs a short program once for every vertex (the same expression language as field.expr: out = ..., p position, n
smoothed normal, uv, i vertex number, count, attr("layer") reads a per-vertex number layer, noises perlin/fbm/ridged/voronoi). mode says
what out is: offset (vec3 added to p), position (vec3), along_normal (a number) or layer (a number stored as layer_name for a later
program to read). Faces stay as they are; the result draws as a smooth surface. A rippled hill sheet (angles in the text language are degrees):
  grid = poly.grid@1(width=8, height=8, columns=96, rows=96)
  land = poly.program@1(grid, mode="offset", code="q = p.xy - vec2(0.0, 4.0); out = vec3(0.0, 0.0, 0.9 * fbm(vec3(q.x, q.y, 0.0) * 0.35, 4.0, 3.0))")
  flat = xform.rotate@1(land, angles=[-90, 0, 0])
A mistake names its line and column; a program of the wrong result type (vec3 wanted, float given) says both types.
poly.script is the escape hatch for logic the nodes cannot express (a maze, a growth rule): sandboxed JavaScript that ends with a Mesh
(const m = new Mesh(); const a = m.vertex(x, y, z); m.face(a, b, c); m), can read a linked mesh as input (points, faces), has seeded
Math.random, no clock, files or network, a work budget, a memory cap and 200,000 points or faces at most; errors name line and column.
Inside the script op(id, params, inputs) calls any other operator and returns its result as data (a mesh is points and faces, a curve is
points and closed, a number is number): s = op('solid.difference@1', {}, {in: [a, b]}) where a and b came from earlier op calls; params are the
node's parameters by name in metres and radians, an array under a port feeds a variadic port, at most 128 calls, and a script cannot call a script.
Keep it on one line per statement group inside code="..." with single quotes, or use \n between lines. Prefer poly.program and the
editing nodes first; use the script when a loop with state or a data structure is the natural way to say it.
The same language writes shapes from scratch. curve.expr: a curve from t (0 to 1; also i, count), sampled samples times, closed=true
for a loop; feed it to curve.tube. poly.surface_expr: a surface from u and v (0 to 1) over columns by rows quads; which side is the front
follows from the formula, so set flip=true if it looks inside out. inst.program: copies placed by programs of i, count and f (0 to 1
across the copies): code (vec3 place), scale_code (number above 0), yaw_code (degrees). A helix tube, a torus and a spiral of pillars:
  helix = curve.expr@1(samples=160, code="a = t * 25.13; out = vec3(0.7 * cos(a), t * 2.5, 0.7 * sin(a))")
  coil = curve.tube@1(helix, radius=0.07, sides=10)
  torus = poly.surface_expr@1(columns=64, rows=32, flip=true, code="r = 1.0 + 0.35 * cos(v * 6.2832); out = vec3(r * cos(u * 6.2832), 0.35 * sin(v * 6.2832), r * sin(u * 6.2832))")
  spiral = inst.program@1(pillar, count=40, code="out = vec3(cos(i * 0.45) * (0.5 + f * 2.5), 0.3, sin(i * 0.45) * (0.5 + f * 2.5))", scale_code="out = 1.0 - 0.5 * f", yaw_code="out = i * 25.8")
poly.grid creates normalized corner UVs; subdivision carries paired F32 corner u/v UVs with seams and F32 point attributes (up to 8 channels) through
the refined mesh. F64 point attributes, other corner/edge attributes and face hole tags have typed refusals.
poly.select: element = face | edge | vertex | loop; by = all | normal (direction, angle) | box (box_min,
box_max, tested at each element's centre) | attribute (attribute name, value) | sharp (edges, angle);
expect="one" refuses when several match, and choosing nothing refuses (selection.lost). A selection works
only on the mesh it was made from. Inset and extrude tag what they make: attribute role is 1 for the inner
or moved cap and 2 for ring and side faces. A facade: poly.grid, select all faces, poly.inset_faces,
select role 1, poly.extrude_faces with individual=true. Edits: poly.flip_faces, poly.weld,
poly.merge_coplanar, poly.dissolve_faces, poly.dissolve_edge and poly.dissolve_vertex (select one),
poly.delete_faces (leaves an open boundary), poly.extrude_faces (a planar patch moves as one),
poly.inset_faces (planar convex faces), poly.bridge_loops (element="loop": exactly two loops with the
same vertex count), poly.bevel_edges (width, segments 1 to 4; edges must be disjoint, between planar
faces and meet three faces at each end, so merge_coplanar a solid first). Refusals name the face, edge or
vertex and keep a code such as poly.inset_too_large; the document is left unchanged.

Blender-backed polygon tools (work on any layout, not only simple ones; merge_coplanar a solid first so faces are whole polygons):
poly.bevel_network (rounds ANY selected edges at once: a whole box, a loop, edges meeting at a corner; width, segments 1 to 16,
profile 0.1 to 1, offset = offset | width | depth | percent; select edges with poly.select element="edge", by="sharp" for the hard
edges, or by="box"). Use poly.bevel_network instead of poly.bevel_edges whenever edges touch, and instead of solid.round when
you want a small round on a polygon model. poly.inset_region (thickness, depth, even, boundary, outset), poly.extrude_region
(offset vector, on selected faces), poly.solidify (thickness; no selection means every face; turns an open surface such as a
poly.grid into a slab), poly.poke (offset, center; a pyramid on each selected face), poly.wireframe (thickness; struts like a cage),
poly.subdivide_edges (cuts 1 to 8 on selected edges), poly.dissolve_limit (angle; merges flat faces), poly.decimate (ratio of
faces to keep, e.g. 0.25: a lower-detail LOD version of a dense mesh; start from poly.from_solid of any solid), poly.unsubdivide
(iterations 1 to 4; undoes a subdivision to recover a coarse cage), poly.symmetrize
(axis, keep; mirrors one half onto the other), and poly.unwrap_uv (writes corner UVs u and v packed in the
unit square; with no selection it cuts seams by itself: it splits the surface into near-flat charts within seam_angle (a box gives six, a sphere several patches) and opens tubes; select seam
edges only to override. Read its warnings: poly.uv.stretched means add seams or lower seam_angle, poly.uv.flipped and poly.uv.overlap say where
it folds; poly.uv_unwrap_failed means the surface cannot be flattened as is). All keep face, point and corner attributes. poly.inset_region and poly.extrude_region tag what they make in the face attribute role
(1 = the inner face of an inset or the moved cap of an extrusion, 2 = the ring or the side walls), so pick the inner face with
poly.select by="attribute", attribute="role", value=1: no coordinates needed. For any other face use a TIGHT box around its centre
(a box big enough to hold neighbouring faces' centres selects them too and gives a broken result). Check results with measure instead of
estimating: the short report gives volume, size, triangles, watertight and wall thickness, and says what to do when a part is not closed.
A house with a recessed door, rounded edges and a separate red roof (two colours kept
through the union):
  wall_box = solid.box@1(size=[4, 2.5, 3])
  tris = poly.from_solid@1(wall_box)
  flat = poly.merge_coplanar@1(tris)
  front = poly.select@1(flat, by="normal", direction=[0, 0, 1], angle=5deg)
  door_panel = poly.inset_region@1(flat, front, thickness=0.6)
  door_face = poly.select@1(door_panel, by="attribute", attribute="role", value=1)
  recessed = poly.extrude_region@1(door_panel, door_face, offset=[0, 0, -0.08])
  edges = poly.select@1(recessed, element="edge", by="sharp", angle=30deg)
  soft = poly.bevel_network@1(recessed, edges, width=0.03, segments=2)
  walls_solid = poly.to_solid@1(soft)
  walls = solid.material@1(walls_solid, color=[0.87, 0.74, 0.55], roughness=0.8)
  slab = solid.box@1(size=[4.4, 0.1, 3.4])
  slab_tris = poly.from_solid@1(slab)
  slab_flat = poly.merge_coplanar@1(slab_tris)
  slab_top = poly.select@1(slab_flat, by="normal", direction=[0, 1, 0], angle=5deg)
  peak = poly.poke@1(slab_flat, slab_top, offset=1.4)
  roof_solid = poly.to_solid@1(peak)
  roof_lit = solid.material@1(roof_solid, color=[0.55, 0.18, 0.12], roughness=0.6)
  roof = xform.move@1(roof_lit, by=[0, 1.3, 0])
  house = solid.union@1(walls, roof)
  output house
A raised rounded panel on a block:
  box = solid.box@1(size=[1, 1, 1])
  tris = poly.from_solid@1(box)
  flat = poly.merge_coplanar@1(tris)
  top = poly.select@1(flat, by="normal", direction=[0, 1, 0], angle=0.05)
  inset = poly.inset_region@1(flat, top, thickness=0.15)
  inner = poly.select@1(inset, by="attribute", attribute="role", value=1)
  raised = poly.extrude_region@1(inset, inner, offset=[0, 0.1, 0])
  hard = poly.select@1(raised, element="edge", by="sharp", angle=0.5)
  rounded = poly.bevel_network@1(raised, hard, width=0.02, segments=3)
  solid = poly.to_solid@1(rounded)
  output solid

More polygon tools. poly.round_edges@1(mesh, width=0.02, segments=3, angle=30deg) rounds every hard edge in one
step (merge_coplanar a solid first). poly.select by="height" (height=, axis=, tolerance=) or by="extreme" (side="max"
is the top, "min" the bottom, axis="y") picks elements without coordinates. poly.bisect@1(mesh, point=[0,0,0],
normal=[0,1,0], keep="below", fill=true) cuts with a plane and keeps a side. poly.mirror@1(mesh, point, normal,
merge=true) adds the mirror image and welds the seam; mirror an OPEN half (bisect with fill=false), or the two caps
stay as inner faces. poly.spin@1(profile, center=[x,0,0], axis=[0,1,0], angle=360deg, steps=32, merge=true) revolves
faces or selected edges like a lathe; a full turn with merge=true gives a closed ring. poly.loop_cut@1(mesh, at=[x,y,z],
cuts=1) cuts the ring of quads through the edge nearest "at". poly.pack_uv after poly.unwrap_uv packs the islands;
poly.lod@1(mesh, levels=2, level=1, ratio=0.5) is a simplified copy, and parts named base_lod0, base_lod1 ... export
as one glTF LOD chain. solid.hull and solid.collision_boxes make collision shapes; inst.scatter_on scatters copies over
a solid's surface. To check or bake a result use the measure, snapshot and bake tools.

Groups (build once, use many times): a group is an assembly with its own nodes, a result and the
parameters you choose to expose; an instance is one use of it. Write the group first, then instances:
  group wheel
    tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)
    hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)
    spoke = solid.box@1(size=[0.22, 0.03, 0.03])
    spokes = xform.radial@1(spoke, count=4, radius=0.14)
    rim = solid.union@1(tyre, hub, spokes)
    expose radius = tyre.major
    result rim
  end
  front = wheel()
  big = wheel(radius=0.4)
  moved = xform.move@1(big, by=[1, 0, 0])
Rules: the nodes inside a group read only each other; "result" names the member that is the group's
result (it must be a solid); "expose name = member.param" lets an instance override that parameter
with name=value (the member's own limits apply). An instance is written like a node whose operator is
the group's name (no dot, no @version), and it can be moved, combined, given a material or made an
output like any solid. A group may take solids as inputs: inside it write
  mount = group.input@1(name="mount")
and bind it on an instance with mount=post. Changing a node inside a group changes every instance;
equal instances are computed once. Group names are plain words (letters, digits, underscore).

Rigid alignment: xform.align@1(solid, from=[x,y,z], to=[x,y,z], pivot=[0,0,0]). Directions must be
nonzero; their magnitude is ignored. The pivot stays fixed. Opposite directions use a deterministic
half-turn.
Assembly sockets: frame.add@1(solid, name="mount", position=[x,y,z], rotation=[x,y,z]) attaches a
named rigid frame (angles in degrees in text) to a solid. xform.attach@1(source, to=parent,
socket="base", to_socket="mount") matches position and orientation; names travel with evaluated
results through groups and do not change group interface versions. Missing sockets return a named
diagnostic listing available sockets. xform.mirror namespaces reflected sockets as mirror.name.
frame.add and xform.attach accept solids only; realize an instance set before attaching it.
Shared placements (no union): inst.array@1(solid_or_set, count_x=1, count_y=1, count_z=1, spacing=[x,y,z]),
inst.radial@1(solid_or_set, count, radius=0), inst.repeat@1(solid_or_set, count, step=[x,y,z], rotate_step=[x,y,z]),
inst.mirror@1(solid_or_set, axis="x"). Sets share one prototype and allow at most 16384 placements.
Move/rotate/scale also work on sets. A set is a valid output. Use inst.realize@1(set) only when one
boolean solid is needed; triangle and memory limits apply before union. Example fence:
  post = solid.box@1(size=[0.05,1,0.05])
  posts = inst.array@1(post, count_x=20, spacing=[0.2,0,0])
  output fence = posts
For a later boolean operation, write fused = inst.realize@1(posts) and feed fused to the boolean.

Colour and finish: solid.material@1(solid, color=[r,g,b], metallic=0, roughness=0.5, opacity=1, emissive=[0,0,0])
gives a solid a surface (colour is sRGB 0..1 as in a colour picker, metallic 0 plastic/paint/wood to 1
bare metal, roughness 0 mirror to 1 matte, opacity 0 clear to 1 solid, emissive glows above 1). Use opacity=0.3 for tinted glass or 0.6 for murky water; for example, a window pane can use glass = solid.material@1(pane, color=[0.55,0.8,1], opacity=0.3), and a water slab can use water = solid.material@1(surface, color=[0.1,0.4,0.8], opacity=0.6). The shape is unchanged. Put it on
the solid you output, for example:
  red = solid.material@1(vase, color=[0.85, 0.08, 0.08], roughness=0.3)
  output vase = red
Transforms and booleans keep the first input's surface. solid.material also takes a placement set
(inst.array, inst.repeat, ...) and colours every placement without realizing it. Differently coloured
parts keep their colours through solid.union, solid.difference, solid.intersect, moves, rotations,
scales, mirrors and arrays, so one fused solid can be many colours (a cutter without a colour of its own
leaves the body's colour on the faces it cuts; give it one to inlay). A material put on top of such a
solid recolours all of it. Twists, tapers and bends keep only the first input's colour.

Procedural textures: use texture_nodes with a query to search or node to inspect a Material Maker node.
Attach a bounded semicolon-separated DSL program with solid.texture@1. Example:
  textured = solid.texture@1(painted, program="grain = fbm(scale_x=5, scale_y=5); albedo = colorize(grain, gradient=[0:#553322, 1:#d6a15b]); roughness = grain")
  output textured
The channels are albedo, roughness, metallic, height (converted to tangent-space normals), and emissive.
Texture memory: the viewer holds a fixed amount (192 MiB) of baked maps, 1024, 512 or 256 pixels square. solid.texture@1 takes
size="auto"|"256"|"512"|"1024" (auto is 1024): give small or distant things (a leaf, a pebble, a prop) 256 or 512 and a scene can
hold hundreds of textured parts, where 1024 maps fit about 36 in all. When the memory still runs out the viewer shrinks the biggest
maps first (warning texture.viewport_shrunk), and only at 256 does it drop channels (glow, then shininess, then bumps; warning
texture.viewport_budget). Identical programs share one map. The exported .glb is not affected.
Shader expressions: the expr node draws a texture from a few lines of code, and composes with any other node. In the graph text
write the program string with escaped quotes (\" inside program="..."):
  program="n = expr(code=\"w = p; for i in 0..6 { w = rot(w, 0.7) * 1.9 + snoise(w * 2) * 0.4 }; out = hsv(w.x, 0.6, 0.8)\"); albedo = n"
Language: name = expression lines (newline or ;), the last must be out = float (grey), vec3 or vec4. Built in: p (uv 0..1), q (centred
-1..1), t (0), PI, TAU; floats and vec2-vec4 with swizzles (.xy .rgb); + - * / %; sin cos tan asin acos atan abs sign floor ceil fract
sqrt exp log pow min max clamp mix step smoothstep mod length distance dot cross normalize reflect; and hash(vec2), noise(vec2) 0..1,
snoise(vec2) -1..1, fbm(vec2) 0..1, hsv(h, s, v), rot(vec2, angle), voronoi(vec2) = vec2(nearest, second-nearest cell distance; unit cells:
.x is a pore or a scale, .y - .x the wall between cells), stretch(vec2, angle, ratio) (noise of it runs in streaks along angle: fbm(stretch(p*20, 0.4, 6)) for skin
creases), warp(vec2, amount) (bends straight or cellular patterns organic). Loops: for i in 0..N { ... } with a whole-number N up to 64,
two deep at most (256 passes in all). No if, while, comparisons or texture reads (use step, mix and smoothstep). At most 600
characters. Variables first set inside a loop stay inside it. A mistake names the line, column and the fix.
Photographs: image(name="Concrete034_Color", repeat_x=4, repeat_y=6) is a node holding one map of a shipped CC0 material (ADR 0056), repeated over the texture; it composes with blend,
colorize and the channels like any node, so the photograph carries the realism and expr nodes layer windows, grime and wear on top. Materials (ambientCG, CC0), each with _Color, _Roughness,
_Normal: Concrete034, Concrete048, Bricks113A (sand stone blocks), Bricks106A, Plaster001, Tiles141, CorrugatedSteel009, Asphalt033, Road012A, PavingStones151. Use the normal map as the
\`normal\` channel, the roughness map as \`roughness\`. blend(s1, s2, a, blend_type=normal|multiply, amount=1) layers s1 over s2 with node \`a\` as the opacity (make an \`a\` of 1 with
expr(code="out = vec3(1.0)")). Whole-program form for artist-made clothes and hair: image(albedo="long01_diffuse", normal="..."). A worked example is tools/worlds/city_district.py.
Cut-outs: solid.foliage@1(shape, alpha_cutoff=0.5, sway=0, flutter=0) after solid.material and solid.texture cuts the surface away
where its albedo's alpha is under the cutoff, in the picture, in its shadow and in the exported glTF (alphaMode MASK), and draws it
two-sided. End the expr code with a vec4 whose last number is the alpha to draw the shape: out = vec4(0.1, 0.5, 0.1, 1.0 - step(0.8,
length(q * vec2(1.0, 1.4)))) is a leaf. The shape needs UVs over the whole surface (poly.grid gives one quad with them, or unwrap a
mesh with poly.unwrap_uv); a solid.box has its six faces in one atlas, so a shape drawn into it is split across them. Wind: sway
bends the whole shape (tall parts more, so give it vertices to bend: a flat quad or box only shears), flutter trembles the surface;
both 0 is an ordinary surface. The wind itself is set on scene.environment (or scene.sky) with wind_direction (degrees, 0 is +x,
90 is +z) and wind_strength (0 still, 0.5 gentle breeze, 2 gale); the viewer shows it, the exported glTF does not.
Global illumination: scene.environment(gi=true) (also on scene.sky) lights the scene with traced light probes instead of the
constant sky ambient, so a room lit through a window is dark away from it and walls pick up neighbouring colour. It is traced a
little each frame after an edit (the scene looks as before until done); gi_spacing in metres (0 sizes it from the scene),
gi_bounces 1 to 4, gi_rays 32 to 512. Static geometry only.
Export glTF bakes these in the browser, requires WebGL2 plus EXT_color_buffer_float, and embeds PNG maps.
The viewport shows these baked maps on the model (up to 64 maps in all; later parts lose emissive, then
metallic-roughness, then normal maps first, with a warning). Give a part UVs with poly.unwrap_uv before
texturing it if it is a polygon mesh; other parts get a box projection.

Animation: anim.track@1(shape, property, keys, interpolation, loop, pivot) makes a shape move and passes it
through unchanged. property is position (metres), rotation (degrees about X, Y, Z) or scale (factors);
keys are "seconds:x,y,z" entries separated by semicolons, at least two, times increasing, every entry with
all three values; interpolation is linear, smooth (default, eases in and out) or step; loop repeats (default)
or holds the last key; pivot is the point, in the shape's own coordinates, that rotation and scale act about. For a part that is a
bone (body.bone or rig.bone) write pivot_at="head" instead of typing the joint: the track turns about the bone's own head (its joint);
pivot_at="tail" uses the far end. anim.wave has it too. A part that is not a bone is refused.
Chain several anim.track nodes to move and turn together. Examples:
  swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 1.5:0,90,0", pivot=[-0.45, 0, 0], loop=false)
  output swing
  (a 0.9 m wide door hinged at its left edge swings open 90 degrees about Y in 1.5 s and stays open)
  spin = anim.track@1(blades, property="rotation", keys="0:0,0,0; 1:0,360,0", interpolation="linear")
  (a fan turning once a second, forever; put the pivot on the axle if it is not at the shape's origin)
The viewer plays it automatically, and export_glb writes it as a glTF animation. Put the track node after
solid.material or solid.texture (order does not matter, both are kept) and make it the output.

Colour: anim.track property="color" (keys "seconds:r,g,b", each 0 to 1) fades the shape's surface colour between the
keys and replaces the colour it was authored with, for example a light that turns from red to green:
  tint = anim.track@1(lamp_body, property="color", keys="0:1,0,0; 1:0,1,0", interpolation="linear", loop=false)
Curves (from Blender): interpolation="bezier" makes ONE smooth curve through all the keys (Blender's automatic
clamped handles): motion flows through a middle key instead of easing to a stop at it, never overshoots the keys,
and starts and ends gently. Use it for any path with three or more keys (a swinging arm, a camera-like sweep, a
waving flag). "back" rushes a little past the next key and returns, "bounce" bounces against it, "elastic" springs past
it and settles (use them with two keys, e.g. a pop-up or a dropped ball); "smooth" eases to a stop at every key.
More motion: interpolation also accepts ease_in (slow start) and ease_out (slow finish). property="visible" (keys
"seconds:1" shown, "seconds:0" hidden, use interpolation="step") hides and shows a part, and a hidden part stops
giving light. anim.wave@1(shape, property, amplitude, base, period, phase, shape, pivot) is for endless oscillation
without keyframes: value = base + amplitude * wave, once per period, wave is sine (default) or triangle. Use it for
bobbing, swaying, pulsing and flickering; use anim.track for one-off or timed motion. Examples:
  bob = anim.wave@1(ball, property="position", amplitude=[0, 0.15, 0], period=2)   (floats 15 cm up and down)
  pulse = anim.wave@1(lamp, property="glow", amplitude=[0.3, 0, 0], period=0.4, shape="triangle")   (flicker)
  sway = anim.wave@1(tree, property="rotation", amplitude=[0, 0, 3], period=4, pivot=[0, 0, 0])
Rotation and scale waves act about pivot; scale and glow centre on 1 unless base is set. phase (0 to 1) offsets the
start so two shapes can move in opposite phase.

Clips and state machines (a character that does different things): give each motion a clip name with clip="walk" on its
anim.track nodes (the clip is the set of tracks sharing that name on all parts; a track with no clip is the main clip, which plays
all the time). Nothing chooses between named clips by itself: attach anim.machine@1(root_part, program="...") to the ROOT part of the
hierarchy (the part the others follow with anim.parent; a plain solid with no tracks is fine) and it picks the clips and blends them
by weight. The program is a few statements separated by semicolons (or newlines):
  entry ground
  state ground blend=speed idle@0 walk@1.5 run@4     (a blend space: the clip follows the number parameter speed)
  state jump clip=jump once                          (one clip; once plays it a single time, loop is the default)
  transition any -> jump when trigger:jump fade=0.1  (any state; a trigger is raised by the game or the page)
  transition jump -> ground exit=0.9 when grounded=true
Conditions are speed>0.5, speed<0.5, grounded=true and trigger:name, joined with &. fade is the crossfade in seconds; exit is the
fraction of a one-shot clip that must have played. Parameters are named freely; the viewer sets speed from how fast the root part
moves, a game rule raises a trigger with effect:anim:jump@tag:hero, and the page can set any parameter. A clip the machine plays
that no track defines is reported as anim.machine.missing_clip.
Layers and events in the same program: a LAYER plays a clip over the machine's own blend on some parts only, so a character can WAVE while it
walks: layer wave clip=wave weight=waving mask=arm_l,arm_r fade=0.2 (weight is a number 0 to 1 or a float parameter, here "waving", which you or a
game set with animation.set_float; the clip fades in and out over fade seconds and restarts each time; mask names output parts, each with
everything that hangs from it, and no mask means every part the machine drives; add the word additive to ADD the clip's change from its first
frame to the walk instead of replacing the masked parts' motion; the word once plays it a single time; speed=1.5 plays faster). A part with no
track in the layer's clip is left alone. At most 4 layers. An EVENT is a named moment in a clip: event walk footstep at=0.25 (and at=0.75 for
the other foot); every time a playing clip passes it the world reports the event "anim.footstep" with part (the machine's part) and clip,
which a script hears with on("anim.footstep", fn): use it for footstep sounds, dust puffs (world fx.burst), hit frames and spawn points. A clip
that is nearly faded out stays quiet. At most 64 events.
CHECK MOTION YOURSELF: you cannot feel a walk, so look and measure. snapshot with times=[0,0.25,0.5,0.75] (or states=[{"speed":0},
{"speed":1.5}] for a machine) shows the clip at those moments in one picture; check.motion@1(foot_part, feet="leg_a,leg_b",
clip="walk", duration=1, speed=1.4) samples the clip and warns when a foot slides while it should be planted (a planted foot moves
backwards at the speed the body moves forwards) or when the loop pops at its seam (make the last key equal the first and loop=true).
Fix what it names, then look again.

Scripts (behaviour you write): when no node says what you want, write it in JavaScript. Two ways. The script tool (action start, name,
source) runs a script NOW against the running world and lasts until you stop it or the page reloads: use it to try something. The node
script.js@1(source="...", name="bobbing") saves the same script IN THE DOCUMENT: it starts when the world loads, restarts when you edit
its source (set_param), stops when you disable or remove it, and undo brings it back; use it for behaviour that belongs to the world. The
source is the body of an async function with world, on, every, log, time, random (get_guide topic="script_api" lists them); in the
node's string write newlines as \n and quotes as \". Scripts move parts through the world tool's components (a part's Transform is an
offset from where it was built; a part with animation tracks or a parent has its Transform set by the animation, so script static parts),
should send what they change every frame in ONE world.batch, and should keep their own state in variables. They hear the world with on("click", fn) or on("click:crate_left", fn) (also hover, unhover, press, release, grab, drop; e.part is the part's name) and a game rule's effect with on("game.message:name", fn); on a desktop the first click on the view locks the mouse (look mode, the centre dot picks, Esc releases), and only a released cursor hovers and clicks parts under it, so clicks in a headless test land on parts only while the cursor is free. A script can spawn visible things at run time with world.call("world.spawn_entity", {shape: {kind: "box", size: [0.1, 0.1, 0.1], color: [1, 0.5, 0]}, position: [x, y, z]}) (also kind sphere with radius, cylinder with radius and height; emissive, metallic, roughness, opacity) and remove them with world.despawn_entity: bullets, debris, markers; spawning is limited to 2000 live entities, so despawn what you finish with. world.call("animation.set_float", {part, name, value}), animation.set_bool and animation.trigger drive an animation state machine. A script runs in a worker
with no network and is stopped if a frame takes over 250 ms. Look at the result (snapshot with times) and read the logs (script tool,
action logs) before you say it works.

Bendy things (skinning): a mesh that bends as bones move. A BONE is an ordinary part (a small box inside the mesh) with
rig.bone@1(part, head=[x,y,z], tail=[x,y,z]) giving its rest segment (head = the joint it turns about, tail = its far end,
world coordinates); move bones with anim.track / anim.wave (rotation with pivot at the bone's head) and join them with
anim.parent (a forearm bone follows an upper-arm bone). The skin is rig.skin@1(mesh, bones="upper_bone; lower_bone")
(OUTPUT names of the bone parts, in any order, at most 32): every vertex then follows the nearest bones with a smooth
blend. Cut the mesh into rings first so it has vertices to bend (poly.from_solid, poly.merge_coplanar, select the lengthwise
edges with poly.select element="edge" by="box", poly.subdivide_edges cuts=7) or the bend is one long stretch; falloff (1 to 8,
default 3) makes the blend tighter, influences=1 makes rigid pieces. A skinned mesh must NOT have its own anim.track, parent
or behaviour: move its bones. Bones and the skinned mesh are all outputs. Example, a tube that bends at its middle:
  hip = solid.box@1(size=[0.05, 0.05, 0.05])   lower_pos = xform.move@1(hip, by=[0, -0.25, 0])
  lower_rig = rig.bone@1(lower_pos, head=[0, -0.5, 0], tail=[0, 0, 0])
  swing = anim.track@1(lower_rig, property="rotation", keys="0:0,0,0; 1:0,0,90", interpolation="bezier", pivot=[0, 0, 0], loop=false)
  output lower_bone = swing
  (and an upper bone the same way without the track, output upper_bone)
  skinned = rig.skin@1(cut_tube, bones="lower_bone; upper_bone")   output sleeve = skinned
Skinned meshes export to glTF with a real skin (joints are the bone parts).
Twisting limbs: add blend="dual_quaternion" to rig.skin when a bone turns about its own length (a forearm roll, a twisting
torso); the default linear blend pinches the mesh to a thread at the joint, dual_quaternion keeps its width (the glTF
export is always linear). To fix a joint that bends badly, paint weights with paint="bone x y z radius strength; ..."
(at most 64 stamps): strength 1 hands vertices under the brush centre to that bone, -1 takes the bone away from them,
fading to nothing at the radius, for example paint="forearm 0 0.5 0 0.2 1; upper_arm 0 0.5 0 0.2 -1".

Reaching (inverse kinematics): rig.ik@1(target, bones="upper_arm; forearm; hand", iterations=32) makes the shape you give
it a TARGET that a chain of bone parts reaches for: as the target moves (animate it with anim.track, or make it grabbable
with behavior.on trigger="hold") the chain bends so its tip stays on the target; out of reach it stretches straight at it.
Build the chain like a skeleton: each bone is a part with rig.bone (head = its joint, tail = its far end, world
coordinates), each bone after the first is anim.parent-ed to the one before, and each starts exactly where the previous
one ends (tail of one = head of the next). Bones are listed ROOT FIRST by their output names and must all be outputs.
Do not animate the bones yourself: the solver sets them. Make bone parts visible (they are the arm) or hide them
and skin a mesh to them (rig.skin). Example, a two-bone arm that follows a ball:
  upper = rig.bone@1(upper_box, head=[0, 0, 0], tail=[0.5, 0, 0])   output upper_arm = upper
  fore = rig.bone@1(fore_box, head=[0.5, 0, 0], tail=[1, 0, 0])
  fore_child = anim.parent@1(fore, parent="upper_arm")   output forearm = fore_child
  path = anim.track@1(ball, property="position", keys="0:0,0,0; 2:-0.5,0.6,0", loop=true)
  reach = rig.ik@1(path, bones="upper_arm; forearm")   output target = reach
IK is not part of a glTF export (the bones export at rest).

Assemblies: anim.parent@1(shape, parent="<output name of another part>") makes a shape move with that part (a
forearm on an upper arm, a wheel on a car). Author every part at its rest position in the same world
coordinates; a child's own anim.track still works on top (an elbow bending about its own pivot, in world
coordinates). Both parts are separate outputs, and the parent is named by OUTPUT name. Example:
  shoulder = anim.track@1(upper, property="rotation", keys="0:0,0,0; 1:0,0,45", pivot=[0, 0.3, 0])
  output upper_arm = shoulder
  attached = anim.parent@1(lower_placed, parent="upper_arm")
  elbow = anim.track@1(attached, property="rotation", keys="0:0,0,0; 1:90,0,0", pivot=[0, -0.3, 0])
  output forearm = elbow
Chains up to 8 deep work; a missing name, a loop, or following itself is dropped with a warning.
Glow: anim.track property="glow" (keys are "seconds:factor", one number) scales the shape's emissive colour AND the
light it emits (solid.material emissive + emits_light=true), so a lamp is a bulb with a glow track that starts at 0:
  fade = anim.track@1(bulb_lit, property="glow", keys="0:0; 0.3:1", loop=false)
  listen = behavior.on@1(fade, trigger="signal")
and a switch with target="<lamp output name>" turns it on and off. Glow, colour and visible tracks also export to glTF
(through the KHR_animation_pointer, KHR_materials_emissive_strength and KHR_node_visibility extensions); a part with a glow
or colour track gets a material of its own, so it never changes other parts that share its look.

Behaviours: behavior.on@1(shape, trigger, action, radius) makes an animated shape REACT instead of play on a
loop. Put it after the anim.track nodes (it refuses a shape with no track, and a shape can have only one
behavior.on). trigger is select (the person clicks it in play mode) or near (the viewer comes within radius
metres of its centre, default 2); action is toggle (default: runs the clip forward, then backward on the next
trigger, so a door opens and closes; with near it opens as you come close and closes as you leave) or play
(runs the clip once and holds the end). The shape rests at the first key until triggered, and its tracks hold
at both ends whatever loop says. Example, a door that opens when clicked:
  swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 1:0,90,0", pivot=[-0.45, 0, 0], loop=false)
  react = behavior.on@1(swing, trigger="select", action="toggle")
  output react
A switch that runs ANOTHER part: give the sender target="<output name of the other part>" and make the other
part a listener with trigger="signal" (it needs its own anim.track; a listener never has a target). The
sender needs no clip of its own, but if it has one (a lever) it plays too. Both parts are separate outputs,
and the target is the OUTPUT name. Example, a button on the wall that opens a door:
  swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 1:0,90,0", pivot=[-0.45, 0, 0], loop=false)
  listen = behavior.on@1(swing, trigger="signal")
  output front_door = listen
  placed = xform.move@1(button, by=[0.8, 1, 0.1])
  press = behavior.on@1(placed, trigger="select", target="front_door")
  output wall_button = press
The listener's action decides what each signal does (toggle opens then closes; play opens once).
Logic with flags: flags are scene-wide on/off values (lowercase names like has_key, powered), all off when play
starts. behavior.on takes sets="name=on|off|toggle; other=on" (what happens to flags when it fires),
needs="has_key; !alarm" (all must hold or the trigger does nothing) and, for trigger="flag", watch="powered" (fires
whenever that flag changes; it cannot set the flag it watches). action="follow" with trigger="flag" runs the part's
clip forward while the flag is on and backward while it is off, so a lamp always matches its switch. A part may be
pure logic (no clip) if it sets flags. Examples:
  locked door:   latch = behavior.on@1(swing, trigger="select", action="toggle", needs="has_key")
  the key:       take = behavior.on@1(vanish_track, trigger="select", action="play", sets="has_key")
  switch + lamp: flip = behavior.on@1(button, trigger="select", sets="powered=toggle")
                 mirror = behavior.on@1(glow_track, trigger="flag", watch="powered", action="follow")
  AND gate:      gate = behavior.on@1(marker, trigger="flag", watch="a", needs="b", sets="c=toggle")
Carrying things: trigger="grab" fires when a hand (or the mouse) picks the part up in play mode, trigger="release"
when it is let go, trigger="hold" does both (its sets apply on pick-up and REVERSE on release, so sets="holding"
is on only while held; its clip runs forward while held and back when released). Such parts become grabbable in play
mode only; they can be carried anywhere and are put back when play stops. Example, picking up the key unlocks the door:
  take = behavior.on@1(key_part, trigger="grab", sets="has_key")
Fields of moving copies: anim.track / anim.wave work on a placed set (inst.array, inst.radial, inst.repeat), and every copy
plays the motion in its own frame (rotation and scale pivots are in the prototype's coordinates, so build the prototype
standing on its pivot). phase_step=<seconds> makes each copy play that much ahead of the one before (a wave along the
row) and phase_jitter=<seconds> adds a fixed pseudo-random offset per copy so a field does not move in step. Only motion
tracks (position, rotation, scale) apply to copies. Example, reeds swaying out of step:
  standing = xform.move@1(reed, by=[0, 0.4, 0])
  row = inst.array@1(standing, count_x=20, spacing=[0.15, 0, 0])
  sway = anim.wave@1(row, property="rotation", amplitude=[0, 0, 12], period=3, pivot=[0, 0, 0], phase_jitter=3)
  output reeds = sway
Sets of up to 256 copies export their motion to glTF (each copy gets its own channels); larger sets export static.
Clips and sequences: give anim.track / anim.wave nodes clip="name" to put their motion in a named clip; one part can have
several (clip="wave", clip="reach"), they run separately and their poses combine. A behaviour runs one clip: behavior.on
clip="reach"; a sender picks a clip of its target with target="arm_out:wave". A sequence is clip="a", then="b; c": a runs,
then b, then c, one after the other, and clicking again plays it again. Example, an arm with two buttons:
  wave = anim.track@1(arm, property="rotation", keys="0:0,0,0; 1:0,0,50", clip="wave", loop=false)
  reach = anim.track@1(wave, property="rotation", keys="0:0,0,0; 2:70,0,0", clip="reach", loop=false)
  listen = behavior.on@1(reach, trigger="signal", action="play")   output arm_out = listen
  press = behavior.on@1(button, trigger="select", target="arm_out:wave")   output wave_button = press
Each clip exports as its own glTF animation.
Timers: trigger="timer" with after=<seconds> fires that long after play mode starts, and every=<seconds> (0.1 or more)
repeats it; with action="toggle" a repeating timer opens and closes, blinks or alternates forever, and with sets="lit=toggle"
it flips a flag on a beat. Example, a door that opens by itself after 2 seconds:
  auto = behavior.on@1(swing, trigger="timer", action="play", after=2)
Sockets: trigger="socket" with accepts="<output name of the key part>" and radius makes a part a socket: when that part,
which must itself be grabbable (trigger grab, release or hold), is let go within radius of the socket it SNAPS onto it, and
the socket fires like hold (its sets apply and its clip runs forward); picking the part out again reverses them. A key that
unlocks a door:
  key_part = behavior.on@1(brass_key, trigger="hold", sets="carried")   output key_out = key_part
  fits = behavior.on@1(lock_plate, trigger="socket", accepts="key_out", radius=0.4, sets="unlocked")   output lock_out = fits
  latch = behavior.on@1(swing, trigger="select", action="toggle", needs="unlocked")   output door_out = latch
Use targets for "this click moves that part"; use flags when something must be remembered or combined.
Behaviours only react in play mode; in edit mode a click selects. A glTF export keeps the clip and drops the trigger.

How to work: call get_text to see the current document. apply_text ADDS the nodes of a program to
the document (it never replaces existing ones) and may read nodes already there by their label (see
get_nodes), as the example above reads vase; a new node cannot reuse an existing label, and
"output name = node" points an existing output at a different node. To change something that
already exists use apply_commands, which addresses nodes by ID (get_nodes lists each node's id, label, operator,
parameters and inputs; set_param is the usual command). Read the result after each call. A refused edit leaves the
document unchanged and says why (for example constraint.min_wall with the measured thickness).`;

// The guide is served in pieces: a short core every session reads, and topics asked for by name.
const GUIDE_SPLIT = splitGuide(GUIDE);
if (GUIDE_SPLIT.missing.length > 0) log(`guide topics not found in the guide text (left in the core): ${GUIDE_SPLIT.missing.join(', ')}`);
// Topics written for the guide rather than cut out of its text: make_character, looks, and how to build any body.
for (const [id, extra] of Object.entries({ ...EXTRA_TOPICS, ...TOOL_TOPICS, creatures: CREATURES, effects: EFFECTS, sim: SIM, limits: LIMITS_TOPIC })) {
  GUIDE_SPLIT.sections.set(id, extra.text);
  GUIDE_SPLIT.topics.push({ id, summary: extra.about });
}

// `bun tools/mcp/server.mjs --guide [topic]` prints the guide for a person or a model that has no MCP connection, and exits.
if (GUIDE_ARG >= 0) {
  const topic = (process.argv[GUIDE_ARG + 1] ?? '').trim().toLowerCase();
  console.log(topic ? topicText(GUIDE_SPLIT, topic) : coreWithMenu(GUIDE_SPLIT));
  process.exit(0);
}

const object = (properties, required = []) => ({ type: 'object', properties, required });
const OPERATOR_SELECTION = object({ query: { type: 'string' }, family: { type: 'string' }, ids: { type: 'array', items: { type: 'string' } }, detail: { type: 'string', enum: ['index', 'full'] } });
const CAMERA = object({ position: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 }, target: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 }, label: { type: 'string' } }, ['position', 'target']);
const INSTRUCTIONS = `1. Build and edit interactive 3D scenes in the spatial-engine editor. Call get_guide once with topics (include core and relevant how-tos) and an operators selection (query, family or ids) to get the language and operator docs together. Read only what the task needs.
2. Begin with start_from when the request resembles a template: list the templates, take a working skeleton, and edit it to fit. Write programs with pipes and loops; include topic compact in your guide request for syntax and limits.
3. Use batch for any known sequence of tool calls; use run for loops or conditions and return only the results you need. Both call the same tools while keeping intermediate output out of the conversation.
4. Use survey to plan cameras (position, target and label) or snapshot {tour:true} for a tour in labelled sheets with the previous view restored; get_guide topic="survey" explains coverage and options.
5. Use edit by node label for small changes. Look again after edits, run check before you finish, and fix the problems it reports.`;
const TOOLS = [
  {name: 'get_guide', description: 'Read once with topics and operators together. Omit topics for core and menu; topic selects one, all returns everything.', inputSchema: object({ topic: { type: 'string' }, topics: { type: 'array', items: { type: 'string' } }, operators: OPERATOR_SELECTION })},
  {name: 'list_operators', description: 'Search the operator and kit index; ids requests full specs or kit source. See get_guide(topic="operator_reference").', inputSchema: OPERATOR_SELECTION},
  {name: 'help_field', description: 'The exact documentation of one operator parameter, one operator or one family, by a dotted path: "solid.sweep.scale_end" (type, range, unit, default, options, full text), "solid.sweep" (inputs, output, every parameter), "solid" (the family), or a bare parameter name. A wrong path answers with the nearest real ones. Use it instead of reading a whole operator when you only need one setting.', inputSchema: object({ path: { type: 'string', description: 'For example solid.sweep.scale_end.' } }, ['path'])},
  {"name":"texture_nodes","description":"Search the procedural texture node catalog with query, or describe one exact node with node. Use this before authoring a solid.texture@1 program.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"node":{"type":"string"},"limit":{"type":"number"}},"required":[]}},
  {"name":"get_text","description":"The current document and revision as compact text by default, or canonical text with format=\"canonical\".","inputSchema":{"type":"object","properties":{"format":{"type":"string","enum":["compact","canonical"],"default":"compact"}},"required":[]}},
  {"name":"get_nodes","description":"Every node with its id, label, operator, parameters, inputs, flags, the group it belongs to (nodes inside a group definition) or its instance data, plus the outputs and the groups. Use the ids in apply_commands.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"get_status","description":"Revision, node count, evaluation state (last failure, timings) and the last projected geometry.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {name: 'survey', description: 'Plan cameras and reachable surface coverage. See get_guide(topic="survey").', inputSchema: object({ mode: { type: 'string', enum: ['auto', 'walk', 'orbit'] }, eye_height: { type: 'number' }, spacing: { type: 'number' }, max_views: { type: 'integer' }, yaws: { type: 'integer' }, target_coverage: { type: 'number' } })},
  {"name":"measure","description":"Measure evaluated parts; summary by default, full JSON on request. See get_guide(topic=\"measure_reference\").","inputSchema":{"type":"object","properties":{"part":{"type":"string"},"detail":{"type":"string","enum":["summary","full"]},"checks":{"type":"array","items":{"type":"string","enum":["thickness","symmetry","uv","collision"]}}},"required":[]}},
  {"name":"bake","description":"Bake a high-poly part's normals and ambient occlusion onto a low-poly part's UVs; the model must have unwrapped the low part with poly.unwrap_uv.","inputSchema":{"type":"object","properties":{"low":{"type":"string"},"high":{"type":"string"},"size":{"type":"number","description":"64 to 2048; default 512."},"maps":{"type":"array","items":{"type":"string","enum":["normal","ao"]}},"cage":{"type":"number"},"max_distance":{"type":"number"},"rays":{"type":"number","description":"1 to 256; default 64."},"bias":{"type":"number"}},"required":["low","high"]}},
  {"name":"export_glb","description":"Exports the model on screen as a binary glTF (.glb) file with PBR materials, shared instances, UVs, and embedded PNG maps for solid.texture programs. Give a path ending in .glb (relative paths are under the current directory); the file is written there. Baked textures require WebGL2 and EXT_color_buffer_float.","inputSchema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}},
  {"name":"import_file","description":"Brings your own mesh file into the document: reads a plain-text .obj, ascii .ply or ascii .stl from path (on the machine running this server) and adds it as an editable poly.import node, outputting it unless output is false. scale multiplies coordinates (0.001 for a file in millimetres), up is the file's up axis (z is turned to Y up). Nothing is fetched or bundled, and binary files are refused with the way out. Up to about 990,000 bytes.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"name":{"type":"string"},"format":{"type":"string","enum":["obj","ply","stl"]},"scale":{"type":"number"},"up":{"type":"string","enum":["y","z"]},"output":{"type":"boolean"}},"required":["path"]}},
  {"name":"apply_text","description":"Atomically add a text program and evaluate it, returning diagnostics and a state delta. dry_run applies, measures and undoes. See get_guide(topic=\"apply_text_reference\") for limits and delta fields.","inputSchema":{"type":"object","properties":{"text":{"type":"string"},"label":{"type":"string"},"dry_run":{"type":"boolean"}},"required":["text"]}},
  {"name":"scene_overview","description":"Scene parts, totals, bounds, evaluation time, warnings and provisional Quest budget. detail=\"full\" lists every part.","inputSchema":{"type":"object","properties":{"detail":{"type":"string","enum":["summary","full"]}},"required":[]}},
  {"name":"inspect","description":"Why did this node give this? Without node: every node in run order with what it made. With node (a label or id): its operator and parameters, what fed it, what it produced (kind, size, counts), what it changed from its main input, and the chain of nodes upstream, nearest first. Use it when a size, a count or a selection is not what you expected.","inputSchema":{"type":"object","properties":{"node":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":40},"kind":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}},"required":[]}},
  {"name":"find","description":"Find document nodes by query, family, or one label/id with its inputs and users. See get_guide(topic=\"find_reference\").","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"family":{"type":"string"},"node":{"type":"string"},"limit":{"type":"number"}},"required":[]}},
  {"name":"check","description":"Looks for problems you can repair and answers each as {id, severity, message, node, fix}: a failed evaluation, the provisional Quest budget (over or near), operator warnings, and game kinds that name a scene part that does not exist. ok is false when any error is found. Run it after a build and before you call the work done; apply the fix, then run it again.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"game_describe","description":"The game the document defines (nodes game.var, game.kind, game.spawn, game.input, game.rule, game.goal joined by game.collect and output as `output rules = ...`): a one-line summary, its rules as text, and what validation found. Says how to build one when there is none.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"playtest","description":"Play headless with a deterministic policy and seed; aim:<thing tag>:<target tag>[,<target tag>...] swings a hand-held tool live toward targets. See get_guide(topic=\"playtest_reference\").","inputSchema":{"type":"object","properties":{"policy":{"type":"string"},"seed":{"type":"number"},"steps":{"type":"number"}},"required":[]}},
  {"name":"make_character","description":"Add an animated character from a body plan and sliders or custom creature. See get_guide(topic=\"character_reference\") for every field.","inputSchema":{"type":"object","properties":{"plan":{"type":"string","enum":["biped","quadruped","serpent"]},"sliders":{"type":"object"},"creature":{"type":"object"},"name":{"type":"string"},"label":{"type":"string"}},"required":[]}},
  {"name":"apply_commands","description":"Atomically apply typed commands by node id or unique label, returning a state delta. dry_run measures and undoes. See get_guide(topic=\"command_reference\").","inputSchema":{"type":"object","properties":{"commands":{"type":"array","items":{"type":"object"}},"label":{"type":"string"},"dry_run":{"type":"boolean"}},"required":["commands"]}},
  {"name":"edit","description":"Atomically edit nodes by unique label; returns the apply_text delta. Ops: set {node,params}, rename {node,to}, delete {node}, replace {node,text} (one replacement node line). See get_guide(topic=\"edit_diff\").","inputSchema":{"type":"object","properties":{"ops":{"type":"array","minItems":1,"items":{"type":"object","properties":{"op":{"type":"string","enum":["set","rename","delete","replace"]},"node":{"type":"string"},"params":{"type":"object"},"to":{"type":"string"},"text":{"type":"string"}},"required":["op","node"]}},"label":{"type":"string"},"dry_run":{"type":"boolean"}},"required":["ops"]}},
  {"name":"diff","description":"Compact added/changed/removed node lines since a revision (default previous revision); at most 80 lines, with an omitted count. See get_guide(topic=\"edit_diff\").","inputSchema":{"type":"object","properties":{"since":{"type":"integer","minimum":0}},"required":[]}},
  {"name":"undo","description":"Undoes the last committed edit.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"redo","description":"Redoes the last undone edit.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"batch","description":"Run up to 32 tool calls in order. Each step is {tool, args}; stop_on_error defaults to true and max_chars defaults to 3000 per step. batch and run cannot be nested.","inputSchema":{"type":"object","properties":{"steps":{"type":"array","items":{"type":"object"}},"stop_on_error":{"type":"boolean"},"max_chars":{"type":"integer"}},"required":["steps"]}},
  {"name":"run","description":"Run an async JavaScript function body with tools.<name>(args), log(...), and return. Use loops and conditions; at most 200 tool calls and VE_RUN_TIMEOUT_MS (default 120000). No process, require, fetch, or Bun globals.","inputSchema":{"type":"object","properties":{"code":{"type":"string"}},"required":["code"]}},
  {"name":"world","description":"Reads and changes the running world by component name, beyond what document nodes can do. Pass method and params; see get_guide(topic=\"world_api\") for all methods and their parameters.","inputSchema":{"type":"object","properties":{"method":{"type":"string","description":"For example world.parts or world.get_components."},"params":{"type":"object","description":"The method parameters."}},"required":["method"]}},
  {"name":"script","description":"Start, stop, list or read logs of JavaScript behaviour in the running world. See get_guide(topic=\"script_api\") for the full API and execution limits.","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["start","stop","list","logs"]},"name":{"type":"string"},"source":{"type":"string"}},"required":["action"]}},
  {name: 'snapshot', description: 'Capture cameras, views, motion or a survey tour. See get_guide(topic="snapshot_modes").', inputSchema: { ...object({ tour: { type: ['boolean', 'object'], properties: { mode: { type: 'string', enum: ['auto', 'walk', 'orbit'] } } }, camera: { $ref: '#/$defs/camera' }, cameras: { type: 'array', minItems: 1, maxItems: 6, items: { $ref: '#/$defs/camera' } }, times: { type: 'array', items: { type: 'number' } }, states: { type: 'array', items: { type: 'object' } }, view: { type: 'string', enum: ['front', 'back', 'side', 'left', 'top', 'iso'] }, views: { type: 'array', items: { type: 'string', enum: ['front', 'back', 'side', 'left', 'top', 'iso'] } }, focus: { type: 'string' }, name: { type: 'string' }, image: { type: 'boolean' }, width: { type: 'number' } }), $defs: { camera: CAMERA } }},
  {"name":"frame_view","description":"Moves the editor camera so the whole model fills the view, from a three-quarter angle. Call it after building something so the person watching sees it.","inputSchema":{"type":"object","properties":{},"required":[]}},
  {"name":"set_view","description":"Moves the editor camera: position and target as [x, y, z] metres, so the person watching can see the result.","inputSchema":{"type":"object","properties":{"position":{"type":"array","items":{"type":"number"}},"target":{"type":"array","items":{"type":"number"}}},"required":["position","target"]}},
  {"name":"start_from","description":"List working scene templates or return one to edit; apply=true sends it through apply_text.","inputSchema":{"type":"object","properties":{"template":{"type":"string"},"apply":{"type":"boolean"}},"required":[]}},
  {"name":"recipes","description":"Save, list, show, or delete reusable groups. Show gives source and a pasteable call line.","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["list","save","show","delete"]},"name":{"type":"string"},"text":{"type":"string"},"about":{"type":"string"}},"required":["action"]}},
];

const TOOL_ORDER = [
  'get_guide', 'list_operators', 'help_field', 'texture_nodes', 'get_text', 'get_nodes', 'get_status', 'survey', 'measure', 'bake', 'export_glb',
  'import_file', 'apply_text', 'scene_overview', 'find', 'inspect', 'check', 'game_describe', 'playtest', 'make_character', 'apply_commands', 'edit',
  'diff', 'undo', 'redo', 'batch', 'run', 'world', 'script', 'snapshot', 'frame_view', 'set_view', 'start_from', 'recipes',
];
TOOLS.sort((a, b) => TOOL_ORDER.indexOf(a.name) - TOOL_ORDER.indexOf(b.name));

const send = (message) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');
const content = (value, isError = false) => ({ content: [{ type: 'text', text: typeof value === 'string' ? value : JSON.stringify(value, null, 2) }], isError });

// Asks the page for the glb and writes it to a file the caller named. Only .glb files are written.
async function exportGlb({ path: target }) {
  if (typeof target !== 'string' || !target.toLowerCase().endsWith('.glb')) return content('path must end in .glb', true);
  const reply = await callPage('export_glb', {});
  if (reply.error) return content(reply.error, true);
  const result = reply.result ?? {};
  if (!result.exported) return content(result.reason ?? 'nothing was exported', true);
  const file = resolvePath(target);
  writeFileSync(file, Buffer.from(result.base64, 'base64'));
  return content({ exported: true, path: file, bytes: result.bytes, parts: result.parts, triangles: result.triangles });
}

async function callTool(name, args, attachTip = true) {
  const reply = await dispatchTool(name, args);
  const blocks = reply.content ?? [];
  const texts = blocks.filter(block => block.type === 'text');
  let failed = reply.isError === true;
  if (name === 'apply_text') {
    for (const block of texts) {
      try {
        const result = JSON.parse(block.text);
        failed ||= result.failure != null || result.would_apply === false || (result.applied === false && result.dry_run !== true);
      } catch { /* plain-text errors are already marked by the dispatcher */ }
    }
  }
  const tip = hints.note(name, args, failed);
  if (tip) pendingTips.push(tip);
  const structured = texts.some(block => /^[\s]*[\[{]/.test(block.text));
  if (attachTip && name !== 'batch' && name !== 'run' && !structured && blocks.every(block => block.type === 'text') && texts.length && pendingTips.length) {
    texts[texts.length - 1].text += `\n\n${pendingTips.shift()}`;
  }
  return reply;
}

async function dispatchTool(name, args) {
  const invalid = validateToolArguments(name, args ?? {});
  if (invalid) return content(invalid, true);
  if (name === 'get_guide') {
    if (args?.topics !== undefined && (!Array.isArray(args.topics) || args.topics.some(topic => typeof topic !== 'string'))) return content('topics must be a list of topic names', true);
    if (args?.operators !== undefined && (!args.operators || typeof args.operators !== 'object' || Array.isArray(args.operators))) return content('operators must be an object with query, family, ids or detail', true);
    const topics = args?.topics?.map(topic => topic.trim().toLowerCase());
    const requested = topics ?? [typeof args?.topic === 'string' ? args.topic.trim().toLowerCase() || 'core' : 'core'];
    const unknown = requested.find(topic => !['core', 'all'].includes(topic) && !GUIDE_SPLIT.sections.has(topic));
    if (unknown !== undefined) return content(topicText(GUIDE_SPLIT, unknown), true);
    let text = topics ? requested.map(topic => `## ${topic}\n\n${topicText(GUIDE_SPLIT, topic)}`).join('\n\n') : topicText(GUIDE_SPLIT, requested[0]);
    if (args?.operators !== undefined) {
      const reply = await dispatchTool('list_operators', args.operators);
      text += `\n\n## operators\n\n${reply.content.map(block => block.text ?? '').join('\n')}`;
    }
    return content(text);
  }
  if (name === 'help_field') {
    if (typeof args?.path !== 'string' || !args.path.trim()) return content('path is needed, for example "solid.sweep.scale_end", "solid.sweep" or "solid"', true);
    const reply = await callPage('list_operators', {});
    if (reply.error) return content(reply.error, true);
    const answer = helpField(reply.result, args.path);
    return content(answer.text, !answer.ok);
  }
  if (name === 'start_from') {
    try {
      const result = callStartFrom(args ?? {}, callPage);
      if (result && typeof result.then === 'function') {
        const reply = await result;
        return reply?.error ? content(reply.error, true) : content(reply?.result ?? reply);
      }
      return content(result);
    } catch (error) { return content(error.message, true); }
  }
  if (name === 'recipes') {
    try { return content(callRecipes(args ?? {}, KITS.map(kit => kit.name))); }
    catch (error) { return content(error.message, true); }
  }
  if (name === 'batch') return callBatch(args ?? {}, (tool, input) => callTool(tool, input, false));
  if (name === 'run') return callRun(args ?? {}, (tool, input) => callTool(tool, input, false));
  if (!TOOLS.some((t) => t.name === name)) return content(`unknown tool ${name}`, true);
  if (name === 'export_glb') return exportGlb(args ?? {});
  if (name === 'import_file') {
    const built = buildImport(args ?? {});
    if (built.error) return content(built.error, true);
    return dispatchTool('apply_text', { text: built.text, label: `import ${built.name}` });
  }
  const reply = await callPage(name, args ?? {});
  if (reply.error) return content(reply.error, true);
  if (name === 'snapshot') {
    // The picture goes back as an image block and everything else as text, so the model sees it instead of reading base64.
    const { jpegBase64, sheets, ...rest } = reply.result ?? {};
    const pictures = [jpegBase64];
    if (Array.isArray(sheets)) rest.sheets = sheets.map(({ jpegBase64, ...sheet }) => { pictures.push(jpegBase64); return sheet; });
    const images = pictures.filter(Boolean).slice(0, 2).map(data => ({ type: 'image', data, mimeType: 'image/jpeg' }));
    return { content: [...images, { type: 'text', text: JSON.stringify(rest, null, 2) }], isError: rest.ok === false };
  }
  if (name === 'survey') return content(reply.result, Boolean(reply.result?.error));
  if (name === 'bake') {
    const { maps = {}, ...summary } = reply.result ?? {};
    const images = Object.entries(maps).map(([, data]) => ({ type: 'image', data, mimeType: 'image/png' }));
    return { content: [...images, { type: 'text', text: JSON.stringify({ ...summary, maps: Object.keys(maps) }, null, 2) }], isError: summary.ok === false };
  }
  if (name === 'list_operators' && Array.isArray(reply.result)) {
    const kits = KITS.map(kit => ({
      id: `kit.${kit.name}`, readiness: 'ready', category: 'kit', doc: kit.doc,
      source: kit.source,
      usage: kit.source.startsWith('group ') ? `Paste source once, then call ${kit.name}()` : 'Paste source once at the top of the program, then call its macros; the header comments of the source show how',
    }));
    const everything = [...reply.result, ...kits];
    const { ids, detail, query, family } = args ?? {};
    if (Array.isArray(ids) && ids.length > 0) {
      const { found, missing } = fullSpecs(everything, ids.map(String));
      return content({ operators: found, ...(missing.length ? { unknown: missing, hint: 'ids look like solid.box@1 or kit.door; list_operators without ids shows them all' } : {}) }, found.length === 0);
    }
    if (detail === 'full') return content(select(everything, { query, family }));
    return content(indexText(everything, { query, family }));
  }
  // A write answers with what changed, not with the whole document: every edit used to echo all of the scene text (tens of kilobytes for a big scene)
  // and the lights and environment, which the model already knows. get_text reads the document when it is wanted; VE_MCP_ECHO=1 brings the old reply back.
  if (['apply_text', 'edit', 'apply_commands'].includes(name) && process.env.VE_MCP_ECHO !== '1' && reply.result && typeof reply.result === 'object') {
    const { text, stats, ...rest } = reply.result;
    const slim = { ...rest };
    if (typeof text === 'string') slim.text_omitted = `${text.length} characters of document text left out; call get_text to read it`;
    if (stats && typeof stats === 'object') {
      const { lights, environment, scripts, ...keep } = stats;
      slim.stats = keep;
    }
    return content(slim);
  }
  return content(reply.result ?? {});
}

let buffer = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', async (chunk) => {
  buffer += chunk;
  let newline;
  while ((newline = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    if (!line) continue;
    let message;
    try { message = JSON.parse(line); } catch { continue; }
    const { id, method, params } = message;
    if (method === 'initialize') {
      send({ id, result: { protocolVersion: params?.protocolVersion ?? '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'spatial-engine', version: '0.1.0' }, instructions: INSTRUCTIONS } });
    } else if (method === 'tools/list') {
      send({ id, result: { tools: TOOLS } });
    } else if (method === 'tools/call') {
      send({ id, result: await callTool(params?.name, params?.arguments) });
    } else if (method === 'ping') {
      send({ id, result: {} });
    } else if (id !== undefined) {
      send({ id, error: { code: -32601, message: `method not found: ${method}` } });
    }
  }
});
process.stdin.on('end', () => process.exit(0));
