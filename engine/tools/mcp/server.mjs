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
import { writeFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';
import { coreWithMenu, splitGuide, topicText } from './guide-topics.mjs';
import { fullSpecs, indexText, select } from './operator-index.mjs';

const PORT = Number(process.env.VE_MCP_PORT ?? 8765);
const TIMEOUT_MS = 90000;
let page = null;
let counter = 0;
const pending = new Map();

const log = (...args) => console.error('[spatial-engine mcp]', ...args);

Bun.serve({
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
log(`bridge listening on ws://127.0.0.1:${PORT}`);

function callPage(method, params) {
  if (!page) {
    return Promise.resolve({ error: `No editor page is connected. Open the engine editor with ?mcp=1 (for example http://127.0.0.1:5173/?mcp=1) and try again.` });
  }
  const id = ++counter;
  return new Promise((resolve) => {
    const timer = setTimeout(() => { pending.delete(id); resolve({ error: `The page did not answer ${method} within ${TIMEOUT_MS / 1000} s.` }); }, TIMEOUT_MS);
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
sqrt, floor, ceil, round, clamp, sin, cos, radians; constants pi and tau. Values: value.number,
value.math and value.size make numbers that you link into any number parameter by its name (a vector
component as size.y), for example r = value.number@1(value=0.5), d = value.math@1(op="mul", a=r, b=2),
ball = solid.sphere@1(radius=d), box = solid.box@1(size=[1, 1, 1], size.y=h). value.size@1(part,
measure="height") measures a part (width, height, depth, longest, shortest, volume, area), so one part
can be sized from another. Only operators from
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
Shader expressions: the expr node draws a texture from a few lines of code, and composes with any other node. In the graph text
write the program string with escaped quotes (\" inside program="..."):
  program="n = expr(code=\"w = p; for i in 0..6 { w = rot(w, 0.7) * 1.9 + snoise(w * 2) * 0.4 }; out = hsv(w.x, 0.6, 0.8)\"); albedo = n"
Language: name = expression lines (newline or ;), the last must be out = float (grey), vec3 or vec4. Built in: p (uv 0..1), q (centred
-1..1), t (0), PI, TAU; floats and vec2-vec4 with swizzles (.xy .rgb); + - * / %; sin cos tan asin acos atan abs sign floor ceil fract
sqrt exp log pow min max clamp mix step smoothstep mod length distance dot cross normalize reflect; and hash(vec2), noise(vec2) 0..1,
snoise(vec2) -1..1, fbm(vec2) 0..1, hsv(h, s, v), rot(vec2, angle). Loops: for i in 0..N { ... } with a whole-number N up to 64,
two deep at most (256 passes in all). No if, while, comparisons or texture reads (use step, mix and smoothstep). At most 600
characters. Variables first set inside a loop stay inside it. A mistake names the line, column and the fix.
Cut-outs: solid.foliage@1(shape, alpha_cutoff=0.5, sway=0, flutter=0) after solid.material and solid.texture cuts the surface away
where its albedo's alpha is under the cutoff, in the picture, in its shadow and in the exported glTF (alphaMode MASK), and draws it
two-sided. End the expr code with a vec4 whose last number is the alpha to draw the shape: out = vec4(0.1, 0.5, 0.1, 1.0 - step(0.8,
length(q * vec2(1.0, 1.4)))) is a leaf. The shape needs UVs over the whole surface (poly.grid gives one quad with them, or unwrap a
mesh with poly.unwrap_uv); a solid.box has its six faces in one atlas, so a shape drawn into it is split across them. Wind: sway
bends the whole shape (tall parts more), flutter trembles the surface; both 0 is an ordinary surface.
Export glTF bakes these in the browser, requires WebGL2 plus EXT_color_buffer_float, and embeds PNG maps.
The viewport shows these baked maps on the model (up to 64 maps in all; later parts lose emissive, then
metallic-roughness, then normal maps first, with a warning). Give a part UVs with poly.unwrap_uv before
texturing it if it is a polygon mesh; other parts get a box projection.

Animation: anim.track@1(shape, property, keys, interpolation, loop, pivot) makes a shape move and passes it
through unchanged. property is position (metres), rotation (degrees about X, Y, Z) or scale (factors);
keys are "seconds:x,y,z" entries separated by semicolons, at least two, times increasing, every entry with
all three values; interpolation is linear, smooth (default, eases in and out) or step; loop repeats (default)
or holds the last key; pivot is the point, in the shape's own coordinates, that rotation and scale act about.
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

const object = (properties, required = []) => ({ type: 'object', properties, required });
const TOOLS = [
  { name: 'get_guide', description: 'How the text language works, with an example. Read this first: it is short and ends with how to work. For a subject you need (curves, terrain, poly, groups, look, animation, rigging, behaviour) call it again with that topic; topic="all" returns the whole guide.', inputSchema: object({ topic: { type: 'string', description: 'curves, terrain, poly, groups, look, animation, rigging, behaviour, or all. Omit for the core guide and the topic list.' } }) },
  { name: 'list_operators', description: 'The operators you may use. By default a compact index: one line per operator with its inputs, output, parameters (type, range, unit, default) and what it does, grouped by family, plus the ready-made kits. Narrow it with query (words in the id or help) or family (solid, poly, inst, xform, value ...). Pass ids=["solid.box@1", "kit.door"] to get the full documentation of just those (a kit returns its source), or detail="full" for everything (large).', inputSchema: object({ query: { type: 'string', description: 'Words that must appear in the operator id or help, for example "scatter surface".' }, family: { type: 'string', description: 'Only this family: the part of the id before the dot, such as poly or inst.' }, ids: { type: 'array', items: { type: 'string' }, description: 'Full specs of exactly these operator ids (for example solid.box@1) or kits (kit.door).' }, detail: { type: 'string', enum: ['index', 'full'], description: 'index (default) is one line per operator; full is every operator with all parameter documentation.' } }) },
  { name: 'texture_nodes', description: 'Search the procedural texture node catalog with query, or describe one exact node with node. Use this before authoring a solid.texture@1 program.', inputSchema: object({ query: { type: 'string' }, node: { type: 'string' }, limit: { type: 'number' } }) },
  { name: 'get_text', description: 'The current document as canonical text, and its revision.', inputSchema: object({}) },
  { name: 'get_nodes', description: 'Every node with its id, label, operator, parameters, inputs, flags, the group it belongs to (nodes inside a group definition) or its instance data, plus the outputs and the groups. Use the ids in apply_commands.', inputSchema: object({}) },
  { name: 'get_status', description: 'Revision, node count, evaluation state (last failure, timings) and the last projected geometry.', inputSchema: object({}) },
  { name: 'measure', description: 'Measures the evaluated parts on screen, through the same geometry the viewer shows. By default a short text report per part: triangles, size, volume, watertight, thickness and the first problem with its fix. detail "full" gives the complete JSON report (topology, components, bounding shapes, symmetry, UVs). part names one output/part (default every part); checks (thickness, symmetry, uv, collision) limits which optional sections the full report keeps. A part that is not closed says where and what to do.', inputSchema: object({ part: { type: 'string' }, detail: { type: 'string', enum: ['summary', 'full'] }, checks: { type: 'array', items: { type: 'string', enum: ['thickness', 'symmetry', 'uv', 'collision'] } } }) },
  { name: 'bake', description: "Bake a high-poly part's normals and ambient occlusion onto a low-poly part's UVs; the model must have unwrapped the low part with poly.unwrap_uv.", inputSchema: object({ low: { type: 'string' }, high: { type: 'string' }, size: { type: 'number', description: '64 to 2048; default 512.' }, maps: { type: 'array', items: { type: 'string', enum: ['normal', 'ao'] } }, cage: { type: 'number' }, max_distance: { type: 'number' }, rays: { type: 'number', description: '1 to 256; default 64.' }, bias: { type: 'number' } }, ['low', 'high']) },
  { name: 'export_glb', description: 'Exports the model on screen as a binary glTF (.glb) file with PBR materials, shared instances, UVs, and embedded PNG maps for solid.texture programs. Give a path ending in .glb (relative paths are under the current directory); the file is written there. Baked textures require WebGL2 and EXT_color_buffer_float.', inputSchema: object({ path: { type: 'string' } }, ['path']) },
  { name: 'apply_text', description: 'Adds the nodes described by a program in the text language to the document, evaluates it, and reports the outcome or the diagnostics (each with a line and column). Atomic: nothing changes unless all of it is valid. Limits: 256 KiB source text; translated batches allow 1,000 typed commands and 1 MiB.', inputSchema: object({ text: { type: 'string' }, label: { type: 'string' } }, ['text']) },
  { name: 'make_character', description: 'Adds a whole animated character or creature to the document: one smooth body skinned to a skeleton, with walk (main clip), run and idle clips, toon shaded with outlines. Give a body `plan` ("biped", "quadruped" or "serpent") with optional `sliders` (size in metres 0.2 to 20; head, limbs, torso, legs, neck, tail as multipliers 0.5 to 2; color [r,g,b] 0 to 1; horns and wings for a quadruped), or a whole `creature` you write yourself: bones (name, parent, head, tail, radius [at head, at tail], role leg|arm|spine|neck|head|tail|wing|free, pair for left/right copies defined on +x, phase) and blobs (name, center, radii). The creature stands on y=0 and faces +z. A `name` keeps several characters apart. Errors say which bone or slider to change. Call frame_view afterwards.', inputSchema: object({ plan: { type: 'string', enum: ['biped', 'quadruped', 'serpent'] }, sliders: { type: 'object' }, creature: { type: 'object' }, name: { type: 'string' }, label: { type: 'string' } }) },
  { name: 'apply_commands', description: 'Applies typed commands, for example [{"set_param":{"node":"n_abc12345","param":"segments","value":96}}]. Commands: add_node, remove_node, connect, disconnect, set_param, set_label, set_flags, set_output, and for groups set_instance_param, clear_instance_param, upgrade_group, expose_param, unexpose_param, set_group_output. Atomic. Limit: 1,000 commands and 1 MiB per transaction.', inputSchema: object({ commands: { type: 'array', items: { type: 'object' } }, label: { type: 'string' } }, ['commands']) },
  { name: 'undo', description: 'Undoes the last committed edit.', inputSchema: object({}) },
  { name: 'redo', description: 'Redoes the last undone edit.', inputSchema: object({}) },
  { name: 'snapshot', description: 'Looks at the view: returns a small picture (JPEG, up to 512 px wide) of what the editor shows, plus what changed since the last snapshot with the same name (share of the view that changed, where, and a box around it). Take one after an edit to check it did what you meant; pass image=false to get only the comparison, the cheapest way to ask whether an edit changed anything on screen. Frame or set the view first so the same angle is compared.', inputSchema: object({ name: { type: 'string', description: 'Which earlier snapshot to compare with (default "last"); use different names to keep separate baselines.' }, image: { type: 'boolean', description: 'false returns only the comparison, without the picture.' }, width: { type: 'number', description: 'Picture width in pixels, 64 to 512 (default 512).' } }) },
  { name: 'frame_view', description: 'Moves the editor camera so the whole model fills the view, from a three-quarter angle. Call it after building something so the person watching sees it.', inputSchema: object({}) },
  { name: 'set_view', description: 'Moves the editor camera: position and target as [x, y, z] metres, so the person watching can see the result.', inputSchema: object({ position: { type: 'array', items: { type: 'number' } }, target: { type: 'array', items: { type: 'number' } } }, ['position', 'target']) },
];

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

async function callTool(name, args) {
  if (name === 'get_guide') {
    const topic = typeof args?.topic === 'string' ? args.topic.trim().toLowerCase() : '';
    return content(topic ? topicText(GUIDE_SPLIT, topic) : coreWithMenu(GUIDE_SPLIT));
  }
  if (!TOOLS.some((t) => t.name === name)) return content(`unknown tool ${name}`, true);
  if (name === 'export_glb') return exportGlb(args ?? {});
  const reply = await callPage(name, args ?? {});
  if (reply.error) return content(reply.error, true);
  if (name === 'snapshot') {
    // The picture goes back as an image block and everything else as text, so the model sees it instead of reading base64.
    const { jpegBase64, ...rest } = reply.result ?? {};
    const text = { type: 'text', text: JSON.stringify(rest, null, 2) };
    return { content: jpegBase64 ? [{ type: 'image', data: jpegBase64, mimeType: 'image/jpeg' }, text] : [text], isError: rest.ok === false };
  }
  if (name === 'bake') {
    const { maps = {}, ...summary } = reply.result ?? {};
    const images = Object.entries(maps).map(([, data]) => ({ type: 'image', data, mimeType: 'image/png' }));
    return { content: [...images, { type: 'text', text: JSON.stringify({ ...summary, maps: Object.keys(maps) }, null, 2) }], isError: summary.ok === false };
  }
  if (name === 'list_operators' && Array.isArray(reply.result)) {
    const kits = KITS.map(kit => ({
      id: `kit.${kit.name}`, readiness: 'ready', category: 'kit', doc: kit.doc,
      source: kit.source, usage: `Paste source once, then call ${kit.name}()`,
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
      send({ id, result: { protocolVersion: params?.protocolVersion ?? '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'spatial-engine', version: '0.1.0' } } });
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
