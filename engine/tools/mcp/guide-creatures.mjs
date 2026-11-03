// The "creatures" guide topic: how to build any body (robot, spider, tentacled horror, human) with the operators that exist. The IK leg and the
// tentacle recipes were applied through apply_text in a real MCP session and rendered (a spider and a tentacled horror); what was not tried
// says so in the text.

export const CREATURES = {
  about: 'building any body: spiders, tentacled horrors, robots, humans. Which technique for which body, with tested recipes',
  text: `Creatures and bodies. Any body is built from the same few pieces; choose the technique by how the thing moves.

1. Hard parts that turn on joints (robot, mech, vehicle, crab shell): ordinary solids joined with anim.parent, each joint an anim.track or
   anim.wave with pivot at the joint (topics behaviour and animation). No skinning.
5. A released object (magazine, tool, shell casing): use anim.drop to bake gravity, floor bounce and rest into position keys; add spin for a turn about the part's centre.
2. Legs that plant feet (spider, insect, quadruped, walker): a bone chain per leg and rig.ik, with a small TARGET shape at each foot that
   you animate. The legs follow the targets. For a two-bone limb (arm, human or animal leg) add solver="two_bone" and a pole: pole=[0, 0, 1]
   makes a knee point forward (+z) and never flip; soften=0.3 eases the limb in before it locks straight (no popping knee); twist rolls
   the knee or elbow. Other chain lengths (tentacles, tails) keep the default solver.
   To make a head, eye or turret LOOK at something: rig.aim@1(target_shape, bone="head") on a small target; the bone turns so its
   head-to-tail direction points at the target and its up stays towards the pole (default [0, 1, 0]).
   To let a bone-built character go LIMP (death, a hard hit): put phys.ragdoll@1(marker, name="hero", bones="pelvis; spine; head; arm_l; forearm_l; ...",
   hinges="forearm_l; shin_l", swing=60, twist=25) on any small shape, bones listed parents first (each a rig.bone part, anim.parent-ed to its parent), and
   give the floor and walls phys.body kind="static". In play mode, world physics.ragdoll {name:"hero", impulse:[0,0,-40]} drops it from the pose it is in.
3. Soft limbs (tentacle, tail, spine, vine, snake): ONE tube skinned to a chain of bones that wave with a phase offset along the chain.
4. A smooth whole body in one call: make_character(plan="biped"|"quadruped"|"serpent") or a custom creature = { bones, blobs } (topic
   characters). Edit the graph it writes if you want to change a part.
Mix them: a horror is a lumpy body (solid.sphere + xform.scale) with technique 3 tentacles and glowing eyes; a robot is technique 1 with
technique 2 legs.

Things that go wrong (each of these was a refused edit or a black picture while testing):
- A list of points is nested: points=[[0, 0, 0], [1, 2, 0]]. A flat list is refused with "expected a point".
- Every bone and every skinned mesh must be an OUTPUT, and anim.parent / rig.ik / rig.skin name the OUTPUT names, not the node names.
  Write the output line right after the bone: output leg0_upper = leg0_up_bone.
- Bones are listed root first. A bone's head is where it turns; the next bone's head equals this bone's tail. Coordinates are world
  coordinates at rest.
- Without a light the scene is black. Add sky = scene.sky@1(sun_elevation=32, sun_azimuth=40, sun_intensity=4) and output sky = sky.
  The ground needs a surface too: a flat solid.box with solid.material.
- At most 32 bones in one rig.skin. Several skins are fine (one per tentacle).
- A document holds at most 64 NAMED OUTPUTS ("too many named outputs"), and every bone, skinned mesh, IK target and visible part is
  one. A spider (8 legs of upper, lower and target) uses 26; a horror with six 6-bone tentacles uses 46, so the two do not fit in one
  document: give a horror four tentacles, or keep one big creature per document. Plan the bone count against 64 before you write it.
- Repeated parts are written out: generate the lines for leg 0 to leg 7 yourself with the numbers changed (a spider with 8 legs is 112
  lines, a six-tentacle horror 260). apply_text takes up to 256 KiB.

Recipe, one IK leg (copy it per leg; hip on the body edge, knee up and out, foot on the ground y = 0). A spider has 8, around the
body at angles (i + 0.5) / 8 * 360 degrees:
  leg_up_path = curve.polyline@1(points=[[0.277, 0.75, 0.115], [0.785, 1.1, 0.325]])
  leg_up_tube = curve.tube@1(leg_up_path, radius=0.05, end_radius=0.035, sides=10)
  leg_up_paint = solid.material@1(leg_up_tube, color=[0.12, 0.1, 0.1], roughness=0.5)
  leg_up_bone = rig.bone@1(leg_up_paint, head=[0.277, 0.75, 0.115], tail=[0.785, 1.1, 0.325])
  output leg_upper = leg_up_bone
  leg_lo_path = curve.polyline@1(points=[[0.785, 1.1, 0.325], [1.34, 0.02, 0.555]])
  leg_lo_tube = curve.tube@1(leg_lo_path, radius=0.035, end_radius=0.012, sides=10)
  leg_lo_paint = solid.material@1(leg_lo_tube, color=[0.12, 0.1, 0.1], roughness=0.5)
  leg_lo_bone = rig.bone@1(leg_lo_paint, head=[0.785, 1.1, 0.325], tail=[1.34, 0.02, 0.555])
  leg_lo_child = anim.parent@1(leg_lo_bone, parent="leg_upper")
  output leg_lower = leg_lo_child
  foot = solid.sphere@1(radius=0.03, segments=8)
  foot_at = xform.move@1(foot, by=[1.34, 0.02, 0.555])
  foot_step = anim.wave@1(foot_at, property="position", amplitude=[0, 0.18, 0], base=[0, 0.18, 0], period=1.2, shape="triangle", phase=0)
  leg_reach = rig.ik@1(foot_step, bones="leg_upper; leg_lower")
  output leg_target = leg_reach
Gait: give neighbouring legs phase 0 and 0.5 (alternating), so half the feet are down at any moment. The legs stepped in place in the
test. A walk that travels (body and targets moving forward together) was not tried.

Recipe, one tentacle (six bones on a curved path; a horror has several):
  path = curve.polyline@1(points=[[0.7, 0.45, 0], [0.95, 0.94, 0], [1.2, 1.34, 0], [1.45, 1.59, 0], [1.7, 1.64, 0], [1.95, 1.49, 0], [2.2, 1.16, 0]])
  dense = curve.resample@1(path, count=40)
  tube = curve.tube@1(dense, radius=0.16, end_radius=0.015, sides=10)
  skin_paint = solid.material@1(tube, color=[0.45, 0.16, 0.2], roughness=0.35)
  b0_box = solid.box@1(size=[0.03, 0.03, 0.03])      b0_at = xform.move@1(b0_box, by=[0.7, 0.45, 0])
  b0_rig = rig.bone@1(b0_at, head=[0.7, 0.45, 0], tail=[0.95, 0.94, 0])
  b0_wave = anim.wave@1(b0_rig, property="rotation", amplitude=[8, 0, 16], period=2.2, phase=0, pivot=[0.7, 0.45, 0])
  output t_bone0 = b0_wave
  (bone i: head = point i, tail = point i + 1, b1_child = anim.parent@1(b1_rig, parent="t_bone0") before its wave, phase = i * 0.09,
   pivot = its head, a little more amplitude toward the tip)
  skinned = rig.skin@1(skin_paint, bones="t_bone0; t_bone1; t_bone2; t_bone3; t_bone4; t_bone5", falloff=3, influences=2)
  output tentacle = skinned
The tube needs many rings to bend, which is why it is resampled to 40 points first. A different period and phase per tentacle makes
them writhe out of step. Eyes: solid.sphere with solid.material emissive=[1.2, 1, 0.2].

Detailed organic bodies (an octopus): build the surface as a FIELD, then mesh it once.
- Body: field.ellipsoid parts moved with field.move and joined with field.smooth_union (radius 0.1 to 0.3 blends them), then
  field.to_solid(min, max, edge_length). If the field touches the bounds it is refused with field.clipped and the point: enlarge min and max.
- Arm: field.tube(points=[...polyline...], radius=0.2, end_radius=0.03) along the arm's curve, then a cupped sucker row: for each sucker a
  small field.sphere moved onto the underside and field.smooth_union-ed in (the rim), then a smaller sphere moved slightly outward and removed
  with field.subtract (the cup). Fields have no rotate, so use spheres: they look the same from every side.
- Mesh ONE arm in its own frame, then copy it: xform.scale, then xform.rotate(angles=[0, yaw, 0]) per arm. xform.rotate about +Y maps
  (x, z) to (x cos(yaw) + z sin(yaw), -x sin(yaw) + z cos(yaw)); write each copy's bones at the same scaled and rotated positions, five
  bones per arm, each waving about its own head, and skin each copy to its own bones. The axis of an arm's bend is its local z turned by yaw:
  use amplitude [sin(yaw) * a, small, cos(yaw) * a].
- Keep skinned meshes small: an arm meshed at edge_length 0.025 broke into floating fragments (a skinned part past roughly 65 thousand
  vertices), at 0.036 it was clean. Prefer edge_length of 0.035 or more for an arm-sized skinned part; the body may be finer because it is
  not skinned. Tips thinner than the edge length dissolve into specks, so keep end_radius at about the edge length or more.
- Joints between a static body and skinned limbs show as a gap: the limbs are separate meshes. Close it by letting each limb START INSIDE
  the body (its first point well within the body volume, at the height of the body's lower sides) and by making the body low, with a thin
  wide field disc (a web) fused in with field.smooth_union so the limbs leave it instead of tucking under a brim. A hat-brim skirt with
  limbs hanging below it reads as a hat on stilts. Thicker limbs hide the seam better but mean more vertices: at base radius 0.25 the
  skinned arms broke up again at edge_length 0.036, at 0.17 to 0.22 they held.
- Order for looks: solid.material first, then solid.texture (a texture put before the material is overridden by its colour).
- Skin: use 3D noise, which follows the surface (a 2D expr noise showed triangular patches on a field mesh). The tested program, spotted
  skin: n = tex3d_fbm(noise=perlin, scale_x=3, scale_y=3, scale_z=3, iterations=4); base = tex3d_colorize(n, g=[0:#5e120c, 0.5:#a93b1d,
  1:#e08a58]); cell = tex3d_fbm(noise=cellular, scale_x=17, scale_y=17, scale_z=17, iterations=1); mask = tex3d_colorize(cell, g=[0:#000000,
  0.62:#000000, 0.68:#ffffff, 1:#ffffff]); spot = tex3d_colorize(cell, g=[0:#240407, 1:#4a0d0f]); fine = tex3d_fbm(noise=perlin, scale_x=28,
  scale_y=28, scale_z=28, iterations=2); albedo = tex3d_blend(spot, base, mask, blend_type=normal, amount=0.85); height = fine
  tex3d_colorize takes its gradient as g=[...], not gradient=. A texture program that fails to compile is not an error in apply_text: the
  part just shows without maps and the reason is only a console warning ("texture bake for the viewport failed"). If a texture does nothing,
  suspect the program.
- Eyes that look alive are layers, not one sphere: take a small sphere out of the head with field.subtract so the eye sits in a socket with a
  rim of skin, then stack an amber ball, a clear glossy cornea (solid.material opacity=0.16, roughness=0.02), a golden iris and a darker ring
  (spheres scaled to [1, 1, 0.3]), a black horizontal slit pupil (a sphere scaled to [1.05, 0.17, 0.3]) and a tiny emissive white glint.
- Eight arms of five bones with eight skins, two eyes with iris and pupil, a siphon and the mantle come to 56 outputs: close to the 64 limit.

More detail than the mesh edge allows:
- Relief inside the surface: field.noise@1(field, amplitude=0.02, scale=0.16, octaves=2, mode="ridged", seed=5) before field.to_solid. The surface
  moves by at most amplitude, so give the to_solid bounds that much room (else field.clipped). Its features need an edge_length of about a
  third of scale or finer; a field.to_solid over a whole creature is limited by the field-work budget (about 64 million units: an 8 m dragon
  meshes no finer than about 0.07 m), so use relief at scale 0.1 to 0.2 on big bodies and finer relief on small parts (a head, an eye ridge).
- Surface layers: solid.finish@1(part, clearcoat=0.6, clearcoat_roughness=0.08) for wet or lacquered skin, scales and eyes; translucency=0.4 for
  wings, leaves, ears and wax (light shows through); anisotropy for brushed metal and silk. Put it after solid.material.
- Relief in the pixel shader, finer than any mesh and free of texture memory: solid.finish@1(part, relief=0.6, relief_scale=22, relief_style="ridged")
  (smooth lumps, ridged creases, scales = round cells; relief_scale is features per metre: 8 bark, 30 lizard scales, 120 pores). It is
  tested on spheres and on a skinned dragon (a skinned part reads the noise at its rest position, so it should stay on the body as it
  moves; that I checked only in still frames). It replaces the toon look on that part, so build a relief creature without solid.toon.
- A low-poly body carrying fine detail: hi = field.to_solid (fine edge) of the noisy field; lo = poly.to_solid(poly.unwrap_uv(poly.decimate(poly.from_solid(hi),
  ratio=0.05))); baked = bake.maps(lo, hi, size=1024, maps="normal", max_distance=0.1, rays=32). It runs, but decimating a noisy mesh keeps its
  jagged silhouette, so decimate a smooth version (the field without noise at a coarse edge) as the low mesh and bake the noisy one onto it;
  poly.unwrap_uv reported poly.uv.stretched on the noisy sphere, so add seams or use method=angle.

Secondary motion (a tail, ear, antenna or hair that lags the body and springs back): build the chain as bone parts exactly as for a
tentacle or rig.ik (each bone anim.parent-ed to the one before, the first one to the body part, each starting where the last ends, all
outputs), skin a tube to them, and attach rig.secondary to the TIP bone:
  tip = rig.secondary@1(tip_bone, bones="tb0; tb1; tb2", stiffness=0.15, damping=3, gravity=3)   output tb2 = tip
stiffness 0 hangs free (hair), 0.3 a tail; gravity 0 floats. A spring simulation runs every frame in the viewer (ve-chain), so the chain
follows whatever moves the body (an anim.track, physics, a walk) with lag and springs back when it stops. The bones' own motion is replaced.
Tested with a body gliding back and forth and a three-bone tail: the tail trails and curves. At most 8 bones per chain, 8 chains in a scene.

Walking: a legged body walks when its body part MOVES and each leg's foot target is driven by rig.gait instead of by hand. Parent every
leg's top bone to the body (anim.parent(bone, parent="body")), give the body an anim.track (or physics, or a behaviour), build each leg as a
rig.ik chain as in the leg recipe above, and put rig.gait after rig.ik on the foot target:
  walk = rig.gait@1(reach_node, body="body", phase=0, stride_length=0.9, step_height=0.2, duty=0.6)   output leg0_target = walk
Each foot plants on the ground (ground=0 by default) while the body passes over it, then swings forward in an arc, never farther from its hip
than the leg's length. Give alternate legs phase 0 and 0.5 (spider), or 0, 0.25, 0.5, 0.75 (four legs); stride_length is metres of body travel
per step cycle, duty the share of time on the ground (0.6 walk, 0.4 run, 0.8 creep). Tested: an 8-legged spider glides along a path and its
feet plant and step; the body's own path does not turn (feet follow the body's yaw, so rotate the body part to turn it). Each rig.ik or rig.gait
chain counts toward the scene limit of 24 chains.

A real person: do NOT sculpt a head from primitives (tried: it drifts to ape proportions). body.human@1 is a real, anthropometric human with a
face, from open CC0 data:
  person = body.human@1(gender=0.2, age=0.667, height=0.55, weight=0.45, muscle=0.55, stature=1.82, waist=0.84)
  skin = solid.material@1(person, color=[0.78, 0.57, 0.46], roughness=0.55)
  fin = solid.finish@1(skin, subsurface=0.5, translucency=0.15, sheen=0.1)      output person = fin
  eyes = body.eye@1(person, iris_color=[0.28, 0.4, 0.5], gaze=[0, 0, 1])        output eyes = eyes
Sliders run 0 to 1 (0.5 average): gender 0 male to 1 female, age 0 baby, 0.333 child, 0.667 young adult, 1 old; height, weight, muscle,
proportions, and african / asian / caucasian shares. stature, waist, hip, chest (metres) and mass (kg) FIT the body to real measurements,
changing as little as possible; a request the body cannot meet gets the nearest body and a human.fit_partial warning that says what was
reached. face="jawOpen=0.8; mouthSmileLeft=0.6; eyeBlinkLeft=1" sets any of 52 face actions (0 to 1) and local="l-upperarm-fat-incr=0.5;
nose-width1-incr=-0.3" any of 256 local changes (-1 to 1; the nose, eyes, mouth, ears, chin, neck, torso and limbs each have some; a wrong name
lists the valid ones). The body is a polygon mesh (13,700 vertices, UVs
included), so every poly.* node takes it. It stands on y=0 facing +z, about 27,000 triangles; body.eye needs the body.human node itself as
input (before solid.material). cupsize and firmness (0 to 1) shape the bust; at=[x, y, z] and turn=degrees place and turn the person (the eyes and
hair follow). For fuller hips ask for a bigger hip (1.15 m) and add local="hip-scale-horiz-incr=0.8; buttocks-volume-incr=0.7".
Hair: body.hair@1(person, length=0.22, density=0.6, volume=0.3, wave=0.25, fringe=0.3, width=0.016, seed=1) grows ribbon wisps from that head's
own scalp with a natural hairline (0.02 buzz cut, 0.15 bob, 0.3 past the shoulders, no longer than 0.35); colour it with solid.material and
solid.finish(sheen=0.1). Set fine=1 for individual narrow strands and cover=true to hide the scalp beneath a hair shell; connect the body.human node itself, like body.eye. Not yet there: eyebrows,
teeth and a separate beard.
ANIMATING A PERSON (real skeleton, real skin weights, fingers): body.bone@1(person, bone="upperarm01_l") makes a tiny hidden part at that joint, a
bone, with its rest segment from the joint to the next one. Name each bone's OUTPUT exactly like the bone (output upperarm01_l = ...), join each to
its parent with anim.parent (the nearest bone you animate above it), and turn it with anim.track (property="rotation", pivot = the bone's head,
keys in degrees about X, Y, Z applied Y then X then Z, in the rest frame; the parent's motion is applied on top). body.skin@1(person,
bones="root; spine01; ...; wrist_l; finger2_1_l") skins the body to the bones you list with the pack's own weights (up to 64 bones; a bone you
leave out hands its weight to its nearest listed ancestor, so the legs can stay with root). Bone names: root, spine05 to spine01, neck01 to neck03,
head, clavicle_l, shoulder01_l, upperarm01_l, upperarm02_l (twist), lowerarm01_l (elbow), lowerarm02_l (twist), wrist_l, finger1_1_l (thumb) to
finger5_3_l (little finger), upperleg01_l, lowerleg01_l, foot_l, toe1_1_l; the same ending _r for the right side (the body's right is -x). A skinned body cannot
carry its own tracks; solid.material goes after body.skin. Clothes: body.garment@1(person, cover="long_sleeves", offset=0.008, bones="<the same
list>", hem=true) cuts a shirt, trousers (cover="pants"), shorts or shoes (shirt, long_sleeves, pants, shorts, shoes, gloves) out of the body's own surface,
lifts it off the skin, makes clean planar waist and cuff edges, and skins it with the same weights; omit hem=true to keep the original ragged edge. Use fit="tailored", ease=0.03, folds=0.006, rim=0.002 for loose cloth with deterministic folds and a closed, thick hem; tailored fit ignores hem. Give it a fabric. Hold things by parenting them to a hand bone (anim.parent
parent="wrist_r"). The arms are not solved for you: compute the joint angles for where the hands must be (a two-bone solve plus a wrist) and write
them as keys; a finger curls about the axis across its direction and the palm. Every track of a looping clip must end at the same time, or the
 clip is as long as the longest track. For real clothes and hair use \`body.outfit\` (ADR 0056): \`body.outfit(person, asset="female_casualsuit01", standoff=0.012, bones="..." )\` fits the artist's T-shirt and jeans to the body and skins them with its weights, and \`solid.texture(program="image(albedo=\"female_casualsuit01_diffuse\", normal=\"female_casualsuit01_normal\")")\` gives it the artist's pictures; hair (\`ponytail01\`, \`long01\`) is cut out with \`solid.foliage\` on the picture's alpha. A worked example, a person reloading an AK, is worlds/ak_reload.txt.

Making it read as a character: asymmetry (different lengths, a missing or extra limb, uneven phases) reads as horror; clean symmetry reads as
machine or animal. solid.style@1(part, style="anime") gives cel shading and outlines (topic styles). To move a creature through a world,
combine it with physics and behaviours (topic behaviour).`,
};
