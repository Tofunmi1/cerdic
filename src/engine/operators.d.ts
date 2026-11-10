// Generated from the operator registry (ve-graph). Run bun run schema:generate.
// Typed calls for every ready operator: `ve.solid.box({ size: [1, 1, 1] })` makes a node; the kinds of value are in the types.

export type Vec2 = [number, number];
export type Vec3 = [number, number, number];

/** The kinds of value a node can make. */
export type Kind = 'Solid' | 'Profile' | 'Curve' | 'Sdf' | 'Field' | 'Inst' | 'Light' | 'Environment' | 'Check' | 'Poly' | 'Selection' | 'Number' | 'Pattern' | 'Game' | 'Volume';

/** The id of every ready operator, `family.name@version`. */
export type OperatorId = 'anim.drop@1'
  | 'anim.machine@1'
  | 'anim.parent@1'
  | 'anim.retarget@1'
  | 'anim.simplify@1'
  | 'anim.track@1'
  | 'anim.wave@1'
  | 'bake.maps@1'
  | 'behavior.on@1'
  | 'body.bone@1'
  | 'body.eye@1'
  | 'body.garment@1'
  | 'body.hair@1'
  | 'body.human@1'
  | 'body.outfit@1'
  | 'body.skin@1'
  | 'body.skin_to@1'
  | 'building.asset@1'
  | 'check.attached@1'
  | 'check.min_wall@1'
  | 'check.motion@1'
  | 'city.buildings@1'
  | 'city.ground@1'
  | 'cloth.drape@1'
  | 'cloth.sim@1'
  | 'curve.bezier@1'
  | 'curve.circle@1'
  | 'curve.expr@1'
  | 'curve.fillet@1'
  | 'curve.fit@1'
  | 'curve.hang@1'
  | 'curve.helix@1'
  | 'curve.polyline@1'
  | 'curve.resample@1'
  | 'curve.reverse@1'
  | 'curve.spline@1'
  | 'curve.tube@1'
  | 'detail.panels@1'
  | 'field.bend@1'
  | 'field.box@1'
  | 'field.capsule@1'
  | 'field.cylinder@1'
  | 'field.ellipsoid@1'
  | 'field.expr@1'
  | 'field.intersect@1'
  | 'field.mask_from@1'
  | 'field.mirror@1'
  | 'field.move@1'
  | 'field.noise@1'
  | 'field.offset@1'
  | 'field.plane@1'
  | 'field.repeat@1'
  | 'field.scale@1'
  | 'field.shell@1'
  | 'field.smooth_union@1'
  | 'field.sphere@1'
  | 'field.subtract@1'
  | 'field.taper@1'
  | 'field.to_solid@1'
  | 'field.torus@1'
  | 'field.tube@1'
  | 'field.twist@1'
  | 'field.union@1'
  | 'flora.tree@1'
  | 'fluid.sim@1'
  | 'frame.add@1'
  | 'fx.collide@1'
  | 'fx.emitter@1'
  | 'fx.force@1'
  | 'fx.sub@1'
  | 'fx.trail@1'
  | 'game.ability@1'
  | 'game.agent@1'
  | 'game.brain@1'
  | 'game.camera@1'
  | 'game.collect@1'
  | 'game.controller@1'
  | 'game.goal@1'
  | 'game.ground@1'
  | 'game.health@1'
  | 'game.hud@1'
  | 'game.input@1'
  | 'game.kind@1'
  | 'game.level@1'
  | 'game.nav@1'
  | 'game.projectile@1'
  | 'game.rule@1'
  | 'game.script@1'
  | 'game.spawn@1'
  | 'game.state@1'
  | 'game.surface@1'
  | 'game.targeting@1'
  | 'game.terrain@1'
  | 'game.ui@1'
  | 'game.var@1'
  | 'game.weapon@1'
  | 'group.input@1'
  | 'group.use@1'
  | 'inst.along_curve@1'
  | 'inst.array@1'
  | 'inst.drop@1'
  | 'inst.jitter@1'
  | 'inst.mirror@1'
  | 'inst.place@1'
  | 'inst.program@1'
  | 'inst.radial@1'
  | 'inst.realize@1'
  | 'inst.repeat@1'
  | 'inst.scatter@1'
  | 'inst.scatter_on@1'
  | 'inst.vary@1'
  | 'light.point@1'
  | 'light.spot@1'
  | 'light.sun@1'
  | 'material.preset@1'
  | 'phys.body@1'
  | 'phys.joint@1'
  | 'phys.ragdoll@1'
  | 'phys.world@1'
  | 'poly.beautify@1'
  | 'poly.bevel_edges@1'
  | 'poly.bevel_network@1'
  | 'poly.bisect@1'
  | 'poly.bridge_loops@1'
  | 'poly.circularize@1'
  | 'poly.connect_vertices@1'
  | 'poly.decimate@1'
  | 'poly.delete_faces@1'
  | 'poly.dissolve_degenerate@1'
  | 'poly.dissolve_edge@1'
  | 'poly.dissolve_faces@1'
  | 'poly.dissolve_limit@1'
  | 'poly.dissolve_vertex@1'
  | 'poly.extrude_faces@1'
  | 'poly.extrude_region@1'
  | 'poly.fill_holes@1'
  | 'poly.flip_faces@1'
  | 'poly.fracture@1'
  | 'poly.from_solid@1'
  | 'poly.grid@1'
  | 'poly.import@1'
  | 'poly.inset_faces@1'
  | 'poly.inset_region@1'
  | 'poly.join_triangles@1'
  | 'poly.lod@1'
  | 'poly.loop_cut@1'
  | 'poly.merge_coplanar@1'
  | 'poly.mirror@1'
  | 'poly.pack_uv@1'
  | 'poly.planar_faces@1'
  | 'poly.poke@1'
  | 'poly.program@1'
  | 'poly.recalc_normals@1'
  | 'poly.relax_loops@1'
  | 'poly.remesh_quads@1'
  | 'poly.remove_doubles@1'
  | 'poly.rotate_edges@1'
  | 'poly.round_edges@1'
  | 'poly.script@1'
  | 'poly.sculpt@1'
  | 'poly.select@1'
  | 'poly.skin@1'
  | 'poly.smooth@1'
  | 'poly.solidify@1'
  | 'poly.space_loops@1'
  | 'poly.spin@1'
  | 'poly.split_edges@1'
  | 'poly.subdivide@1'
  | 'poly.subdivide_edges@1'
  | 'poly.surface_expr@1'
  | 'poly.symmetrize@1'
  | 'poly.to_solid@1'
  | 'poly.triangulate@1'
  | 'poly.unsubdivide@1'
  | 'poly.unwrap_uv@1'
  | 'poly.uv_layout@1'
  | 'poly.uv_transform@1'
  | 'poly.weld@1'
  | 'poly.wireframe@1'
  | 'profile.circle@1'
  | 'profile.difference@1'
  | 'profile.fillet@1'
  | 'profile.offset@1'
  | 'profile.polygon@1'
  | 'profile.rect@1'
  | 'profile.smooth@1'
  | 'rig.aim@1'
  | 'rig.bone@1'
  | 'rig.gait@1'
  | 'rig.ik@1'
  | 'rig.morph@1'
  | 'rig.secondary@1'
  | 'rig.skin@1'
  | 'river.carve@1'
  | 'river.water@1'
  | 'rooms.plan@1'
  | 'scatter.place@1'
  | 'scene.environment@1'
  | 'scene.sky@1'
  | 'script.js@1'
  | 'sign.text@1'
  | 'sim.program@1'
  | 'solid.box@1'
  | 'solid.capsule@1'
  | 'solid.chamfer@1'
  | 'solid.collision_boxes@1'
  | 'solid.cone@1'
  | 'solid.cylinder@1'
  | 'solid.difference@1'
  | 'solid.extrude@1'
  | 'solid.finish@1'
  | 'solid.foliage@1'
  | 'solid.hull@1'
  | 'solid.intersect@1'
  | 'solid.loft@1'
  | 'solid.material@1'
  | 'solid.revolve@1'
  | 'solid.round@1'
  | 'solid.sphere@1'
  | 'solid.style@1'
  | 'solid.sweep@1'
  | 'solid.texture@1'
  | 'solid.toon@1'
  | 'solid.torus@1'
  | 'solid.union@1'
  | 'street.prop@1'
  | 'street.road@1'
  | 'terrain.biomes@1'
  | 'terrain.erode@1'
  | 'terrain.flatten@1'
  | 'terrain.heightfield@1'
  | 'terrain.mask@1'
  | 'terrain.material@1'
  | 'terrain.place@1'
  | 'terrain.tile@1'
  | 'value.math@1'
  | 'value.motion@1'
  | 'value.number@1'
  | 'value.point@1'
  | 'value.size@1'
  | 'volume.combine@1'
  | 'volume.from_mesh@1'
  | 'volume.offset@1'
  | 'volume.smooth@1'
  | 'volume.to_mesh@1'
  | 'water.fill@1'
  | 'water.surface@1'
  | 'xform.align@1'
  | 'xform.array@1'
  | 'xform.attach@1'
  | 'xform.bend@1'
  | 'xform.lattice@1'
  | 'xform.mirror@1'
  | 'xform.move@1'
  | 'xform.noise@1'
  | 'xform.radial@1'
  | 'xform.repeat@1'
  | 'xform.rotate@1'
  | 'xform.scale@1'
  | 'xform.shear@1'
  | 'xform.taper@1'
  | 'xform.twist@1'
  | 'xform.wave@1';

declare const kindOf: unique symbol;
/** A node of a program; `K` is the kind (or kinds) of value it makes. */
export interface Node<K extends Kind = Kind> {
  readonly [kindOf]?: K;
  readonly id: string;
  readonly label: string;
}

/** Bakes a physical fall into ordinary animation keys: fixed-step gravity, floor contact, bounded bouncing and rest. Contact uses the rest bounding-box bottom, so tumbling does not change contact height. */
export interface AnimDropV1Params {
  /** Seconds when the part is released (default 0; 0..60) */
  start?: number | Node<'Number'>;
  /** Velocity at release in metres per second (default [0, 0, 0]; -50..50 each) */
  velocity?: Vec3;
  /** Turning rate at release in degrees per second about the part's bounding-box centre (default [0, 0, 0]; -2000..2000 each) */
  spin?: Vec3;
  /** World y height of the floor plane (default 0; metres, -1000..1000) */
  floor?: number | Node<'Number'>;
  /** Vertical restitution at impact (default 0.3; a ratio, 0..1) */
  bounce?: number | Node<'Number'>;
  /** Downward acceleration in metres per second squared (default 9.81; 0.1..50) */
  gravity?: number | Node<'Number'>;
  /** Length of the baked tracks in seconds (default 4; 0.5..60) */
  duration?: number | Node<'Number'>;
  /** Named animation clip; empty is the main clip, following anim.track naming rules (default ""; text up to 24 bytes) */
  clip?: string;
}

/** Chooses which named animation clip a part plays in response to game parameters. Write one statement per line, or separate statements with `;`: `entry idle; state idle clip=idle; state run clip=run; transition idle -> run when speed > 1; transition run -> idle when speed < 1`. */
export interface AnimMachineV1Params {
  /** State machine statements separated by semicolons or newlines (required; text up to 4096 bytes) */
  program: string;
}

/** Attaches a shape to another part so it moves with it, like a forearm on an upper arm or a wheel on a car. `parent` is the OUTPUT name of the part to follow. */
export interface AnimParentV1Params {
  /** The output name of the part to follow. Author this shape at its rest position in the same world coordinates; its pivots stay in those coordinates (required; text up to 64 bytes) */
  parent: string;
}

/** Copies the keyframe tracks of one bone part onto another: a walk or wave authored on one character plays on a taller one, a left-arm clip becomes a right-arm clip. `in` is the part that receives the clips (its own tracks stay) and `from` is the part whose tracks are copied; both are usually bones (rig.bone, body.bone). */
export interface AnimRetargetV1Params {
  /** Factor on position keys and on the pivot's distance from the joint: target height over source height (default 1; 0.05..20) */
  scale?: number | Node<'Number'>;
  /** Reflect the clip across the middle plane (x to minus x): a left-side clip for a right-side bone (default false) */
  mirror?: boolean;
  /** Clip names to copy, separated by commas (empty copies every clip, including the part's main clip) (default ""; text up to 256 bytes) */
  clips?: string;
}

/** Thins the keyframes of a part's tracks without changing the motion you can see: a clip a script sampled at every few milliseconds drops keys that lie on what the interpolation would give anyway. Every track of the input is thinned while it stays within `tolerance` of the original everywhere. */
export interface AnimSimplifyV1Params {
  /** How far the thinned motion may stray from the original at a bone's tip, in metres (2 mm by default) (default 0.002; metres, 0..1) */
  tolerance?: number | Node<'Number'>;
  /** The most a rotation may stray, in degrees, whatever the bone's length (default 1; an angle, 0..45) */
  max_angle?: number | Node<'Number'>;
}

/** Makes a shape move: attaches one keyframed track (position, rotation or scale over time) and passes the shape through unchanged. Chain several anim.track nodes to move and turn together. */
export interface AnimTrackV1Params {
  /** What changes: position (metres, added to where the shape is), rotation (degrees about X, Y, Z), scale (factors), glow (a brightness factor on the shape's emissive colour and the light it emits; keys are `seconds:factor`, 1 is as authored, 0 is dark), visible (keys `seconds:1` shown or `seconds:0` hidden; use interpolation step) or color (keys `seconds:r,g,b` with each 0 to 1; the shape's surface colour fades between them, replacing the authored colour) (default "position") */
  property?: 'position' | 'rotation' | 'scale' | 'glow' | 'visible' | 'color';
  /** Keyframes as `seconds:x,y,z` separated by semicolons, at least two with increasing times, for example "0:0,0,0; 2:0,90,0" swings 90 degrees about Y over two seconds. Every key names all three values (required; text up to 2048 bytes) */
  keys: string;
  /** How values move between keys: linear, smooth (ease in and out of every key), ease_in (start slowly), ease_out (end slowly), bezier (one smooth curve through ALL the keys without stopping at each, no overshoot; the best choice for motion with three or more keys), back (overshoots the next key a little and returns), bounce (bounces against the next key), elastic (springs past the next key and settles) or step (hold until the next key) (default "smooth") */
  interpolation?: 'linear' | 'smooth' | 'ease_in' | 'ease_out' | 'bezier' | 'back' | 'bounce' | 'elastic' | 'step';
  /** Start over after the last key. Off holds the last key's value (default true) */
  loop?: boolean;
  /** The named clip this track belongs to (lowercase, for example "wave" or "reach"); empty is the part's main clip. A behaviour runs one clip at a time, so a part can have several separate motions. The poses of all its clips combine (default ""; text up to 24 bytes) */
  clip?: string;
  /** For a set of placed copies (inst.array, inst.radial, inst.repeat): seconds each copy plays ahead of the one before, so a motion runs along a row like a wave. Only for placed sets (default 0; -60..60) */
  phase_step?: number | Node<'Number'>;
  /** For a set of placed copies: up to this many extra seconds of a fixed pseudo-random offset per copy, so a field of reeds or flags does not move in step. Only for placed sets (default 0; 0..60) */
  phase_jitter?: number | Node<'Number'>;
  /** The point, in the shape's own coordinates, that rotation and scale act about (default [0, 0, 0]; metres, -1000..1000 each) */
  pivot?: Vec3;
  /** Take the pivot from the bone this part is instead of typing it: head is the joint the bone turns about (what an arm or a leg should rotate about), tail is where it ends. The part must be a bone (body.bone or rig.bone); `pivot` is then ignored. none: use `pivot` (default "none") */
  pivot_at?: 'none' | 'head' | 'tail';
}

/** Makes a shape oscillate forever without writing keyframes: a bob, a sway, a pulse, a flicker. It becomes one looping anim.track (so it plays, chains and exports like any track). */
export interface AnimWaveV1Params {
  /** What oscillates: position (metres), rotation (degrees), scale (factors around base 1) or glow (brightness factor around base 1; only the first value is used) (default "position") */
  property?: 'position' | 'rotation' | 'scale' | 'glow';
  /** sine is smooth; triangle moves at constant speed and turns sharply at the ends (default "sine") */
  shape?: 'sine' | 'triangle';
  /** How far each axis swings from base (metres, degrees or a factor, depending on property). [0, 0.15, 0] bobs 15 cm up and down (default [0, 0, 0]; -1000..1000 each) */
  amplitude?: Vec3;
  /** The middle value. Leave it at 0 for position and rotation; for scale and glow an untouched base of [0, 0, 0] means [1, 1, 1] (default [0, 0, 0]; -1000..1000 each) */
  base?: Vec3;
  /** Seconds for one full swing (default 2; 0.05..600) */
  period?: number | Node<'Number'>;
  /** Where in the swing it starts, as a fraction of the period (0.5 starts half a swing in, so two shapes can move in opposite phase) (default 0; 0..1) */
  phase?: number | Node<'Number'>;
  /** The named clip this wave belongs to; empty is the part's main clip (see anim.track) (default ""; text up to 24 bytes) */
  clip?: string;
  /** For a set of placed copies: seconds each copy plays ahead of the one before (a wave along a row). Only for placed sets (default 0; -60..60) */
  phase_step?: number | Node<'Number'>;
  /** For a set of placed copies: up to this many extra seconds of a fixed pseudo-random offset per copy, so a field does not sway in step. Only for placed sets (default 0; 0..60) */
  phase_jitter?: number | Node<'Number'>;
  /** The point, in the shape's own coordinates, that rotation and scale act about (default [0, 0, 0]; metres, -1000..1000 each) */
  pivot?: Vec3;
  /** Take the pivot from the bone this part is instead of typing it: head is the joint the bone turns about (what an arm or a leg should rotate about), tail is where it ends. The part must be a bone (body.bone or rig.bone); `pivot` is then ignored. none: use `pivot` (default "none") */
  pivot_at?: 'none' | 'head' | 'tail';
}

/** Bakes the detail of a high-poly part onto a low-poly part: a tangent-space normal map and an ambient-occlusion map in the low part's UV layout. The result is the low part itself with the maps attached, so it keeps them through moves and materials, shows the normal map in the viewer and exports both in glTF. */
export interface BakeMapsV1Params {
  /** Side of each baked image in pixels (default 512; whole number 64..2048) */
  size?: number | Node<'Number'>;
  /** Which maps to bake: both, normal, or ao (default "both") */
  maps?: 'both' | 'normal' | 'ao';
  /** How far above the low surface each ray starts, in metres (0 = automatic). Raise it when the high part rises above the low surface (default 0; metres, 0..1000000) */
  cage?: number | Node<'Number'>;
  /** How far below the cage a ray may reach to find the high surface, in metres (0 = automatic) (default 0; metres, 0..1000000) */
  max_distance?: number | Node<'Number'>;
  /** Rays per texel for the occlusion map. size x size x rays may not exceed 64 million (default 64; whole number 1..256) */
  rays?: number | Node<'Number'>;
  /** How far above the surface the occlusion rays start, in metres (0 = automatic) (default 0; metres, 0..1000000) */
  bias?: number | Node<'Number'>;
}

/** Makes a shape react in play mode: when it is clicked (`select`) or the viewer comes near it (`near`), it runs its own anim.track clip. Put it after the anim.track nodes (a switch with a `target` needs no track of its own). */
export interface BehaviorOnV1Params {
  /** What starts it: select (clicked in play mode), near (the viewer comes within `radius`), signal (only another part's behavior.on that names this part as its `target` starts it), flag (the flag named in `watch` changes), grab (a hand or the mouse picks the part up in play mode; it can then be carried around and is put back when play stops), release (it is let go), hold (both: `sets` apply when picked up and reverse when let go, and the clip runs forward while held and back when released), timer (fires `after` seconds after play mode starts, then every `every` seconds if set) or socket (the part named in `accepts`, which must itself be grabbable with trigger grab, release or hold, snaps onto this part when let go within `radius` of it, and this fires like hold: `sets` as written and the clip forward; picking it out again reverses them) or collide (this part, which needs phys.body, touches another body in play mode: any body, or only the part named in `accepts`; fires when the touching starts, so a ball hitting a bell or a door slamming can light a lamp or set a flag) (default "select") */
  trigger?: 'select' | 'near' | 'signal' | 'flag' | 'grab' | 'release' | 'hold' | 'timer' | 'socket' | 'collide';
  /** With trigger="socket": the output name of the grabbable part that fits in this socket (a key for a lock). With trigger="collide": optional, the output name of the one part whose touch counts (empty means touching anything) (default ""; text up to 64 bytes) */
  accepts?: string;
  /** Which of this part's own clips (the `clip` of its anim.track nodes) this behaviour runs; empty is the main clip. A part with clips "wave" and "reach" can have a lever that runs one and a button that runs the other, by naming the clip on the target (target="arm:wave") (default ""; text up to 24 bytes) */
  clip?: string;
  /** A different clip of this part to start when the clip this behaviour runs reaches its end, for a sequence: "open" then "wave" then "close" by chaining. The chained clip runs in the same direction (default ""; text up to 24 bytes) */
  then?: string;
  /** With trigger="timer": seconds after play mode starts that it first fires (default 0; 0..3600) */
  after?: number | Node<'Number'>;
  /** With trigger="timer": seconds between later firings (0.1 or more); 0 fires only once. A toggle with `every` blinks, opens and closes, or alternates forever (default 0; 0..3600) */
  every?: number | Node<'Number'>;
  /** Flags to change when this fires, `name=on`, `name=off` or `name=toggle`, separated by semicolons (a bare name means on). Flags are scene-wide and start off. Names are lowercase letters, digits and underscores, up to 24 long, at most 8 per behaviour (default ""; text up to 160 bytes) */
  sets?: string;
  /** Flags that must be in this state for the trigger to do anything: `key` (on), `!alarm` or `alarm=off` (off), separated by semicolons. A locked door needs the flag its key sets (default ""; text up to 160 bytes) */
  needs?: string;
  /** With trigger="flag": the flag to watch. The behaviour fires every time that flag changes, and cannot set it itself (default ""; text up to 24 bytes) */
  watch?: string;
  /** The output name of another part whose clip this behaviour runs (a switch that opens a door or lights a lamp). Empty runs this part's own clip. Write `name:clip` to run one named clip of the target ("arm:wave"). The target part needs an anim.track and a behavior.on with trigger="signal" (default ""; text up to 64 bytes) */
  target?: string;
  /** play runs the clip once and holds the end; toggle alternates forward and backward; follow (only with trigger="flag") runs forward while the watched flag is on and backward while it is off, so a lamp always matches its switch; ragdoll (play mode) makes the character whose phys.ragdoll is named by target go limp when the trigger fires (a button, a thrown ball, a timer) (default "toggle") */
  action?: 'play' | 'toggle' | 'follow' | 'ragdoll';
  /** Metres from the shape's centre for a `near` trigger (default 2; metres, 0.1..100) */
  radius?: number | Node<'Number'>;
}

/** A bone of a body.human skeleton, as a part you can animate: tiny and hidden inside the body at the named joint, with its rest segment from that joint (head, where it turns) to the next joint (tail), in the body's world position. Connect the body.human node itself. */
export interface BodyBoneV1Params {
  /** The bone, in the pack's name written lowercase with `_` for `.` and `-`: `upperarm01_l`, `finger2_1_r` (required; text up to 64 bytes) */
  bone: string;
}

/** The two eyes of a body.human, layered like a real eye: a white sclera ball, a coloured iris, a black pupil and a clear glossy cornea over them, sized to the body's eye sockets (a child's eyes are smaller) and looking in the gaze direction. Connect the body.human node itself (before solid.material), and give the result its own output. */
export interface BodyEyeV1Params {
  /** Colour of the iris [r, g, b] (sRGB): blue [0.25, 0.42, 0.55], brown [0.32, 0.18, 0.08], green [0.3, 0.45, 0.25] (default [0.25, 0.42, 0.55]; 0..1 each) */
  iris_color?: Vec3;
  /** Colour of the white of the eye [r, g, b]; slightly warm white is natural (default [0.93, 0.9, 0.86]; 0..1 each) */
  sclera_color?: Vec3;
  /** Iris radius as a share of the eye ball's radius (0.4 to 0.5 is a human) (default 0.44; a ratio, 0..1) */
  iris_size?: number | Node<'Number'>;
  /** Pupil radius as a share of the iris radius: 0.25 in bright light, 0.7 in the dark (default 0.4; a ratio, 0..1) */
  pupil_size?: number | Node<'Number'>;
  /** Direction the eyes look, [x, y, z] in the body's space (+z is forward, +y up, +x the body's left). Need not be normalised (default [0, 0, 1]; -1..1 each) */
  gaze?: Vec3;
}

/** Clothes for a body.human: the part of its surface a garment covers (a shirt, trousers, shoes), lifted off the skin by a few millimetres, as its own surface to give a fabric with solid.material and solid.texture. Connect the body.human node itself. */
export interface BodyGarmentV1Params {
  /** shirt: torso and upper arms (short sleeves). long_sleeves: to the wrist. pants: hips to ankles. shorts: hips to knees. shoes: feet. gloves: hands (default "shirt") */
  cover?: 'shirt' | 'long_sleeves' | 'pants' | 'shorts' | 'shoes' | 'gloves';
  /** How far the cloth stands off the skin in metres: 0.004 tight, 0.01 loose, 0.02 padded (default 0.006; metres, 0..0.05) */
  offset?: number | Node<'Number'>;
  /** The bones of body.skin (the same list), to skin the garment with the body's weights; empty leaves it unskinned (default ""; text up to 2048 bytes) */
  bones?: string;
  /** Put in front of every bone name, so a world can hold several people: with prefix="ken_" the skin follows the parts named `ken_root`, `ken_spine01` and so on (give each body.bone that output name). Empty: the plain bone names. Use the same prefix on every node that skins this person (default ""; text up to 2048 bytes) */
  prefix?: string;
  /** dual: dual quaternions, a shoulder or an elbow keeps its volume when it turns (drawn by the CPU, the default). linear: the renderer's own GPU skinning, cheaper, with some pinching at a raised arm. Use the same blend on every node that skins this person (default "dual") */
  blend?: 'dual' | 'linear';
  /** Cut every open garment edge cleanly in its fitted plane; false preserves the body's original ragged skin-region edge (default false) */
  hem?: boolean;
}

/** Hair grown on the head of a body.human: thousands of tapered ribbon wisps that start on that head's own scalp inside a natural hairline (high forehead, temples, above the ears, down to the nape), leave it outward, fall under gravity, slide over the skull and wave a little. Connect the body.human node itself (before solid.material) and give the result its own output, then colour it with solid.material and add solid.finish(anisotropy=0.6, clearcoat=0.2) for the shine along the strands. */
export interface BodyHairV1Params {
  /** Length of the hair in metres, along the strand (default 0.22; metres, 0.01..1) */
  length?: number | Node<'Number'>;
  /** How many wisps: 0.2 is thin, 0.6 normal, 1 thick (more triangles) (default 0.6; a ratio, 0..1) */
  density?: number | Node<'Number'>;
  /** How far the hair stands off the head: 0 lies flat, 1 is big and full (default 0.3; a ratio, 0..1) */
  volume?: number | Node<'Number'>;
  /** Waviness: 0 straight, 1 loose curls (default 0.25; a ratio, 0..1) */
  wave?: number | Node<'Number'>;
  /** How far the hairline comes down the forehead: 0 a high hairline, 1 bangs to the brows (default 0.3; a ratio, 0..1) */
  fringe?: number | Node<'Number'>;
  /** Width of one wisp in metres at its root; wisps taper to a point (default 0.016; metres, 0.004..0.05) */
  width?: number | Node<'Number'>;
  /** Picks a different arrangement of the same hair (default 1; 0..100000) */
  seed?: number | Node<'Number'>;
  /** Splits each wisp into fine individual strands, up to 2.5 mm wide (default 0; a ratio, 0..1) */
  fine?: number | Node<'Number'>;
  /** Adds a smooth hair shell and short root strands to cover the scalp (default false) */
  cover?: boolean;
}

/** A real human body and head with true proportions, from named sliders: one skinned-ready surface about 27,000 triangles (13,700 vertices) with UVs, standing with its origin at the pelvis, facing +z, in metres. It is built from open CC0 anthropometric data (MakeHuman/Anny), not sculpted. */
export interface BodyHumanV1Params {
  /** 0 is the male prototype (taller, broader), 1 the female one; between blends them (default 0.5; a ratio, 0..1) */
  gender?: number | Node<'Number'>;
  /** 0 baby, 0.333 child, 0.667 young adult, 1 elderly; in between blends (0.5 is a teenager) (default 0.667; a ratio, 0..1) */
  age?: number | Node<'Number'>;
  /** 0 short, 1 tall for the age and gender (default 0.5; a ratio, 0..1) */
  height?: number | Node<'Number'>;
  /** 0 very thin, 0.5 average, 1 heavy (default 0.5; a ratio, 0..1) */
  weight?: number | Node<'Number'>;
  /** 0 soft and slight, 0.5 average, 1 muscular (default 0.5; a ratio, 0..1) */
  muscle?: number | Node<'Number'>;
  /** 0 idealised proportions (long legs, small head), 1 the less common ones (default 0.5; a ratio, 0..1) */
  proportions?: number | Node<'Number'>;
  /** Breast size: 0 small, 0.5 average, 1 large. Only shows on the female end of gender, fading to nothing at gender 0 (default 0.5; a ratio, 0..1) */
  cupsize?: number | Node<'Number'>;
  /** Breast firmness: 0 soft and low, 1 high and firm (default 0.5; a ratio, 0..1) */
  firmness?: number | Node<'Number'>;
  /** Share of the African body and face shape. The three shares are normalised, so only their ratio matters (default 0.5; a ratio, 0..1) */
  african?: number | Node<'Number'>;
  /** Share of the Asian body and face shape (default 0.5; a ratio, 0..1) */
  asian?: number | Node<'Number'>;
  /** Share of the Caucasian body and face shape (default 0.5; a ratio, 0..1) */
  caucasian?: number | Node<'Number'>;
  /** Ask for this height in metres (0 leaves the height slider in charge). The body is fitted to it, changing the least it can: stature=1.82 (default 0; metres, 0..3) */
  stature?: number | Node<'Number'>;
  /** Ask for this waist circumference in metres (0 for none): 0.84 (default 0; metres, 0..2.5) */
  waist?: number | Node<'Number'>;
  /** Ask for this hip circumference in metres (0 for none): 1.02 (default 0; metres, 0..2.5) */
  hip?: number | Node<'Number'>;
  /** Ask for this chest circumference in metres (0 for none): 1.04 (default 0; metres, 0..2.5) */
  chest?: number | Node<'Number'>;
  /** Ask for this body mass in kilograms (0 for none): 78. Mass comes from the surface volume at 980 kg/m3, so it follows the weight and muscle sliders. When several measurements conflict, the report in the warnings says what was reached (default 0; 0..400) */
  mass?: number | Node<'Number'>;
  /** Where the person stands, [x, y, z] in metres: the feet at y = 0 when y is 0. Place people side by side with this (the eyes follow the body) (default [0, 0, 0]; metres, -10000..10000 each) */
  at?: Vec3;
  /** Degrees to turn the person about the vertical axis, around where they stand: 90 faces +x, -90 faces -x, 180 faces the camera in a studio (default 0; -360..360) */
  turn?: number | Node<'Number'>;
  /** Local changes, as `name=value; name=value`, value -1 to 1: `l-upperarm-fat-incr=0.5; nose-width1-incr=-0.3; head-fat-incr=0.2` (left and right limbs are separate, `l-` and `r-`). The valid names are listed by the error message when one is wrong (default ""; text up to 2048 bytes) */
  local?: string;
  /** Face actions, as `name=value; name=value`, value 0 to 1, from the 52 ARKit-style shapes: `jawOpen=0.8; mouthSmileLeft=0.6; eyeBlinkLeft=1` (default ""; text up to 2048 bytes) */
  face?: string;
}

/** Real clothes or hair made by artists (the CC0 MakeHuman library), fitted to a body.human: it is the real garment mesh with its own UVs, bound to the body's surface, so it follows every body slider (height, weight, muscle, gender) and sits on the skin the way the artist made it. `asset` picks one: clothes `female_casualsuit01` (a T-shirt and jeans), `shoes02`, hair `long01`, `bob02`, `short02`. */
export interface BodyOutfitV1Params {
  /** The clothes or hair: female_casualsuit01, shoes02, long01, bob02, short02 (required; text up to 64 bytes) */
  asset: string;
  /** Extra standoff from the body in metres (positive loosens the garment, negative tightens it) (default 0; metres, -0.02..0.05) */
  offset?: number | Node<'Number'>;
  /** The least distance from the skin in metres: cloth made for a slimmer body would sink into a heavier one, so no part of it goes closer than this (default 0.004; metres, 0..0.03) */
  standoff?: number | Node<'Number'>;
  /** The bones of body.skin (the same list), to skin the asset with the body's weights; empty leaves it unskinned (default ""; text up to 2048 bytes) */
  bones?: string;
  /** Put in front of every bone name, so a world can hold several people: with prefix="ken_" the skin follows the parts named `ken_root`, `ken_spine01` and so on (give each body.bone that output name). Empty: the plain bone names. Use the same prefix on every node that skins this person (default ""; text up to 2048 bytes) */
  prefix?: string;
  /** dual: dual quaternions, a shoulder or an elbow keeps its volume when it turns (drawn by the CPU, the default). linear: the renderer's own GPU skinning, cheaper, with some pinching at a raised arm. Use the same blend on every node that skins this person (default "dual") */
  blend?: 'dual' | 'linear';
}

/** Skins a body.human to the bones you list, with the pack's own skin weights (the real ones, so knuckles, elbows and shoulders bend properly, unlike rig.skin's distance weights): each vertex follows the strongest four bones that move it. A bone you do not list hands its share to its nearest listed ancestor, so list only the joints you animate (an upper body with fingers is about 55 bones, the legs can stay with `root`). */
export interface BodySkinV1Params {
  /** Bone names separated by semicolons, root first, for example "root; spine01; head; upperarm01_l; lowerarm01_l; wrist_l" (required; text up to 2048 bytes) */
  bones: string;
  /** Put in front of every bone name, so a world can hold several people: with prefix="ken_" the skin follows the parts named `ken_root`, `ken_spine01` and so on (give each body.bone that output name). Empty: the plain bone names. Use the same prefix on every node that skins this person (default ""; text up to 2048 bytes) */
  prefix?: string;
  /** dual: dual quaternions, a shoulder or an elbow keeps its volume when it turns (drawn by the CPU, the default). linear: the renderer's own GPU skinning, cheaper, with some pinching at a raised arm. Use the same blend on every node that skins this person (default "dual") */
  blend?: 'dual' | 'linear';
}

/** Transfers body.skin weights to a cloth solid from its nearest body surface vertex. */
export interface BodySkinToV1Params {
  /** The body.skin bone list (default ""; text up to 2048 bytes) */
  bones?: string;
  /** Put in front of every bone name, so a world can hold several people: with prefix="ken_" the skin follows the parts named `ken_root`, `ken_spine01` and so on (give each body.bone that output name). Empty: the plain bone names. Use the same prefix on every node that skins this person (default ""; text up to 2048 bytes) */
  prefix?: string;
  /** dual: dual quaternions, a shoulder or an elbow keeps its volume when it turns (drawn by the CPU, the default). linear: the renderer's own GPU skinning, cheaper, with some pinching at a raised arm. Use the same blend on every node that skins this person (default "dual") */
  blend?: 'dual' | 'linear';
}

/** A close-view, rule-built building asset. Twelve architectural grammars change window proportions, floor rhythm, vertical elements and roof equipment. */
export interface BuildingAssetV1Params {
  /** Building origin in metres (x, y, z), so open facade meshes can be placed without a solid boolean transform (default [0, 0, 0]; metres, -1000..1000 each) */
  at?: Vec3;
  /** Building frontage in metres (default 24; metres, 8..80) */
  width?: number | Node<'Number'>;
  /** Building depth in metres (default 18; metres, 8..80) */
  depth?: number | Node<'Number'>;
  /** Number of storeys (default 8; whole number 2..40) */
  floors?: number | Node<'Number'>;
  /** Window bays on each facade (default 6; whole number 2..16) */
  bays?: number | Node<'Number'>;
  /** Storey height in metres (default 3.6; metres, 2.8..5.5) */
  floor_height?: number | Node<'Number'>;
  /** Facade detail tier: mullions, sills and cornice details (default 2; whole number 0..3) */
  detail?: number | Node<'Number'>;
  /** Architectural facade grammar. Each style changes opening proportions, floor rhythm, vertical elements, setbacks and roof equipment (default "art_deco") */
  style?: 'art_deco' | 'classical' | 'tenement' | 'brownstone' | 'precast' | 'curtainwall' | 'industrial' | 'brutalist' | 'modernist' | 'mediterranean' | 'victorian' | 'warehouse';
  /** Material-separated geometry group, including entry doors. Make one node per part to assign distinct materials (default "masonry") */
  part?: 'masonry' | 'glass' | 'trim' | 'roof' | 'doors';
  /** Stable variation seed (default 1; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
}

/** Finds solid parts separated from their nearest neighbour by a gap larger than the tolerance. */
export interface CheckAttachedV1Params {
  /** Largest gap between a part and its nearest neighbour that still counts as attached (default 0.02; metres, 0..1) */
  tolerance?: number | Node<'Number'>;
  /** warning lists the floating parts in the evaluation's warnings; error fails the evaluation (default "warning") */
  severity?: 'warning' | 'error';
}

/** Fails when the wall between an outer and an inner profile is thinner than `min`. */
export interface CheckMinWallV1Params {
  /** Smallest allowed wall thickness in metres (required; metres, 0.000001..1000000) */
  min: number | Node<'Number'>;
}

/** Samples the named parts through a clip to find feet that slide while planted and loops that pop at the seam. `feet` is a comma-separated list of part output names; `speed` is the travel speed of an in-place clip along +x. */
export interface CheckMotionV1Params {
  /** Comma-separated output names of parts whose lowest point should be checked for foot sliding (default ""; text up to 512 bytes) */
  feet?: string;
  /** Clip to sample; empty selects the main clip (default ""; text up to 24 bytes) */
  clip?: string;
  /** Sample duration in seconds (default 1; 0.1..30) */
  duration?: number | Node<'Number'>;
  /** Samples per second; duration times fps is limited to 3,600 frames (default 30; whole number 5..120) */
  fps?: number | Node<'Number'>;
  /** Travel speed in metres per second along +x for an in-place clip (default 0; -20..20) */
  speed?: number | Node<'Number'>;
  /** Largest average planted-foot speed allowed in metres per second (default 0.1; 0..5) */
  slide_max?: number | Node<'Number'>;
  /** warning reports findings; error fails evaluation on the first finding (default "warning") */
  severity?: 'warning' | 'error';
}

/** Every building of a modern district: a shop floor, a shaft that steps back in tiers when tall, and a flat roof with rooftop clutter, on every plot of an irregular street grid, tallest downtown. Wall UVs are facade units (u every 8 m along a wall, v every three floors) so ONE tiling facade texture fits every building. */
export interface CityBuildingsV1Params {
  /** Width along X and depth along Z of the district in metres, centred on the origin (default [300, 300]; metres, 60..1000 each) */
  size?: Vec2;
  /** Where downtown is (X, Z): the tallest towers stand here and buildings fall away from it (default [0, 0]; metres, -1000..1000 each) */
  centre?: Vec2;
  /** Street spacing along X in metres, centre to centre: 40 is a dense Tokyo-like grid, 80 a spread-out modern district (default 40; metres, 15..200) */
  block?: number | Node<'Number'>;
  /** Street spacing along Z. 0 (the default) means the same as block, a square grid; 15 or more makes long thin blocks: block 80 with block_depth 270 is the shape of Manhattan's (default 0; metres, 0..400) */
  block_depth?: number | Node<'Number'>;
  /** Metres between facing buildings: kerb to kerb with the two pavements (default 9; metres, 2..40) */
  street_width?: number | Node<'Number'>;
  /** The smallest building plot side wanted, in metres (default 6; metres, 3..40) */
  lot_min?: number | Node<'Number'>;
  /** Blocks are split into plots until their long side is at most this; at least 1.5 times lot_min (default 14; metres, 4.5..80) */
  lot_max?: number | Node<'Number'>;
  /** 0 is a square grid, 1 bends the streets like an old city (default 0.5; a ratio, 0..1) */
  irregularity?: number | Node<'Number'>;
  /** The same seed gives the same district (default 1; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** shop: the ground floors (and stoops). walls: the shafts above them, with their setback tiers and cornices. roof: roofs, ledges, rooftop clutter, water towers and spires. metal: iron fire escapes (needs detail 0.34 or more). awning: shop awnings (needs detail 0.2 or more). One node per part, each with its own material (default "walls") */
  part?: 'shop' | 'walls' | 'roof' | 'metal' | 'awning';
  /** How many facade styles the buildings are split into: each building belongs to one, picked from its plot. Make one node per style (default 4; whole number 1..8) */
  styles?: number | Node<'Number'>;
  /** Which style this node builds, 0 to styles - 1: only the buildings of that style are in it (default 0; whole number 0..7) */
  style?: number | Node<'Number'>;
  /** Scales every building's height: 0.5 gives a low district, 1.5 a skyline (default 1; a ratio, 0.3..2) */
  height_scale?: number | Node<'Number'>;
  /** No building is lower than this many metres, after height_scale. Raise it for a district of even, mid-rise streets (Paris is about 20 m) (default 3; metres, 3..300) */
  height_min?: number | Node<'Number'>;
  /** No building is taller than this many metres, after height_scale: 25 gives a district with no towers. Must not be below height_min (default 300; metres, 3..300) */
  height_max?: number | Node<'Number'>;
  /** Storey height in metres; sets how often the facade texture's floors repeat (default 3.6; metres, 2.4..6) */
  floor?: number | Node<'Number'>;
  /** Rooftop clutter: air-conditioning units, water tanks, stair cores and antennas. 0 gives flat roofs (default 0.8; a ratio, 0..1) */
  clutter?: number | Node<'Number'>;
  /** New York street detail, 0 (none, the default) to 1: shop awnings from 0.2, a stoop on houses under 16 m from 0.4, fire escapes on buildings of 18 to 32 m from 0.34, cornices and rooftop wooden water towers from 0.5, and on towers over 90 m a second setback and a stepped crown with a mast from 0.3. Fire escapes are in part metal, awnings in part awning (default 0; a ratio, 0..1) */
  detail?: number | Node<'Number'>;
  /** The share of buildings from about 19 m to 50 m tall that get a Paris-style mansard roof: a steep slope one storey high with dormer windows, drawn in part roof (the dormer fronts are in part walls). 0, the default, keeps every roof flat; 1 gives them all one (default 0; a ratio, 0..1) */
  mansard?: number | Node<'Number'>;
  /** The share of big rectangular plots (16 m or more on each side) built as an L or a U round a courtyard instead of one box: a street wing at full height and lower wings behind it. 0, the default, keeps one box on every plot (default 0; a ratio, 0..1) */
  courtyards?: number | Node<'Number'>;
}

/** The ground of a modern district: the road surface under everything and the raised pavement block by block, from the same streets as city.buildings (use the same size, block, street_width, lot sizes, irregularity and seed). UVs are metres / 8, so a tiling texture keeps its scale. */
export interface CityGroundV1Params {
  /** Width along X and depth along Z of the district in metres, centred on the origin (default [300, 300]; metres, 60..1000 each) */
  size?: Vec2;
  /** Where downtown is (X, Z): the tallest towers stand here and buildings fall away from it (default [0, 0]; metres, -1000..1000 each) */
  centre?: Vec2;
  /** Street spacing along X in metres, centre to centre: 40 is a dense Tokyo-like grid, 80 a spread-out modern district (default 40; metres, 15..200) */
  block?: number | Node<'Number'>;
  /** Street spacing along Z. 0 (the default) means the same as block, a square grid; 15 or more makes long thin blocks: block 80 with block_depth 270 is the shape of Manhattan's (default 0; metres, 0..400) */
  block_depth?: number | Node<'Number'>;
  /** Metres between facing buildings: kerb to kerb with the two pavements (default 9; metres, 2..40) */
  street_width?: number | Node<'Number'>;
  /** The smallest building plot side wanted, in metres (default 6; metres, 3..40) */
  lot_min?: number | Node<'Number'>;
  /** Blocks are split into plots until their long side is at most this; at least 1.5 times lot_min (default 14; metres, 4.5..80) */
  lot_max?: number | Node<'Number'>;
  /** 0 is a square grid, 1 bends the streets like an old city (default 0.5; a ratio, 0..1) */
  irregularity?: number | Node<'Number'>;
  /** The same seed gives the same district (default 1; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** road: the asphalt under the whole district. pavement: the raised pavement of every block. Make one node of each and give them materials (default "road") */
  part?: 'road' | 'pavement';
}

/** Drops a flat panel onto a body and lets it settle under gravity, like cloth: a cape over shoulders, a tunic over a torso, a sheet over a table. The panel is a polygon mesh (a poly.grid, bent or moved into place above the body); the result keeps its faces and UVs and only its points move. */
export interface ClothDrapeV1Params {
  /** How long it is left to settle, in seconds (default 3; 0.1..20) */
  seconds?: number | Node<'Number'>;
  /** Solver steps per frame: more is stiffer and steadier but slower (default 8; whole number 2..32) */
  substeps?: number | Node<'Number'>;
  /** Mass per square metre in kilograms (a light cotton is about 0.15, canvas about 0.4) (default 0.2; 0.01..10) */
  density?: number | Node<'Number'>;
  /** How much the cloth can stretch: 0 does not, larger gives like elastic (default 0; 0..1) */
  stretch?: number | Node<'Number'>;
  /** How easily it folds: 0 is stiff like card, larger is limp like silk (default 0.02; 0..1) */
  bend?: number | Node<'Number'>;
  /** How far the cloth keeps from the body, in metres (default 0.01; metres, 0..0.2) */
  thickness?: number | Node<'Number'>;
  /** How much it grips what it lies on: 0 slides freely, 1 sticks (default 0.5; a ratio, 0..1) */
  friction?: number | Node<'Number'>;
  /** Speed lost per second: higher settles sooner and moves less (default 0.5; 0..5) */
  damping?: number | Node<'Number'>;
  /** Downward acceleration in metres per second squared (default 9.81; 0..50) */
  gravity?: number | Node<'Number'>;
  /** Wind velocity in metres per second (needs drag above 0 to act) (default [0, 0, 0]; -100..100 each) */
  wind?: Vec3;
  /** How hard air pushes on the cloth; 0 turns the wind off (default 0; 0..10) */
  drag?: number | Node<'Number'>;
  /** How finely the body is sampled as a collider, in metres; smaller catches thin parts, larger is faster (default 0.03; metres, 0.005..0.5) */
  voxel_size?: number | Node<'Number'>;
  /** Height of a floor the cloth cannot sink through, in metres; leave at the default for none (default -1000000; metres, -1000000..1000000) */
  floor?: number | Node<'Number'>;
}

/** Makes a garment live cloth that moves while the scene runs: a cape that streams behind a running figure, a banner in the wind, a skirt that swings. The garment is a polygon mesh (made with poly.grid or any polygon node, shaped as you like); it keeps its shape in the editor and is simulated every frame in the viewer. */
export interface ClothSimV1Params {
  /** Output name of the bone part the pinned points follow; empty keeps them fixed in the world (default ""; text up to 64 bytes) */
  pin_bone?: string;
  /** Output names of bone parts (separated by commas or semicolons, at most 16) that push the cloth away as capsules (default ""; text up to 512 bytes) */
  collide_bones?: string;
  /** Radius of each collider capsule around its bone, in metres (default 0.07; metres, 0..2) */
  collider_radius?: number | Node<'Number'>;
  /** Solver steps per frame: more is stiffer and steadier but slower (default 6; whole number 2..32) */
  substeps?: number | Node<'Number'>;
  /** Mass per square metre in kilograms (default 0.2; 0.01..10) */
  density?: number | Node<'Number'>;
  /** How much the cloth can stretch: 0 does not (default 0; 0..1) */
  stretch?: number | Node<'Number'>;
  /** How easily it folds: 0 is stiff like card, larger is limp like silk (default 0.02; 0..1) */
  bend?: number | Node<'Number'>;
  /** How far the cloth keeps from colliders, in metres (default 0.01; metres, 0..0.2) */
  thickness?: number | Node<'Number'>;
  /** How much it grips what it lies on (default 0.5; a ratio, 0..1) */
  friction?: number | Node<'Number'>;
  /** Speed lost per second (default 0.5; 0..5) */
  damping?: number | Node<'Number'>;
  /** Downward acceleration in metres per second squared (default 9.81; 0..50) */
  gravity?: number | Node<'Number'>;
  /** Wind velocity in metres per second (needs drag above 0 to act) (default [0, 0, 0]; -100..100 each) */
  wind?: Vec3;
  /** How hard air pushes on the cloth; 0 turns the wind off (default 0; 0..10) */
  drag?: number | Node<'Number'>;
  /** Height of a floor the cloth cannot sink through, in metres; the default means none (default -1000000; metres, -1000000..1000000) */
  floor?: number | Node<'Number'>;
}

/** A cubic Bézier curve sampled deterministically. */
export interface CurveBezierV1Params {
  /** Four cubic Bézier controls [x, y, z] in metres (required; 4 to 4 points) */
  points: Vec3[];
  /** Deterministic subdivisions along the cubic (default 24; whole number 1..512) */
  steps?: number | Node<'Number'>;
  /** Require matching endpoints and make the sampled path periodic (default false) */
  closed?: boolean;
}

/** A closed, evenly sampled circle in the XZ plane. */
export interface CurveCircleV1Params {
  /** Circle center [x, y, z] in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  center?: Vec3;
  /** Circle radius in metres (default 1; metres, 0.000001..1000000) */
  radius?: number | Node<'Number'>;
  /** Number of unique samples around the closed circle (default 48; whole number 8..512) */
  segments?: number | Node<'Number'>;
}

/** A space curve written as a program of `t`, from 0 to 1 along the curve, sampled `samples` times (also `i`, the sample number, and `count`). `out` is the point: a helix is `out = vec3(cos(t * 6.2832 * 3.0), t * 2.0, sin(t * 6.2832 * 3.0))`. */
export interface CurveExprV1Params {
  /** The program. The last statement sets `out`, a vec3 (a point in metres) (default "out = vec3(t, 0.0, 0.0)"; text up to 8192 bytes) */
  code?: string;
  /** Points along the curve (default 64; whole number 2..4096) */
  samples?: number | Node<'Number'>;
  /** Join the last point back to the first (default false) */
  closed?: boolean;
}

/** Rounds the corners of a curve (a polyline path with sharp bends becomes a smooth pipe route). Each interior corner is replaced by a smooth curve that starts `distance` before the corner and ends `distance` after it. */
export interface CurveFilletV1Params {
  /** How far from each corner the rounding starts, in metres; shortened automatically on short segments (default 0.1; metres, 0.0001..1000) */
  distance?: number | Node<'Number'>;
  /** Samples per rounded corner (default 6; whole number 1..32) */
  steps?: number | Node<'Number'>;
}

/** Fits a smooth curve to sketched or freehand points: give the points a pen or a mouse would trace (or measured data) and get a chain of cubic Béziers that stays within `tolerance` of every one of them, sampled `steps` times per piece, instead of the jagged polyline. Where the path turns sharper than `corner_angle` it is cut and each side fitted on its own, so a corner stays a corner and a smooth bend stays smooth; repeated points are ignored. */
export interface CurveFitV1Params {
  /** Ordered points [x, y, z] in metres (required; 2 to 4096 points) */
  points: Vec3[];
  /** The most the fitted curve may stray from an input point, in metres: larger gives fewer, smoother pieces; smaller follows the points closely (default 0.02; metres, 0.000001..1000) */
  tolerance?: number | Node<'Number'>;
  /** Turns sharper than this are kept as corners (60 degrees by default); use 180 for a corner-free curve (default 1.0471975511965976; an angle, 0..3.141592653589793) */
  corner_angle?: number | Node<'Number'>;
  /** Treat the points as a loop and join the last back to the first smoothly (default false) */
  closed?: boolean;
  /** Samples along each fitted piece of the curve (default 12; whole number 2..64) */
  steps?: number | Node<'Number'>;
}

/** A catenary rope hanging under gravity between two tie points. */
export interface CurveHangV1Params {
  /** First tie point [x, y, z] in metres (default [0, 3, 0]; metres, -1000000..1000000 each) */
  from?: Vec3;
  /** Second tie point [x, y, z] in metres (default [4, 3, 0]; metres, -1000000..1000000 each) */
  to?: Vec3;
  /** Extra rope length as a fraction of the straight distance (default 0.05; a ratio, 0..2) */
  slack?: number | Node<'Number'>;
  /** Number of samples along the rope (default 32; whole number 3..512) */
  points?: number | Node<'Number'>;
}

/** A helix about a vertical axis (a spring, a corkscrew, a spiral staircase rail, a coil). Differing end_radius makes a cone-shaped spiral. */
export interface CurveHelixV1Params {
  /** The point on the axis where the helix starts (default [0, 0, 0]; metres, -1000000..1000000 each) */
  center?: Vec3;
  /** Radius at the start in metres (default 0.5; metres, 0.001..1000) */
  radius?: number | Node<'Number'>;
  /** Radius at the end; set it different from radius for a conical spiral (default 0.5; metres, 0.001..1000) */
  end_radius?: number | Node<'Number'>;
  /** How far it rises per full turn, in metres (0 gives a flat circular spiral) (default 0.2; metres, 0..1000) */
  pitch?: number | Node<'Number'>;
  /** Number of full turns (default 3; whole number 1..64) */
  turns?: number | Node<'Number'>;
  /** Straight pieces per turn; more is rounder (default 16; whole number 4..128) */
  segments_per_turn?: number | Node<'Number'>;
}

/** A space curve through ordered points. */
export interface CurvePolylineV1Params {
  /** Ordered control points [x, y, z] in metres (required; 2 to 4096 points) */
  points: Vec3[];
  /** Connect the final sample back to the first to form a periodic path (default false) */
  closed?: boolean;
}

/** Replaces a curve by evenly spaced points along its length (same shape, uniform spacing), for even rungs, rails and repeats. */
export interface CurveResampleV1Params {
  /** How many points; the first and last stay at the ends of an open curve (default 16; whole number 2..8192) */
  count?: number | Node<'Number'>;
}

/** Reverses the direction of a curve (swap which end is the start), which flips which end a sweep starts from. */
export interface CurveReverseV1Params {
}

/** A smooth curve that passes through every point you give (a centripetal Catmull-Rom spline: no loops or overshoot between points). Use it for a winding path, a pipe or rope route, a rail, an organic outline. */
export interface CurveSplineV1Params {
  /** Points [x, y, z] in metres the curve passes through, in order (required; 2 to 256 points) */
  points: Vec3[];
  /** Samples between each pair of points; more is smoother (default 8; whole number 1..64) */
  steps?: number | Node<'Number'>;
  /** Join the last point back to the first (needs at least 3 points) (default false) */
  closed?: boolean;
}

/** A round tube along a curve for trunks, branches, vines, pipes, roots, and cables. The radius runs from radius to end_radius along the length (a taper), or through the stations of radii. */
export interface CurveTubeV1Params {
  /** Radius at the start in metres (default 0.1; metres, 0..100) */
  radius?: number | Node<'Number'>;
  /** Radius at the end; zero keeps the start radius (default 0; metres, 0..100) */
  end_radius?: number | Node<'Number'>;
  /** Number of sides around the tube (default 8; whole number 3..64) */
  sides?: number | Node<'Number'>;
  /** Flat end caps on an open curve. Leave this on: a tube with open ends is not a solid and is refused. A closed curve has no ends (default true) */
  caps?: boolean;
  /** Optional radii in metres, 2 to 16 separated by commas, at equal steps along the length, for example "0.3, 0.12, 0.1" for a flared base. Replaces radius and end_radius (default ""; text up to 256 bytes) */
  radii?: string;
}

/** Panel lines and greebles on one face of a solid: a grid of plates laid on the face, some standing out, some vented, the seams between them left open. It makes a NEW solid of just the plates (the input is not part of it), sitting exactly on the face so it touches the wall (join it with solid.union, texture it with material.preset, check it with check.attached). */
export interface DetailPanelsV1Params {
  /** Which side of the input's bounding box the plates cover (default "+z") */
  face?: '+x' | '-x' | '+y' | '-y' | '+z' | '-z';
  /** Plates along the face's first direction (x for +z and -z, z for +x and -x, x for +y and -y) (default 4; whole number 1..32) */
  cols?: number | Node<'Number'>;
  /** Plates along the face's second direction (y for the x and z faces, z for the y faces) (default 3; whole number 1..32) */
  rows?: number | Node<'Number'>;
  /** Seam between neighbouring plates in metres; each plate is inset by half of it on every side. A gap as wide as a plate is refused (default 0.02; metres, 0..1) */
  gap?: number | Node<'Number'>;
  /** Thickness of a flat plate in metres (default 0.04; metres, 0.001..1) */
  depth?: number | Node<'Number'>;
  /** Share of plates that stand out, each 1.5 to 3 times `depth` thick (default 0.3; a ratio, 0..1) */
  raised?: number | Node<'Number'>;
  /** Share of plates drawn as a vent: a flat plate with 4 to 6 slats standing on it (default 0.1; a ratio, 0..1) */
  vents?: number | Node<'Number'>;
  /** Chooses which plates are raised or vented and how tall (default 1; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
}

/** Bends a field: the axis curves into a circular arc toward `toward` through `angle` degrees between `from` and `to` (the point at `from` stays put; beyond `to` the axis continues straight along the end tangent): a tail, a horn, an arm. Example: field.bend@1(f, axis="y", toward="x", from=-0.5, to=0.5, angle=90). */
export interface FieldBendV1Params {
  /** Axis the warp follows (default "y") */
  axis?: 'x' | 'y' | 'z';
  /** Where along the axis the warp starts, in metres; before it the warp is held at its starting value. A bend keeps this point fixed (default 0; metres, -1000000..1000000) */
  from?: number | Node<'Number'>;
  /** Where along the axis the warp ends, in metres; it must differ from `from` (less than `from` works the other way round); after it the warp is held at its end value (default 1; metres, -1000000..1000000) */
  to?: number | Node<'Number'>;
  /** Total rotation in degrees (default 0; -360..360) */
  angle?: number | Node<'Number'>;
  /** The axis the bar curves toward (x, y or z, not the bend axis itself); auto picks the next one cyclically: x to y, y to z, z to x. A positive angle curves toward its positive side (default "auto") */
  toward?: 'auto' | 'x' | 'y' | 'z';
}

/** Exact rounded box field centered at the origin. */
export interface FieldBoxV1Params {
  /** Half extents in metres (required; metres, 0.000001..1000000 each) */
  half: Vec3;
  /** Corner radius in metres; 0 gives sharp corners (default 0; metres, 0..1000000) */
  rounding?: number | Node<'Number'>;
}

/** Exact capsule field along the Y axis. */
export interface FieldCapsuleV1Params {
  /** Capsule radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Half-length of its central segment (required; metres, 0.000001..1000000) */
  half_length: number | Node<'Number'>;
}

/** Exact finite cylinder field along the Y axis. */
export interface FieldCylinderV1Params {
  /** Cylinder radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Cylinder half-height in metres (required; metres, 0.000001..1000000) */
  half_height: number | Node<'Number'>;
}

/** Axis-aligned ellipsoid; unequal radii use a conservative distance lower bound. */
export interface FieldEllipsoidV1Params {
  /** Positive XYZ radii in metres (required; metres, 0.000001..1000000 each) */
  radii: Vec3;
}

/** A function of position, written as a short program: `out = ...` is the result, built from the position `p`, the surface normal `n` (`n.y` is 1 on flat ground and falls on slopes), `uv`, numbers, vec2/vec3/vec4, swizzles (`p.xz`), + - * / %, comparisons, && || !, and the functions abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length distance dot cross normalize select, ramp(x, stop, value, ...) and curve(x, stop, value, ...) for piecewise-linear curves, and the noises perlin(p, seed), fbm(p, octaves, seed), ridged(p, octaves, seed), voronoi(p, seed). Statements are separated by `;`, for example `h = smoothstep(10.0, 40.0, p.y); out = h * select(n.y > 0.9, 1.0, 0.0)`. */
export interface FieldExprV1Params {
  /** The expression program, at most 8,192 bytes and 256 operations. The last statement must set `out` (default "out = 1.0"; text up to 8192 bytes) */
  code?: string;
}

/** Hard intersection of two fields. */
export interface FieldIntersectV1Params {
}

/** A terrain layer as a pattern: the value of a named layer of a terrain tile (made by terrain.mask, erosion or a river) at any position, smoothly interpolated, 0 outside the tile. Use it to let one mask drive several things (scatter, other patterns). */
export interface FieldMaskFromV1Params {
  /** Name of the layer, made first with terrain.mask or a node that writes layers (default ""; text up to 32 bytes) */
  layer?: string;
}

/** Mirrors a field across selected origin planes. */
export interface FieldMirrorV1Params {
  /** XYZ mirror switches as 0 or 1 (required; 0..1 each) */
  axes: Vec3;
}

/** Translates a field by an XYZ offset. */
export interface FieldMoveV1Params {
  /** Translation in metres (required; metres, -1000000..1000000 each) */
  by: Vec3;
}

/** Displaces a field surface by bounded deterministic smooth or ridged noise. */
export interface FieldNoiseV1Params {
  /** Maximum displacement in metres (default 0.02; metres, 0..1000000) */
  amplitude?: number | Node<'Number'>;
  /** First octave wavelength in metres (default 0.25; metres, 0.001..1000000) */
  scale?: number | Node<'Number'>;
  /** Number of noise octaves (default 1; whole number 1..6) */
  octaves?: number | Node<'Number'>;
  /** Weight ratio between octaves (default 0.5; 0..1) */
  persistence?: number | Node<'Number'>;
  /** Deterministic integer noise seed (default 0; whole number 0..4294967295) */
  seed?: number | Node<'Number'>;
  /** Smooth noise or creased ridged noise (default "smooth") */
  mode?: 'smooth' | 'ridged';
}

/** Offsets the zero surface along its field value. */
export interface FieldOffsetV1Params {
  /** Signed offset in metres (default 0; metres, -1000000..1000000) */
  amount?: number | Node<'Number'>;
}

/** Normalized half-space plane; bounded meshing requires a closed field composition. */
export interface FieldPlaneV1Params {
  /** Nonzero plane normal (required; -1000000..1000000 each) */
  normal: Vec3;
  /** Signed plane offset in metres (default 0; metres, -1000000..1000000) */
  offset?: number | Node<'Number'>;
}

/** Boundedly repeats a field around the origin with folded coordinates. */
export interface FieldRepeatV1Params {
  /** Positive XYZ cell periods in metres (required; metres, 0.000001..1000000 each) */
  period: Vec3;
  /** Integer copy counts per axis, from 0 to 16 on each side (required; 0..16 each) */
  count: Vec3;
}

/** Uniformly scales a field around the origin. */
export interface FieldScaleV1Params {
  /** Uniform scale factor (default 1; a ratio, 0.000001..1000000) */
  factor?: number | Node<'Number'>;
}

/** Creates a symmetric shell around the zero surface. */
export interface FieldShellV1Params {
  /** Shell half-width in metres (required; metres, 0.000001..1000000) */
  half_width: number | Node<'Number'>;
}

/** Polynomial smooth union; its distance guarantee is None. */
export interface FieldSmoothUnionV1Params {
  /** Blend radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
}

/** Exact sphere field centered at the origin. */
export interface FieldSphereV1Params {
  /** Sphere radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
}

/** Subtracts field B from field A. */
export interface FieldSubtractV1Params {
}

/** Tapers a field along an axis: the cross-section is scaled linearly from `scale_start` at `from` to `scale_end` at `to` (held beyond): a nose, a horn, a column that narrows. Example: field.taper@1(f, axis="y", from=-0.5, to=0.5, scale_start=1, scale_end=0.5). */
export interface FieldTaperV1Params {
  /** Axis the warp follows (default "y") */
  axis?: 'x' | 'y' | 'z';
  /** Where along the axis the warp starts, in metres; before it the warp is held at its starting value. A bend keeps this point fixed (default 0; metres, -1000000..1000000) */
  from?: number | Node<'Number'>;
  /** Where along the axis the warp ends, in metres; it must differ from `from` (less than `from` works the other way round); after it the warp is held at its end value (default 1; metres, -1000000..1000000) */
  to?: number | Node<'Number'>;
  /** Cross-section scale at from (default 1; 0.01..100) */
  scale_start?: number | Node<'Number'>;
  /** Cross-section scale at to (default 1; 0.01..100) */
  scale_end?: number | Node<'Number'>;
}

/** Meshes a field over explicit bounds and edge length. */
export interface FieldToSolidV1Params {
  /** Minimum XYZ bounds in metres (required; metres, -1000000..1000000 each) */
  min: Vec3;
  /** Maximum XYZ bounds in metres (required; metres, -1000000..1000000 each) */
  max: Vec3;
  /** Grid spacing in metres; smaller values use more memory and time (default 0.05; metres, 0.0001..1000) */
  edge_length?: number | Node<'Number'>;
}

/** Exact torus field around the Y axis. */
export interface FieldTorusV1Params {
  /** Major radius in metres (required; metres, 0.000001..1000000) */
  major: number | Node<'Number'>;
  /** Tube radius in metres (required; metres, 0.000001..1000000) */
  tube: number | Node<'Number'>;
}

/** Tube around a bounded ordered polyline: constant radius, or tapering from `radius` at the first point to `end_radius` at the last (a limb, tail or horn). */
export interface FieldTubeV1Params {
  /** Ordered tube centerline points [x, y, z] in metres (required; 2 to 128 points) */
  points: Vec3[];
  /** Tube radius in metres (at the first point when tapering) (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Radius at the last point; the radius changes linearly with distance along the polyline. 0 keeps the radius constant (default 0; metres, 0..1000000) */
  end_radius?: number | Node<'Number'>;
}

/** Twists a field about an axis: the cross-section turns by 0 degrees at `from` up to `angle` degrees at `to` (held beyond), keeping the volume: a tusk, a shell, a rope. Example: field.twist@1(f, axis="y", from=-0.5, to=0.5, angle=90). */
export interface FieldTwistV1Params {
  /** Axis the warp follows (default "y") */
  axis?: 'x' | 'y' | 'z';
  /** Where along the axis the warp starts, in metres; before it the warp is held at its starting value. A bend keeps this point fixed (default 0; metres, -1000000..1000000) */
  from?: number | Node<'Number'>;
  /** Where along the axis the warp ends, in metres; it must differ from `from` (less than `from` works the other way round); after it the warp is held at its end value (default 1; metres, -1000000..1000000) */
  to?: number | Node<'Number'>;
  /** Total rotation in degrees (default 0; -3600..3600) */
  angle?: number | Node<'Number'>;
}

/** Hard union of two or more spatial fields. */
export interface FieldUnionV1Params {
}

/** A real tree: a branching trunk with species bark, smooth curved limbs and a lobed root flare, and leaf cards with cut-out leaf shapes that let the sun through, grown from a named species (oak, pine, birch, willow, shrub, palm) at the height you ask, the same tree for the same seed. Its base is at y = 0, so it plants straight into scatter.place. */
export interface FloraTreeV1Params {
  /** The species: oak is broad, pine is a tall cone with drooping whorls, birch slender, willow long and drooping, shrub low with many stems, palm a curved trunk with fronds (default "oak") */
  kind?: 'oak' | 'pine' | 'birch' | 'willow' | 'shrub' | 'palm';
  /** Height in metres (default 8; metres, 0.3..80) */
  height?: number | Node<'Number'>;
  /** Picks a different tree of the same species (default 0; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Sides of the trunk tube at its thickest: 6 is plenty at a distance, 12 for a trunk you can touch (default 12; whole number 3..16) */
  sides?: number | Node<'Number'>;
  /** Bark colour [r, g, b] (sRGB). Left at the default, each species wears its own bark (furrowed brown oak, white birch with dark marks, plated red pine); any other colour recolours the bark pattern (default [0.3, 0.2, 0.13]; 0..1 each) */
  bark?: Vec3;
  /** The bark pattern: auto picks the species' own. furrowed: deep vertical ridges. white: pale with dark horizontal marks (birch). plated: scaly plates (pine). rings: horizontal rings (palm). smooth: even bark (young stems) (default "auto") */
  bark_style?: 'auto' | 'furrowed' | 'white' | 'plated' | 'rings' | 'smooth';
  /** How much the trunk swells into lobed roots where it meets the ground (0 a plain cylinder, 1 about double the radius at the soil) (default 1; a ratio, 0..3) */
  flare?: number | Node<'Number'>;
  /** Extra rings between branch nodes on a smooth curve, so limbs bend instead of kinking (0 keeps straight segments; each ring adds triangles) (default 2; whole number 0..6) */
  smooth?: number | Node<'Number'>;
  /** Leaf colour [r, g, b] (sRGB); leaves vary a little around it, and a few lean yellow (default [0.2, 0.42, 0.12]; 0..1 each) */
  leaves?: Vec3;
  /** The time of year, when `leaves` is left at its default green: summer is full green, spring a pale fresh green, autumn orange and red with yellow leaves among them, winter bare branches. Any other `leaves` colour wins over the season (winter is always bare) (default "summer") */
  season?: 'summer' | 'spring' | 'autumn' | 'winter';
  /** clumps: several cards fanned around each leaf point (full, soft crowns). crossed: two cards crossed (reads from any side). quads: one card (cheapest) (default "clumps") */
  leaf_style?: 'clumps' | 'crossed' | 'quads';
  /** Multiplier of the leaf card size: larger covers the crown more fully (default 1; a ratio, 0.3..3) */
  leaf_size?: number | Node<'Number'>;
  /** How much the leaves move in the scene's wind (0 still) (default 0.5; a ratio, 0..1) */
  sway?: number | Node<'Number'>;
  /** Level of detail: 0 is the full tree, each level up has about half the triangles (default 0; whole number 0..4) */
  lod?: number | Node<'Number'>;
}

/** A liquid that runs while the scene runs: water pouring into a basin, a wave in a tank, honey sliding down a ramp. `min` and `max` are the corners of the block of liquid at the start; `spacing` is how far apart its particles are (0.05 m with a 0.4 x 0.5 x 0.4 m block is 640 particles; keep it between about 1,000 and 8,000 particles for 60 frames a second). */
export interface FluidSimV1Params {
  /** Lower corner of the block of liquid at the start, in metres (default [0, 0, 0]; metres, -1000..1000 each) */
  min?: Vec3;
  /** Upper corner of the block of liquid at the start (default [0.4, 0.5, 0.4]; metres, -1000..1000 each) */
  max?: Vec3;
  /** Distance between particles at rest, in metres; smaller is finer and slower (default 0.05; metres, 0.01..0.5) */
  spacing?: number | Node<'Number'>;
  /** Lower corner of the container the liquid stays in (the floor is its y) (default [-50, 0, -50]; metres, -100000..100000 each) */
  bounds_min?: Vec3;
  /** Upper corner of the container (default [50, 100, 50]; metres, -100000..100000 each) */
  bounds_max?: Vec3;
  /** Downward acceleration in metres per second squared (default 9.81; 0..50) */
  gravity?: number | Node<'Number'>;
  /** How thick it is: velocity blending with neighbours (water 0.02, honey 0.6) (default 0.02; a ratio, 0..1) */
  viscosity?: number | Node<'Number'>;
  /** How much it grips the container and bones (default 0.1; a ratio, 0..1) */
  friction?: number | Node<'Number'>;
  /** Solver steps per frame: more is steadier but slower (default 3; whole number 1..8) */
  substeps?: number | Node<'Number'>;
  /** Density passes per step: more keeps the volume better but is slower (default 4; whole number 1..8) */
  iterations?: number | Node<'Number'>;
  /** Output names of bone parts (commas or semicolons, at most 16) that push the liquid as capsules (default ""; text up to 512 bytes) */
  collide_bones?: string;
  /** Radius of each collider capsule around its bone, in metres (default 0.07; metres, 0..2) */
  collider_radius?: number | Node<'Number'>;
  /** Grid size the surface is meshed on; 0 picks 0.7 times the spacing. Smaller is smoother and costs more (default 0; metres, 0..0.5) */
  cell?: number | Node<'Number'>;
  /** Where the surface sits in the particle density: lower swells the liquid, higher shrinks it (default 0.4; a ratio, 0.2..0.8) */
  level?: number | Node<'Number'>;
  /** How the liquid is solved. pbf: position-based, light and cheap (up to 20,000 particles), a bit like a springy gel. apic: a pressure-projected grid and particle solver that keeps its volume and sloshes, splashes and settles like real water (up to 60,000 particles; the container's `bounds` should be tight, because the solver's grid covers them; with huge bounds it covers a window round the liquid); `iterations` scales how hard it solves the pressure and `substeps` caps how often a frame is split (default "pbf") */
  method?: 'pbf' | 'apic';
  /** Most triangles the surface may have while it runs; the part is padded to this so it can grow (default 4000; whole number 200..12000) */
  max_triangles?: number | Node<'Number'>;
}

/** Declares one unique named rigid socket; rotations are X, then Y, then Z. */
export interface FrameAddV1Params {
  /** Unique socket name (required; text up to 64 bytes) */
  name: string;
  /** Position in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  position?: Vec3;
  /** Rotation in radians, X then Y then Z (default [0, 0, 0]; an angle, -1000000..1000000 each) */
  rotation?: Vec3;
}

/** Makes an emitter's particles hit the ground (a flat plane at `ground`): they bounce, stick or disappear. Sparks bounce, rain splashes (kill, with fx.sub on collide), embers stick. */
export interface FxCollideV1Params {
  /** Name of the emitter to change; empty means the last one made (default ""; text up to 32 bytes) */
  emitter?: string;
  /** bounce: rebound with `restitution` and slide with `friction`. stick: stop where they land. kill: disappear on impact (default "bounce") */
  reaction?: 'bounce' | 'stick' | 'kill';
  /** The particle's size for collisions, metres (default 0.02; metres, 0..10) */
  radius?: number | Node<'Number'>;
  /** bounce: how much speed a rebound keeps (0 dead, 1 perfect) (default 0.4; a ratio, 0..1) */
  restitution?: number | Node<'Number'>;
  /** bounce: how much a slide is slowed (0 ice, 1 rough) (default 0.3; 0..10) */
  friction?: number | Node<'Number'>;
  /** World height of the ground plane (applies to the whole effect) (default 0; metres, -1000..1000) */
  ground?: number | Node<'Number'>;
}

/** Makes this shape EMIT particles (fire, smoke, sparks, dust, rain, magic): it gives birth to small sprites that fly, fade and die. The effect rides on the shape, so it follows it when the shape moves (anim.track, anim.parent, carried in a hand). */
export interface FxEmitterV1Params {
  /** A name for this emitter (letters, digits, _ or -), used by fx.force, fx.collide, fx.trail and fx.sub. Empty names it e0, e1,  (default ""; text up to 32 bytes) */
  name?: string;
  /** Particles born per second, steadily. 0 for none (use burst) (default 20; 0..5000) */
  rate?: number | Node<'Number'>;
  /** Particles born at once at `burst_at` seconds: an explosion, a splash, a puff. 0 for none (default 0; whole number 0..5000) */
  burst?: number | Node<'Number'>;
  /** When the burst happens, seconds after the emitter starts (default 0; 0..60) */
  burst_at?: number | Node<'Number'>;
  /** Seconds before the emitter starts (default 0; 0..600) */
  delay?: number | Node<'Number'>;
  /** Seconds the emitter stays on; 0 is forever (default 0; 0..600) */
  duration?: number | Node<'Number'>;
  /** Start over every `duration` seconds (needs a duration above 0): a repeating burst (default false) */
  loop?: boolean;
  /** How long a particle lives, seconds: [shortest, longest] (default [1, 2]; 0.05..60 each) */
  lifetime?: Vec2;
  /** Birth speed in metres per second: [slowest, fastest] (default [1, 2]; 0..100 each) */
  speed?: Vec2;
  /** The direction particles leave in, in the shape's own frame (it turns with the shape) (default [0, 1, 0]; -1000..1000 each) */
  direction?: Vec3;
  /** Spread around the direction, in degrees (about 0 to 85): 0 is a jet, 30 a plume, 85 a fan (default 0; an angle, 0..1.5) */
  cone?: number | Node<'Number'>;
  /** Where particles are born around the shape: point, inside a sphere, inside a box, on a flat disc, or along a vertical line (default "point") */
  shape?: 'point' | 'sphere' | 'box' | 'disc' | 'line';
  /** Size of the birth shape in metres: sphere and disc use the first number as radius, box uses all three as half-sizes, line is twice the second long (default [0.1, 0.1, 0.1]; 0..1000 each) */
  extent?: Vec3;
  /** Most particles alive at once from this emitter (default 500; whole number 1..20000) */
  max_particles?: number | Node<'Number'>;
  /** Sprite size in metres: [at birth, at death]. A ribbon's width (default [0.2, 0.2]; metres, 0..100 each) */
  size?: Vec2;
  /** Linear colour at birth (values above 1 glow when `glow` is set) (default [1, 1, 1]; 0..16 each) */
  colour?: Vec3;
  /** Linear colour at death: fire goes from yellow-white to deep red (default [1, 1, 1]; 0..16 each) */
  colour_end?: Vec3;
  /** Opacity: [at birth, at death]. [1, 0] fades out (default [1, 0]; a ratio, 0..1 each) */
  alpha?: Vec2;
  /** How much the particle shines on its own (0 none, 4 fire, 8 bright sparks) (default 0; 0..16) */
  glow?: number | Node<'Number'>;
  /** alpha: ordinary transparency (smoke, dust, rain). additive: adds light (fire, sparks, magic) (default "alpha") */
  blend?: 'alpha' | 'additive';
  /** sprite: a soft round dot per particle. ribbon: a streak along the path each particle took (needs fx.trail). cube: a small LIT box that tumbles (debris, shards, chips; size is its edge, colour its paint, blend is ignored, glow still brightens it) (default "sprite") */
  render?: 'sprite' | 'ribbon' | 'cube';
  /** cube only: how fast each box tumbles, radians per second (default 3; 0..50) */
  spin?: number | Node<'Number'>;
}

/** Adds a force to an emitter's particles: gravity, wind, drag, a vortex, an attractor, or turbulence (swirling curl noise with no sources or sinks: the way to make smoke and fire look alive). Several forces add up. */
export interface FxForceV1Params {
  /** Name of the emitter to change; empty means the last one made (default ""; text up to 32 bytes) */
  emitter?: string;
  /** gravity: constant acceleration (`vector`, default [0, -9.8, 0]). wind: drags particles towards the speed `vector` at `strength` per second. drag: slows them (`strength`). vortex: swirls them round the axis `vector` through `centre` (`strength`, `radius`). attractor: pulls them to `centre` (`strength`, `radius`). turbulence: swirling noise (`strength`, `scale`, `speed`) (default "gravity") */
  kind?: 'gravity' | 'wind' | 'drag' | 'vortex' | 'attractor' | 'turbulence';
  /** gravity: the acceleration (0 means [0, -9.8, 0]). wind: the wind velocity. vortex: the axis (0 means up) (default [0, 0, 0]; -1000..1000 each) */
  vector?: Vec3;
  /** vortex and attractor: the world position (default [0, 0, 0]; -100000..100000 each) */
  centre?: Vec3;
  /** How strong: drag and wind per second, vortex, attractor and turbulence in metres per second squared (default 1; 0..1000) */
  strength?: number | Node<'Number'>;
  /** vortex and attractor: how far the force reaches; it fades towards the edge (default 5; metres, 0.01..1000) */
  radius?: number | Node<'Number'>;
  /** turbulence: features per metre (0.5 big slow eddies, 3 fine ones) (default 1; 0.01..100) */
  scale?: number | Node<'Number'>;
  /** turbulence: how fast the pattern changes (default 0.3; 0..100) */
  speed?: number | Node<'Number'>;
}

/** Makes particles of one emitter spawn particles of another emitter of the same effect when they are born, die or hit the ground: smoke from dying embers, a splash from each raindrop (parent kill-on-collide), sparks from a fuse. The child emitter usually has rate=0. */
export interface FxSubV1Params {
  /** Name of the emitter whose particles trigger it (default ""; text up to 32 bytes) */
  parent?: string;
  /** Name of the emitter that spawns particles where the trigger happens (default ""; text up to 32 bytes) */
  child?: string;
  /** When: death (dies of old age), birth, or collide (hits the ground) (default "death") */
  on?: 'death' | 'birth' | 'collide';
  /** Particles spawned each time (default 3; whole number 1..1000) */
  count?: number | Node<'Number'>;
}

/** Makes an emitter's particles remember the path they took (the last `points` positions). Needed for render="ribbon": streaks, comet tails, sword swooshes, tracer bullets. */
export interface FxTrailV1Params {
  /** Name of the emitter to change; empty means the last one made (default ""; text up to 32 bytes) */
  emitter?: string;
  /** How many positions to keep (the newest is the particle itself) (default 12; whole number 2..64) */
  points?: number | Node<'Number'>;
  /** A point is recorded after the particle moves this far, metres: small is smooth, large is angular (default 0.05; metres, 0.005..10) */
  min_step?: number | Node<'Number'>;
}

/** A button-triggered action with a cost and a cooldown, for the owner's ability bar. Pressing its input (game.input@1 with key="press") runs the do actions with a = the owner and b = its target (game.targeting@1), pays cost from the owner's prop cost_prop, and starts the cooldown; the owner's props cd_NAME (seconds left) and cdmax_NAME drive a slot on screen (game.ui@1 type="slot"). */
export interface GameAbilityV1Params {
  /** Unique name (required; text up to 4096 bytes) */
  name: string;
  /** Who has it: a selector such as tag:hero (required; text up to 4096 bytes) */
  owner: string;
  /** The key input that triggers it (required; text up to 4096 bytes) */
  input: string;
  /** Seconds before it can be used again (default 1; 0..3600) */
  cooldown?: number | Node<'Number'>;
  /** Taken from the owner's prop cost_prop when used (default 0; 0..1000000) */
  cost?: number | Node<'Number'>;
  /** The owner's prop that pays the cost (default "focus"; text up to 4096 bytes) */
  cost_prop?: string;
  /** The farthest target it works on in metres (0 for any) (default 0; 0..1000) */
  range?: number | Node<'Number'>;
  /** Whether it needs a target in range (default false) */
  needs_target?: boolean;
  /** Semicolon-separated actions, as in game.rule@1 (default ""; text up to 4096 bytes) */
  do?: string;
}

/** Make a kind patrol, notice and chase the hero. Example: `watch = game.agent@1(kind="creature", patrol=[[0, 0], [6, 0]], speed=1.5)`. */
export interface GameAgentV1Params {
  /** Declared kind to control (required; text up to 4096 bytes) */
  kind: string;
  /** Ordered loop of horizontal x,z points in metres (required; 2 to 2048 points) */
  patrol: Vec2[];
  /** Positive patrol speed in metres per second (required; -1000000000..1000000000) */
  speed: number | Node<'Number'>;
  /** Positive chase speed; 0 selects 1.6 times speed (default 0; -1000000000..1000000000) */
  chase_speed?: number | Node<'Number'>;
  /** Positive notice distance in metres (default 5; -1000000000..1000000000) */
  sight?: number | Node<'Number'>;
  /** Positive chase loss distance; 0 selects 1.5 times sight (default 0; -1000000000..1000000000) */
  lose?: number | Node<'Number'>;
}

/** Make a kind move and fight on its own: companions and enemies. With a foe (an entity matching foes that has health) within engage metres it closes to its weapon's reach (keep metres for a ranged weapon) and uses the weapon (game.weapon@1, aim at the foes); with none it follows its leader to within follow metres. */
export interface GameBrainV1Params {
  /** The kind it drives (required; text up to 4096 bytes) */
  kind: string;
  /** Who it fights: a selector such as tag:enemy (required; text up to 4096 bytes) */
  foes: string;
  /** Whom it follows when it has nothing to fight, such as tag:hero (empty: it stays put) (default ""; text up to 4096 bytes) */
  leader?: string;
  /** Metres per second (default 3; 0.1..100) */
  speed?: number | Node<'Number'>;
  /** How close it stays to its leader (default 3; 0..100) */
  follow?: number | Node<'Number'>;
  /** How far away it notices a foe (default 15; 0..1000) */
  engage?: number | Node<'Number'>;
  /** The weapon it fights with (default ""; text up to 4096 bytes) */
  weapon?: string;
  /** How far from a foe a ranged fighter stands (-1 uses 6) (default -1; -1..1000) */
  keep?: number | Node<'Number'>;
}

/** Say how the view follows the player in play mode. The first camera starts active; a rule switches with the action `effect:camera:NAME` (cut) or `effect:camera:NAME:SECONDS` (blend). */
export interface GameCameraV1Params {
  /** The camera's name, used by the switch action (required; text up to 4096 bytes) */
  name: string;
  /** How the view follows the target (default "third") */
  mode?: 'first' | 'third' | 'top_down' | 'side' | 'iso' | 'fixed';
  /** Selector of the entity it follows, such as tag:hero or kind:player (default "tag:hero"; text up to 4096 bytes) */
  target?: string;
  /** Arm length (third), height above the target (top_down), or distance back from the target's plane (side, iso), in metres; 0 uses the mode's default (default 0; 0..1000) */
  distance?: number | Node<'Number'>;
  /** Height of the pivot above the target's feet (the eye height in first person); -1 uses the mode's default (default -1; -1..100) */
  height?: number | Node<'Number'>;
  /** Third person: sideways offset of the camera to the right of the target, in metres (negative is left) (default 0; -10..10) */
  shoulder?: number | Node<'Number'>;
  /** Fixed-angle modes: the pitch in degrees (negative looks down); 999 uses the mode's default (default 999; -1000..1000) */
  pitch?: number | Node<'Number'>;
  /** Fixed-angle modes: the yaw in degrees (0 looks along +z, 90 along +x); 999 uses the mode's default (default 999; -1000..1000) */
  yaw?: number | Node<'Number'>;
  /** Vertical field of view in degrees (default 60; 10..150) */
  fov?: number | Node<'Number'>;
  /** Seconds in which the view closes most of a gap to the target; 0 follows rigidly, -1 uses the mode's default (default -1; -1..60) */
  smoothing?: number | Node<'Number'>;
  /** Third person: whether the arm shortens against walls instead of passing through them (default true) */
  collide?: boolean;
  /** Half-size in metres of the area around the view in which the target can move without moving the view (a side-scroller's loose follow) (default 0; 0..100) */
  deadzone?: number | Node<'Number'>;
  /** Fixed camera only: where it stands, as x,y,z in metres (default ""; text up to 4096 bytes) */
  at?: string;
  /** Keeps the view's pivot inside the level: x_min,x_max,z_min,z_max in metres (default ""; text up to 4096 bytes) */
  bounds?: string;
  /** Third person: lowest look angle in degrees (default -60; -89..89) */
  min_pitch?: number | Node<'Number'>;
  /** Third person: highest look angle in degrees (default 75; -89..89) */
  max_pitch?: number | Node<'Number'>;
}

/** Join game fragments in link order. Example: `all = game.collect@1(score, hero, spawn, rule, goal, name="coins")`. */
export interface GameCollectV1Params {
  /** Definition name; the last collect name wins (default "game"; text up to 4096 bytes) */
  name?: string;
}

/** Move a kind from the player's input like a character: it speeds up and slows down, turns, falls under gravity, jumps (variable height, extra jumps, a short grace after leaving a ledge, a jump pressed just before landing still counts), lands on the ground and on the tops of solid boxes, and can move relative to the camera. The entity's y is its feet. */
export interface GameControllerV1Params {
  /** Declared kind to move; every entity of it is controlled (required; text up to 4096 bytes) */
  kind: string;
  /** walk: 3D ground movement. platformer: movement along one axis in a plane (lock defaults to z). top_down: no gravity and no jump (default "walk") */
  mode?: 'walk' | 'platformer' | 'top_down';
  /** Fastest horizontal speed in metres per second; 0 uses the kind's speed (4 if it has none) (default 0; 0..1000) */
  speed?: number | Node<'Number'>;
  /** How fast speed rises toward the input, metres per second squared; 0 means ten times the speed (full speed in a tenth of a second) (default 0; 0..100000) */
  accel?: number | Node<'Number'>;
  /** How fast speed falls to rest when the input stops; 0 means the same as accel. Small values slide like ice (default 0; 0..100000) */
  decel?: number | Node<'Number'>;
  /** Share of accel and decel that works in the air: 1 steers as on the ground, 0 keeps the speed it left the ground with (default 0.6; 0..1) */
  air?: number | Node<'Number'>;
  /** Upward speed of a jump in metres per second (0 means no jumping); the height is jump squared over twice gravity: 8 with gravity 20 is 1.6 m (default 0; 0..1000) */
  jump?: number | Node<'Number'>;
  /** Downward acceleration in metres per second squared (0 floats) (default 20; 0..1000) */
  gravity?: number | Node<'Number'>;
  /** Seconds after walking off a ledge during which a jump still works (default 0.1; 0..2) */
  coyote?: number | Node<'Number'>;
  /** Seconds a jump pressed too early is remembered, to go off on landing (default 0.1; 0..2) */
  buffer?: number | Node<'Number'>;
  /** Jumps allowed in the air after leaving the ground (1 is a double jump) (default 0; whole number 0..5) */
  extra_jumps?: number | Node<'Number'>;
  /** Share of the upward speed kept when the jump button is released early: 1 always jumps full height, 0.3 makes taps short hops (default 0.5; 0..1) */
  cut?: number | Node<'Number'>;
  /** Turn rate toward the direction of motion in degrees per second; 0 faces the motion at once (default 0; 0..100000) */
  turn?: number | Node<'Number'>;
  /** Highest ledge in metres it walks up onto without jumping; taller boxes block the way (default 0.3; 0..10) */
  step?: number | Node<'Number'>;
  /** A height in metres below which the hero is put back on the last safe ground it stood on and the event `fell` is raised (a rule can take a life or restart: game.rule@1(on="fell", do="add:lives:-1")). -10000 means off; set it below the lowest platform, with game.ground@1(y=...) lower still (default -10000; -10000..10000) */
  fall_limit?: number | Node<'Number'>;
  /** world: the movement axes are world x and z. camera: forward is where the camera looks (the host supplies `camera.yaw`), so a third-person or first-person hero walks where the player looks (default "world") */
  relative?: 'world' | 'camera';
  /** A horizontal axis it may not move along (a side-scroller locks z); none uses the mode's default (default "none") */
  lock?: 'none' | 'x' | 'z';
  /** Name of the input whose axes (`NAME.x`, `NAME.z`) move it (default "move"; text up to 4096 bytes) */
  move?: string;
  /** Name of the press input that jumps (default "jump"; text up to 4096 bytes) */
  jump_input?: string;
}

/** Declare a terminal condition. Example: `won = game.goal@1(outcome="win", when="score>=8")`. */
export interface GameGoalV1Params {
  /** Whether this condition wins or loses (required) */
  outcome: 'win' | 'lose';
  /** Condition expression (required; text up to 4096 bytes) */
  when: string;
}

/** Set the ground plane height. Example: `ground = game.ground@1(y=0.25)`. */
export interface GameGroundV1Params {
  /** Ground height in metres (default 0; -1000000000..1000000000) */
  y?: number | Node<'Number'>;
}

/** Give a kind hit points. Its entities get the props health and max_health (bind a bar to them with game.ui@1: value="hero.health", max_var="hero.max_health"). */
export interface GameHealthV1Params {
  /** The kind that has health (required; text up to 4096 bytes) */
  kind: string;
  /** Starting and maximum hit points (required; 1..1000000) */
  hp: number | Node<'Number'>;
  /** Seconds untouchable after being hurt (default 0; 0..60) */
  iframes?: number | Node<'Number'>;
  /** Metres per second it is pushed away from the source of a hurt (default 0; 0..100) */
  knock?: number | Node<'Number'>;
  /** Height of the shape shots hit, in metres (-1 uses twice the kind's size) (default -1; -1..100) */
  height?: number | Node<'Number'>;
}

/** Show text bound to game variables as a procedural pixel HUD. Example: `panel = game.hud@1(text="Score {score}", anchor="gaze", size=0.005)`. */
export interface GameHudV1Params {
  /** HUD text; `{name}` inserts a game variable (required; text up to 4096 bytes) */
  text: string;
  /** HUD position anchor (default "gaze") */
  anchor?: 'gaze' | 'left' | 'right' | 'top' | 'bottom';
  /** Pixel edge length in metres (a letter is 7 pixels tall: 0.005 gives 3.5 cm letters, a 20-character line about 0.6 m wide, readable at arm's length) (default 0.005; metres, 0..1) */
  size?: number | Node<'Number'>;
}

/** Declare a player axis or press input. Example: `move = game.input@1(name="move", axis="x,z", target="tag:hero")`. */
export interface GameInputV1Params {
  /** Input name (required; text up to 4096 bytes) */
  name: string;
  /** Comma-separated axes, such as x,z (default ""; text up to 4096 bytes) */
  axis?: string;
  /** Button kind, currently press (default "") */
  key?: '' | 'press';
  /** Target selector, such as tag:hero (required; text up to 4096 bytes) */
  target: string;
  /** Move matching entities to the viewer position (viewer) or to a tracked controller (left_hand, right_hand) each step. A hand that is not tracked leaves its entity where it was (default "") */
  follow?: '' | 'viewer' | 'left_hand' | 'right_hand';
  /** Keyboard keys that drive it, as KeyboardEvent codes separated by commas (Space, KeyC, ArrowUp, Mouse0). An axis needs a positive and a negative key per axis, in order: axis="x,z" keys="KeyD,KeyA,KeyW,KeyS". Empty uses the page's defaults. The player can rebind them (default ""; text up to 4096 bytes) */
  keys?: string;
  /** Gamepad controls: button<N> for a press (button0 is A), stick:left or stick:right for a two-axis input, axis<N>+ and axis<N>- for one axis. Empty uses the page's defaults (default ""; text up to 4096 bytes) */
  pad?: string;
  /** What a prompt and the controls screen call this action (default: the name) (default ""; text up to 4096 bytes) */
  label?: string;
}

/** Declare an entity kind. Examples: `hero = game.kind@1(name="player", size=0.5, speed=4, tags=["hero"])`; `ball = game.kind@1(name="ball", size=0.03, roll=true)` rolls on a table or field; `tool = game.kind@1(name="tool", swing=true, reach=0.9, loft=20, power=1.2)` swings into it; `pocket = game.kind@1(name="pocket", size=0.07, pocket=true)` captures it. */
export interface GameKindV1Params {
  /** Kind name (required; text up to 4096 bytes) */
  name: string;
  /** Collision radius in metres (default 0.5; metres, 0..1000000) */
  size?: number | Node<'Number'>;
  /** Movement speed in metres per second (default 0; metres, 0..1000000) */
  speed?: number | Node<'Number'>;
  /** Tags used by selectors (default []; up to 64 texts of 64 bytes) */
  tags?: string[];
  /** Scene output that draws this kind; empty makes it invisible (default ""; text up to 4096 bytes) */
  part?: string;
  /** Full width, height and depth in metres of a box centred on the entity that the hero cannot walk into (a door, a wall, a gate); [0, 0, 0] makes the kind not an obstacle. The box disappears when the entity is destroyed (default [0, 0, 0]; metres, 0..1000 each) */
  solid?: Vec3;
  /** True makes the kind grabbable: a squeezed grip within reach (the hand's size plus this kind's size) picks up the nearest one and carries it until the grip opens (events grab and release); carried things still touch sockets, doors and each other. A hand is an entity too: give it a kind, an input with follow="left_hand" or "right_hand", and a part to draw it (default false) */
  grab?: boolean;
  /** Apply gravity and ground bounces (default false) */
  fall?: boolean;
  /** Fraction of vertical speed retained on bounce (default 0.3; 0..0.95) */
  bounce?: number | Node<'Number'>;
  /** Rolling thing follows the ground on a table or field (default false) */
  roll?: boolean;
  /** A hand-held tool swings its swept head into rolling things (default false) */
  swing?: boolean;
  /** Tool head reach in metres along the hand's local -Z axis (default 0; metres, 0..1000000000) */
  reach?: number | Node<'Number'>;
  /** Upward launch angle in degrees (default 0; 0..80) */
  loft?: number | Node<'Number'>;
  /** Swing power multiplier, greater than zero and at most 5 (default 1; 0..5) */
  power?: number | Node<'Number'>;
  /** Kind is a pocket that captures slow grounded rolling things (default false) */
  pocket?: boolean;
}

/** Declare a level: spawns with level=NAME belong to it. The first one declared loads at the start; the action level:NAME replaces the loaded level's entities with the named level's and puts every hero (tag:hero) at its start. */
export interface GameLevelV1Params {
  /** Unique name (required; text up to 4096 bytes) */
  name: string;
  /** Where heroes are put when the level loads, in metres (used when place_heroes is on) (default [0, 0, 0]; metres, -1000000..1000000 each) */
  start?: Vec3;
  /** Whether loading the level puts every hero at start (default false) */
  place_heroes?: boolean;
}

/** Declare the level parts to build navigation from. Example: `nav = game.nav@1(parts=["floor", "stairs"], cell=0.2)`. */
export interface GameNavV1Params {
  /** Walkable floor, ramp and stair part names (required; up to 2048 texts of 4096 bytes) */
  parts: string[];
  /** Positive grid cell size in metres (default 0.25; -1000000000..1000000000) */
  cell?: number | Node<'Number'>;
  /** Nonnegative agent radius in metres (default 0.3; -1000000000..1000000000) */
  radius?: number | Node<'Number'>;
  /** Positive agent height in metres (default 1.8; -1000000000..1000000000) */
  height?: number | Node<'Number'>;
  /** Nonnegative maximum step height in metres (default 0.4; -1000000000..1000000000) */
  step?: number | Node<'Number'>;
  /** Maximum slope in degrees, between 0 and 90 (default 45; -1000000000..1000000000) */
  slope?: number | Node<'Number'>;
}

/** Make a kind a shot: its entities fly, hurt the first victim (an entity with health) they meet, are used up, and stop at walls (kinds with solid=...) or when their life ends (raising impact). pierce lets it pass through that many extra victims; splash hurts neighbours within that radius (less with distance); homing turns it toward the nearest victim by that many degrees a second; gravity bends the flight. */
export interface GameProjectileV1Params {
  /** The kind of the shot (declare it with game.kind@1; give it a part to draw it) (required; text up to 4096 bytes) */
  kind: string;
  /** Metres per second (default 20; 0.1..1000) */
  speed?: number | Node<'Number'>;
  /** Seconds before it expires (default 2; 0.05..600) */
  life?: number | Node<'Number'>;
  /** Damage on a direct hit (default 10; 0..1000000) */
  damage?: number | Node<'Number'>;
  /** Downward acceleration in metres per second squared (default 0; -100..100) */
  gravity?: number | Node<'Number'>;
  /** Extra victims it passes through (default 0; whole number 0..100) */
  pierce?: number | Node<'Number'>;
  /** Blast radius in metres on impact (0 for none) (default 0; 0..1000) */
  splash?: number | Node<'Number'>;
  /** Degrees a second it may turn toward the nearest victim (0 for none) (default 0; 0..3600) */
  homing?: number | Node<'Number'>;
}

/** Declare an event rule and semicolon-separated actions. Example: `pick = game.rule@1(on="touch", a="tag:hero", b="tag:coin", do="add:score:1;destroy:b")`. */
export interface GameRuleV1Params {
  /** Event name, such as touch, tick or var:score (required; text up to 4096 bytes) */
  on: string;
  /** Optional first entity selector (default ""; text up to 4096 bytes) */
  a?: string;
  /** Optional second entity selector (default ""; text up to 4096 bytes) */
  b?: string;
  /** Optional condition expression (default ""; text up to 4096 bytes) */
  when?: string;
  /** Optional tick interval in seconds (default 0; 0..1000000) */
  every?: number | Node<'Number'>;
  /** Limit the rule to one flow state (declared with game.state@1), such as playing or paused. Every rule on an event sees the state the event began in, so one pause key can have a rule in playing and another in paused (default ""; text up to 4096 bytes) */
  state?: string;
  /** Semicolon-separated action list. Besides add, set, destroy, spawn, timer, emit, effect, win and lose, flow actions: go:STATE (enter a state), restart or restart:STATE (rebuild the game from its start, then enter the state), level:NAME (load a level), checkpoint and respawn (store the hero's place and return to it; an optional :SELECTOR picks other entities) (required; text up to 4096 bytes) */
  do: string;
}

/** A JavaScript behaviour that runs inside the game's fixed step, for anything the typed game nodes do not say. The source runs once when the game starts and registers handlers: `on(event, fn)`, `on(event, {a: "tag:hero", b: "tag:coin"}, fn)`, `every(seconds, fn)`, `after(seconds, fn)`. */
export interface GameScriptV1Params {
  /** Unique lowercase name (letters, digits, underscores); empty uses the node label or id (default ""; text up to 4096 bytes) */
  name?: string;
  /** JavaScript source, up to 64 KB (not needed with lib) (default ""; text up to 65536 bytes) */
  source?: string;
  /** Load a standard library instead of source; declare it before the scripts that use it. rpg: attributes with modifiers, tags, effects (duration, period, stacking), abilities (cost, cooldown, requirements) and a damage pipeline (resistances, armour, block, poise and stagger, death) in global `rpg`. combat (needs rpg first): timed melee attacks, combos, hitstop, dodge, block and parry. inventory: deterministic item definitions and capacity-limited stacks in global `inventory`; add, remove, count and query items. RPG, combat and inventory state is saved in the game snapshot; see each library header for calls (default "") */
  lib?: '' | 'rpg' | 'combat' | 'inventory';
}

/** Place entities of a declared kind, at a point or one per placement of a set (scatter.place, inst.array ...), standing exactly where each placement is, so pickups sit on hilly ground. heading sets the initial facing in degrees. */
export interface GameSpawnV1Params {
  /** Declared kind to spawn (required; text up to 4096 bytes) */
  kind: string;
  /** Spawn position in metres (ignored when a set is linked) (default [0, 0, 0]; metres, -1000000..1000000 each) */
  at?: Vec3;
  /** Number of entities (default 1; whole number 1..2048) */
  count?: number | Node<'Number'>;
  /** Scatter square width in metres (default 0; metres, 0..1000000) */
  spread?: number | Node<'Number'>;
  /** Initial facing direction in degrees (default 0; -360..360) */
  heading?: number | Node<'Number'>;
  /** Belongs to this level (declared with game.level@1): it appears when the level loads and goes when another loads. Empty spawns at the start and stays (default ""; text up to 4096 bytes) */
  level?: string;
}

/** Declare a state of the game's flow, such as title, playing, paused, game_over or win. The first one declared is where the game starts. */
export interface GameStateV1Params {
  /** Unique name; rules use it in go:NAME, state=NAME and on=state:NAME (required; text up to 4096 bytes) */
  name: string;
  /** Whether the simulation stands still in this state (default false) */
  frozen?: boolean;
}

/** Declare a surface for a rolling ball on a table. Example: `felt = game.surface@1(name="felt", parts=["table"], decel=0.5, bounce=0.5)`; parts are the output names of authored parts. */
export interface GameSurfaceV1Params {
  /** Surface name (required; text up to 4096 bytes) */
  name: string;
  /** Non-empty output names of authored parts using this surface (required; up to 256 texts of 128 bytes) */
  parts: string[];
  /** Positive roll deceleration (required; 0..1000000000) */
  decel: number | Node<'Number'>;
  /** Vertical bounce fraction (default 0.5; 0..1) */
  bounce?: number | Node<'Number'>;
}

/** Choose the leader's target. The leader's prop target is the id of the foe it faces (the nearest inside the cone, else the nearest of all), kept until that foe dies or leaves range; the foe's prop targeted is 1, so the UI can show it (text="{targeted.health}", a bar with value="targeted.health"). */
export interface GameTargetingV1Params {
  /** Whose target it is: a selector such as tag:hero (required; text up to 4096 bytes) */
  leader: string;
  /** What can be targeted: a selector such as tag:enemy (required; text up to 4096 bytes) */
  foes: string;
  /** How far away a foe can be targeted (default 25; 0..1000) */
  range?: number | Node<'Number'>;
  /** Foes within this many degrees of where the leader faces (or the camera looks) are preferred (default 80; 0..360) */
  cone?: number | Node<'Number'>;
}

/** Declare ground sampling for rolling things on a field. Example: `terrain = game.terrain@1(parts=["field", "path"], cell=0.25)`; parts are the output names of authored parts. */
export interface GameTerrainV1Params {
  /** Output names of authored parts covered by this terrain (required; up to 256 texts of 128 bytes) */
  parts: string[];
  /** Terrain sampling cell size in metres, greater than zero (default 0.25; metres, 0..1000000) */
  cell?: number | Node<'Number'>;
}

/** One element of the game's screen UI (title, pause menu, HUD, prompts, minimap, dialogue choices). Drawn over the 3D view in fractions of the screen height (x runs from 0 to the aspect ratio) with a procedural font, so it needs no assets. */
export interface GameUiV1Params {
  /** Unique id; a button raises ui:NAME; parent= refers to it (required; text up to 4096 bytes) */
  name: string;
  /** What the element is. slot: an ability square with an icon, a key label and a cooldown overlay (value="hero.cd_smash", max="hero.cdmax_smash" from game.ability@1). portrait: a round badge with an initial (text="V", fill=colour). floaters: draws the damage numbers, +healing and Dodge! that float up from where things were hurt (one per game, no size) (required) */
  type: 'text' | 'bar' | 'panel' | 'button' | 'stack' | 'grid' | 'minimap' | 'slot' | 'portrait' | 'floaters';
  /** Show the element only while this expression is true, for example targeted.health>0 (names are variables or KIND.prop, and targeted.PROP is the leader's current target): a target frame that appears only while something is targeted (default ""; text up to 4096 bytes) */
  when?: string;
  /** A slot's picture, drawn from strokes: sword, shield, bolt, heal, dash, star, flame or skull (default ""; text up to 4096 bytes) */
  icon?: string;
  /** A slot's key label, usually text="{key:ACTION}" so it follows the player's device (default ""; text up to 4096 bytes) */
  key?: string;
  /** A stack or grid declared earlier that places this element; empty places it by anchor (default ""; text up to 4096 bytes) */
  parent?: string;
  /** Flow states (comma separated) in which it is visible; empty means always (default ""; text up to 4096 bytes) */
  show?: string;
  /** Flow states in which it is hidden (default ""; text up to 4096 bytes) */
  hide?: string;
  /** Words for text, button and the like; {name} inserts a variable, {key:ACTION} the key or button the player uses for an input (default ""; text up to 4096 bytes) */
  text?: string;
  /** Actions a button runs when accepted, as in game.rule@1: go:playing, restart, level:cave, add:score:1  (default ""; text up to 4096 bytes) */
  do?: string;
  /** A button's tooltip, shown under it while it has the focus (default ""; text up to 4096 bytes) */
  tip?: string;
  /** Where on the screen: tl, tc, tr, cl, c, cr, bl, bc, br (default tl) (default ""; text up to 4096 bytes) */
  anchor?: string;
  /** Offset from the anchor in screen heights, as x,y (for example 0.02,0.03) (default ""; text up to 4096 bytes) */
  at?: string;
  /** Width,height in screen heights for bar, panel, button and minimap, written as text such as "0.3,0.03" (default ""; text up to 4096 bytes) */
  size?: string;
  /** The letter height of a text or a button's words, in screen heights (-1 uses 0.03; 0.06 is a heading) (default -1; -1..1000) */
  letter?: number | Node<'Number'>;
  /** A bar's value: the name of a variable (default ""; text up to 4096 bytes) */
  value?: string;
  /** A bar's full value as a number (-1 uses 1) (default -1; -1..1000) */
  max?: number | Node<'Number'>;
  /** A bar's full value as the name of a variable (used instead of max) (default ""; text up to 4096 bytes) */
  max_var?: string;
  /** Words drawn on a bar (default ""; text up to 4096 bytes) */
  label?: string;
  /** A stack's direction: down or right (default ""; text up to 4096 bytes) */
  dir?: string;
  /** Space between a container's children (-1 uses the default) (default -1; -1..1000) */
  gap?: number | Node<'Number'>;
  /** Space inside a container's edge (-1 uses the default) (default -1; -1..1000) */
  pad?: number | Node<'Number'>;
  /** A stack's alignment across its direction: start, center or end (default ""; text up to 4096 bytes) */
  align?: string;
  /** A grid's columns (-1 uses 3) (default -1; -1..1000) */
  cols?: number | Node<'Number'>;
  /** A grid's cell width,height (default ""; text up to 4096 bytes) */
  cell?: string;
  /** A minimap's reach in metres from the centre to the edge; for other elements the corner rounding (default -1; -1..1000) */
  radius?: number | Node<'Number'>;
  /** A minimap's centre: a selector such as tag:hero (default ""; text up to 4096 bytes) */
  follow?: string;
  /** A minimap shows entities of kinds with these tags (comma separated) (default ""; text up to 4096 bytes) */
  tags?: string;
  /** A minimap shows entities of these kinds (comma separated) (default ""; text up to 4096 bytes) */
  kinds?: string;
  /** Background colour of a panel, stack, grid or button (default ""; text up to 4096 bytes) */
  fill?: string;
  /** Border colour (default ""; text up to 4096 bytes) */
  border?: string;
  /** Text or bar colour (default ""; text up to 4096 bytes) */
  color?: string;
  /** Font preset: sans, serif, mono, hand  (default ""; text up to 4096 bytes) */
  font?: string;
  /** true puts a dark plate behind a text (default ""; text up to 4096 bytes) */
  plate?: string;
}

/** Declare a numeric game variable. Example: `score = game.var@1(name="score", value=0)`. */
export interface GameVarV1Params {
  /** Variable name (required; text up to 4096 bytes) */
  name: string;
  /** Initial numeric value (required; -1000000000..1000000000) */
  value: number | Node<'Number'>;
}

/** A weapon carried by the entities matching owner. It fires when its input (game.input@1 with key="press") is pressed, held when auto is on, or when a rule runs fire:NAME (an enemy: `game.rule@1(on="sight", a="kind:turret", do="fire:spit@a")`), and keeps a magazine in the owner's prop ammo_NAME. */
export interface GameWeaponV1Params {
  /** Unique name; fire:NAME and the prop ammo_NAME use it (required; text up to 4096 bytes) */
  name: string;
  /** A flying shot, an instant ray, or a melee swing that hurts everything within range inside the arc (default "projectile") */
  mode?: 'projectile' | 'hitscan' | 'melee';
  /** Who it can hurt, as a selector such as tag:enemy (empty: any entity with health but the owner). Name the enemy for a party's weapons so allies never hurt each other (default ""; text up to 4096 bytes) */
  foes?: string;
  /** Melee: the width of the swing in degrees, centred on the aim (default 120; 1..360) */
  arc?: number | Node<'Number'>;
  /** Who carries it: a selector such as tag:hero or kind:turret (required; text up to 4096 bytes) */
  owner: string;
  /** The key input that fires it (none: only rules fire it) (default "none"; text up to 4096 bytes) */
  input?: string;
  /** The key input that reloads it (default ""; text up to 4096 bytes) */
  reload_input?: string;
  /** Projectile mode: the kind of the shot (default ""; text up to 4096 bytes) */
  shot?: string;
  /** Rounds per magazine (0 is endless) (default 0; whole number 0..10000) */
  ammo?: number | Node<'Number'>;
  /** Seconds to reload (default 1; 0..60) */
  reload?: number | Node<'Number'>;
  /** Seconds between shots (default 0.2; 0..60) */
  cooldown?: number | Node<'Number'>;
  /** Scatter in degrees either side of the aim (default 0; 0..90) */
  spread?: number | Node<'Number'>;
  /** heading, camera, or a selector to aim at (default "heading"; text up to 4096 bytes) */
  aim?: string;
  /** Height of the muzzle above the owner's feet, in metres (default 1; 0..10) */
  muzzle?: number | Node<'Number'>;
  /** Hitscan: how far the ray reaches, in metres (default 0; 0..10000) */
  range?: number | Node<'Number'>;
  /** Hitscan: damage of a hit (default 10; 0..1000000) */
  damage?: number | Node<'Number'>;
  /** Keep firing while the input is held (default false) */
  auto?: boolean;
}

/** A solid that an instance of the group supplies, by this name. */
export interface GroupInputV1Params {
  /** The input's name; an instance links to it by this name (required; text up to 64 bytes) */
  name: string;
}

/** An instance of a group: the group's result, with the parameters it exposes. */
export interface GroupUseV1Params {
}

/** Shared copies of a shape spaced evenly by length along a curve, each turned to follow it (a fence along a path, rungs along a rail, beads on a wire, links of a chain). An open curve gets one copy at each end and the rest evenly between; a closed curve spaces them all the way round. */
export interface InstAlongCurveV1Params {
  /** Placement count; products must fit the instance budget (default 8; whole number 1..100000) */
  count?: number | Node<'Number'>;
  /** Turn each copy to follow the curve; otherwise copies keep their orientation and only move (default true) */
  align?: boolean;
}

/** Shared grid of copies, no union. spacing is the distance between neighbours in metres, or with relative=true a multiple of the shape's own size along each axis (spacing [1.1, 0, 0] leaves a tenth of the width between copies, whatever the shape's size). */
export interface InstArrayV1Params {
  /** Placement count; products must fit the instance budget (default 1; whole number 1..100000) */
  count_x?: number | Node<'Number'>;
  /** Placement count; products must fit the instance budget (default 1; whole number 1..100000) */
  count_y?: number | Node<'Number'>;
  /** Placement count; products must fit the instance budget (default 1; whole number 1..100000) */
  count_z?: number | Node<'Number'>;
  /** Vector in metres or radians (required; metres, -1000000..1000000 each) */
  spacing: Vec3;
  /** Read spacing as multiples of the shape's size (its bounding box) instead of metres (default false) */
  relative?: boolean;
}

/** Sets every placement down onto a surface (trees, rocks and grass on a terrain; props on a table): each copy slides straight down onto the highest point of `surface` under it. Scatter over the surface's footprint first, then drop. */
export interface InstDropV1Params {
  /** Extra height above (or sink below, if negative) the surface in metres; a small negative value buries a tree trunk's base (default 0; metres, -100..100) */
  offset?: number | Node<'Number'>;
  /** 0 keeps copies upright; 1 tilts each to stand square on the slope under it; in between tilts part of the way (default 0; a ratio, 0..1) */
  align?: number | Node<'Number'>;
  /** What to do with copies not over the surface: remove them, or keep them where they were (default "remove") */
  outside?: 'remove' | 'keep';
}

/** Varies each placement by a bounded random shift, turn and scale; element i depends only on the seed and i. */
export interface InstJitterV1Params {
  /** Vector in metres or radians (default [0, 0, 0]; metres, -1000000..1000000 each) */
  shift?: Vec3;
  /** Authored random seed; the same seed gives the same result on every platform (default 0; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Largest random turn about Y, either way (default 0; an angle, 0..3.141592653589793) */
  yaw_jitter?: number | Node<'Number'>;
  /** Uniform scale varies by up to this fraction either way (0.2 means 0.8 to 1.2) (default 0; a ratio, 0..0.9) */
  scale_jitter?: number | Node<'Number'>;
}

/** Original placements and their reflection, with corrected winding. */
export interface InstMirrorV1Params {
  /** Reflection plane normal (required) */
  axis: 'x' | 'y' | 'z';
}

/** Copies of a shape at positions you list, as one shared set (the shape is stored once): six jars at chosen spots, a sack here, a lantern there. Add yaw_scale for a turn and size per copy. */
export interface InstPlaceV1Params {
  /** Where each copy stands: [x, y, z] in metres, one entry per copy (required; 1 to 4096 points) */
  at: Vec3[];
  /** Optional, one [yaw, scale] per copy in the same order as `at`: yaw in degrees about the vertical axis, scale a size factor above 0 (1 is unchanged) (default []; 0 to 4096 points) */
  yaw_scale?: Vec2[];
}

/** Shared copies placed by programs of the copy number, in the field-expression language of field.expr: `code` gives where each copy goes (`out` a vec3), `scale_code` its size (a number above 0), `yaw_code` its turn about Y in degrees. They see `i` (0 for the first copy), `count` and `f` (0 for the first copy, 1 for the last). */
export interface InstProgramV1Params {
  /** Placement count; products must fit the instance budget (default 16; whole number 1..100000) */
  count?: number | Node<'Number'>;
  /** Where copy `i` goes: the last statement sets `out`, a vec3 in metres (default "out = vec3(i, 0.0, 0.0)"; text up to 8192 bytes) */
  code?: string;
  /** Size of copy `i`: `out` is a number above 0 (default "out = 1.0"; text up to 8192 bytes) */
  scale_code?: string;
  /** Turn of copy `i` about Y in degrees: `out` is a number (default "out = 0.0"; text up to 8192 bytes) */
  yaw_code?: string;
}

/** Shared copies around Y; no union. */
export interface InstRadialV1Params {
  /** Placement count; products must fit the instance budget (required; whole number 1..100000) */
  count: number | Node<'Number'>;
  /** Initial X offset (default 0; metres, 0..1000000) */
  radius?: number | Node<'Number'>;
}

/** Explicitly union placements, after geometry/storage preflight. */
export interface InstRealizeV1Params {
}

/** Shared copies rotated then translated; no union. */
export interface InstRepeatV1Params {
  /** Placement count; products must fit the instance budget (required; whole number 1..100000) */
  count: number | Node<'Number'>;
  /** Vector in metres or radians (default [0, 0, 0]; metres, -1000000..1000000 each) */
  step?: Vec3;
  /** Vector in metres or radians (default [0, 0, 0]; an angle, -1000000..1000000 each) */
  rotate_step?: Vec3;
}

/** Seeded scatter over a width x depth rectangle in the XZ plane centred on the origin; min_spacing keeps centres apart or the node refuses. */
export interface InstScatterV1Params {
  /** Number of placements; refused rather than reduced when the spacing cannot be met (required; whole number 1..16384) */
  count: number | Node<'Number'>;
  /** Extent along X (required; metres, 0..1000000) */
  width: number | Node<'Number'>;
  /** Extent along Z (required; metres, 0..1000000) */
  depth: number | Node<'Number'>;
  /** Smallest centre distance (default 0; metres, 0..1000000) */
  min_spacing?: number | Node<'Number'>;
  /** Authored random seed; the same seed gives the same result on every platform (default 0; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Largest random turn about Y, either way (default 0; an angle, 0..3.141592653589793) */
  yaw_jitter?: number | Node<'Number'>;
  /** Uniform scale varies by up to this fraction either way (0.2 means 0.8 to 1.2) (default 0; a ratio, 0..0.9) */
  scale_jitter?: number | Node<'Number'>;
}

/** Seeded scatter over the surface of a solid, in any orientation (pebbles on a hill's sides, bolts on a tank, moss on a wall): points are drawn uniformly by area over `surface`, and with align each copy's +Y axis follows the surface normal there. min_distance keeps copies apart or the node refuses (it never places fewer than count). */
export interface InstScatterOnV1Params {
  /** Number of placements; refused rather than reduced when min_distance cannot be met (default 100; whole number 1..5000) */
  count?: number | Node<'Number'>;
  /** Smallest distance between two placements, in metres (0 for none) (default 0; metres, 0..1000000) */
  min_distance?: number | Node<'Number'>;
  /** Turn each copy so its +Y axis is the surface normal; false keeps the prototype's orientation (default true) */
  align?: boolean;
  /** Turn each copy by a seeded random angle about its own up axis (default true) */
  spin?: boolean;
  /** Largest seeded lean away from the surface normal (0 to 45 degrees) (default 0; an angle, 0..0.7853981633974483) */
  tilt?: number | Node<'Number'>;
  /** Scale each copy by 1 plus or minus up to this fraction (default 0; 0..1) */
  scale_jitter?: number | Node<'Number'>;
  /** Where on the surface: all, up (normal pointing up), side, or down (default "all") */
  region?: 'all' | 'up' | 'side' | 'down';
  /** Authored random seed; the same seed gives the same result on every platform (default 0; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
}

/** Varies every placement from patterns sampled at its position: scale, world-up yaw and lift. Example: `s = field.expr@1(code="out = 1.0 + step(10.0, p.x)"); v = inst.vary@1(in, scale=s)` doubles copies to the right of x=10. */
export interface InstVaryV1Params {
}

/** A point light with bounded range. */
export interface LightPointV1Params {
  /** World position in metres (required; metres, -1000000..1000000 each) */
  position: Vec3;
  /** Light colour in sRGB (required; 0..1 each) */
  color: Vec3;
  /** Radiant intensity; point and spot values are calibrated at one metre (default 10; 0..100000) */
  intensity?: number | Node<'Number'>;
  /** Distance in metres where light falloff reaches zero (default 8; metres, 0.001..1000000) */
  range?: number | Node<'Number'>;
}

/** A cone light with smooth inner and outer angles. */
export interface LightSpotV1Params {
  /** World position in metres (required; metres, -1000000..1000000 each) */
  position: Vec3;
  /** World direction; must be non-zero (required; -1..1 each) */
  direction: Vec3;
  /** Light colour in sRGB (required; 0..1 each) */
  color: Vec3;
  /** Radiant intensity; point and spot values are calibrated at one metre (default 10; 0..100000) */
  intensity?: number | Node<'Number'>;
  /** Distance in metres where light falloff reaches zero (default 8; metres, 0.001..1000000) */
  range?: number | Node<'Number'>;
  /** Full cone angle in radians at full intensity (default 0.35; an angle, 0.001..1.56) */
  inner_angle?: number | Node<'Number'>;
  /** Full cone angle in radians where the beam reaches zero (default 0.6; an angle, 0.002..1.57) */
  outer_angle?: number | Node<'Number'>;
}

/** A directional sun; direction points from the light toward the scene. */
export interface LightSunV1Params {
  /** World direction; must be non-zero (required; -1..1 each) */
  direction: Vec3;
  /** Light colour in sRGB (required; 0..1 each) */
  color: Vec3;
  /** Radiant intensity; point and spot values are calibrated at one metre (default 10; 0..100000) */
  intensity?: number | Node<'Number'>;
}

/** Applies a named tiled procedural surface with optional edge wear, crease dirt, top-facing moss and wetness. oak: warm wood; planks: boards; red_brick: fired brick; cobblestone: rounded stone; concrete: cast grey stone; plaster: pale wall; rusted_iron: oxidized metal; brushed_steel: satin steel; leather: pebbled hide; linen: woven cloth; sand: fine grains; mud: damp earth; snow: soft snow; ice: blue ice; asphalt: dark road surface (give it wet for a night street); corrugated_metal: ribbed steel sheet; painted_metal: dark painted panel that shows bare metal on its edges; wall_tile: small glazed tiles. */
export interface MaterialPresetV1Params {
  /** Named surface look (default "oak") */
  name?: 'oak' | 'planks' | 'red_brick' | 'cobblestone' | 'concrete' | 'plaster' | 'rusted_iron' | 'brushed_steel' | 'leather' | 'linen' | 'sand' | 'mud' | 'snow' | 'ice' | 'asphalt' | 'corrugated_metal' | 'painted_metal' | 'wall_tile';
  /** Length in metres of one repeat (default 1; metres, 0.1..10) */
  tile?: number | Node<'Number'>;
  /** Edge wear amount (default 0.3; 0..1) */
  wear?: number | Node<'Number'>;
  /** Dirt gathered in creases (default 0.2; 0..1) */
  dirt?: number | Node<'Number'>;
  /** Moss on upward-facing surfaces (default 0; 0..1) */
  moss?: number | Node<'Number'>;
  /** Wetness darkening and gloss (default 0; 0..1) */
  wet?: number | Node<'Number'>;
  /** Side in pixels of the baked maps (default "512") */
  size?: '256' | '512' | '1024';
}

/** Makes a shape a physical object in play mode: it falls under gravity, collides with other bodies, and can be picked up and thrown. kind=static for floors, walls and terrain (never moves), dynamic for things that fall, roll and stack (balls, crates, props), kinematic for things that move by hand or script and shove others. */
export interface PhysBodyV1Params {
  /** static: never moves. dynamic: falls and collides. kinematic: moved by hand or play, pushes dynamic bodies (default "dynamic") */
  kind?: 'static' | 'dynamic' | 'kinematic';
  /** What it collides as. auto and convex use the part's vertices (a good fit for most solids); box, sphere, capsule and cylinder fit the bounding box (fastest and most stable, best for crates, balls and pillars); mesh is the exact surface for static terrain, floors and buildings only (default "auto") */
  shape?: 'auto' | 'box' | 'sphere' | 'capsule' | 'cylinder' | 'convex' | 'mesh';
  /** Mass per cubic metre: 1 gives light props; wood is about 600, water 1000, stone 2500. Heavier things push lighter ones (default 1; 0.001..20000) */
  density?: number | Node<'Number'>;
  /** Sliding grip: 0 is ice, 0.6 is typical, 1 or more is rubber (default 0.6; 0..3) */
  friction?: number | Node<'Number'>;
  /** Bounciness: 0 stops dead, 0.8 is a rubber ball. A contact bounces as much as the bouncier of the two bodies (default 0; a ratio, 0..1) */
  restitution?: number | Node<'Number'>;
  /** Air drag: fraction of speed lost per second (default 0; 0..50) */
  linear_damping?: number | Node<'Number'>;
  /** Fraction of spin lost per second (raise it so rolling things settle) (default 0.05; 0..50) */
  angular_damping?: number | Node<'Number'>;
}

/** Connects a physics body to another part or to the world with a joint, for mechanisms: a door on hinges, a pendulum, a drawer, a rope link, a wheel on an axle, a spring-hung weight. Put it AFTER phys.body on the part that moves; `to` is the OUTPUT name of the other part (which must also have phys.body) or empty to join this part to the world. */
export interface PhysJointV1Params {
  /** hinge: turns about an axis. slider: slides along an axis. ball: turns freely about the anchor. fixed: welded. spring: elastic pull between two points (default "hinge") */
  kind?: 'hinge' | 'slider' | 'ball' | 'fixed' | 'spring';
  /** Output name of the other part (it needs phys.body too), or empty to attach to the world (default ""; text up to 64 bytes) */
  to?: string;
  /** The joint point in world coordinates at the start (the hinge line, the pivot, where the spring hooks on this part) (default [0, 0, 0]; metres, -1000..1000 each) */
  anchor?: Vec3;
  /** Hinge axis or slider direction in world coordinates: [0,1,0] is a vertical hinge like a door, [0,0,1] a horizontal one like a swing or pendulum (default [0, 1, 0]; -1000..1000 each) */
  axis?: Vec3;
  /** Springs only: where the spring hooks onto the other part (or the world). Leave at the anchor for other kinds (default [0, 0, 0]; metres, -1000..1000 each) */
  anchor_b?: Vec3;
  /** Whether low and high limit the movement (a door that opens 100 degrees, a drawer that slides 0.4 m) (default false) */
  limited?: boolean;
  /** Lowest allowed angle in degrees (hinge) or distance in metres (slider) from the starting pose (default 0; -100000..100000) */
  low?: number | Node<'Number'>;
  /** Highest allowed angle in degrees (hinge) or distance in metres (slider) from the starting pose (default 0; -100000..100000) */
  high?: number | Node<'Number'>;
  /** Spring pull as acceleration per metre of stretch: 5 is gentle, 50 stiff; heavy and light bodies swing alike (default 20; 0..10000) */
  stiffness?: number | Node<'Number'>;
  /** Spring damping, or the friction that slows a hinge or slider (0 swings forever) (default 1; 0..1000) */
  damping?: number | Node<'Number'>;
  /** Motor aim in degrees per second (hinge) or metres per second (slider); needs motor_force above 0 (default 0; -10000..10000) */
  motor_speed?: number | Node<'Number'>;
  /** Strongest push or torque the motor may use; 0 means no motor (default 0; 0..1000000) */
  motor_force?: number | Node<'Number'>;
  /** Springs only: the distance the spring rests at; -1 means the distance between the anchors at the start (default -1; metres, -1..1000) */
  rest_length?: number | Node<'Number'>;
}

/** Makes a rigged character (a skeleton of rig.bone parts, each anim.parent-ed to the one above it) able to go LIMP: on command it stops following its animation and falls and tumbles as a ragdoll, colliding with every phys.body in the scene, starting from whatever pose it was in. Put this on any small shape (a marker is fine); `bones` lists the OUTPUT names of the bone parts, parents first, for example "pelvis; spine; head; arm_l; forearm_l; thigh_l; shin_l". */
export interface PhysRagdollV1Params {
  /** The ragdoll's name (letters, digits, _ or -), used to trigger it (default ""; text up to 32 bytes) */
  name?: string;
  /** Output names of the bone parts, separated by semicolons, parents before children (1 to 32) (default ""; text up to 512 bytes) */
  bones?: string;
  /** Socket freedom: how far a limb may swing away from its parent's line, in degrees (about 60 for shoulders and hips, 30 for a neck) (default 1; an angle, 0..3.1416) */
  swing?: number | Node<'Number'>;
  /** How far a limb may twist about its own length either way, in degrees (default 0.5; an angle, 0..3.1416) */
  twist?: number | Node<'Number'>;
  /** Output names of the bones that are hinges (elbows, knees), separated by semicolons; the others are sockets (default ""; text up to 512 bytes) */
  hinges?: string;
  /** Lowest and highest hinge angle in degrees: [0, 140] bends one way only, like a knee (default [0, 2.4]; an angle, -3.1416..3.1416 each) */
  hinge_limits?: Vec2;
  /** Which way a straight hinge bone bends about (world axis); a bone that is already bent at rest bends in its own plane. A person standing facing +z has knees [-1, 0, 0] (default [1, 0, 0]; -1000..1000 each) */
  hinge_axis?: Vec3;
  /** Mass per cubic metre of the bones (a person is about 500 to 1000) (default 500; 10..5000) */
  density?: number | Node<'Number'>;
  /** How much the joints resist moving: 0 is limp jelly, 3 is stiff (default 0.3; 0..20) */
  damping?: number | Node<'Number'>;
  /** Thickness of every bone's collision capsule in metres; 0 sizes each from its length (default 0; metres, 0..2) */
  radius?: number | Node<'Number'>;
}

/** Sets the physics of the whole scene: gravity, and whether you WALK through it in play mode. Put it after phys.body on any ONE body (the floor is natural); a second one is ignored with a warning. */
export interface PhysWorldV1Params {
  /** Gravity in metres per second squared: [0,-9.81,0] Earth, [0,-1.62,0] Moon, [0,0,0] weightless (default [0, -9.81, 0]; -1000..1000 each) */
  gravity?: Vec3;
  /** Walk through the scene in play mode (WASD, Space to jump) instead of flying (default false) */
  walker?: boolean;
  /** The walker's height in metres (eye level is just below the top) (default 1.7; metres, 0.6..3) */
  height?: number | Node<'Number'>;
  /** The walker's radius in metres; doorways and gaps must be wider than twice this (default 0.3; metres, 0.1..1) */
  radius?: number | Node<'Number'>;
  /** Walking speed in metres per second (3 is a brisk walk, Shift runs at double) (default 3; 0.5..20) */
  speed?: number | Node<'Number'>;
  /** Upward speed of a jump in metres per second: 4.5 jumps about 1 m high, 0 disables jumping (default 4.5; 0..15) */
  jump?: number | Node<'Number'>;
}

/** Flips the shared edge of triangle pairs where Blender's beauty measure prefers the other diagonal, to even out a triangulated mesh before smoothing or displacing it. The measure is Blender's own (by area or by angle); on a given quad the two can disagree, and a pair it already likes is left alone. */
export interface PolyBeautifyV1Params {
  /** Which of Blender's beauty measures decides the diagonal (default "area") */
  method?: 'area' | 'angle';
}

/** Rounds or chamfers selected edges. Edges must be disjoint, between two planar faces, with three faces at each end. */
export interface PolyBevelEdgesV1Params {
  /** How far the bevel reaches onto each face, in metres (required; metres, 0.000001..1000000) */
  width: number | Node<'Number'>;
  /** Profile segments: 1 is a flat chamfer, more round it (default 1; whole number 1..4) */
  segments?: number | Node<'Number'>;
}

/** Rounds or chamfers selected edges of any layout: single edges, loops around a box, edges meeting at a corner, a whole cube. Select edges with poly.select (element=edge; by=sharp picks the hard edges). */
export interface PolyBevelNetworkV1Params {
  /** Bevel size in metres (a percentage, 0 to 100, when offset=percent). It is clamped so the bevel never overlaps itself (required; metres, 0.000001..1000000) */
  width: number | Node<'Number'>;
  /** Segments across the bevel: 1 is a flat chamfer, more round it (default 3; whole number 1..16) */
  segments?: number | Node<'Number'>;
  /** Profile shape: 0.5 is a circular arc, lower is flatter and sharper, 1 is full (default 0.5; 0.1..1) */
  profile?: number | Node<'Number'>;
  /** What width measures: offset (how far each new edge moves from the original along the faces), width (the bevel face's width), depth (how deep the bevel cuts) or percent (of the adjacent edges' length) (default "offset") */
  offset?: 'offset' | 'width' | 'depth' | 'percent';
}

/** Cuts the whole polygon mesh with a plane; above is the side the normal points toward. */
export interface PolyBisectV1Params {
  /** Point on the plane in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  point?: Vec3;
  /** Plane normal; must be non-zero (default [0, 1, 0]; -1000000..1000000 each) */
  normal?: Vec3;
  /** Which side to keep (default "both") */
  keep?: 'both' | 'above' | 'below';
  /** Close the cut boundary when a side is removed (default true) */
  fill?: boolean;
  /** Distance used to classify vertices on the plane (default 0.0001; metres, 0..1) */
  distance?: number | Node<'Number'>;
}

/** Joins two boundary loops with the same number of vertices by a ring of quads (select two loops). */
export interface PolyBridgeLoopsV1Params {
}

/** Reshapes the selected edge loops into circles: an oval hole becomes round, a ragged ring a clean one (select the loop's edges with poly.select element=edge). Radius 0 fits a circle to the loop's own size. */
export interface PolyCircularizeV1Params {
  /** How far toward a circle: 0 nothing, 1 a full circle (default 1; 0..1) */
  factor?: number | Node<'Number'>;
  /** Circle radius in metres; 0 fits one to the loop (default 0; metres, 0..1000) */
  radius?: number | Node<'Number'>;
  /** Rotates the vertices around the circle (default 0; an angle, -6.283185307179586..6.283185307179586) */
  angle?: number | Node<'Number'>;
  /** How far the loop is flattened onto its best-fit plane (default 1; 0..1) */
  flatten?: number | Node<'Number'>;
  /** Space the vertices evenly around the circle instead of keeping their old spacing (default true) */
  regular?: boolean;
}

/** Adds edges between the selected vertices of each face, splitting the face along them (select vertices with poly.select element=vertex; at least two, and two on the same face to make a cut). Cuts a quad across its diagonal or a polygon into pieces. */
export interface PolyConnectVerticesV1Params {
  /** Refuse cuts that would overlap or cross each other (default true) */
  check_degenerate?: boolean;
}

/** Reduces the face count by collapsing the edges that change the shape least: level of detail (LOD) for far-away or cheap versions of a model. ratio is the fraction of faces kept (0.25 keeps a quarter). */
export interface PolyDecimateV1Params {
  /** Fraction of the faces to keep, for example 0.5 for half (required; 0.01..1) */
  ratio: number | Node<'Number'>;
  /** Keep the result mirror-symmetric about this axis plane (default "none") */
  symmetry?: 'none' | 'x' | 'y' | 'z';
  /** Triangulate first (needed unless the mesh is already triangles) (default true) */
  triangulate?: boolean;
}

/** Removes selected faces and leaves an open boundary. Refuses to delete every face or isolate a vertex. */
export interface PolyDeleteFacesV1Params {
}

/** Removes edges shorter than a distance and faces with almost no area, which cause shading glitches and failed operations after a script or a boolean. */
export interface PolyDissolveDegenerateV1Params {
  /** Edges shorter than this, in metres, are removed (default 0.0001; metres, 0.000000001..1000) */
  distance?: number | Node<'Number'>;
}

/** Removes one interior edge and merges its two faces (select one edge). */
export interface PolyDissolveEdgeV1Params {
}

/** Merges a connected patch of selected faces into one face (select faces). */
export interface PolyDissolveFacesV1Params {
}

/** Merges neighbouring faces that are flatter than an angle: cleans triangulated or over-detailed flat areas back into big polygons. */
export interface PolyDissolveLimitV1Params {
  /** Faces whose normals differ by less than this merge (default 0.0873; an angle, 0..3.141592653589793) */
  angle?: number | Node<'Number'>;
  /** Also dissolve along open boundaries (default false) */
  boundaries?: boolean;
}

/** Removes one vertex and merges its faces (select one vertex). */
export interface PolyDissolveVertexV1Params {
}

/** Pushes selected planar faces out by an offset and joins their border to the mesh with side faces. A connected patch moves as one; with individual each face moves on its own. */
export interface PolyExtrudeFacesV1Params {
  /** Where the faces move, in metres; it must point out of the face (required; metres, -1000000..1000000 each) */
  offset: Vec3;
  /** Extrude each selected face on its own (default false) */
  individual?: boolean;
}

/** Extrudes the selected faces and moves the new faces by an offset, leaving walls around them. Select faces with poly.select. */
export interface PolyExtrudeRegionV1Params {
  /** The vector the new faces move by, in metres (required; metres, -1000000..1000000 each) */
  offset: Vec3;
}

/** Closes the open boundary loops of a mesh with faces (a hole left by deleting faces, an open end). A hole with more sides than max_sides stays open, so a big opening is not filled by accident. */
export interface PolyFillHolesV1Params {
  /** The most edges a hole may have and still be filled (4 fills square holes; raise it for bigger ones) (default 4; whole number 3..1000) */
  max_sides?: number | Node<'Number'>;
}

/** Reverses the winding of selected faces. Whole connected components always work; a partial flip may not add inconsistent edges unless allow_inconsistent is true. */
export interface PolyFlipFacesV1Params {
  /** Allow a partial flip that leaves winding conflicts (default false) */
  allow_inconsistent?: boolean;
}

/** Breaks a closed shape into pieces along a random Voronoi pattern, the way a wall, a vase or a boulder shatters. Each node makes ONE piece, number `piece` of `pieces`; write a loop over piece = 0 to pieces - 1 to make them all, each with its own output, then give each a physics body (phys.body, shape convex) to let them fall, or move them apart with xform.move to show the break. */
export interface PolyFractureV1Params {
  /** How many pieces the shape breaks into (2 to 64) (default 8; whole number 2..64) */
  pieces?: number | Node<'Number'>;
  /** Which piece this node makes, 0 to pieces - 1 (default 0; whole number 0..63) */
  piece?: number | Node<'Number'>;
  /** Pattern seed: the same seed breaks the same way (default 1; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** How finely the shape is sampled, in metres; keep it well under the piece size (default 0.04; metres, 0.005..0.5) */
  voxel_size?: number | Node<'Number'>;
  /** Width of the cracks between pieces, in metres (0 leaves them touching) (default 0.01; metres, 0..0.2) */
  gap?: number | Node<'Number'>;
}

/** Turns a solid into an editable polygon mesh of its triangles. Merge coplanar triangles with poly.dissolve_faces. */
export interface PolyFromSolidV1Params {
}

/** A flat grid of quads in the z = 0 plane with normalized u/v corner UVs, width wide centred on x = 0 and height tall centred on y = 0 (so it spans -height/2 to height/2: move it up to stand it on the floor), facing +z. */
export interface PolyGridV1Params {
  /** Total width in metres (required; metres, 0.000001..1000000) */
  width: number | Node<'Number'>;
  /** Total height in metres (required; metres, 0.000001..1000000) */
  height: number | Node<'Number'>;
  /** Quads across (default 4; whole number 1..512) */
  columns?: number | Node<'Number'>;
  /** Quads up (default 4; whole number 1..512) */
  rows?: number | Node<'Number'>;
}

/** Brings your own mesh file into the document as an editable polygon mesh: the file's text goes in `data` and `format` says how to read it (obj, ascii ply or ascii stl). Nothing is fetched and nothing is bundled with the engine; the document carries the text, so the result is the same everywhere. */
export interface PolyImportV1Params {
  /** The file's text (required; text up to 1000000 bytes) */
  data: string;
  /** How to read `data`: Wavefront obj, ascii ply or ascii stl (default "obj") */
  format?: 'obj' | 'ply' | 'stl';
  /** Multiplies every coordinate; the file's unit times this is metres (default 1; a ratio, 0.000001..1000000) */
  scale?: number | Node<'Number'>;
  /** Which axis the file calls up; z is turned to Y up (default "y") */
  up?: 'y' | 'z';
}

/** Shrinks each selected planar convex face inward by a distance, leaving a ring of quads and an inner face. */
export interface PolyInsetFacesV1Params {
  /** How far the border moves in, in metres (required; metres, 0.000001..1000000) */
  distance: number | Node<'Number'>;
}

/** Insets the selected faces: shrinks the region inward by a distance, leaving a ring of faces, and can raise or sink the inner face. Works on any polygon, not only planar convex ones. */
export interface PolyInsetRegionV1Params {
  /** Width of the ring, in metres (required; metres, 0.000001..1000000) */
  thickness: number | Node<'Number'>;
  /** How far the inner face is raised along its normal (negative sinks it), in metres (default 0; metres, -1000000..1000000) */
  depth?: number | Node<'Number'>;
  /** Scale the offset so the ring has even thickness at corners (default true) */
  even?: boolean;
  /** Scale the offset by the surrounding geometry (default false) */
  relative?: boolean;
  /** Also inset the region's outer boundary (default true) */
  boundary?: boolean;
  /** Grow the region outward instead of shrinking it inward (default false) */
  outset?: boolean;
}

/** Joins triangle pairs into quads where the pair is flat and square enough: turns a triangulated mesh back into quads for clean subdivision and editing. Tighter limits join fewer pairs. */
export interface PolyJoinTrianglesV1Params {
  /** Largest angle between the two triangles' normals that still joins (0.698 rad is 40 degrees) (default 0.698; an angle, 0..3.141592653589793) */
  face_angle?: number | Node<'Number'>;
  /** Largest deviation of the new quad's corners from right angles that still joins (0.698 rad is 40 degrees) (default 0.698; an angle, 0..3.141592653589793) */
  shape_angle?: number | Node<'Number'>;
}

/** Builds one level of detail by repeatedly simplifying a polygon mesh; level 0 returns the input. */
export interface PolyLodV1Params {
  /** Number of simplified levels in the chain (default 3; 1..4) */
  levels?: number | Node<'Number'>;
  /** Level to return; 0 is the input and 1..levels are progressively coarser (default 1; 0..4) */
  level?: number | Node<'Number'>;
  /** Fraction of faces kept at each level (default 0.5; 0.05..0.9) */
  ratio?: number | Node<'Number'>;
  /** Preserve UV discontinuities while simplifying (default false) */
  protect_uv_seams?: boolean;
  /** Preserve open mesh boundaries while simplifying (default false) */
  protect_boundary?: boolean;
}

/** Cuts a closed ring of quad faces with evenly spaced parallel edges. */
export interface PolyLoopCutV1Params {
  /** Point in metres used to choose the nearest edge (default [0, 0, 0]; metres, -1000000..1000000 each) */
  at?: Vec3;
  /** Number of parallel cuts (default 1; whole number 1..32) */
  cuts?: number | Node<'Number'>;
  /** Smoothness passed to subdivision (default 0; 0..1) */
  smooth?: number | Node<'Number'>;
}

/** Merges connected coplanar faces into single faces, such as a solid's triangles into its flat faces. A patch that cannot become one simple face (the wall face around a window, a pinch) stays as it is by default, so a cut wall can then be rounded with poly.round_edges. */
export interface PolyMergeCoplanarV1Params {
  /** How far a face's normal may differ from the patch's first face and still count as coplanar, in radians (default 0.01; an angle, 0..0.5) */
  angle?: number | Node<'Number'>;
  /** keep leaves a patch that cannot become one simple face (a hole or pinch) unmerged; refuse fails with the reason instead (default "keep") */
  holes?: 'keep' | 'refuse';
}

/** Adds the mesh reflected across a plane and welds vertices near the plane. */
export interface PolyMirrorV1Params {
  /** Point on the plane in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  point?: Vec3;
  /** Plane normal; must be non-zero (default [1, 0, 0]; -1000000..1000000 each) */
  normal?: Vec3;
  /** Weld original and reflected vertices within distance (default true) */
  merge?: boolean;
  /** Maximum merge distance in metres (default 0.0001; metres, 0..1000) */
  distance?: number | Node<'Number'>;
}

/** Packs existing corner UV islands into the 0..1 square; UV discontinuities define island boundaries. */
export interface PolyPackUvV1Params {
  /** Allow islands to rotate while packing (default true) */
  rotate?: boolean;
  /** Preserve authored island scale and allow unused space (default false) */
  keep_density?: boolean;
  /** Use axis-aligned bounds or convex hulls for packing (default "aabb") */
  shape?: 'aabb' | 'convex';
  /** Fractional gap around islands (default 0.02; 0..0.5) */
  margin?: number | Node<'Number'>;
}

/** Flattens each face toward its best-fit plane by moving its vertices, so quads that were bent become flat. Neighbouring faces pull on each other, so several passes settle it. */
export interface PolyPlanarFacesV1Params {
  /** Passes, 1 to 50 (default 10; whole number 1..50) */
  iterations?: number | Node<'Number'>;
  /** How far vertices move toward the plane each pass (default 1; 0..1) */
  factor?: number | Node<'Number'>;
}

/** Adds a vertex in the middle of each selected face (all faces when none are selected) and fans triangles to it, raising a pyramid. */
export interface PolyPokeV1Params {
  /** How far the new vertex is raised along the normal, in metres (default 0; metres, -1000000..1000000) */
  offset?: number | Node<'Number'>;
  /** Where the new vertex sits: the mean of the corners, the angle-weighted mean, or the centre of the bounding box (default "median") */
  center?: 'median' | 'weighted' | 'bounds';
  /** Scale the offset by the face's size (default false) */
  relative?: boolean;
}

/** Runs a short program once for every vertex and uses the result: `out = ...` in the field-expression language. The program sees the vertex position `p`, its smoothed normal `n`, its first UV `uv`, its number `i` (0 for the first vertex) and the vertex count `count`, plus `attr("name")` for any per-vertex number layer of the mesh. */
export interface PolyProgramV1Params {
  /** The program, at most 8,192 bytes and 256 operations. The last statement must set `out`, of the type `mode` asks for (has a default; text up to 8192 bytes) */
  code?: string;
  /** What `out` is: the new position, an offset added to the position, a distance along the normal, or a number stored in a layer (default "offset") */
  mode?: 'position' | 'offset' | 'along_normal' | 'layer';
  /** Name of the per-vertex layer written when mode is `layer` (default "program"; text up to 32 bytes) */
  layer_name?: string;
}

/** Makes every face point the same way, outward from the mesh (or inward), whatever its winding was. Use it after a script, a mirror or a boolean leaves some faces inside out (dark patches, faces seen through). */
export interface PolyRecalcNormalsV1Params {
  /** Which side the faces face (default "outward") */
  direction?: 'outward' | 'inward';
}

/** Smooths the selected edge loops so they flow in a gentler curve (select the loop's edges with poly.select element=edge). More iterations smooth more and can shrink a loop a little. */
export interface PolyRelaxLoopsV1Params {
  /** Passes, 1 to 50 (default 3; whole number 1..50) */
  iterations?: number | Node<'Number'>;
  /** Smooth curve or straight segments (default "cubic") */
  interpolation?: 'cubic' | 'linear';
  /** Also space the vertices evenly along the loop (default false) */
  even_spacing?: boolean;
}

/** Rebuilds a mesh as clean, even quads of about the size you ask for (Instant Meshes): turns the uneven triangles of a field mesh, a boolean or a script into a tidy quad mesh that subdivides, smooths and deforms well. For smooth organic shapes (creatures, rocks, blobs). */
export interface PolyRemeshQuadsV1Params {
  /** About how many quads to make (within about 10 percent) (default 1000; whole number 8..20000) */
  faces?: number | Node<'Number'>;
  /** Picks one of the equally good layouts; the same seed gives the same mesh (default 1; whole number 0..4294967295) */
  seed?: number | Node<'Number'>;
}

/** Merges vertices within a distance in metres. With no selection, checks every vertex. */
export interface PolyRemoveDoublesV1Params {
  /** Maximum merge distance in metres (1e-6 to 1e3) (required; metres, 0.000001..1000000) */
  distance: number | Node<'Number'>;
}

/** Spins each selected edge to the other diagonal of the two faces it joins (triangle pairs become the other split, quad pairs are re-cut). Select edges with poly.select element=edge; each must join exactly two faces. */
export interface PolyRotateEdgesV1Params {
  /** Which way the edge turns (default "clockwise") */
  direction?: 'clockwise' | 'counter_clockwise';
}

/** Rounds every hard edge of a polygon mesh in one step: the edges between faces turned apart by at least angle get a bevel of the given width. Replaces poly.select by=sharp followed by poly.bevel_network. */
export interface PolyRoundEdgesV1Params {
  /** Bevel size in metres; clamped so a bevel never overlaps itself (required; metres, 0.000001..1000000) */
  width: number | Node<'Number'>;
  /** Segments across each bevel: 1 is a flat chamfer, more round it (default 3; whole number 1..16) */
  segments?: number | Node<'Number'>;
  /** The least turn between two faces' normals, in radians, for their edge to count as hard (0.5 is about 30 degrees) (default 0.5; an angle, 0..3.141592653589793) */
  angle?: number | Node<'Number'>;
}

/** A JavaScript program builds a polygon mesh, for shapes the other nodes cannot describe (a loop with branches, a maze, a rule that grows geometry). Make a `Mesh`: `const m = new Mesh(); const a = m.vertex(x, y, z); ... */
export interface PolyScriptV1Params {
  /** The JavaScript program, at most 16,384 bytes. Its last expression is the result (has a default; text up to 16384 bytes) */
  code?: string;
  /** Seed of the script's Math.random() (default 1; whole number 0..4294967295) */
  seed?: number | Node<'Number'>;
  /** How much work the script may do before it is stopped, in units of about 10,000 steps (default 2000; whole number 1..20000) */
  budget?: number | Node<'Number'>;
}

/** Sculpts a mesh with brush strokes, like clay: write the strokes as text, separated by semicolons, and they are applied in order. A stroke is `brush cx cy cz radius strength` (the point on or near the surface in metres, how far the brush reaches, how hard it works); the grab brush is `grab cx cy cz radius dx dy dz` (the vector to move by). */
export interface PolySculptV1Params {
  /** The strokes, `brush cx cy cz radius strength;` separated by semicolons (grab takes a vector instead of a strength) (default "draw 0 0 0 0.3 0.05"; text up to 4096 bytes) */
  strokes?: string;
  /** Measure brush distance through space (`sphere`) or along connected mesh edges (`surface`). Surface mode avoids sculpting through the back of a head, limb or thin shell; weld seams first if the brush should cross them (default "sphere") */
  falloff?: 'sphere' | 'surface';
  /** Mirror every stroke about the plane through the origin that this axis is perpendicular to (x: left and right) (default "none") */
  symmetry?: 'none' | 'x' | 'y' | 'z';
}

/** Chooses faces, edges, vertices or boundary loops of a polygon mesh by a rule, for the edit operators. Choosing nothing refuses (selection.lost); expect=one refuses when several match (selection.ambiguous). */
export interface PolySelectV1Params {
  /** What to choose (default "face") */
  element?: 'face' | 'edge' | 'vertex' | 'loop';
  /** The rule: all; normal (faces facing a direction); box (centre inside a box); attribute (faces or points with a tagged value); sharp (edges between faces turned apart by an angle); height (centre at one coordinate along an axis, give height and tolerance); extreme (the highest or lowest elements along an axis, give side) (default "all") */
  by?: 'all' | 'normal' | 'box' | 'attribute' | 'sharp' | 'height' | 'extreme';
  /** For by=normal: the direction the faces face (any length) (default [0, 1, 0]; -1000000..1000000 each) */
  direction?: Vec3;
  /** For by=normal: how far a face may turn from the direction. For by=sharp: the least angle between the two faces' normals (default 0.1; an angle, 0..3.141592653589793) */
  angle?: number | Node<'Number'>;
  /** For by=height and by=extreme: the axis the coordinate is read along (y is up) (default "y") */
  axis?: 'x' | 'y' | 'z';
  /** For by=height: the coordinate along the axis, in metres. Faces, edges and loops are tested at their centre (default 0; metres, -1000000..1000000) */
  height?: number | Node<'Number'>;
  /** For by=height and by=extreme: how far from the target coordinate still counts, in metres (default 0.0001; metres, 0..1000000) */
  tolerance?: number | Node<'Number'>;
  /** For by=extreme: max chooses the elements furthest along the axis (the top for y), min the ones furthest back (the bottom) (default "max") */
  side?: 'max' | 'min';
  /** For by=box: the lower corner. Faces, edges and loops are tested at their centre (default [-100000, -100000, -100000]; metres, -1000000..1000000 each) */
  box_min?: Vec3;
  /** For by=box: the upper corner (default [100000, 100000, 100000]; metres, -1000000..1000000 each) */
  box_max?: Vec3;
  /** For by=attribute: the name of an integer face or point attribute (region is the colour region a solid came from) (default "region"; text up to 64 bytes) */
  attribute?: string;
  /** For by=attribute: the value to match (default 0; whole number 0..2147483647) */
  value?: number | Node<'Number'>;
  /** any accepts one or more matches; one refuses with selection.ambiguous when several match (default "any") */
  expect?: 'any' | 'one';
}

/** Skins a skeleton into ONE closed surface of mostly quads (Blender's Skin modifier, the B-Mesh method): give the joints of a body, an animal or a machine frame, each with a position and two radii, and the bones between them, and get a smooth, clean base mesh with proper junctions at every hip, shoulder and branch. Subdivide it with poly.subdivide and shape it with poly.program or poly.sculpt. */
export interface PolySkinV1Params {
  /** The joints: `name x y z rx rz` separated by `;`, plus the word `root` on one joint of each connected piece. At most 4,096 joints; names are letters, digits and underscores (has a default; text up to 16384 bytes) */
  joints?: string;
  /** The bones: `a-b` by joint name, separated by `;` (default "root-tip"; text up to 16384 bytes) */
  bones?: string;
  /** Relaxes the surface around branches (0 none, 1 full). Rounds hips and shoulders; costs a little detail (default 0; a ratio, 0..1) */
  branch_smoothing?: number | Node<'Number'>;
  /** Axes of mirror symmetry in the skeleton; keeps quads from crossing the mirror plane so a symmetric figure skins symmetrically. Use `x` for a body (default "none") */
  symmetry?: 'none' | 'x' | 'y' | 'z' | 'xy' | 'xz' | 'yz' | 'xyz';
  /** Add joints along long bones so the cross-section rings stay about one radius apart (Blender's behaviour). Turn off to get exactly one ring per joint (default true) */
  subdivide?: boolean;
}

/** Evens out a surface by moving every vertex toward the average of its neighbours, a little each pass. Rounds lumpy meshes; too many passes shrinks and softens detail. */
export interface PolySmoothV1Params {
  /** How far a vertex moves toward its neighbours each pass: 0 nothing, 1 all the way (default 0.5; 0..1) */
  factor?: number | Node<'Number'>;
  /** Passes, 1 to 50 (default 1; whole number 1..50) */
  iterations?: number | Node<'Number'>;
}

/** Gives a surface thickness: adds a second surface offset along the normals and joins the two at open edges. With no selection every face is thickened. */
export interface PolySolidifyV1Params {
  /** Distance along each face's normal in metres, not zero (negative goes inward) (default 0.05; metres, -1000000..1000000) */
  thickness?: number | Node<'Number'>;
}

/** Moves the vertices of the selected edge loops to even spacing along each loop's own curve, without changing its shape (select the loop's edges with poly.select element=edge, for example by=height on a tube). Evens out a ring after an uneven cut. */
export interface PolySpaceLoopsV1Params {
  /** How far toward even spacing: 0 nothing, 1 fully even (default 1; 0..1) */
  factor?: number | Node<'Number'>;
  /** Follow the loop with a smooth curve or straight segments (default "cubic") */
  interpolation?: 'cubic' | 'linear';
}

/** Revolves selected faces or edges, or the whole mesh, around an axis. */
export interface PolySpinV1Params {
  /** Point on the axis in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  center?: Vec3;
  /** Axis direction; must be non-zero (default [0, 1, 0]; -1000000..1000000 each) */
  axis?: Vec3;
  /** Rotation angle in radians (default 6.283185307179586; 0.001..6.283185307179586) */
  angle?: number | Node<'Number'>;
  /** Number of rotation segments (default 32; whole number 2..256) */
  steps?: number | Node<'Number'>;
  /** Weld the seam for a full turn and vertices within distance (default true) */
  merge?: boolean;
  /** Maximum merge distance in metres (default 0.0001; metres, 0.000000001..1000) */
  distance?: number | Node<'Number'>;
}

/** Disconnects the faces along the selected edges: each side gets its own vertices, so the edge becomes a hard seam that shades sharp, can take a UV seam, and lets the two sides move apart (select edges with poly.select element=edge, for example by=sharp). */
export interface PolySplitEdgesV1Params {
}

/** Final-step uniform Catmull-Clark subdivision of a polygon mesh. Preserve and edit the cage before subdivision: v1 polygon edits may refuse curved refined faces and bevel networks. */
export interface PolySubdivideV1Params {
  /** Uniform Catmull-Clark refinement levels (1–4) (default 1; whole number 1..4) */
  levels?: number | Node<'Number'>;
  /** Sharpness applied to selected crease edges (0 smooth, 10 fully sharp) (default 10; 0..10) */
  sharpness?: number | Node<'Number'>;
  /** Sharpness applied to selected corner vertices (0 smooth, 10 fully sharp) (default 10; 0..10) */
  corner_sharpness?: number | Node<'Number'>;
  /** Boundary interpolation rule for open meshes (default "edges_and_corners") */
  boundary?: 'edges' | 'edges_and_corners';
}

/** Cuts the selected edges into pieces and splits the faces around them: adds loops and detail where you ask (select edges with poly.select element=edge). */
export interface PolySubdivideEdgesV1Params {
  /** Cuts per edge (default 1; whole number 1..8) */
  cuts?: number | Node<'Number'>;
  /** How much the new points are pushed to smooth the surface (default 0; 0..1) */
  smooth?: number | Node<'Number'>;
  /** Fill quads with a grid of quads (default true) */
  grid_fill?: boolean;
}

/** A surface written as a program of `u` and `v`, each from 0 to 1 across `columns` by `rows` quads (the mesh has UVs, drawn smooth). `out` is the point: a torus is `out = vec3((1.0 + 0.3 * cos(v * 6.2832)) * cos(u * 6.2832), 0.3 * sin(v * 6.2832), (1.0 + 0.3 * cos(v * 6.2832)) * sin(u * 6.2832))`. */
export interface PolySurfaceExprV1Params {
  /** The program. The last statement sets `out`, a vec3 (a point in metres) (default "out = vec3(u, 0.0, v)"; text up to 8192 bytes) */
  code?: string;
  /** Quads along u (default 32; whole number 1..512) */
  columns?: number | Node<'Number'>;
  /** Quads along v (default 32; whole number 1..512) */
  rows?: number | Node<'Number'>;
  /** Reverse which side of the surface is the front. The front follows from the formula; if the surface looks inside out (dark, or seen through), set this (default false) */
  flip?: boolean;
}

/** Keeps one side of an axis plane through the origin and mirrors it onto the other, so a half model becomes a symmetric whole. */
export interface PolySymmetrizeV1Params {
  /** The axis the mirror plane is perpendicular to (default "x") */
  axis?: 'x' | 'y' | 'z';
  /** Which side is kept and copied onto the other (default "positive") */
  keep?: 'positive' | 'negative';
  /** Vertices closer than this to the plane, or to their mirror, are merged (default 0.0001; metres, 0.000001..1000000) */
  distance?: number | Node<'Number'>;
}

/** Turns a closed polygon mesh back into a solid (faces are triangulated properly, never fanned). Refuses an open or non-manifold mesh. */
export interface PolyToSolidV1Params {
}

/** Splits selected polygons into triangles using Blender's beauty triangulation; with no selection, triangulates every face. */
export interface PolyTriangulateV1Params {
}

/** Reverses a subdivision: removes the vertices a regular grid of quads could be rebuilt without. Use it to get a coarse cage back from a smooth, dense mesh. */
export interface PolyUnsubdivideV1Params {
  /** How many levels to undo (default 1; whole number 1..4) */
  iterations?: number | Node<'Number'>;
}

/** Unwraps a polygon mesh to UV coordinates (corner attributes u and v) with Blender's LSCM or angle-based flattening and packs the islands into the unit square. Select seam edges with poly.select element=edge (for example by=sharp on a box) to cut a closed shape into islands; an already open mesh needs none. */
export interface PolyUnwrapUvV1Params {
  /** conformal is fast and keeps angles; angle (ABF++) distorts angles less on curved shapes but is slower (default "conformal") */
  method?: 'conformal' | 'angle';
  /** Pack the islands into the unit square; otherwise they overlap near the origin (default true) */
  pack?: boolean;
  /** Let packing rotate islands to fit tighter (default false) */
  rotate?: boolean;
  /** Space between packed islands, as a fraction of the unit square (default 0.02; 0..0.5) */
  margin?: number | Node<'Number'>;
  /** When no seam edges are selected, cut automatically: the surface is split into charts whose faces stay within seam_angle of the chart's first face (a box gives six, a sphere several near-flat patches), plus paths that open closed or tube-shaped pieces. Selected seams are always used as given (default true) */
  auto_seams?: boolean;
  /** Width of a chart's cone when auto_seams is on (1.1 rad is about 63 degrees): smaller gives more, flatter charts and less stretch but more seams; larger gives fewer charts (default 1.1; an angle, 0..3.141592653589793) */
  seam_angle?: number | Node<'Number'>;
}

/** Turns the UV layout into a flat mesh you can look at: every face is drawn where its UVs put it on the XY plane (u to the right, v up), so islands show as separate patches, seams as gaps, overlaps as faces drawn on top of each other, stretching as faces that look nothing like their 3D shape. A face whose UVs run clockwise shows its back. */
export interface PolyUvLayoutV1Params {
  /** Width of the whole 0 to 1 UV square in metres (default 1; metres, 0.001..1000) */
  size?: number | Node<'Number'>;
}

/** Moves, scales and turns UV coordinates: shift a layout, shrink it, rotate an island. With no selection it changes every UV; with a selection of faces (poly.select element=face, for example by=attribute to take one region) it changes only those faces' UVs, which is how one island is moved out of the way of another or scaled to a different texel density. */
export interface PolyUvTransformV1Params {
  /** Shift along U, applied last (1 is a whole texture width) (default 0; -100..100) */
  offset_u?: number | Node<'Number'>;
  /** Shift along V, applied last (default 0; -100..100) */
  offset_v?: number | Node<'Number'>;
  /** Scale about the pivot (default 1; 0.001..1000) */
  scale?: number | Node<'Number'>;
  /** Turn about the pivot, counter-clockwise in UV space (default 0; an angle, -6.283185307179586..6.283185307179586) */
  rotate?: number | Node<'Number'>;
  /** What scale and rotate pivot about: the centre of the affected UVs, or the UV origin (0, 0) (default "center") */
  pivot?: 'center' | 'origin';
}

/** Merges vertices closer than a distance. Refuses when a face would collapse. */
export interface PolyWeldV1Params {
  /** Merge distance in metres (default 0.0001; metres, 0.000001..1000000) */
  distance?: number | Node<'Number'>;
}

/** Turns faces (all when none are selected) into a lattice of struts of a given thickness, like a cage or grille. */
export interface PolyWireframeV1Params {
  /** Strut thickness in metres (required; metres, 0.000001..1000000) */
  thickness: number | Node<'Number'>;
  /** Offset of the struts along the normal, in metres (default 0; metres, -1000000..1000000) */
  offset?: number | Node<'Number'>;
  /** Scale thickness so struts keep an even width at corners (default true) */
  even?: boolean;
  /** Scale thickness by the surrounding geometry (default false) */
  relative?: boolean;
  /** Also make struts along open boundaries (default true) */
  boundary?: boolean;
  /** Remove the original faces (default true) */
  replace?: boolean;
}

/** A regular polygon approximating a circle, centred on the origin of the XY plane. */
export interface ProfileCircleV1Params {
  /** Radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Facets around the axis (default 32; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** The first profile minus all the others. */
export interface ProfileDifferenceV1Params {
}

/** Rounds convex corners of a planar profile while preserving its original outer bounds. */
export interface ProfileFilletV1Params {
  /** Corner radius in metres; the eroded core must remain nonempty (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
}

/** Grows (positive) or shrinks (negative) a profile, with mitred or round corners. */
export interface ProfileOffsetV1Params {
  /** Distance in metres; an erosion that removes everything gives an empty profile (required; metres, -1000000..1000000) */
  delta: number | Node<'Number'>;
  /** Corner treatment: miter keeps sharp corners (limit 2); round arcs them (64 segments per circle) (default "miter") */
  join?: 'miter' | 'round';
}

/** A closed polygon from points in the XY plane. */
export interface ProfilePolygonV1Params {
  /** Corner points [x, y] in metres (required; 3 to 4096 points) */
  points: Vec2[];
}

/** A rectangle centred on the origin of the XY plane. */
export interface ProfileRectV1Params {
  /** Width and height in metres (required; metres, 0.000001..1000000 each) */
  size: Vec2;
}

/** Rounds every corner of a profile into a smooth curve by repeated corner cutting. */
export interface ProfileSmoothV1Params {
  /** Each round doubles the points and halves the sharpness: 2 is gentle, 4 is very smooth. The shape shrinks slightly inward at the corners (default 2; whole number 1..5) */
  iterations?: number | Node<'Number'>;
  /** Set true for a profile you will revolve: points on the axis (x = 0) stay put, so the base stays flat and the axis edge is not trimmed (default false) */
  keep_axis?: boolean;
}

/** Makes this shape a TARGET that ONE bone part turns to look at (a head, an eye, a turret, a searchlight), in play and in the viewer. `bone` is the OUTPUT name of a part that carries rig.bone: the bone's own direction, from its head to its tail at rest, is what points at the target, and `up` (a direction at rest, default [0, 1, 0]) is the side that stays towards the `pole` so a head does not roll. */
export interface RigAimV1Params {
  /** Output name of the bone part that looks at this target, for example "head" (required; text up to 64 bytes) */
  bone: string;
  /** The bone's own up direction at rest (the top of a head is [0, 1, 0]) (default [0, 1, 0]; -1000..1000 each) */
  up?: Vec3;
  /** The world direction the bone's up leans towards while looking: [0, 1, 0] keeps a head upright. [0, 0, 0] keeps the current up (default [0, 1, 0]; -1000..1000 each) */
  pole?: Vec3;
  /** Extra roll around the line of sight, in degrees (a head tilted while looking) (default 0; an angle, -6.3..6.3) */
  twist?: number | Node<'Number'>;
  /** How much of the turn to apply: 0.5 looks half-way towards the target (a lazy glance) (default 1; a ratio, 0..1) */
  weight?: number | Node<'Number'>;
}

/** Marks a part as a bone of a skeleton and records its rest segment, from `head` (the joint it turns about) to `tail`, in world coordinates. Bones are ordinary parts: move them with anim.track, anim.wave and behaviours, and join them with anim.parent (a forearm bone follows an upper-arm bone). */
export interface RigBoneV1Params {
  /** Where the bone starts at rest: its joint, in world coordinates (required; metres, -1000..1000 each) */
  head: Vec3;
  /** Where the bone ends at rest, in world coordinates. Must differ from head (required; metres, -1000..1000 each) */
  tail: Vec3;
}

/** Makes a foot target WALK. Put it after rig.ik on the shape that the leg's bone chain reaches for: instead of you animating that target, it plants on the ground while its body passes over it and swings forward in an arc to the next step (a procedural gait from the body's own motion; Spore did the same for any creature). */
export interface RigGaitV1Params {
  /** Output name of the body part this leg is attached to; its motion drives the walk (required; text up to 64 bytes) */
  body: string;
  /** Where in the cycle this foot starts: alternate legs 0 and 0.5 (default 0; 0..1) */
  phase?: number | Node<'Number'>;
  /** Metres the body travels per full step cycle of this foot: longer is a lazier stride (default 0.6; metres, 0.05..20) */
  stride_length?: number | Node<'Number'>;
  /** How high the foot lifts while swinging (default 0.15; metres, 0..5) */
  step_height?: number | Node<'Number'>;
  /** The share of the cycle the foot is planted: 0.6 walks, 0.4 runs, 0.8 creeps (default 0.6; 0.1..1) */
  duty?: number | Node<'Number'>;
  /** The farthest the foot may be from the hip; 0 uses the length of the leg (default 0; metres, 0..50) */
  reach?: number | Node<'Number'>;
  /** Height of the ground the feet plant on (default 0; metres, -1000..1000) */
  ground?: number | Node<'Number'>;
}

/** Makes this shape a TARGET that a chain of bones reaches for (inverse kinematics): in play and in the viewer, an arm, leg or tentacle built of bone parts bends so its tip follows the target as the target moves (animate it, or make it grabbable and carry it). `bones` lists the OUTPUT names of the bone parts from root to tip; each must carry rig.bone, be anim.parent-ed to the one before, and start where it ends. */
export interface RigIkV1Params {
  /** Output names of the bone parts, root to tip, separated by semicolons (1 to 8), for example "upper_arm; forearm; hand" (required; text up to 256 bytes) */
  bones: string;
  /** Solver passes per frame. More passes reach a target near full stretch more exactly (default 32; whole number 1..64) */
  iterations?: number | Node<'Number'>;
  /** ccd only: the most any one bone may turn from its rest pose, in degrees (180 is no limit). A tentacle that must not fold double uses 60; a tail 40. A target the limits make unreachable is approached as far as they allow (default 180; an angle, 1..180) */
  limit?: number | Node<'Number'>;
  /** ccd: any chain of 1 to 8 bones (tentacles, tails). two_bone: exactly two bones, an arm or a leg, solved exactly with a pole so the elbow or knee bends the way you say (no flipping), with soften and twist (default "ccd") */
  solver?: 'ccd' | 'two_bone';
  /** two_bone only: the world direction the elbow or knee bends towards (a knee points forward [0, 0, 1], an elbow back [0, 0, -1] or out). [0, 0, 0] keeps the bend it has at rest (default [0, 0, 0]; -1000..1000 each) */
  pole?: Vec3;
  /** two_bone only: 0 snaps the limb straight at full stretch; 0.2 to 0.5 eases it in before it locks, which avoids the popping knee. 1 eases over the whole reach (default 0; a ratio, 0..1) */
  soften?: number | Node<'Number'>;
  /** two_bone only: extra swing of the elbow or knee around the line from shoulder to target, in degrees (default 0; an angle, -6.3..6.3) */
  twist?: number | Node<'Number'>;
}

/** Adds one Blender-style shape key to a character mesh. Make the target by duplicating the base and sculpting or moving its points; point count and face topology must stay identical. */
export interface RigMorphV1Params {
  /** Unique name for this shape key, such as smile, blink_left or brow_raise (required; text up to 32 bytes) */
  name: string;
  /** Current blend from neutral (0) to target (1); negative values invert the sculpted delta (default 1; a ratio, -1..1) */
  weight?: number | Node<'Number'>;
}

/** Gives a chain of bones secondary motion: a tail, ear, antenna, hair or cloth edge that lags behind the body and springs back, from a spring simulation run every frame in the viewer (inertia, damping, gravity and a pull back to the rest pose). The shape you attach it to only carries it (use the tip bone). */
export interface RigSecondaryV1Params {
  /** Output names of the bone parts, root to tip, separated by semicolons (1 to 8), for example "tail1; tail2; tail3" (required; text up to 256 bytes) */
  bones: string;
  /** How hard the chain springs back to its rest pose: 0 hangs free (hair, a loose rope), 0.3 a tail, 1 stiff (default 0.3; 0..1) */
  stiffness?: number | Node<'Number'>;
  /** Drag per second: low swings on for a long time, high stops quickly (default 4; 0..50) */
  damping?: number | Node<'Number'>;
  /** How hard the chain is pulled down, in m/s^2: 9.8 is normal, 0 floats (tentacles under water) (default 9.8; 0..50) */
  gravity?: number | Node<'Number'>;
}

/** Makes a shape bend with moving bones: each vertex follows the bones nearest to it (a blend, so a tube bends smoothly at a joint). `bones` lists the OUTPUT names of parts that carry rig.bone. */
export interface RigSkinV1Params {
  /** Output names of the bone parts, separated by semicolons (at most 64), for example "upper_arm; forearm" (required; text up to 1024 bytes) */
  bones: string;
  /** How sharply a bone's influence drops with distance. Higher gives tighter, more local bending; 1 blends broadly (default 3; whole number 1..8) */
  falloff?: number | Node<'Number'>;
  /** Most bones allowed to move one vertex (1 makes every vertex follow a single bone, which gives rigid pieces) (default 4; whole number 1..4) */
  influences?: number | Node<'Number'>;
  /** How bones combine on a vertex. linear is fast but a strong twist pinches a limb thin; dual_quaternion keeps its volume through twists. The glTF export is always linear (default "linear") */
  blend?: 'linear' | 'dual_quaternion';
  /** Brush stamps that correct the automatic weights, separated by semicolons (at most 64): "bone x y z radius strength". strength 1 hands vertices under the brush centre to that bone, -1 takes the bone away from them, fading smoothly to nothing at the radius. Example: "forearm 0 0.5 0 0.2 1; upper_arm 0 0.5 0 0.2 -1" (default ""; text up to 2048 bytes) */
  paint?: string;
}

/** Cuts a river channel into a terrain.tile along a path: a bed below the ground, banks that blend back into the land, and the data the water needs (flow direction and speed, water height, channel and bank layers). The path is a list of world x,z points written as text, in the direction the water flows, for example path="10,64; 60,70; 118,64"; give every tile of a world the same path and the river continues across tiles. */
export interface RiverCarveV1Params {
  /** The river's course as `x,z; x,z; ...` in world metres, downstream order, at least two points, for example "0,64; 100,80; 256,64". It is smoothed into a curve (default ""; text up to 4000 bytes) */
  path?: string;
  /** local follows the ground beside each cell and lets tiles of one world agree; descending makes water fall steadily along the whole path and stay level across the river, suited to terraced, ridged or mountainous ground and lakes in valleys (build it on the whole grid) (default "local") */
  level?: 'local' | 'descending';
  /** Width of the water and channel floor in metres: 3 a stream, 8 a river, 30 a great river (default 8; metres, 0.5..500) */
  width?: number | Node<'Number'>;
  /** How far the bed lies below the surrounding ground in metres (default 2.5; metres, 0.1..100) */
  depth?: number | Node<'Number'>;
  /** Steepness of the banks in DEGREES: 20 gentle, 35 natural, 70 a gorge (default 35; 5..85) */
  bank_slope?: number | Node<'Number'>;
  /** Extra width on each side over which the banks blend into the land (default 6; metres, 0..200) */
  bank_width?: number | Node<'Number'>;
  /** How much the valley floor is smoothed across the channel (default 0.3; a ratio, 0..1) */
  flatten?: number | Node<'Number'>;
  /** How far the water surface lies below the bank top in metres (default 0.4; metres, 0..20) */
  water_offset?: number | Node<'Number'>;
}

/** The water surface of the rivers carved into a terrain.tile by river.carve: a smooth sheet at the water height with flow direction, depth and shore foam carried per vertex, drawn with a flowing, depth-tinted, foamy water look that needs no texture. It fades to nothing where it meets the bank, so it has no hard edge. */
export interface RiverWaterV1Params {
  /** Main wave travel direction in degrees; the per-vertex river current still drives streaming (default 0; an angle, -6.283185307179586..6.283185307179586) */
  direction?: number | Node<'Number'>;
  /** Repeatable wave pattern seed (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Colour of clear shallow water [r, g, b] (sRGB) (default [0.16, 0.42, 0.45]; 0..1 each) */
  shallow?: Vec3;
  /** Colour of deep water (default [0.02, 0.12, 0.2]; 0..1 each) */
  deep?: Vec3;
  /** How many metres of depth it takes to go from the shallow to the deep colour: small is murky, large is clear (default 3; metres, 0.2..50) */
  clarity?: number | Node<'Number'>;
  /** How fast the surface pattern streams along the current (1 is natural) (default 1; a ratio, 0..4) */
  flow?: number | Node<'Number'>;
  /** Strength of the small waves and glints on the surface (default 0.5; a ratio, 0..2) */
  ripple?: number | Node<'Number'>;
  /** How much white foam forms at the banks and in fast water (default 0.5; a ratio, 0..1) */
  foam?: number | Node<'Number'>;
  /** How opaque the water is in its deepest parts (default 0.85; a ratio, 0.1..1) */
  opacity?: number | Node<'Number'>;
}

/** A building from a list of rooms and the links between them: it lays the rooms out around a corridor (every room gets a door onto it, every link is a short walk), then makes a floor slab under every room and corridor, a wall along every edge, and an opening where each door is. There is no roof, so the inside shows and can be lit. */
export interface RoomsPlanV1Params {
  /** The rooms: `name:minWidth,minDepth,maxWidth,maxDepth` in metres, separated by semicolons (1 to 64 rooms) (has a default; text up to 2048 bytes) */
  rooms?: string;
  /** Pairs of room numbers that must be a short walk apart, like `0-1; 1-2`; together they must connect every room (default "0-1; 0-2"; text up to 1024 bytes) */
  links?: string;
  /** Chooses the room sizes, their order along the corridor and where the doors are (default 1; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Height of the walls in metres (default 2.8; metres, 1..10) */
  wall_height?: number | Node<'Number'>;
  /** Thickness of the walls in metres (default 0.15; metres, 0.05..1) */
  wall?: number | Node<'Number'>;
  /** Narrowest corridor in metres (default 1.6; metres, 0.8..4) */
  corridor_width?: number | Node<'Number'>;
  /** Narrowest door in metres (default 1; metres, 0.6..2.5) */
  door_width?: number | Node<'Number'>;
}

/** Plants copies of a shape (a tree, a rock, a tuft of grass) over a terrain.tile: blue-noise spacing so they never clump or line up, the same plants in the same places whichever tile you build first, and each copy stands on the ground at its own height. Say where they grow with a mask layer made by terrain.mask (density follows the layer), a height window and a steepest slope; vary size and facing with scale and spin. */
export interface ScatterPlaceV1Params {
  /** Smallest distance between two copies in metres: about a tree's crown width for trees, a few centimetres more than the shape's width for grass (default 6; metres, 0.2..500) */
  radius?: number | Node<'Number'>;
  /** How much of the possible ground is planted: 1 is as full as the spacing allows, 0.3 a sparse scatter (default 1; a ratio, 0..1) */
  density?: number | Node<'Number'>;
  /** Picks a different arrangement; the same seed gives the same plants on every tile and platform (default 0; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Name of a terrain.mask layer that sets where they grow: density follows it (1 full, 0 none). Empty plants everywhere the other limits allow (default ""; text up to 32 bytes) */
  layer?: string;
  /** Lowest ground height that is planted, in metres (default -1000; metres, -1000..20000) */
  min_height?: number | Node<'Number'>;
  /** Highest ground height that is planted, in metres (a tree line) (default 20000; metres, -1000..20000) */
  max_height?: number | Node<'Number'>;
  /** Steepest ground that is planted, in DEGREES from flat: 30 for trees and grass, 60 for clinging shrubs and rocks (default 90; 0..90) */
  max_slope?: number | Node<'Number'>;
  /** Smallest and largest size multiplier, picked per copy (default [0.8, 1.2]; 0.01..100 each) */
  scale?: Vec2;
  /** Nearest camera distance in metres at which copies are drawn; 0 draws them from any distance (default 0; metres, 0..20000) */
  show_from?: number | Node<'Number'>;
  /** Farthest camera distance in metres at which copies are drawn; 0 means no limit. Must be 0 or greater than show_from (default 0; metres, 0..20000) */
  show_to?: number | Node<'Number'>;
  /** Width of the cross-fade at each end of the band, in metres; 0 switches abruptly. Neighbouring bands should use the same fade and meet in the middle of it (default 0; metres, 0..200) */
  show_fade?: number | Node<'Number'>;
  /** How much each copy leans with the ground: 0 stands upright (trees), 1 lies flat on the slope (rocks, moss) (default 0; a ratio, 0..1) */
  align?: number | Node<'Number'>;
  /** Turn each copy by a random angle about its own up axis (default true) */
  spin?: boolean;
  /** Push copies down into the ground by this many metres (rocks look bedded in at 0.1 to 0.3) (default 0; metres, -10..10) */
  sink?: number | Node<'Number'>;
  /** Shares of the planting between variants, as weights separated by commas: `0.5, 0.3, 0.2` splits the plants 50/30/20 between three shapes. Each variant is its own scatter.place with the same radius, density, seed and ground, the same `mix`, and its own `pick`; every plant goes to exactly one variant, so variants never overlap. The default `1` means this node plants everything (default "1"; text up to 120 bytes) */
  mix?: string;
  /** Which share of `mix` this node plants, counting from 0: with `mix` of `0.5, 0.3, 0.2`, pick 0 plants the half, pick 1 the three tenths, pick 2 the rest (default 0; whole number 0..15) */
  pick?: number | Node<'Number'>;
}

/** Sky and ground ambient irradiance, approximated as a low-order hemisphere; set gi=true for traced light probes (global illumination) instead. */
export interface SceneEnvironmentV1Params {
  /** Diffuse light from above, in sRGB (default [0.68, 0.76, 0.9]; 0..1 each) */
  sky_color?: Vec3;
  /** Diffuse light from below, in sRGB (default [0.46, 0.39, 0.3]; 0..1 each) */
  ground_color?: Vec3;
  /** Ambient brightness multiplier (default 0.15; 0..8) */
  intensity?: number | Node<'Number'>;
  /** Strength of the reflected studio sky for metals and gloss (default 0; 0..1) */
  reflection?: number | Node<'Number'>;
  /** Warm ground-bounce strength added to downward-facing surfaces (default 0; 0..1) */
  bounce?: number | Node<'Number'>;
  /** Ground-bounce colour, in sRGB (default [0.6, 0.44, 0.28]; 0..1 each) */
  bounce_color?: Vec3;
  /** How thick the distance haze is: 0 is clear air, 0.2 a light morning mist, 1 a thick fog (default 0; 0..1) */
  haze?: number | Node<'Number'>;
  /** Colour of the distance haze, in sRGB (default [0.72, 0.78, 0.86]; 0..1 each) */
  haze_color?: Vec3;
  /** Height in metres over which ground haze thins: 0 makes it uniform; above 0 it is thickest at the ground (default 0; metres, 0..10000) */
  haze_height?: number | Node<'Number'>;
  /** How visible the sun's light shafts are through the air: above 0 the sun scatters through it (rays past trees and walls), drawn by volumetric fog. Needs a sun with shadows (default 0; 0..1) */
  light_shafts?: number | Node<'Number'>;
  /** Depth of field: the distance in metres the camera focuses on; what is nearer or farther blurs. 0 turns it off (default 0; metres, 0..100000) */
  focus_distance?: number | Node<'Number'>;
  /** Lens aperture in f-stops for depth of field: 1.4 is a very shallow focus, 2.8 portrait-like, 8 or more nearly everything sharp (default 2.8; 0.5..22) */
  aperture?: number | Node<'Number'>;
  /** Motion blur strength: 0 is off, 0.5 a film shutter, 1 a long smear. Things that move fast across the screen streak (default 0; 0..1) */
  motion_blur?: number | Node<'Number'>;
  /** Short-range shadows traced in screen space: they close the gap under feet, props and pebbles that the sun's shadow map misses (default false) */
  contact_shadows?: boolean;
  /** Light the scene with traced light probes (global illumination) instead of the constant sky ambient: the sky as each place sees it (sheltered corners are darker), and sun and glowing surfaces bounced off other surfaces, so walls pick up the colour of what is next to them. It is calculated a little at a time after each edit, and the scene looks as before until it is done. Not seen by anything that is not drawn in the viewer (default false) */
  gi?: boolean;
  /** Distance between light probes in metres: 0 sizes it from the scene (about 24 probes across its longest side). Smaller captures finer light and takes longer; at most 64 probes along an axis (default 0; metres, 0..64) */
  gi_spacing?: number | Node<'Number'>;
  /** How many times light bounces between surfaces: 1 is sky, sun and glow bounced once, 2 or more also light bounced off bounced light (default 2; whole number 1..4) */
  gi_bounces?: number | Node<'Number'>;
  /** Rays traced per probe for each bounce: more is smoother and slower (default 128; whole number 32..512) */
  gi_rays?: number | Node<'Number'>;
  /** Direction the wind blows toward, in degrees around the vertical axis: 0 is +x, 90 is +z (default 0; -360..360) */
  wind_direction?: number | Node<'Number'>;
  /** How hard the wind blows: 0 is still air, 0.5 a gentle breeze, 1 a strong breeze, 2 a gale. It moves only foliage with sway or flutter (solid.foliage); the viewer shows it, an exported glTF does not (default 0.5; 0..2) */
  wind_strength?: number | Node<'Number'>;
}

/** Procedural analytic sky with a sun, atmosphere gradient and optional deterministic clouds. Supplies an approximate ambient hemisphere and sun light; a separate scene.environment overrides its ambient lighting while keeping this background. */
export interface SceneSkyV1Params {
  /** Sun angle above the horizon in degrees; negative values place it just below the horizon (default 35; -5..90) */
  sun_elevation?: number | Node<'Number'>;
  /** Sun compass angle in degrees around the vertical axis (default 0; -180..180) */
  sun_azimuth?: number | Node<'Number'>;
  /** Direct sunlight brightness; drives the scene's shadow-casting sun (default 3; 0..100) */
  sun_intensity?: number | Node<'Number'>;
  /** Atmospheric haze; high values warm and soften the horizon (default 3; 1..10) */
  turbidity?: number | Node<'Number'>;
  /** Procedural horizon ground colour in sRGB (default [0.37, 0.29, 0.2]; 0..1 each) */
  ground_color?: Vec3;
  /** Draw a few-octave procedural cloud layer (default false) */
  clouds?: boolean;
  /** Coverage threshold for the seeded cloud layer (default 0.35; 0..1) */
  cloud_cover?: number | Node<'Number'>;
  /** Picks a deterministic cloud pattern (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** How thick the distance haze is: 0 is clear air, 0.2 a light morning mist, 1 a thick fog (default 0; 0..1) */
  haze?: number | Node<'Number'>;
  /** Colour of the distance haze, in sRGB (default [0.72, 0.78, 0.86]; 0..1 each) */
  haze_color?: Vec3;
  /** Height in metres over which ground haze thins: 0 makes it uniform; above 0 it is thickest at the ground (default 0; metres, 0..10000) */
  haze_height?: number | Node<'Number'>;
  /** How visible the sun's light shafts are through the air: above 0 the sun scatters through it (rays past trees and walls), drawn by volumetric fog. Needs a sun with shadows (default 0; 0..1) */
  light_shafts?: number | Node<'Number'>;
  /** Depth of field: the distance in metres the camera focuses on; what is nearer or farther blurs. 0 turns it off (default 0; metres, 0..100000) */
  focus_distance?: number | Node<'Number'>;
  /** Lens aperture in f-stops for depth of field: 1.4 is a very shallow focus, 2.8 portrait-like, 8 or more nearly everything sharp (default 2.8; 0.5..22) */
  aperture?: number | Node<'Number'>;
  /** Motion blur strength: 0 is off, 0.5 a film shutter, 1 a long smear. Things that move fast across the screen streak (default 0; 0..1) */
  motion_blur?: number | Node<'Number'>;
  /** Short-range shadows traced in screen space: they close the gap under feet, props and pebbles that the sun's shadow map misses (default false) */
  contact_shadows?: boolean;
  /** Light the scene with traced light probes (global illumination) instead of the constant sky ambient: the sky as each place sees it (sheltered corners are darker), and sun and glowing surfaces bounced off other surfaces, so walls pick up the colour of what is next to them. It is calculated a little at a time after each edit, and the scene looks as before until it is done. Not seen by anything that is not drawn in the viewer (default false) */
  gi?: boolean;
  /** Distance between light probes in metres: 0 sizes it from the scene (about 24 probes across its longest side). Smaller captures finer light and takes longer; at most 64 probes along an axis (default 0; metres, 0..64) */
  gi_spacing?: number | Node<'Number'>;
  /** How many times light bounces between surfaces: 1 is sky, sun and glow bounced once, 2 or more also light bounced off bounced light (default 2; whole number 1..4) */
  gi_bounces?: number | Node<'Number'>;
  /** Rays traced per probe for each bounce: more is smoother and slower (default 128; whole number 32..512) */
  gi_rays?: number | Node<'Number'>;
  /** Direction the wind blows toward, in degrees around the vertical axis: 0 is +x, 90 is +z (default 0; -360..360) */
  wind_direction?: number | Node<'Number'>;
  /** How hard the wind blows: 0 is still air, 0.5 a gentle breeze, 1 a strong breeze, 2 a gale. It moves only foliage with sway or flutter (solid.foliage); the viewer shows it, an exported glTF does not (default 0.5; 0..2) */
  wind_strength?: number | Node<'Number'>;
}

/** A script is behaviour that runs against the live world and is saved with the document. `source` is the body of an async function with `world`, `on`, `every`, `log`, `time` and `random`; `name` is a unique lowercase identifier (empty uses this node's label), and `enabled` controls whether it runs. */
export interface ScriptJsV1Params {
  /** Body of the async script function; 1 to 65,536 bytes and not blank (required; text up to 65536 bytes) */
  source: string;
  /** Unique lowercase letters, digits and underscores, starting with a letter; empty uses the node label or id (default ""; text up to 65536 bytes) */
  name?: string;
  /** Whether this script should run (default true) */
  enabled?: boolean;
}

/** Lit text as geometry, for neon and LED signs: the letters (a 5 by 7 pixel font) become small boxes, centred on x = 0, standing on y = 0 (the bottom row at y = 0), reaching from z = 0 to z = `depth`, facing +z. The node sets a dark housing colour and an emissive glow in `color` (emissive = color x glow, so glow 0 is unlit); a later solid.material or material.preset overrides it. */
export interface SignTextV1Params {
  /** The words, 1 to 48 characters (capitals, digits and common punctuation) (default "SIGN"; text up to 48 bytes) */
  text?: string;
  /** Height of the letters in metres (default 0.5; metres, 0.05..10) */
  height?: number | Node<'Number'>;
  /** How far the letters stand out, in metres (default 0.04; metres, 0.005..1) */
  depth?: number | Node<'Number'>;
  /** Colour of the letters [r, g, b]; the housing is this colour scaled to 0.15, the glow is this colour times `glow` (default [1, 0.1, 0.6]; 0..100 each) */
  color?: Vec3;
  /** Brightness multiplier of the emissive light; 0 is unlit, above 1 blooms (default 3; 0..20) */
  glow?: number | Node<'Number'>;
}

/** A simulation you write, run live: cloth, hair, ropes, flocks, swarms, crowds, soft bodies, fire, anything with state that changes by rules. The `source` declares `param` settable numbers, `state` arrays with one value per element (`state pos: vec3[256]`), `kernel`s that run for each element of an array (`kernel move each i in pos { pos[i] += vel[i] * dt }`; a kernel reads the state as it was when it began and writes only its own element) or once, a `schedule` listing the kernels to run each step (`schedule flock; move`, with `repeat 6 { ... */
export interface SimProgramV1Params {
  /** The program in SimL (default ""; text up to 65536 bytes) */
  source?: string;
  /** A ready-made program used when `source` is empty: cloth (needs an `in` mesh), or bridge (41 planks hung between two posts that snap at the link `cut` when you set it with sim.set_param; see the sim guide) (default "none") */
  library?: 'none' | 'cloth' | 'bridge';
  /** Settings for the program's `param`s, `name=number` or `name=x,y,z` separated by semicolons, for example "collider_radius=0.2; collider_top=0,0.7,-0.05". Unknown names and wrong sizes are refused (default ""; text up to 2048 bytes) */
  values?: string;
  /** Seeds hash() and rand(); the same seed gives the same simulation (default 1; whole number 0..2147483647) */
  seed?: number | Node<'Number'>;
  /** Steps per second: every step advances the program by 1/rate seconds, however fast frames come (default 60; 1..240) */
  rate?: number | Node<'Number'>;
  /** Width of the small tetrahedron drawn for every particle, in metres (default 0.05; metres, 0.001..5) */
  size?: number | Node<'Number'>;
}

/** Axis-aligned box centred on the origin. */
export interface SolidBoxV1Params {
  /** Width, height and depth in metres (required; metres, 0.000001..1000000 each) */
  size: Vec3;
}

/** A cylinder with rounded ends standing along Y, centred on the origin. */
export interface SolidCapsuleV1Params {
  /** Radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Total length including both ends, at least twice the radius (required; metres, 0.000001..1000000) */
  height: number | Node<'Number'>;
  /** Facets around the axis (default 24; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** Breaks convex edges with a flat 45-degree facet by eroding then expanding the solid with an octahedron; bounded to 20,000 input and 250,000 output triangles. Concave edges stay sharp. */
export interface SolidChamferV1Params {
  /** Chamfer width in metres; less than half the thinnest solid dimension (required; metres, 0.000001..1000000) */
  width: number | Node<'Number'>;
}

/** Replaces a solid by the simple shape that fits it best, for collision and level-of-detail: the axis-aligned box, the oriented box (from the solid's principal axes), or a capsule along its longest axis. Emits a warning naming the chosen shape and its volume ratio to the solid. */
export interface SolidCollisionBoxesV1Params {
  /** Which fitted shape to build: best (the tightest of the three), an axis-aligned box, an oriented box, or a capsule (default "best") */
  shape?: 'best' | 'box' | 'oriented_box' | 'capsule';
}

/** A cone or truncated cone standing along Y, centred on the origin. */
export interface SolidConeV1Params {
  /** Height in metres (required; metres, 0.000001..1000000) */
  height: number | Node<'Number'>;
  /** Radius at the bottom in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Radius at the top; 0 makes a point (default 0; metres, 0..1000000) */
  top_radius?: number | Node<'Number'>;
  /** Facets around the axis (default 32; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** Cylinder standing along Y, centred on the origin. */
export interface SolidCylinderV1Params {
  /** Radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Height in metres (required; metres, 0.000001..1000000) */
  height: number | Node<'Number'>;
  /** Facets around the axis (default 24; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** The first solid minus all the others. */
export interface SolidDifferenceV1Params {
}

/** Stands a profile along Y from 0 to height (profile Y maps to -Z). */
export interface SolidExtrudeV1Params {
  /** Height in metres (required; metres, 0.000001..1000000) */
  height: number | Node<'Number'>;
  /** Vertical slabs; use 16 or more with a twist (default 1; whole number 1..256) */
  divisions?: number | Node<'Number'>;
  /** Rotation of the top relative to the base, in radians (default 0; an angle, -62.83185307179586..62.83185307179586) */
  twist?: number | Node<'Number'>;
  /** Size of the top relative to the base, per axis (default [1, 1]; a ratio, 0.001..1000 each) */
  top_scale?: Vec2;
}

/** Adds layers on top of a surface that the physically based renderer draws: a glossy clear coat (car paint, wet skin, glazed ceramic, scales, eyes), highlights stretched along the surface (brushed metal, silk, hair) and translucency, light passing through a thin body (leaves, wax, ears, wings). Colours, roughness and textures are kept: put it after solid.material and solid.texture. */
export interface SolidFinishV1Params {
  /** Strength of a glossy clear layer over the surface: 0 none, 1 a full lacquer. A wet look is about 0.6 with clearcoat_roughness 0.05 (default 0; 0..1) */
  clearcoat?: number | Node<'Number'>;
  /** Roughness of the clear layer: 0 a mirror, 1 a satin finish (default 0.1; 0..1) */
  clearcoat_roughness?: number | Node<'Number'>;
  /** How much highlights stretch along the surface (brushed metal, silk), 0 to 1 (default 0; 0..1) */
  anisotropy?: number | Node<'Number'>;
  /** Share of diffuse light that passes through a thin body and lights the far side: 0 opaque, 0.4 leaves, wax and ears, 1 a lampshade (default 0; 0..1) */
  translucency?: number | Node<'Number'>;
  /** Procedural surface relief drawn in the pixel shader, finer than the mesh and free of texture memory: 0 none, 0.3 skin pores or leather, 1 deep scales or bark. It follows a part that moves as a whole; on a skinned part it slides, so use a baked normal map there (default 0; 0..1) */
  relief?: number | Node<'Number'>;
  /** How many relief features fit in a metre: 8 coarse bark, 30 scales on a lizard, 120 fine pores (default 30; 0.1..500) */
  relief_scale?: number | Node<'Number'>;
  /** A wet look: the surface darkens and turns glossy, most of all in the low parts of its relief (crevices stay wet). 0.5 damp skin, 1 just out of the water (default 0; 0..1) */
  wetness?: number | Node<'Number'>;
  /** Colour that shifts with the viewing angle, like a beetle shell, fish scales or a soap film: 0.3 a hint, 1 strong rainbow (default 0; 0..1) */
  iridescence?: number | Node<'Number'>;
  /** A soft brightening at grazing angles (velvet, peach fuzz, fine hair, moss): 0.3 a soft edge glow, 1 strong (default 0; 0..1) */
  sheen?: number | Node<'Number'>;
  /** Light scattering under the surface, which softens the shadow edge with a warm tint (skin, wax, ears, jelly): 0.5 skin, 1 wax (default 0; 0..1) */
  subsurface?: number | Node<'Number'>;
  /** Sunlight passing through a thin body: with the sun behind it the surface glows in its own colour on the side facing you (wing membranes, leaves, ears, candle wax, paper, thin cloth): 0.4 a faint glow, 1 a lantern. Use it on thin parts only: it does not check whether something else shades the sun, so a thick body would glow too (default 0; 0..1) */
  thickness?: number | Node<'Number'>;
  /** smooth is soft lumps, ridged is creases and wrinkles, scales is round overlapping cells (default "smooth") */
  relief_style?: 'smooth' | 'ridged' | 'scales';
}

/** Makes a surface behave like leaves, grass, a fence or any thin cut-out card. It is cut away wherever its albedo map's alpha is under alpha_cutoff, so a leaf shape drawn into a texture (a texture program or an expr node that ends in a vec4 whose last number is the alpha) becomes a leaf-shaped silhouette in the picture, in its shadow and in the exported glTF (alphaMode MASK). */
export interface SolidFoliageV1Params {
  /** Enable the grass look: soil-coloured roots, upward normals and blades shrinking with camera distance (default false) */
  grass?: boolean;
  /** Host soil colour [r,g,b] in sRGB, used at the blade roots (default [0.25, 0.18, 0.1]; 0..1 each) */
  ground_color?: Vec3;
  /** Strength of the root colour blend; 0 disables it, 1 fully matches the soil at the root (default 0; 0..1) */
  ground_blend?: number | Node<'Number'>;
  /** Blades remain full size through this camera distance in metres (default 25; metres, 0..100000) */
  fade_near?: number | Node<'Number'>;
  /** Blades shrink linearly to zero here; at or below fade_near disables fading (default 70; metres, 0..100000) */
  fade_far?: number | Node<'Number'>;
  /** Sunlight through the blade, strongest at the tip; ignored at Low quality (default 0; 0..1) */
  translucency?: number | Node<'Number'>;
  /** Blade colour multiplier, floored at 0.1 by the renderer (default 1; 0..10) */
  exposure_boost?: number | Node<'Number'>;
  /** Pixels whose albedo alpha is below this are cut away: 0 keeps everything, 0.5 is the usual leaf edge, near 1 keeps only fully opaque pixels (default 0.5; 0..1) */
  alpha_cutoff?: number | Node<'Number'>;
  /** How far the whole shape bends with the wind, 0 still to 1 strongly; a tree swings from its base, so tall parts move more (default 0; 0..1) */
  sway?: number | Node<'Number'>;
  /** How much the surface trembles at a small scale (leaves in a breeze), 0 to 1 (default 0; 0..1) */
  flutter?: number | Node<'Number'>;
}

/** Replaces a solid by its convex hull: the smallest convex shape that contains every one of its points, as a closed solid. Refused when the solid has more points than max_vertices (decimate it first); the hull is at most 2 * max_vertices triangles. */
export interface SolidHullV1Params {
  /** Most points the hull accepts; a solid with more is refused, so decimate it first or raise this (default 256; whole number 4..2000) */
  max_vertices?: number | Node<'Number'>;
}

/** The volume shared by all solids. */
export interface SolidIntersectV1Params {
}

/** Lofts matching convex profile loops at even spacing along Y; steps above 1 blend smoothly between them. */
export interface SolidLoftV1Params {
  /** Distance between adjacent sections in metres (default 0.1; metres, 0.000001..1000000) */
  spacing?: number | Node<'Number'>;
  /** Rows per span between sections. 1 keeps straight segments (cones and cylinders); 4 to 8 blend a smooth curve through the sections, as for a vase belly (default 1; whole number 1..8) */
  steps?: number | Node<'Number'>;
}

/** Gives a solid, a set of placements or a polygon mesh a surface: colour, metal, roughness and glow. The shape is unchanged. */
export interface SolidMaterialV1Params {
  /** Base colour [r, g, b], each 0 to 1, as picked in a colour chooser (sRGB) (default [0.8, 0.8, 0.8]; 0..1 each) */
  color?: Vec3;
  /** 0 for plastic, wood, stone, paint; 1 for bare metal (default 0; 0..1) */
  metallic?: number | Node<'Number'>;
  /** 0 is a mirror, 1 is fully matte (default 0.5; 0..1) */
  roughness?: number | Node<'Number'>;
  /** Surface opacity: 0 is clear, 0.3 is tinted glass, 0.6 is murky water, 1 is solid (default 1; 0..1) */
  opacity?: number | Node<'Number'>;
  /** Light given off, [r, g, b] in linear colour; values above 1 glow (a flame is about [3, 1.6, 0.4]) (default [0, 0, 0]; 0..100 each) */
  emissive?: Vec3;
  /** Create a bounded point light from this material's emissive colour and surface area (default false) */
  emits_light?: boolean;
  /** Bounded object-space procedural surface pattern; no external textures are loaded (default "none") */
  recipe?: 'none' | 'wood' | 'planks' | 'brushed_metal' | 'leather' | 'fabric' | 'brick' | 'concrete' | 'marble' | 'painted_wall';
  /** Object-space pattern frequency multiplier; larger values repeat more often (default 1; 0.1..50) */
  texture_scale?: number | Node<'Number'>;
}

/** Spins a profile about the Y axis. */
export interface SolidRevolveV1Params {
  /** Sweep angle in radians (default 6.283185307179586; an angle, 0.001..6.283185307179586) */
  angle?: number | Node<'Number'>;
  /** Facets around the axis (default 32; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** Rounds a closed solid by eroding then expanding it with a small sphere; bounded to 20,000 input and 250,000 output triangles. */
export interface SolidRoundV1Params {
  /** Edge radius in metres; less than half the thinnest solid dimension (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Sphere profile resolution; higher values cost more kernel time (default 12; whole number 6..24) */
  segments?: number | Node<'Number'>;
}

/** Sphere centred on the origin. */
export interface SolidSphereV1Params {
  /** Radius in metres (required; metres, 0.000001..1000000) */
  radius: number | Node<'Number'>;
  /** Facets around the axis (default 24; whole number 3..512) */
  segments?: number | Node<'Number'>;
}

/** Sets the whole look of a surface with one word, keeping its colours, roughness, textures and shape. `anime`: two-band cel shading with a tinted shadow, rim light and an outline. */
export interface SolidStyleV1Params {
  /** The look: anime, realistic, painterly, flat, retro or ink (default "anime") */
  style?: 'anime' | 'realistic' | 'painterly' | 'flat' | 'retro' | 'ink';
  /** Multiplies the style's outline width: 0 removes the outline, 2 doubles it (default 1; 0..4) */
  outline_scale?: number | Node<'Number'>;
}

/** Sweeps a single closed profile along a curve (a pipe, rope, rail, tentacle, horn). scale_end tapers the tube to a different size at the far end of an open curve (0.2 gives a pointed horn or tail) and twist turns the profile along the way (a twisted rope or ribbon). */
export interface SolidSweepV1Params {
  /** Size of the profile at the end of the curve relative to the start; it changes steadily along the length. Open curves only (default 1; a ratio, 0.05..4) */
  scale_end?: number | Node<'Number'>;
  /** Total turn of the profile about the path from start to end, in radians. Open curves only; only visible for a profile that is not round (default 0; an angle, -50.26548245743669..50.26548245743669) */
  twist?: number | Node<'Number'>;
}

/** Attaches a procedural Material Maker texture program to a solid. The base material and geometry are preserved. */
export interface SolidTextureV1Params {
  /** Blend three shifted samples to hide tiled repetition; ignored at Low quality (default false) */
  no_repeat?: boolean;
  /** Grid cells per tile for non-repeating sampling; larger values make smaller patches (default 1; 0.25..8) */
  no_repeat_scale?: number | Node<'Number'>;
  /** Procedural texture DSL with albedo, roughness, metallic, height and emissive channels (required; text up to 8192 bytes) */
  program: string;
  /** 0 lays the texture out once over the part. Above 0 it repeats every this many metres in world space (use 1 to 4 for ground, 0.5 to 2 for walls) so big surfaces stay sharp (default 0; metres, 0..100) */
  tile?: number | Node<'Number'>;
  /** Side in pixels of the maps the viewer bakes: auto is 1024. The viewer holds a fixed amount of texture memory, so small things (a leaf, a pebble, a distant prop) can say 256 or 512 and leave room for many more textured parts; when memory still runs out the viewer shrinks the biggest maps first and tells you. It does not change the exported file (default "auto") */
  size?: 'auto' | '256' | '512' | '1024';
}

/** Gives a surface an anime / cel-shaded look. Light falls into flat colour bands (with a tinted shadow colour) instead of smooth shading. */
export interface SolidToonV1Params {
  /** Steps between the shadow colour and the lit colour: 1 is two-tone, 2 adds a mid tone, up to 4 (default 2; whole number 1..4) */
  bands?: number | Node<'Number'>;
  /** Softness of each band edge: 0 is a razor edge, 0.1 a crisp anime edge, 1 a soft blend (default 0.08; 0..1) */
  feather?: number | Node<'Number'>;
  /** Moves every band edge toward the light (positive: less shadow) or away from it (negative: more shadow) (default 0; -1..1) */
  shift?: number | Node<'Number'>;
  /** Size of a small hard highlight from the sun; 0 for none, 0.3 for hair or plastic shine (default 0; 0..1) */
  spec?: number | Node<'Number'>;
  /** What the surface colour is multiplied by in shadow, [r, g, b] (sRGB). A cool purple-blue gives the classic anime shadow; white gives none (default [0.62, 0.58, 0.82]; 0..1 each) */
  shade_color?: Vec3;
  /** Strength of a rim light on the sunlit edge facing away from the viewer; 0.3 is gentle (default 0; 0..1) */
  rim?: number | Node<'Number'>;
  /** Outline thickness in screen pixels, constant at any distance; 0 for none, 1.5 to 3 for a character, 0.8 for fine detail. Works best on smooth, rounded shapes (a character, a sphere, a rock); a hard-edged box shows gaps at its corners (default 0; 0..16) */
  outline?: number | Node<'Number'>;
  /** Outline colour [r, g, b] (sRGB). A dark purple-brown reads softer than black; use a darker version of the surface colour for a painted look (default [0.1, 0.06, 0.14]; 0..1 each) */
  outline_color?: Vec3;
}

/** A ring (doughnut) lying in the XZ plane, centred on the origin. */
export interface SolidTorusV1Params {
  /** Distance from the centre to the middle of the tube, in metres (required; metres, 0.000001..1000000) */
  major: number | Node<'Number'>;
  /** Radius of the tube in metres; must be smaller than major (required; metres, 0.000001..1000000) */
  minor: number | Node<'Number'>;
  /** Facets around the axis (default 48; whole number 3..512) */
  segments?: number | Node<'Number'>;
  /** Facets around the tube (default 24; whole number 3..256) */
  tube_segments?: number | Node<'Number'>;
}

/** Joins solids into one (a single solid joins to itself). */
export interface SolidUnionV1Params {
}

/** A deterministic, placeable street prop kit. Build matching kind/position/axis nodes for body, accents and (for lights) lenses to assign materials. */
export interface StreetPropV1Params {
  /** Prop base position in metres (x, y, z) (default [0, 0, 0]; metres, -1000..1000 each) */
  at?: Vec3;
  /** Mast or prop height in metres; fixed-size furniture ignores it (default 6; metres, 1..18) */
  height?: number | Node<'Number'>;
  /** Cardinal alignment; x runs along X and z runs along Z, keeping repeated street modules deterministic (default "x") */
  axis?: 'x' | 'z';
  /** Reusable street object: lighting, signal mast, utility pole, sign, pedestrian furniture or shelter (default "streetlight") */
  kind?: 'mast' | 'streetlight' | 'traffic_signal' | 'sign' | 'utility_mast' | 'bollard' | 'hydrant' | 'bench' | 'bin' | 'bus_shelter';
  /** Separate structural body and accents for materials. lens is available on streetlight and traffic_signal (default "body") */
  part?: 'body' | 'accent' | 'lens';
}

/** A straight, metre-scaled road kit module: asphalt, lane paint, crosswalks, paired raised kerbs and pavements. Match length, width, axis and centre when making its material-separated parts; join segments to build streets and intersections. */
export interface StreetRoadV1Params {
  /** Road segment centre in metres (x, y, z) (default [0, 0, 0]; metres, -1000..1000 each) */
  at?: Vec3;
  /** Segment length in metres (default 80; metres, 8..240) */
  length?: number | Node<'Number'>;
  /** Asphalt width in metres, excluding pavement (default 12; metres, 3..36) */
  width?: number | Node<'Number'>;
  /** Lane count; paint is spaced evenly with dashed dividers and a double centre line on two-way roads (default 2; whole number 1..8) */
  lanes?: number | Node<'Number'>;
  /** Cardinal alignment; x runs along X and z runs along Z, keeping repeated street modules deterministic (default "x") */
  axis?: 'x' | 'z';
  /** Build each part as its own node, then assign asphalt, paint, stone and concrete materials (default "asphalt") */
  part?: 'asphalt' | 'markings' | 'crosswalk' | 'kerb' | 'pavement';
}

/** Sorts the ground of a terrain.tile into biomes that blend into each other: forest where it is low, damp and gentle, meadow where it is drier, scree where it is steep, snow above the tree line. Each biome is written as soft windows over the ground's height (metres), steepness (degrees from flat) and moisture (a slow noise, 0 dry to 1 wet): `forest: height 0..80, slope 0..30, wet 0.45..1; meadow: slope 0..20, wet 0..0.5; rock: slope 35..` (either end of a window may be left out). */
export interface TerrainBiomesV1Params {
  /** The biomes, separated by `;`: `name: height low..high, slope low..high, wet low..high`. height in metres, slope in degrees from flat, wet 0 to 1. A missing window means anywhere; a missing end is open (has a default; text up to 2048 bytes) */
  rules?: string;
  /** How soft the edges are, as a share of a window's width: 0 cuts sharply, 1 fades across the whole window (default 0.25; a ratio, 0..1) */
  blend?: number | Node<'Number'>;
  /** Size of the moisture patches in metres (default 300; metres, 10..20000) */
  wet_scale?: number | Node<'Number'>;
  /** Picks a different moisture pattern; the same seed gives the same one on every tile (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Layers are named prefix + biome name (default "biome_"; text up to 16 bytes) */
  prefix?: string;
}

/** Weathers a terrain.tile like real mountains: rivers cut valleys (large scale, the same for every tile), loose rock slides down steep slopes, and rain moves soil downhill (fine detail). Neighbouring tiles eroded with the same settings meet without a seam. */
export interface TerrainErodeV1Params {
  /** How strongly rivers carve valleys: 0 off, 0.5 moderate, 1 deep gorges (default 0.5; a ratio, 0..1) */
  river_erosion?: number | Node<'Number'>;
  /** Passes of river carving (more is deeper and more drained) (default 30; whole number 1..200) */
  river_iterations?: number | Node<'Number'>;
  /** Steepest slope that loose rock holds, in DEGREES (not radians); steeper slides down. 0 turns it off. 30 to 40 is natural (default 35; 0..89) */
  talus?: number | Node<'Number'>;
  /** Passes of rock sliding (default 20; whole number 1..100) */
  thermal_iterations?: number | Node<'Number'>;
  /** Rainfall per pass in metres for the fine water erosion; 0 turns it off (default 0.01; a ratio, 0..0.1) */
  rain?: number | Node<'Number'>;
  /** Passes of rain erosion (default 20; whole number 1..100) */
  hydraulic_iterations?: number | Node<'Number'>;
  /** How strongly fine detail is held to the large-scale valleys (0 free, 1 locked to them) (default 0.3; a ratio, 0..1) */
  pull?: number | Node<'Number'>;
  /** Metres between points of the large-scale erosion map; smaller is finer and slower (default 16; metres, 4..128) */
  coarse_cell?: number | Node<'Number'>;
  /** Width of the world square around the origin that the large-scale erosion covers. Use the same value on every tile (default 4096; metres, 256..16384) */
  world_size?: number | Node<'Number'>;
}

/** Levels a round pad in the ground of a terrain.tile, with a soft shoulder that blends back into the land: the place for a house, a camp, a road end or a landing. By default the pad keeps the height of the ground at its centre; set `to` to "height" to level it to a height you choose (for a shared floor level across several pads). */
export interface TerrainFlattenV1Params {
  /** World x and z of the pad's centre in metres (a tile covers tile_x * size.x to (tile_x + 1) * size.x) (default [0, 0]; metres, -1000000..1000000 each) */
  at?: Vec2;
  /** Radius of the level part in metres: a building's half width plus a metre or two (default 8; metres, 0.5..2000) */
  radius?: number | Node<'Number'>;
  /** Width of the shoulder around the pad in metres, where the ground eases back to the land (0 is a sharp step) (default 6; metres, 0..2000) */
  blend?: number | Node<'Number'>;
  /** centre: keep the ground's height at the pad's centre. height: level it to `height` (default "centre") */
  to?: 'centre' | 'height';
  /** Pad height in metres above y = 0 when `to` is "height" (default 0; metres, -1000..20000) */
  height?: number | Node<'Number'>;
}

/** A landscape: a seeded fractal ground (rolling hills, ridged mountains, an island) as one closed solid slab, centred on the origin with its lowest ground at y=0 or above and a flat base under it. It is an ordinary solid, so it takes solid.material, can be cut with booleans, and can carry copies placed by inst.drop. */
export interface TerrainHeightfieldV1Params {
  /** Width along X and depth along Z in metres (default [20, 20]; metres, 1..2000 each) */
  size?: Vec2;
  /** Tallest ground above the base level, in metres (default 3; metres, 0..500) */
  height?: number | Node<'Number'>;
  /** Size of the biggest hills in metres; smaller gives more, tighter hills (default 8; metres, 0.5..1000) */
  scale?: number | Node<'Number'>;
  /** Layers of finer detail (1 is smooth blobs, 5 or 6 is craggy) (default 4; whole number 1..6) */
  octaves?: number | Node<'Number'>;
  /** How strong each finer layer is relative to the one before (higher is rougher) (default 0.5; a ratio, 0.1..0.9) */
  roughness?: number | Node<'Number'>;
  /** Picks a different landscape (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Grid cells along each side (the ground has 2 x resolution x resolution triangles; 64 is smooth enough for a 20 m field, use 128 for large scenes) (default 64; whole number 4..192) */
  resolution?: number | Node<'Number'>;
  /** Thickness of the slab below y=0 in metres (default 0.5; metres, 0.05..100) */
  base?: number | Node<'Number'>;
  /** hills: soft rolling ground. ridged: sharp mountain ridges (default "hills") */
  style?: 'hills' | 'ridged';
  /** 0 keeps the full height to the edge; 0.5 or more slopes the land down toward the edges (an island or a cut-out landscape) (default 0; a ratio, 0..1) */
  edge_falloff?: number | Node<'Number'>;
  /** Fraction of the height below which the ground is flattened: hollows become flat beds you can fill with a water slab at that level (default 0; a ratio, 0..0.95) */
  flat_below?: number | Node<'Number'>;
}

/** Measures the ground of a terrain.tile into a named 0 to 1 layer you can paint materials and scatter plants by: steepness, bumpiness, height, which way the slope faces, or how enclosed a spot is. Look-around masks use the tile's margin, so neighbouring tiles agree at their edge. */
export interface TerrainMaskV1Params {
  /** slope: 0 flat to 1 vertical (cliffs). curvature: 0.5 flat, above on ridges, below in hollows. height: 0 sea level to 1 the tallest ground. aspect: which way the ground faces, 0 north (+z), 0.25 east, 0.5 south, 0.75 west. occlusion: 0 open to 1 deep in a pit (default "slope") */
  mask?: 'slope' | 'curvature' | 'height' | 'aspect' | 'occlusion';
  /** Name of the layer; empty uses the mask's own name. Later nodes refer to it by this name (default ""; text up to 32 bytes) */
  name?: string;
  /** The mask value that becomes 0 (everything below is 0). Use it to pick a band, for example slope from 0.3 to 0.6 (default 0; a ratio, -10..10) */
  from?: number | Node<'Number'>;
  /** The mask value that becomes 1 (everything above is 1) (default 1; a ratio, -10..10) */
  to?: number | Node<'Number'>;
  /** Soften the edges of the band with a smooth curve (default false) */
  smooth?: boolean;
  /** Swap 0 and 1 (flat ground instead of cliffs) (default false) */
  invert?: boolean;
}

/** Dresses a landscape: grass on gentle ground, bare rock on steep faces, sand on the lowest ground and snow above a snow line, blended per pixel from the ground's own slope and height with large-scale variation so it never repeats. No texture is baked, so it looks sharp at any distance and every tile of a world matches. */
export interface TerrainMaterialV1Params {
  /** Blend three shifted terrain samples to hide repetition; ignored at Low quality (default true) */
  no_repeat?: boolean;
  /** Grid cells per terrain tile of detail; larger values make smaller patches (default 1; 0.25..8) */
  no_repeat_scale?: number | Node<'Number'>;
  /** Colour of grass on gentle ground [r, g, b] (sRGB) (default [0.27, 0.38, 0.16]; 0..1 each) */
  grass?: Vec3;
  /** Colour of bare rock on steep faces (default [0.42, 0.39, 0.35]; 0..1 each) */
  rock?: Vec3;
  /** Colour of snow (default [0.92, 0.94, 0.97]; 0..1 each) */
  snow?: Vec3;
  /** Colour of sand or dry earth on the lowest ground (default [0.72, 0.64, 0.45]; 0..1 each) */
  sand?: Vec3;
  /** Slope in DEGREES from flat above which ground turns to rock (30 to 45 is natural) (default 38; 5..85) */
  rock_slope?: number | Node<'Number'>;
  /** How gradually grass gives way to rock, in degrees of slope (default 9; 1..45) */
  rock_blend?: number | Node<'Number'>;
  /** Height in metres above which snow lies on not-too-steep ground. The default is far above any hill, so no snow; set it just below your tallest peaks (default 9000; metres, -1000..20000) */
  snow_line?: number | Node<'Number'>;
  /** How gradually snow thins out below the snow line, in metres (default 18; metres, 0.1..500) */
  snow_blend?: number | Node<'Number'>;
  /** Height in metres below which the ground is sand or dry earth (a beach around a water level) (default 3; metres, -1000..20000) */
  sand_line?: number | Node<'Number'>;
  /** How gradually sand gives way to grass, in metres (default 2; metres, 0.1..500) */
  sand_blend?: number | Node<'Number'>;
  /** How much the thresholds wander across the land so borders are ragged and patchy, not contour lines (default 0.5; a ratio, 0..1) */
  variation?: number | Node<'Number'>;
  /** Strength of the fine bumpiness drawn in the shader, strongest on rock (default 0.5; a ratio, 0..1) */
  relief?: number | Node<'Number'>;
  /** Bump features per metre: 1 is boulder-sized crags, 8 is fine gravel (default 1.2; 0.1..500) */
  relief_scale?: number | Node<'Number'>;
}

/** Stands copies of any shape on the ground of a terrain.tile at x,z positions you list: a hut here, a well there, a lone tree. Each copy sits at the ground height under it (after terrain.flatten, on the pad), so you never need to know the height. */
export interface TerrainPlaceV1Params {
  /** Where each copy stands: [x, z] in world metres, one entry per copy (required; 1 to 4096 points) */
  at: Vec2[];
  /** Optional, one [yaw, scale] per copy in the same order as `at`: yaw in degrees about the vertical axis, scale a size factor above 0 (1 is unchanged) (default []; 0 to 4096 points) */
  yaw_scale?: Vec2[];
  /** How much each copy leans with the ground: 0 stands upright (buildings), 1 lies flat on the slope (rocks, moss) (default 0; a ratio, 0..1) */
  align?: number | Node<'Number'>;
  /** Push copies down into the ground by this many metres (a hut looks bedded in at 0.1) (default 0; metres, -10..10) */
  sink?: number | Node<'Number'>;
}

/** One tile of a world's ground as an editable polygon mesh with a height grid behind it. Heights come from fractal gradient noise evaluated at WORLD coordinates, so neighbouring tiles (tile_x, tile_z) line up exactly and the same seed always gives the same land, whichever tile you build first. */
export interface TerrainTileV1Params {
  /** Width along X and depth along Z of the tile in metres (default [256, 256]; metres, 8..4096 each) */
  size?: Vec2;
  /** Metres between grid points. size / cell must stay under 2048 cells a side; 2 m suits a 256 m tile, 0.5 m a close-up 64 m tile (default 2; metres, 0.25..32) */
  cell?: number | Node<'Number'>;
  /** Which tile along X. The tile covers tile_x * size.x to (tile_x + 1) * size.x (default 0; whole number -1000..1000) */
  tile_x?: number | Node<'Number'>;
  /** Which tile along Z (default 0; whole number -1000..1000) */
  tile_z?: number | Node<'Number'>;
  /** World seed: the same seed gives the same land on every tile (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Tallest ground above y=0 in metres (default 120; metres, 0..3000) */
  height?: number | Node<'Number'>;
  /** Size of the biggest hills in metres; smaller gives more, tighter hills (default 200; metres, 4..20000) */
  scale?: number | Node<'Number'>;
  /** Layers of finer detail (1 is smooth, 5 to 8 is craggy) (default 5; whole number 1..8) */
  octaves?: number | Node<'Number'>;
  /** How strong each finer layer is relative to the one before (default 0.5; a ratio, 0.1..0.9) */
  roughness?: number | Node<'Number'>;
  /** hills: soft rolling ground. ridged: sharp mountain ridges (default "hills") */
  style?: 'hills' | 'ridged';
  /** The noise under the land. classic: the original (default, existing worlds keep their ground). perlin and simplex: smoother, fewer grid-aligned artefacts, simplex the least. value: soft blobs. cellular: round pits and hummocks like a crater field, try style ridged for ridge lines along cell borders (default "classic") */
  basis?: 'classic' | 'perlin' | 'simplex' | 'value' | 'cellular';
  /** Bends the noise sideways by up to this many metres: 0 is plain, 100 to 400 twists ridges into believable ranges (default 0; metres, 0..2000) */
  warp?: number | Node<'Number'>;
  /** 0 leaves the land going on forever. Above 0 the land falls to sea level beyond this radius around the world origin (0, 0), so a continent spans many tiles (default 0; metres, 0..100000) */
  island_radius?: number | Node<'Number'>;
  /** How much of the island radius is a coast sloping down (0 a cliff, 1 a cone) (default 0.5; a ratio, 0..1) */
  island_reach?: number | Node<'Number'>;
  /** 0 off. Above 0 the ground is cut into this many flat steps (rice terraces, mesas) (default 0; whole number 0..64) */
  terraces?: number | Node<'Number'>;
  /** How hard the steps are: 0 none, 1 flat treads with vertical risers (default 1; a ratio, 0..1) */
  terrace_sharpness?: number | Node<'Number'>;
}

/** Arithmetic on two numbers. a and b are number parameters, so either can be a literal or the output of another value node (a=width, b=2). */
export interface ValueMathV1Params {
  /** The operation (default "add") */
  op?: 'add' | 'sub' | 'mul' | 'div' | 'min' | 'max' | 'pow' | 'mod';
  /** The first number; link a value node here to compute from it (default 0; -1000000000..1000000000) */
  a?: number | Node<'Number'>;
  /** The second number; link a value node here to compute from it (default 0; -1000000000..1000000000) */
  b?: number | Node<'Number'>;
}

/** Measures how far a clip moves a part, from its position tracks (anim.track property=position): the straight horizontal `travel` from the first key to the last, the horizontal `path` length, the `duration` in seconds, the `speed` (travel over duration) and the vertical `lift` (highest minus lowest). Use it to read the real speed of a walk or a dash instead of guessing: link `speed` into a state machine's walk threshold or a game unit's speed, or `travel` into how far a part should be placed. */
export interface ValueMotionV1Params {
  /** What to read: travel (straight horizontal distance, metres), path (horizontal path length), duration (seconds), speed (travel over duration, metres per second) or lift (vertical range, metres) (default "travel") */
  measure?: 'travel' | 'path' | 'duration' | 'speed' | 'lift';
  /** The clip to read; empty reads the part's main clip (default ""; text up to 24 bytes) */
  clip?: string;
}

/** A number, to link into any number parameter of another node (a port named like the parameter, or like size.y for one component of a vector). Use it to name a dimension once and reuse it. */
export interface ValueNumberV1Params {
  /** The number (required; -1000000000..1000000000) */
  value: number | Node<'Number'>;
}

/** Reads a place on a part as a number: the head (the joint it turns about) or the tail of a bone (body.bone, rig.bone), or the min, center or max of any solid or polygon mesh's bounding box, along x, y or z. Link it into a number parameter, one component at a time (for example `by.y=hy` or `position.x=hx`), so one part is placed from where another is. */
export interface ValuePointV1Params {
  /** Which place: head or tail (a bone's joint and far end), or min, center, max (the corners and middle of the bounding box) (default "center") */
  at?: 'head' | 'tail' | 'min' | 'center' | 'max';
  /** Which coordinate to read: x, y or z (y is up) (default "y") */
  axis?: 'x' | 'y' | 'z';
}

/** Measures a solid or polygon mesh: the size of its bounding box along x (width), y (height) or z (depth), the longest or shortest of those, its enclosed volume (solids) or its surface area. Link the result into another node's parameter to size one part from another. */
export interface ValueSizeV1Params {
  /** What to measure: width (x extent), height (y), depth (z), longest, shortest, volume or area (default "height") */
  measure?: 'width' | 'height' | 'depth' | 'longest' | 'shortest' | 'volume' | 'area';
}

/** Joins volumes: union (everything in any of them), subtract (the first minus all the others) or intersect (only what is in all of them), optionally with a smooth blend where the shapes meet, like clay run together. Every volume must have the same voxel_size. */
export interface VolumeCombineV1Params {
  /** union, subtract (first minus the rest) or intersect (default "union") */
  operation?: 'union' | 'subtract' | 'intersect';
  /** Metres over which the shapes melt into each other where they meet; 0 is a hard edge. Keep it within the volumes' band (default 0; metres, 0..100) */
  blend?: number | Node<'Number'>;
}

/** Samples a closed mesh into a grid of distances to its surface (negative inside, positive outside), the starting point of the volume nodes: combine volumes like clay (volume.combine), blur them smooth (volume.smooth), grow or shrink them (volume.offset), and turn the result back into a clean all-quad mesh (volume.to_mesh). The mesh must be closed (a solid, or a polygon mesh with no open edges; poly.fill_holes closes one). */
export interface VolumeFromMeshV1Params {
  /** Edge of one voxel in metres: the finest detail the volume can hold and the size of the quads volume.to_mesh makes. Use the same value on volumes you will combine (default 0.05; metres, 0.002..10) */
  voxel_size?: number | Node<'Number'>;
  /** How many voxels out from the surface distances are exact. Offsets and blends reach at most this far (default 6; whole number 2..24) */
  band?: number | Node<'Number'>;
}

/** Grows (positive distance) or shrinks (negative) a volume by moving its surface, in metres: a thicker or thinner shape, a coat around it, a hole made wider. Limited to about 0.9 of band times voxel_size. */
export interface VolumeOffsetV1Params {
  /** How far the surface moves outward (negative: inward), in metres (default 0.05; metres, -100..100) */
  distance?: number | Node<'Number'>;
}

/** Smooths a volume: the surface relaxes like clay, sharp creases and small lumps round off and thin bridges thin out. Each pass moves every voxel's distance toward the average of its neighbours, so more passes round more and shrink thin parts a little. */
export interface VolumeSmoothV1Params {
  /** Smoothing passes (default 2; whole number 1..50) */
  iterations?: number | Node<'Number'>;
  /** How far each pass moves toward the neighbours' average: 0 nothing, 1 all the way (default 0.8; 0..1) */
  factor?: number | Node<'Number'>;
}

/** Turns a volume back into a mesh: the surface where the distance is zero, as a closed mesh of clean quads about one voxel across, facing outward (surface nets). Smoothed volumes give smooth meshes. */
export interface VolumeToMeshV1Params {
  /** Which level to mesh: 0 is the surface, positive a little outside it, negative a little inside (default 0; metres, -10..10) */
  iso?: number | Node<'Number'>;
}

/** Fills the connected hollow below level at a world x,z seed on a terrain.tile. Makes a flat open water sheet with depth tint and shore foam, a one-cell skirt and no side walls. */
export interface WaterFillV1Params {
  /** Seed world x,z in metres; omitted uses the input tile centre. Its cell must be below level (required; metres, -1000000..1000000 each) */
  at: Vec2;
  /** Water surface height in world metres (default 0; metres, -10000..20000) */
  level?: number | Node<'Number'>;
  /** Width of the wet bank in metres (default 3; metres, 0.1..50) */
  shore?: number | Node<'Number'>;
  /** Main wave travel direction in degrees; the per-vertex river current still drives streaming (default 0; an angle, -6.283185307179586..6.283185307179586) */
  direction?: number | Node<'Number'>;
  /** Repeatable wave pattern seed (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Colour of clear shallow water [r, g, b] (sRGB) (default [0.16, 0.42, 0.45]; 0..1 each) */
  shallow?: Vec3;
  /** Colour of deep water (default [0.02, 0.12, 0.2]; 0..1 each) */
  deep?: Vec3;
  /** How many metres of depth it takes to go from the shallow to the deep colour: small is murky, large is clear (default 3; metres, 0.2..50) */
  clarity?: number | Node<'Number'>;
  /** How fast the surface pattern streams along the current (1 is natural) (default 0; a ratio, 0..4) */
  flow?: number | Node<'Number'>;
  /** Strength of the small waves and glints on the surface (default 0.5; a ratio, 0..2) */
  ripple?: number | Node<'Number'>;
  /** How much white foam forms at the banks and in fast water (default 0.5; a ratio, 0..1) */
  foam?: number | Node<'Number'>;
  /** How opaque the water is in its deepest parts (default 0.85; a ratio, 0.1..1) */
  opacity?: number | Node<'Number'>;
  /** Enable interactive shallow-water ripples (default false) */
  sim?: boolean;
  /** Simulation cells along the longer side (default 128; whole number 32..256) */
  sim_resolution?: number | Node<'Number'>;
  /** Velocity damping per second (default 0.6; a ratio, 0..5) */
  sim_damping?: number | Node<'Number'>;
  /** Fallback physical depth in metres, setting ripple speed (default 1; metres, 0.05..50) */
  sim_depth?: number | Node<'Number'>;
  /** Seeded drops per second per 100 square metres of water (default 0; a ratio, 0..1) */
  rain?: number | Node<'Number'>;
}

/** For lakes on a landscape use water.fill, which follows the hollow and shoreline. A closed, transparent water body with a seeded wavy rest mesh and a lightweight animated viewer ripple. */
export interface WaterSurfaceV1Params {
  /** Water width along X and length along Z, in metres (default [20, 20]; metres, 0.1..2000 each) */
  size?: Vec2;
  /** Wave amplitude multiplier; zero is a flat animated sheet (default 1; a ratio, 0..2) */
  ripple?: number | Node<'Number'>;
  /** Wave travel speed multiplier; zero freezes the waves (default 1; a ratio, 0..4) */
  flow?: number | Node<'Number'>;
  /** Main wave travel direction in degrees; zero is positive X (default 0; an angle, -6.283185307179586..6.283185307179586) */
  direction?: number | Node<'Number'>;
  /** Water surface height in world metres (default 0; metres, -10000..10000) */
  level?: number | Node<'Number'>;
  /** How far the water body extends below the surface (default 1; metres, 0.05..1000) */
  depth?: number | Node<'Number'>;
  /** Water tint in sRGB (default [0.12, 0.38, 0.52]; 0..1 each) */
  color?: Vec3;
  /** Water transparency: 0 is clear, 0.6 is water, 1 is opaque (default 0.6; a ratio, 0..1) */
  opacity?: number | Node<'Number'>;
  /** Maximum seeded surface displacement in metres (default 0.08; metres, 0..5) */
  wave_height?: number | Node<'Number'>;
  /** Scale of the broad surface undulation in metres (default 3; metres, 0.1..1000) */
  wave_length?: number | Node<'Number'>;
  /** Viewer ripple speed; zero pauses the ripple (default 1; a ratio, 0..10) */
  speed?: number | Node<'Number'>;
  /** Picks repeatable rest-surface and travelling-wave patterns (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Grid cells per side, capped at 192 (at most 73,728 triangles) (default 64; whole number 4..192) */
  resolution?: number | Node<'Number'>;
  /** Enable interactive shallow-water ripples (default false) */
  sim?: boolean;
  /** Simulation cells along the longer side (default 128; whole number 32..256) */
  sim_resolution?: number | Node<'Number'>;
  /** Velocity damping per second (default 0.6; a ratio, 0..5) */
  sim_damping?: number | Node<'Number'>;
  /** Fallback physical depth in metres, setting ripple speed (default 1; metres, 0.05..50) */
  sim_depth?: number | Node<'Number'>;
  /** Seeded drops per second per 100 square metres of water (default 0; a ratio, 0..1) */
  rain?: number | Node<'Number'>;
}

/** Rotates a direction onto another about a fixed pivot; zero directions are refused. */
export interface XformAlignV1Params {
  /** Nonzero source direction (required; a ratio, -1000000..1000000 each) */
  from: Vec3;
  /** Nonzero target direction (required; a ratio, -1000000..1000000 each) */
  to: Vec3;
  /** Fixed point in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  pivot?: Vec3;
}

/** Copies a solid into a grid; the first copy stays where it is. */
export interface XformArrayV1Params {
  /** Copies along X (default 1; whole number 1..256) */
  count_x?: number | Node<'Number'>;
  /** Copies along Y (default 1; whole number 1..256) */
  count_y?: number | Node<'Number'>;
  /** Copies along Z (default 1; whole number 1..256) */
  count_z?: number | Node<'Number'>;
  /** Distance between neighbours along each axis, in metres (required; metres, -1000000..1000000 each) */
  spacing: Vec3;
}

/** Moves and rotates the source so its named socket coincides with the target socket. */
export interface XformAttachV1Params {
  /** Source socket name (required; text up to 64 bytes) */
  socket: string;
  /** Target socket name (required; text up to 64 bytes) */
  to_socket: string;
}

/** Bends a solid around the Z axis along its local Y extent. */
export interface XformBendV1Params {
  /** Total bend angle from the bottom to top in radians (default 0; an angle, -1.5707963267948966..1.5707963267948966) */
  angle?: number | Node<'Number'>;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Free-form deformation: a grid of control points over the solid's bounding box, some of which you move; the solid follows smoothly (a cartoon squash, a bulge, a tapered bottle). Control point (i j k) runs 0 to divisions-1 along X, Y, Z; (0 0 0) is the minimum corner. */
export interface XformLatticeV1Params {
  /** Control points along each axis (2 to 4). 2 is a plain box of corners; 3 or 4 gives room to bulge (default 3; whole number 2..4) */
  divisions?: number | Node<'Number'>;
  /** Control points to move, separated by semicolons (at most 64): "i j k dx dy dz" in metres. Example, bulge a 3-point lattice's middle outward: "1 1 2 0 0 0.2; 1 1 0 0 0 -0.2" (default ""; text up to 2048 bytes) */
  offsets?: string;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Adds the mirror image of a solid across the plane through the origin perpendicular to an axis. */
export interface XformMirrorV1Params {
  /** The axis the mirror plane is perpendicular to (required) */
  axis: 'x' | 'y' | 'z';
}

/** Moves its input. */
export interface XformMoveV1Params {
  /** Offset in metres (required; metres, -1000000..1000000 each) */
  by: Vec3;
}

/** Roughens a solid with smooth random displacement (rocks, hills, lumpy clay): every point is pushed by smooth noise. The same seed always gives the same shape. */
export interface XformNoiseV1Params {
  /** Largest push in any direction, in metres. Keep it well below the feature scale (default 0.05; metres, 0..100) */
  amplitude?: number | Node<'Number'>;
  /** Size of one lump in metres (default 0.5; metres, 0.01..1000) */
  scale?: number | Node<'Number'>;
  /** Picks a different random shape (default 0; whole number 0..1000000) */
  seed?: number | Node<'Number'>;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Copies a solid evenly around the Y axis; each copy is first moved out along X. */
export interface XformRadialV1Params {
  /** Number of copies (required; whole number 2..256) */
  count: number | Node<'Number'>;
  /** How far out along X each copy is moved before it is turned (default 0; metres, 0..1000000) */
  radius?: number | Node<'Number'>;
}

/** Copies a solid along a path: copy i is turned by i times rotate_step, then moved by i times step. */
export interface XformRepeatV1Params {
  /** Number of copies, including the original (required; whole number 2..256) */
  count: number | Node<'Number'>;
  /** Movement per copy, in metres (default [0, 0, 0]; metres, -1000000..1000000 each) */
  step?: Vec3;
  /** Rotation per copy about X, then Y, then Z (default [0, 0, 0]; an angle, -6.283185307179586..6.283185307179586 each) */
  rotate_step?: Vec3;
}

/** Rotates its input about the origin (X, then Y, then Z). */
export interface XformRotateV1Params {
  /** Rotation about each axis in radians (required; an angle, -10000..10000 each) */
  angles: Vec3;
}

/** Scales its input about the origin. */
export interface XformScaleV1Params {
  /** Scale factor per axis; positive only (required; a ratio, 0.000001..1000000 each) */
  by: Vec3;
}

/** Slants a solid: the top moves sideways by x_shift and z_shift (metres) relative to the bottom, in proportion to height, like a leaning tower or a parallelogram. */
export interface XformShearV1Params {
  /** How far the top moves along X relative to the bottom (default 0; metres, -100..100) */
  x_shift?: number | Node<'Number'>;
  /** How far the top moves along Z relative to the bottom (default 0; metres, -100..100) */
  z_shift?: number | Node<'Number'>;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Tapers a solid along its local Y extent. */
export interface XformTaperV1Params {
  /** X/Z scale at the top relative to the bottom (default 1; a ratio, 0.1..4) */
  top_scale?: number | Node<'Number'>;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Twists a solid around its local Y extent. */
export interface XformTwistV1Params {
  /** Total twist from the bottom to top of the solid in radians (default 0; an angle, -6.283185307179586..6.283185307179586) */
  angle?: number | Node<'Number'>;
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Ripples a solid like a flag or a wavy wall: points move sideways by a sine wave that runs up the solid's height (Y). Subdivide the shape (or let the tolerance refine it) so there are vertices to ripple. */
export interface XformWaveV1Params {
  /** How far the crests move points sideways, in metres (default 0.1; metres, -100..100) */
  amplitude?: number | Node<'Number'>;
  /** Height of one full wave in metres (default 1; metres, 0.01..1000) */
  wavelength?: number | Node<'Number'>;
  /** Where in the wave the bottom starts, in radians (default 0; an angle, -6.283185307179586..6.283185307179586) */
  phase?: number | Node<'Number'>;
  /** Which way points move: x or z (default "x") */
  direction?: 'x' | 'z';
  /** Maximum sampled edge chord deviation in metres; refinement is bounded to two levels (default 0.01; metres, 0.0005..0.1) */
  tolerance?: number | Node<'Number'>;
}

/** Every ready operator, by family and name: `ve.poly.select(wall, { element: 'face' })`. */
export interface Ve {
  anim: {
    /** Bakes a physical fall into ordinary animation keys: fixed-step gravity, floor contact, bounded bouncing and rest. Contact uses the rest bounding-box bottom, so tumbling does not change contact height. */
    drop<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: AnimDropV1Params): T;
    /** Chooses which named animation clip a part plays in response to game parameters. Write one statement per line, or separate statements with `;`: `entry idle; state idle clip=idle; state run clip=run; transition idle -> run when speed > 1; transition run -> idle when speed < 1`. */
    machine<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: AnimMachineV1Params): T;
    /** Attaches a shape to another part so it moves with it, like a forearm on an upper arm or a wheel on a car. `parent` is the OUTPUT name of the part to follow. */
    parent<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: AnimParentV1Params): T;
    /** Copies the keyframe tracks of one bone part onto another: a walk or wave authored on one character plays on a taller one, a left-arm clip becomes a right-arm clip. `in` is the part that receives the clips (its own tracks stay) and `from` is the part whose tracks are copied; both are usually bones (rig.bone, body.bone). */
    retarget(inputs: { in: Node<'Solid' | 'Poly' | 'Inst'>; from: Node<'Solid' | 'Poly' | 'Inst'> }, params?: AnimRetargetV1Params): Node<'Solid' | 'Poly' | 'Inst'>;
    /** Thins the keyframes of a part's tracks without changing the motion you can see: a clip a script sampled at every few milliseconds drops keys that lie on what the interpolation would give anyway. Every track of the input is thinned while it stays within `tolerance` of the original everywhere. */
    simplify<T extends Node<'Solid' | 'Poly' | 'Inst'>>(input: T, params?: AnimSimplifyV1Params): T;
    /** Makes a shape move: attaches one keyframed track (position, rotation or scale over time) and passes the shape through unchanged. Chain several anim.track nodes to move and turn together. */
    track<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: AnimTrackV1Params): T;
    /** Makes a shape oscillate forever without writing keyframes: a bob, a sway, a pulse, a flicker. It becomes one looping anim.track (so it plays, chains and exports like any track). */
    wave<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: AnimWaveV1Params): T;
  };
  bake: {
    /** Bakes the detail of a high-poly part onto a low-poly part: a tangent-space normal map and an ambient-occlusion map in the low part's UV layout. The result is the low part itself with the maps attached, so it keeps them through moves and materials, shows the normal map in the viewer and exports both in glTF. */
    maps(inputs: { low: Node<'Solid' | 'Poly'>; high: Node<'Solid' | 'Poly'> }, params?: BakeMapsV1Params): Node<'Solid'>;
  };
  behavior: {
    /** Makes a shape react in play mode: when it is clicked (`select`) or the viewer comes near it (`near`), it runs its own anim.track clip. Put it after the anim.track nodes (a switch with a `target` needs no track of its own). */
    on<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: BehaviorOnV1Params): T;
  };
  body: {
    /** A bone of a body.human skeleton, as a part you can animate: tiny and hidden inside the body at the named joint, with its rest segment from that joint (head, where it turns) to the next joint (tail), in the body's world position. Connect the body.human node itself. */
    bone(input: Node<'Poly'>, params: BodyBoneV1Params): Node<'Solid'>;
    /** The two eyes of a body.human, layered like a real eye: a white sclera ball, a coloured iris, a black pupil and a clear glossy cornea over them, sized to the body's eye sockets (a child's eyes are smaller) and looking in the gaze direction. Connect the body.human node itself (before solid.material), and give the result its own output. */
    eye(input: Node<'Poly'>, params?: BodyEyeV1Params): Node<'Solid'>;
    /** Clothes for a body.human: the part of its surface a garment covers (a shirt, trousers, shoes), lifted off the skin by a few millimetres, as its own surface to give a fabric with solid.material and solid.texture. Connect the body.human node itself. */
    garment(input: Node<'Poly'>, params?: BodyGarmentV1Params): Node<'Solid'>;
    /** Hair grown on the head of a body.human: thousands of tapered ribbon wisps that start on that head's own scalp inside a natural hairline (high forehead, temples, above the ears, down to the nape), leave it outward, fall under gravity, slide over the skull and wave a little. Connect the body.human node itself (before solid.material) and give the result its own output, then colour it with solid.material and add solid.finish(anisotropy=0.6, clearcoat=0.2) for the shine along the strands. */
    hair(input: Node<'Poly'>, params?: BodyHairV1Params): Node<'Solid'>;
    /** A real human body and head with true proportions, from named sliders: one skinned-ready surface about 27,000 triangles (13,700 vertices) with UVs, standing with its origin at the pelvis, facing +z, in metres. It is built from open CC0 anthropometric data (MakeHuman/Anny), not sculpted. */
    human(params?: BodyHumanV1Params): Node<'Poly'>;
    /** Real clothes or hair made by artists (the CC0 MakeHuman library), fitted to a body.human: it is the real garment mesh with its own UVs, bound to the body's surface, so it follows every body slider (height, weight, muscle, gender) and sits on the skin the way the artist made it. `asset` picks one: clothes `female_casualsuit01` (a T-shirt and jeans), `shoes02`, hair `long01`, `bob02`, `short02`. */
    outfit(input: Node<'Poly'>, params: BodyOutfitV1Params): Node<'Solid'>;
    /** Skins a body.human to the bones you list, with the pack's own skin weights (the real ones, so knuckles, elbows and shoulders bend properly, unlike rig.skin's distance weights): each vertex follows the strongest four bones that move it. A bone you do not list hands its share to its nearest listed ancestor, so list only the joints you animate (an upper body with fingers is about 55 bones, the legs can stay with `root`). */
    skin<T extends Node<'Poly'>>(input: T, params: BodySkinV1Params): T;
    /** Transfers body.skin weights to a cloth solid from its nearest body surface vertex. */
    skin_to(inputs: { cloth: Node<'Solid'>; body: Node<'Poly'> }, params?: BodySkinToV1Params): Node<'Solid'>;
  };
  building: {
    /** A close-view, rule-built building asset. Twelve architectural grammars change window proportions, floor rhythm, vertical elements and roof equipment. */
    asset(params?: BuildingAssetV1Params): Node<'Solid'>;
  };
  check: {
    /** Finds solid parts separated from their nearest neighbour by a gap larger than the tolerance. */
    attached(input: Node<'Solid'>[], params?: CheckAttachedV1Params): Node<'Check'>;
    /** Fails when the wall between an outer and an inner profile is thinner than `min`. */
    min_wall(inputs: { outer: Node<'Profile'>; inner: Node<'Profile'> }, params: CheckMinWallV1Params): Node<'Check'>;
    /** Samples the named parts through a clip to find feet that slide while planted and loops that pop at the seam. `feet` is a comma-separated list of part output names; `speed` is the travel speed of an in-place clip along +x. */
    motion(input: Node<'Solid'>[], params?: CheckMotionV1Params): Node<'Check'>;
  };
  city: {
    /** Every building of a modern district: a shop floor, a shaft that steps back in tiers when tall, and a flat roof with rooftop clutter, on every plot of an irregular street grid, tallest downtown. Wall UVs are facade units (u every 8 m along a wall, v every three floors) so ONE tiling facade texture fits every building. */
    buildings(params?: CityBuildingsV1Params): Node<'Solid'>;
    /** The ground of a modern district: the road surface under everything and the raised pavement block by block, from the same streets as city.buildings (use the same size, block, street_width, lot sizes, irregularity and seed). UVs are metres / 8, so a tiling texture keeps its scale. */
    ground(params?: CityGroundV1Params): Node<'Solid'>;
  };
  cloth: {
    /** Drops a flat panel onto a body and lets it settle under gravity, like cloth: a cape over shoulders, a tunic over a torso, a sheet over a table. The panel is a polygon mesh (a poly.grid, bent or moved into place above the body); the result keeps its faces and UVs and only its points move. */
    drape(inputs: { in: Node<'Poly'>; pins?: Node<'Selection'>; with?: Node<'Solid' | 'Poly' | 'Volume'> }, params?: ClothDrapeV1Params): Node<'Poly'>;
    /** Makes a garment live cloth that moves while the scene runs: a cape that streams behind a running figure, a banner in the wind, a skirt that swings. The garment is a polygon mesh (made with poly.grid or any polygon node, shaped as you like); it keeps its shape in the editor and is simulated every frame in the viewer. */
    sim(inputs: { in: Node<'Poly'>; pins?: Node<'Selection'> }, params?: ClothSimV1Params): Node<'Poly'>;
  };
  curve: {
    /** A cubic Bézier curve sampled deterministically. */
    bezier(params: CurveBezierV1Params): Node<'Curve'>;
    /** A closed, evenly sampled circle in the XZ plane. */
    circle(params?: CurveCircleV1Params): Node<'Curve'>;
    /** A space curve written as a program of `t`, from 0 to 1 along the curve, sampled `samples` times (also `i`, the sample number, and `count`). `out` is the point: a helix is `out = vec3(cos(t * 6.2832 * 3.0), t * 2.0, sin(t * 6.2832 * 3.0))`. */
    expr(params?: CurveExprV1Params): Node<'Curve'>;
    /** Rounds the corners of a curve (a polyline path with sharp bends becomes a smooth pipe route). Each interior corner is replaced by a smooth curve that starts `distance` before the corner and ends `distance` after it. */
    fillet(input: Node<'Curve'>, params?: CurveFilletV1Params): Node<'Curve'>;
    /** Fits a smooth curve to sketched or freehand points: give the points a pen or a mouse would trace (or measured data) and get a chain of cubic Béziers that stays within `tolerance` of every one of them, sampled `steps` times per piece, instead of the jagged polyline. Where the path turns sharper than `corner_angle` it is cut and each side fitted on its own, so a corner stays a corner and a smooth bend stays smooth; repeated points are ignored. */
    fit(params: CurveFitV1Params): Node<'Curve'>;
    /** A catenary rope hanging under gravity between two tie points. */
    hang(params?: CurveHangV1Params): Node<'Curve'>;
    /** A helix about a vertical axis (a spring, a corkscrew, a spiral staircase rail, a coil). Differing end_radius makes a cone-shaped spiral. */
    helix(params?: CurveHelixV1Params): Node<'Curve'>;
    /** A space curve through ordered points. */
    polyline(params: CurvePolylineV1Params): Node<'Curve'>;
    /** Replaces a curve by evenly spaced points along its length (same shape, uniform spacing), for even rungs, rails and repeats. */
    resample(input: Node<'Curve'>, params?: CurveResampleV1Params): Node<'Curve'>;
    /** Reverses the direction of a curve (swap which end is the start), which flips which end a sweep starts from. */
    reverse(input: Node<'Curve'>): Node<'Curve'>;
    /** A smooth curve that passes through every point you give (a centripetal Catmull-Rom spline: no loops or overshoot between points). Use it for a winding path, a pipe or rope route, a rail, an organic outline. */
    spline(params: CurveSplineV1Params): Node<'Curve'>;
    /** A round tube along a curve for trunks, branches, vines, pipes, roots, and cables. The radius runs from radius to end_radius along the length (a taper), or through the stations of radii. */
    tube(input: Node<'Curve'>, params?: CurveTubeV1Params): Node<'Solid'>;
  };
  detail: {
    /** Panel lines and greebles on one face of a solid: a grid of plates laid on the face, some standing out, some vented, the seams between them left open. It makes a NEW solid of just the plates (the input is not part of it), sitting exactly on the face so it touches the wall (join it with solid.union, texture it with material.preset, check it with check.attached). */
    panels(input: Node<'Solid'>, params?: DetailPanelsV1Params): Node<'Solid'>;
  };
  field: {
    /** Bends a field: the axis curves into a circular arc toward `toward` through `angle` degrees between `from` and `to` (the point at `from` stays put; beyond `to` the axis continues straight along the end tangent): a tail, a horn, an arm. Example: field.bend@1(f, axis="y", toward="x", from=-0.5, to=0.5, angle=90). */
    bend(input: Node<'Field'>, params?: FieldBendV1Params): Node<'Field'>;
    /** Exact rounded box field centered at the origin. */
    box(params: FieldBoxV1Params): Node<'Field'>;
    /** Exact capsule field along the Y axis. */
    capsule(params: FieldCapsuleV1Params): Node<'Field'>;
    /** Exact finite cylinder field along the Y axis. */
    cylinder(params: FieldCylinderV1Params): Node<'Field'>;
    /** Axis-aligned ellipsoid; unequal radii use a conservative distance lower bound. */
    ellipsoid(params: FieldEllipsoidV1Params): Node<'Field'>;
    /** A function of position, written as a short program: `out = ...` is the result, built from the position `p`, the surface normal `n` (`n.y` is 1 on flat ground and falls on slopes), `uv`, numbers, vec2/vec3/vec4, swizzles (`p.xz`), + - * / %, comparisons, && || !, and the functions abs sign floor ceil fract sqrt exp log pow sin cos atan2 min max clamp mix step smoothstep length distance dot cross normalize select, ramp(x, stop, value, ...) and curve(x, stop, value, ...) for piecewise-linear curves, and the noises perlin(p, seed), fbm(p, octaves, seed), ridged(p, octaves, seed), voronoi(p, seed). Statements are separated by `;`, for example `h = smoothstep(10.0, 40.0, p.y); out = h * select(n.y > 0.9, 1.0, 0.0)`. */
    expr(inputs: { a?: Node<'Pattern'>; b?: Node<'Pattern'>; c?: Node<'Pattern'>; d?: Node<'Pattern'> }, params?: FieldExprV1Params): Node<'Pattern'>;
    /** Hard intersection of two fields. */
    intersect(inputs: { a: Node<'Field'>; b: Node<'Field'> }): Node<'Field'>;
    /** A terrain layer as a pattern: the value of a named layer of a terrain tile (made by terrain.mask, erosion or a river) at any position, smoothly interpolated, 0 outside the tile. Use it to let one mask drive several things (scatter, other patterns). */
    mask_from(input: Node<'Poly'>, params?: FieldMaskFromV1Params): Node<'Pattern'>;
    /** Mirrors a field across selected origin planes. */
    mirror(input: Node<'Field'>, params: FieldMirrorV1Params): Node<'Field'>;
    /** Translates a field by an XYZ offset. */
    move(input: Node<'Field'>, params: FieldMoveV1Params): Node<'Field'>;
    /** Displaces a field surface by bounded deterministic smooth or ridged noise. */
    noise(input: Node<'Field'>, params?: FieldNoiseV1Params): Node<'Field'>;
    /** Offsets the zero surface along its field value. */
    offset(input: Node<'Field'>, params?: FieldOffsetV1Params): Node<'Field'>;
    /** Normalized half-space plane; bounded meshing requires a closed field composition. */
    plane(params: FieldPlaneV1Params): Node<'Field'>;
    /** Boundedly repeats a field around the origin with folded coordinates. */
    repeat(input: Node<'Field'>, params: FieldRepeatV1Params): Node<'Field'>;
    /** Uniformly scales a field around the origin. */
    scale(input: Node<'Field'>, params?: FieldScaleV1Params): Node<'Field'>;
    /** Creates a symmetric shell around the zero surface. */
    shell(input: Node<'Field'>, params: FieldShellV1Params): Node<'Field'>;
    /** Polynomial smooth union; its distance guarantee is None. */
    smooth_union(inputs: { a: Node<'Field'>; b: Node<'Field'> }, params: FieldSmoothUnionV1Params): Node<'Field'>;
    /** Exact sphere field centered at the origin. */
    sphere(params: FieldSphereV1Params): Node<'Field'>;
    /** Subtracts field B from field A. */
    subtract(inputs: { a: Node<'Field'>; b: Node<'Field'> }): Node<'Field'>;
    /** Tapers a field along an axis: the cross-section is scaled linearly from `scale_start` at `from` to `scale_end` at `to` (held beyond): a nose, a horn, a column that narrows. Example: field.taper@1(f, axis="y", from=-0.5, to=0.5, scale_start=1, scale_end=0.5). */
    taper(input: Node<'Field'>, params?: FieldTaperV1Params): Node<'Field'>;
    /** Meshes a field over explicit bounds and edge length. */
    to_solid(input: Node<'Field'>, params: FieldToSolidV1Params): Node<'Solid'>;
    /** Exact torus field around the Y axis. */
    torus(params: FieldTorusV1Params): Node<'Field'>;
    /** Tube around a bounded ordered polyline: constant radius, or tapering from `radius` at the first point to `end_radius` at the last (a limb, tail or horn). */
    tube(params: FieldTubeV1Params): Node<'Field'>;
    /** Twists a field about an axis: the cross-section turns by 0 degrees at `from` up to `angle` degrees at `to` (held beyond), keeping the volume: a tusk, a shell, a rope. Example: field.twist@1(f, axis="y", from=-0.5, to=0.5, angle=90). */
    twist(input: Node<'Field'>, params?: FieldTwistV1Params): Node<'Field'>;
    /** Hard union of two or more spatial fields. */
    union(input: Node<'Field'>[]): Node<'Field'>;
  };
  flora: {
    /** A real tree: a branching trunk with species bark, smooth curved limbs and a lobed root flare, and leaf cards with cut-out leaf shapes that let the sun through, grown from a named species (oak, pine, birch, willow, shrub, palm) at the height you ask, the same tree for the same seed. Its base is at y = 0, so it plants straight into scatter.place. */
    tree(params?: FloraTreeV1Params): Node<'Solid'>;
  };
  fluid: {
    /** A liquid that runs while the scene runs: water pouring into a basin, a wave in a tank, honey sliding down a ramp. `min` and `max` are the corners of the block of liquid at the start; `spacing` is how far apart its particles are (0.05 m with a 0.4 x 0.5 x 0.4 m block is 640 particles; keep it between about 1,000 and 8,000 particles for 60 frames a second). */
    sim(params?: FluidSimV1Params): Node<'Poly'>;
  };
  frame: {
    /** Declares one unique named rigid socket; rotations are X, then Y, then Z. */
    add(input: Node<'Solid'>, params: FrameAddV1Params): Node<'Solid'>;
  };
  fx: {
    /** Makes an emitter's particles hit the ground (a flat plane at `ground`): they bounce, stick or disappear. Sparks bounce, rain splashes (kill, with fx.sub on collide), embers stick. */
    collide<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: FxCollideV1Params): T;
    /** Makes this shape EMIT particles (fire, smoke, sparks, dust, rain, magic): it gives birth to small sprites that fly, fade and die. The effect rides on the shape, so it follows it when the shape moves (anim.track, anim.parent, carried in a hand). */
    emitter<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: FxEmitterV1Params): T;
    /** Adds a force to an emitter's particles: gravity, wind, drag, a vortex, an attractor, or turbulence (swirling curl noise with no sources or sinks: the way to make smoke and fire look alive). Several forces add up. */
    force<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: FxForceV1Params): T;
    /** Makes particles of one emitter spawn particles of another emitter of the same effect when they are born, die or hit the ground: smoke from dying embers, a splash from each raindrop (parent kill-on-collide), sparks from a fuse. The child emitter usually has rate=0. */
    sub<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: FxSubV1Params): T;
    /** Makes an emitter's particles remember the path they took (the last `points` positions). Needed for render="ribbon": streaks, comet tails, sword swooshes, tracer bullets. */
    trail<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: FxTrailV1Params): T;
  };
  game: {
    /** A button-triggered action with a cost and a cooldown, for the owner's ability bar. Pressing its input (game.input@1 with key="press") runs the do actions with a = the owner and b = its target (game.targeting@1), pays cost from the owner's prop cost_prop, and starts the cooldown; the owner's props cd_NAME (seconds left) and cdmax_NAME drive a slot on screen (game.ui@1 type="slot"). */
    ability(params: GameAbilityV1Params): Node<'Game'>;
    /** Make a kind patrol, notice and chase the hero. Example: `watch = game.agent@1(kind="creature", patrol=[[0, 0], [6, 0]], speed=1.5)`. */
    agent(params: GameAgentV1Params): Node<'Game'>;
    /** Make a kind move and fight on its own: companions and enemies. With a foe (an entity matching foes that has health) within engage metres it closes to its weapon's reach (keep metres for a ranged weapon) and uses the weapon (game.weapon@1, aim at the foes); with none it follows its leader to within follow metres. */
    brain(params: GameBrainV1Params): Node<'Game'>;
    /** Say how the view follows the player in play mode. The first camera starts active; a rule switches with the action `effect:camera:NAME` (cut) or `effect:camera:NAME:SECONDS` (blend). */
    camera(params: GameCameraV1Params): Node<'Game'>;
    /** Join game fragments in link order. Example: `all = game.collect@1(score, hero, spawn, rule, goal, name="coins")`. */
    collect(input: Node<'Game'>[], params?: GameCollectV1Params): Node<'Game'>;
    /** Move a kind from the player's input like a character: it speeds up and slows down, turns, falls under gravity, jumps (variable height, extra jumps, a short grace after leaving a ledge, a jump pressed just before landing still counts), lands on the ground and on the tops of solid boxes, and can move relative to the camera. The entity's y is its feet. */
    controller(params: GameControllerV1Params): Node<'Game'>;
    /** Declare a terminal condition. Example: `won = game.goal@1(outcome="win", when="score>=8")`. */
    goal(params: GameGoalV1Params): Node<'Game'>;
    /** Set the ground plane height. Example: `ground = game.ground@1(y=0.25)`. */
    ground(params?: GameGroundV1Params): Node<'Game'>;
    /** Give a kind hit points. Its entities get the props health and max_health (bind a bar to them with game.ui@1: value="hero.health", max_var="hero.max_health"). */
    health(params: GameHealthV1Params): Node<'Game'>;
    /** Show text bound to game variables as a procedural pixel HUD. Example: `panel = game.hud@1(text="Score {score}", anchor="gaze", size=0.005)`. */
    hud(params: GameHudV1Params): Node<'Game'>;
    /** Declare a player axis or press input. Example: `move = game.input@1(name="move", axis="x,z", target="tag:hero")`. */
    input(params: GameInputV1Params): Node<'Game'>;
    /** Declare an entity kind. Examples: `hero = game.kind@1(name="player", size=0.5, speed=4, tags=["hero"])`; `ball = game.kind@1(name="ball", size=0.03, roll=true)` rolls on a table or field; `tool = game.kind@1(name="tool", swing=true, reach=0.9, loft=20, power=1.2)` swings into it; `pocket = game.kind@1(name="pocket", size=0.07, pocket=true)` captures it. */
    kind(params: GameKindV1Params): Node<'Game'>;
    /** Declare a level: spawns with level=NAME belong to it. The first one declared loads at the start; the action level:NAME replaces the loaded level's entities with the named level's and puts every hero (tag:hero) at its start. */
    level(params: GameLevelV1Params): Node<'Game'>;
    /** Declare the level parts to build navigation from. Example: `nav = game.nav@1(parts=["floor", "stairs"], cell=0.2)`. */
    nav(params: GameNavV1Params): Node<'Game'>;
    /** Make a kind a shot: its entities fly, hurt the first victim (an entity with health) they meet, are used up, and stop at walls (kinds with solid=...) or when their life ends (raising impact). pierce lets it pass through that many extra victims; splash hurts neighbours within that radius (less with distance); homing turns it toward the nearest victim by that many degrees a second; gravity bends the flight. */
    projectile(params: GameProjectileV1Params): Node<'Game'>;
    /** Declare an event rule and semicolon-separated actions. Example: `pick = game.rule@1(on="touch", a="tag:hero", b="tag:coin", do="add:score:1;destroy:b")`. */
    rule(params: GameRuleV1Params): Node<'Game'>;
    /** A JavaScript behaviour that runs inside the game's fixed step, for anything the typed game nodes do not say. The source runs once when the game starts and registers handlers: `on(event, fn)`, `on(event, {a: "tag:hero", b: "tag:coin"}, fn)`, `every(seconds, fn)`, `after(seconds, fn)`. */
    script(params?: GameScriptV1Params): Node<'Game'>;
    /** Place entities of a declared kind, at a point or one per placement of a set (scatter.place, inst.array ...), standing exactly where each placement is, so pickups sit on hilly ground. heading sets the initial facing in degrees. */
    spawn(input: Node<'Inst'> | undefined, params: GameSpawnV1Params): Node<'Game'>;
    /** Declare a state of the game's flow, such as title, playing, paused, game_over or win. The first one declared is where the game starts. */
    state(params: GameStateV1Params): Node<'Game'>;
    /** Declare a surface for a rolling ball on a table. Example: `felt = game.surface@1(name="felt", parts=["table"], decel=0.5, bounce=0.5)`; parts are the output names of authored parts. */
    surface(params: GameSurfaceV1Params): Node<'Game'>;
    /** Choose the leader's target. The leader's prop target is the id of the foe it faces (the nearest inside the cone, else the nearest of all), kept until that foe dies or leaves range; the foe's prop targeted is 1, so the UI can show it (text="{targeted.health}", a bar with value="targeted.health"). */
    targeting(params: GameTargetingV1Params): Node<'Game'>;
    /** Declare ground sampling for rolling things on a field. Example: `terrain = game.terrain@1(parts=["field", "path"], cell=0.25)`; parts are the output names of authored parts. */
    terrain(params: GameTerrainV1Params): Node<'Game'>;
    /** One element of the game's screen UI (title, pause menu, HUD, prompts, minimap, dialogue choices). Drawn over the 3D view in fractions of the screen height (x runs from 0 to the aspect ratio) with a procedural font, so it needs no assets. */
    ui(params: GameUiV1Params): Node<'Game'>;
    /** Declare a numeric game variable. Example: `score = game.var@1(name="score", value=0)`. */
    var(params: GameVarV1Params): Node<'Game'>;
    /** A weapon carried by the entities matching owner. It fires when its input (game.input@1 with key="press") is pressed, held when auto is on, or when a rule runs fire:NAME (an enemy: `game.rule@1(on="sight", a="kind:turret", do="fire:spit@a")`), and keeps a magazine in the owner's prop ammo_NAME. */
    weapon(params: GameWeaponV1Params): Node<'Game'>;
  };
  group: {
    /** A solid that an instance of the group supplies, by this name. */
    input(params: GroupInputV1Params): Node<'Solid'>;
    /** An instance of a group: the group's result, with the parameters it exposes. */
    use(): Node<'Solid'>;
  };
  inst: {
    /** Shared copies of a shape spaced evenly by length along a curve, each turned to follow it (a fence along a path, rungs along a rail, beads on a wire, links of a chain). An open curve gets one copy at each end and the rest evenly between; a closed curve spaces them all the way round. */
    along_curve(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; path: Node<'Curve'> }, params?: InstAlongCurveV1Params): Node<'Inst'>;
    /** Shared grid of copies, no union. spacing is the distance between neighbours in metres, or with relative=true a multiple of the shape's own size along each axis (spacing [1.1, 0, 0] leaves a tenth of the width between copies, whatever the shape's size). */
    array(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstArrayV1Params): Node<'Inst'>;
    /** Sets every placement down onto a surface (trees, rocks and grass on a terrain; props on a table): each copy slides straight down onto the highest point of `surface` under it. Scatter over the surface's footprint first, then drop. */
    drop(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; surface: Node<'Solid'> }, params?: InstDropV1Params): Node<'Inst'>;
    /** Varies each placement by a bounded random shift, turn and scale; element i depends only on the seed and i. */
    jitter(input: Node<'Solid' | 'Inst' | 'Poly'>, params?: InstJitterV1Params): Node<'Inst'>;
    /** Original placements and their reflection, with corrected winding. */
    mirror(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstMirrorV1Params): Node<'Inst'>;
    /** Copies of a shape at positions you list, as one shared set (the shape is stored once): six jars at chosen spots, a sack here, a lantern there. Add yaw_scale for a turn and size per copy. */
    place(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstPlaceV1Params): Node<'Inst'>;
    /** Shared copies placed by programs of the copy number, in the field-expression language of field.expr: `code` gives where each copy goes (`out` a vec3), `scale_code` its size (a number above 0), `yaw_code` its turn about Y in degrees. They see `i` (0 for the first copy), `count` and `f` (0 for the first copy, 1 for the last). */
    program(input: Node<'Solid' | 'Inst' | 'Poly'>, params?: InstProgramV1Params): Node<'Inst'>;
    /** Shared copies around Y; no union. */
    radial(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstRadialV1Params): Node<'Inst'>;
    /** Explicitly union placements, after geometry/storage preflight. */
    realize(input: Node<'Inst'>): Node<'Solid'>;
    /** Shared copies rotated then translated; no union. */
    repeat(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstRepeatV1Params): Node<'Inst'>;
    /** Seeded scatter over a width x depth rectangle in the XZ plane centred on the origin; min_spacing keeps centres apart or the node refuses. */
    scatter(input: Node<'Solid' | 'Inst' | 'Poly'>, params: InstScatterV1Params): Node<'Inst'>;
    /** Seeded scatter over the surface of a solid, in any orientation (pebbles on a hill's sides, bolts on a tank, moss on a wall): points are drawn uniformly by area over `surface`, and with align each copy's +Y axis follows the surface normal there. min_distance keeps copies apart or the node refuses (it never places fewer than count). */
    scatter_on(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; surface: Node<'Solid'> }, params?: InstScatterOnV1Params): Node<'Inst'>;
    /** Varies every placement from patterns sampled at its position: scale, world-up yaw and lift. Example: `s = field.expr@1(code="out = 1.0 + step(10.0, p.x)"); v = inst.vary@1(in, scale=s)` doubles copies to the right of x=10. */
    vary(inputs: { in: Node<'Inst'>; scale?: Node<'Pattern'>; yaw?: Node<'Pattern'>; lift?: Node<'Pattern'> }): Node<'Inst'>;
  };
  light: {
    /** A point light with bounded range. */
    point(params: LightPointV1Params): Node<'Light'>;
    /** A cone light with smooth inner and outer angles. */
    spot(params: LightSpotV1Params): Node<'Light'>;
    /** A directional sun; direction points from the light toward the scene. */
    sun(params: LightSunV1Params): Node<'Light'>;
  };
  material: {
    /** Applies a named tiled procedural surface with optional edge wear, crease dirt, top-facing moss and wetness. oak: warm wood; planks: boards; red_brick: fired brick; cobblestone: rounded stone; concrete: cast grey stone; plaster: pale wall; rusted_iron: oxidized metal; brushed_steel: satin steel; leather: pebbled hide; linen: woven cloth; sand: fine grains; mud: damp earth; snow: soft snow; ice: blue ice; asphalt: dark road surface (give it wet for a night street); corrugated_metal: ribbed steel sheet; painted_metal: dark painted panel that shows bare metal on its edges; wall_tile: small glazed tiles. */
    preset<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: MaterialPresetV1Params): T;
  };
  phys: {
    /** Makes a shape a physical object in play mode: it falls under gravity, collides with other bodies, and can be picked up and thrown. kind=static for floors, walls and terrain (never moves), dynamic for things that fall, roll and stack (balls, crates, props), kinematic for things that move by hand or script and shove others. */
    body<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: PhysBodyV1Params): T;
    /** Connects a physics body to another part or to the world with a joint, for mechanisms: a door on hinges, a pendulum, a drawer, a rope link, a wheel on an axle, a spring-hung weight. Put it AFTER phys.body on the part that moves; `to` is the OUTPUT name of the other part (which must also have phys.body) or empty to join this part to the world. */
    joint<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: PhysJointV1Params): T;
    /** Makes a rigged character (a skeleton of rig.bone parts, each anim.parent-ed to the one above it) able to go LIMP: on command it stops following its animation and falls and tumbles as a ragdoll, colliding with every phys.body in the scene, starting from whatever pose it was in. Put this on any small shape (a marker is fine); `bones` lists the OUTPUT names of the bone parts, parents first, for example "pelvis; spine; head; arm_l; forearm_l; thigh_l; shin_l". */
    ragdoll<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: PhysRagdollV1Params): T;
    /** Sets the physics of the whole scene: gravity, and whether you WALK through it in play mode. Put it after phys.body on any ONE body (the floor is natural); a second one is ignored with a warning. */
    world<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: PhysWorldV1Params): T;
  };
  poly: {
    /** Flips the shared edge of triangle pairs where Blender's beauty measure prefers the other diagonal, to even out a triangulated mesh before smoothing or displacing it. The measure is Blender's own (by area or by angle); on a given quad the two can disagree, and a pair it already likes is left alone. */
    beautify(input: Node<'Poly'>, params?: PolyBeautifyV1Params): Node<'Poly'>;
    /** Rounds or chamfers selected edges. Edges must be disjoint, between two planar faces, with three faces at each end. */
    bevel_edges(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyBevelEdgesV1Params): Node<'Poly'>;
    /** Rounds or chamfers selected edges of any layout: single edges, loops around a box, edges meeting at a corner, a whole cube. Select edges with poly.select (element=edge; by=sharp picks the hard edges). */
    bevel_network(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyBevelNetworkV1Params): Node<'Poly'>;
    /** Cuts the whole polygon mesh with a plane; above is the side the normal points toward. */
    bisect(input: Node<'Poly'>, params?: PolyBisectV1Params): Node<'Poly'>;
    /** Joins two boundary loops with the same number of vertices by a ring of quads (select two loops). */
    bridge_loops(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Reshapes the selected edge loops into circles: an oval hole becomes round, a ragged ring a clean one (select the loop's edges with poly.select element=edge). Radius 0 fits a circle to the loop's own size. */
    circularize(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolyCircularizeV1Params): Node<'Poly'>;
    /** Adds edges between the selected vertices of each face, splitting the face along them (select vertices with poly.select element=vertex; at least two, and two on the same face to make a cut). Cuts a quad across its diagonal or a polygon into pieces. */
    connect_vertices(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolyConnectVerticesV1Params): Node<'Poly'>;
    /** Reduces the face count by collapsing the edges that change the shape least: level of detail (LOD) for far-away or cheap versions of a model. ratio is the fraction of faces kept (0.25 keeps a quarter). */
    decimate(input: Node<'Poly'>, params: PolyDecimateV1Params): Node<'Poly'>;
    /** Removes selected faces and leaves an open boundary. Refuses to delete every face or isolate a vertex. */
    delete_faces(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Removes edges shorter than a distance and faces with almost no area, which cause shading glitches and failed operations after a script or a boolean. */
    dissolve_degenerate(input: Node<'Poly'>, params?: PolyDissolveDegenerateV1Params): Node<'Poly'>;
    /** Removes one interior edge and merges its two faces (select one edge). */
    dissolve_edge(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Merges a connected patch of selected faces into one face (select faces). */
    dissolve_faces(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Merges neighbouring faces that are flatter than an angle: cleans triangulated or over-detailed flat areas back into big polygons. */
    dissolve_limit(input: Node<'Poly'>, params?: PolyDissolveLimitV1Params): Node<'Poly'>;
    /** Removes one vertex and merges its faces (select one vertex). */
    dissolve_vertex(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Pushes selected planar faces out by an offset and joins their border to the mesh with side faces. A connected patch moves as one; with individual each face moves on its own. */
    extrude_faces(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyExtrudeFacesV1Params): Node<'Poly'>;
    /** Extrudes the selected faces and moves the new faces by an offset, leaving walls around them. Select faces with poly.select. */
    extrude_region(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyExtrudeRegionV1Params): Node<'Poly'>;
    /** Closes the open boundary loops of a mesh with faces (a hole left by deleting faces, an open end). A hole with more sides than max_sides stays open, so a big opening is not filled by accident. */
    fill_holes(input: Node<'Poly'>, params?: PolyFillHolesV1Params): Node<'Poly'>;
    /** Reverses the winding of selected faces. Whole connected components always work; a partial flip may not add inconsistent edges unless allow_inconsistent is true. */
    flip_faces(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolyFlipFacesV1Params): Node<'Poly'>;
    /** Breaks a closed shape into pieces along a random Voronoi pattern, the way a wall, a vase or a boulder shatters. Each node makes ONE piece, number `piece` of `pieces`; write a loop over piece = 0 to pieces - 1 to make them all, each with its own output, then give each a physics body (phys.body, shape convex) to let them fall, or move them apart with xform.move to show the break. */
    fracture(input: Node<'Solid' | 'Poly'>, params?: PolyFractureV1Params): Node<'Poly'>;
    /** Turns a solid into an editable polygon mesh of its triangles. Merge coplanar triangles with poly.dissolve_faces. */
    from_solid(input: Node<'Solid'>): Node<'Poly'>;
    /** A flat grid of quads in the z = 0 plane with normalized u/v corner UVs, width wide centred on x = 0 and height tall centred on y = 0 (so it spans -height/2 to height/2: move it up to stand it on the floor), facing +z. */
    grid(params: PolyGridV1Params): Node<'Poly'>;
    /** Brings your own mesh file into the document as an editable polygon mesh: the file's text goes in `data` and `format` says how to read it (obj, ascii ply or ascii stl). Nothing is fetched and nothing is bundled with the engine; the document carries the text, so the result is the same everywhere. */
    import(params: PolyImportV1Params): Node<'Poly'>;
    /** Shrinks each selected planar convex face inward by a distance, leaving a ring of quads and an inner face. */
    inset_faces(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyInsetFacesV1Params): Node<'Poly'>;
    /** Insets the selected faces: shrinks the region inward by a distance, leaving a ring of faces, and can raise or sink the inner face. Works on any polygon, not only planar convex ones. */
    inset_region(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params: PolyInsetRegionV1Params): Node<'Poly'>;
    /** Joins triangle pairs into quads where the pair is flat and square enough: turns a triangulated mesh back into quads for clean subdivision and editing. Tighter limits join fewer pairs. */
    join_triangles(input: Node<'Poly'>, params?: PolyJoinTrianglesV1Params): Node<'Poly'>;
    /** Builds one level of detail by repeatedly simplifying a polygon mesh; level 0 returns the input. */
    lod(input: Node<'Poly'>, params?: PolyLodV1Params): Node<'Poly'>;
    /** Cuts a closed ring of quad faces with evenly spaced parallel edges. */
    loop_cut(input: Node<'Poly'>, params?: PolyLoopCutV1Params): Node<'Poly'>;
    /** Merges connected coplanar faces into single faces, such as a solid's triangles into its flat faces. A patch that cannot become one simple face (the wall face around a window, a pinch) stays as it is by default, so a cut wall can then be rounded with poly.round_edges. */
    merge_coplanar(input: Node<'Poly'>, params?: PolyMergeCoplanarV1Params): Node<'Poly'>;
    /** Adds the mesh reflected across a plane and welds vertices near the plane. */
    mirror(input: Node<'Poly'>, params?: PolyMirrorV1Params): Node<'Poly'>;
    /** Packs existing corner UV islands into the 0..1 square; UV discontinuities define island boundaries. */
    pack_uv(input: Node<'Poly'>, params?: PolyPackUvV1Params): Node<'Poly'>;
    /** Flattens each face toward its best-fit plane by moving its vertices, so quads that were bent become flat. Neighbouring faces pull on each other, so several passes settle it. */
    planar_faces(input: Node<'Poly'>, params?: PolyPlanarFacesV1Params): Node<'Poly'>;
    /** Adds a vertex in the middle of each selected face (all faces when none are selected) and fans triangles to it, raising a pyramid. */
    poke(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params?: PolyPokeV1Params): Node<'Poly'>;
    /** Runs a short program once for every vertex and uses the result: `out = ...` in the field-expression language. The program sees the vertex position `p`, its smoothed normal `n`, its first UV `uv`, its number `i` (0 for the first vertex) and the vertex count `count`, plus `attr("name")` for any per-vertex number layer of the mesh. */
    program(input: Node<'Poly'>, params?: PolyProgramV1Params): Node<'Poly'>;
    /** Makes every face point the same way, outward from the mesh (or inward), whatever its winding was. Use it after a script, a mirror or a boolean leaves some faces inside out (dark patches, faces seen through). */
    recalc_normals(input: Node<'Poly'>, params?: PolyRecalcNormalsV1Params): Node<'Poly'>;
    /** Smooths the selected edge loops so they flow in a gentler curve (select the loop's edges with poly.select element=edge). More iterations smooth more and can shrink a loop a little. */
    relax_loops(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolyRelaxLoopsV1Params): Node<'Poly'>;
    /** Rebuilds a mesh as clean, even quads of about the size you ask for (Instant Meshes): turns the uneven triangles of a field mesh, a boolean or a script into a tidy quad mesh that subdivides, smooths and deforms well. For smooth organic shapes (creatures, rocks, blobs). */
    remesh_quads(input: Node<'Poly'>, params?: PolyRemeshQuadsV1Params): Node<'Poly'>;
    /** Merges vertices within a distance in metres. With no selection, checks every vertex. */
    remove_doubles(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params: PolyRemoveDoublesV1Params): Node<'Poly'>;
    /** Spins each selected edge to the other diagonal of the two faces it joins (triangle pairs become the other split, quad pairs are re-cut). Select edges with poly.select element=edge; each must join exactly two faces. */
    rotate_edges(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolyRotateEdgesV1Params): Node<'Poly'>;
    /** Rounds every hard edge of a polygon mesh in one step: the edges between faces turned apart by at least angle get a bevel of the given width. Replaces poly.select by=sharp followed by poly.bevel_network. */
    round_edges(input: Node<'Poly'>, params: PolyRoundEdgesV1Params): Node<'Poly'>;
    /** A JavaScript program builds a polygon mesh, for shapes the other nodes cannot describe (a loop with branches, a maze, a rule that grows geometry). Make a `Mesh`: `const m = new Mesh(); const a = m.vertex(x, y, z); ... */
    script(input?: Node<'Poly'>, params?: PolyScriptV1Params): Node<'Poly'>;
    /** Sculpts a mesh with brush strokes, like clay: write the strokes as text, separated by semicolons, and they are applied in order. A stroke is `brush cx cy cz radius strength` (the point on or near the surface in metres, how far the brush reaches, how hard it works); the grab brush is `grab cx cy cz radius dx dy dz` (the vector to move by). */
    sculpt(input: Node<'Poly'>, params?: PolySculptV1Params): Node<'Poly'>;
    /** Chooses faces, edges, vertices or boundary loops of a polygon mesh by a rule, for the edit operators. Choosing nothing refuses (selection.lost); expect=one refuses when several match (selection.ambiguous). */
    select(input: Node<'Poly'>, params?: PolySelectV1Params): Node<'Selection'>;
    /** Skins a skeleton into ONE closed surface of mostly quads (Blender's Skin modifier, the B-Mesh method): give the joints of a body, an animal or a machine frame, each with a position and two radii, and the bones between them, and get a smooth, clean base mesh with proper junctions at every hip, shoulder and branch. Subdivide it with poly.subdivide and shape it with poly.program or poly.sculpt. */
    skin(params?: PolySkinV1Params): Node<'Poly'>;
    /** Evens out a surface by moving every vertex toward the average of its neighbours, a little each pass. Rounds lumpy meshes; too many passes shrinks and softens detail. */
    smooth(input: Node<'Poly'>, params?: PolySmoothV1Params): Node<'Poly'>;
    /** Gives a surface thickness: adds a second surface offset along the normals and joins the two at open edges. With no selection every face is thickened. */
    solidify(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params?: PolySolidifyV1Params): Node<'Poly'>;
    /** Moves the vertices of the selected edge loops to even spacing along each loop's own curve, without changing its shape (select the loop's edges with poly.select element=edge, for example by=height on a tube). Evens out a ring after an uneven cut. */
    space_loops(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolySpaceLoopsV1Params): Node<'Poly'>;
    /** Revolves selected faces or edges, or the whole mesh, around an axis. */
    spin(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params?: PolySpinV1Params): Node<'Poly'>;
    /** Disconnects the faces along the selected edges: each side gets its own vertices, so the edge becomes a hard seam that shades sharp, can take a UV seam, and lets the two sides move apart (select edges with poly.select element=edge, for example by=sharp). */
    split_edges(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }): Node<'Poly'>;
    /** Final-step uniform Catmull-Clark subdivision of a polygon mesh. Preserve and edit the cage before subdivision: v1 polygon edits may refuse curved refined faces and bevel networks. */
    subdivide(inputs: { in: Node<'Poly'>; creases?: Node<'Selection'>; corners?: Node<'Selection'> }, params?: PolySubdivideV1Params): Node<'Poly'>;
    /** Cuts the selected edges into pieces and splits the faces around them: adds loops and detail where you ask (select edges with poly.select element=edge). */
    subdivide_edges(inputs: { in: Node<'Poly'>; selection: Node<'Selection'> }, params?: PolySubdivideEdgesV1Params): Node<'Poly'>;
    /** A surface written as a program of `u` and `v`, each from 0 to 1 across `columns` by `rows` quads (the mesh has UVs, drawn smooth). `out` is the point: a torus is `out = vec3((1.0 + 0.3 * cos(v * 6.2832)) * cos(u * 6.2832), 0.3 * sin(v * 6.2832), (1.0 + 0.3 * cos(v * 6.2832)) * sin(u * 6.2832))`. */
    surface_expr(params?: PolySurfaceExprV1Params): Node<'Poly'>;
    /** Keeps one side of an axis plane through the origin and mirrors it onto the other, so a half model becomes a symmetric whole. */
    symmetrize(input: Node<'Poly'>, params?: PolySymmetrizeV1Params): Node<'Poly'>;
    /** Turns a closed polygon mesh back into a solid (faces are triangulated properly, never fanned). Refuses an open or non-manifold mesh. */
    to_solid(input: Node<'Poly'>): Node<'Solid'>;
    /** Splits selected polygons into triangles using Blender's beauty triangulation; with no selection, triangulates every face. */
    triangulate(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }): Node<'Poly'>;
    /** Reverses a subdivision: removes the vertices a regular grid of quads could be rebuilt without. Use it to get a coarse cage back from a smooth, dense mesh. */
    unsubdivide(input: Node<'Poly'>, params?: PolyUnsubdivideV1Params): Node<'Poly'>;
    /** Unwraps a polygon mesh to UV coordinates (corner attributes u and v) with Blender's LSCM or angle-based flattening and packs the islands into the unit square. Select seam edges with poly.select element=edge (for example by=sharp on a box) to cut a closed shape into islands; an already open mesh needs none. */
    unwrap_uv(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params?: PolyUnwrapUvV1Params): Node<'Poly'>;
    /** Turns the UV layout into a flat mesh you can look at: every face is drawn where its UVs put it on the XY plane (u to the right, v up), so islands show as separate patches, seams as gaps, overlaps as faces drawn on top of each other, stretching as faces that look nothing like their 3D shape. A face whose UVs run clockwise shows its back. */
    uv_layout(input: Node<'Poly'>, params?: PolyUvLayoutV1Params): Node<'Poly'>;
    /** Moves, scales and turns UV coordinates: shift a layout, shrink it, rotate an island. With no selection it changes every UV; with a selection of faces (poly.select element=face, for example by=attribute to take one region) it changes only those faces' UVs, which is how one island is moved out of the way of another or scaled to a different texel density. */
    uv_transform(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params?: PolyUvTransformV1Params): Node<'Poly'>;
    /** Merges vertices closer than a distance. Refuses when a face would collapse. */
    weld(input: Node<'Poly'>, params?: PolyWeldV1Params): Node<'Poly'>;
    /** Turns faces (all when none are selected) into a lattice of struts of a given thickness, like a cage or grille. */
    wireframe(inputs: { in: Node<'Poly'>; selection?: Node<'Selection'> }, params: PolyWireframeV1Params): Node<'Poly'>;
  };
  profile: {
    /** A regular polygon approximating a circle, centred on the origin of the XY plane. */
    circle(params: ProfileCircleV1Params): Node<'Profile'>;
    /** The first profile minus all the others. */
    difference(input: Node<'Profile'>[]): Node<'Profile'>;
    /** Rounds convex corners of a planar profile while preserving its original outer bounds. */
    fillet(input: Node<'Profile'>, params: ProfileFilletV1Params): Node<'Profile'>;
    /** Grows (positive) or shrinks (negative) a profile, with mitred or round corners. */
    offset(input: Node<'Profile'>, params: ProfileOffsetV1Params): Node<'Profile'>;
    /** A closed polygon from points in the XY plane. */
    polygon(params: ProfilePolygonV1Params): Node<'Profile'>;
    /** A rectangle centred on the origin of the XY plane. */
    rect(params: ProfileRectV1Params): Node<'Profile'>;
    /** Rounds every corner of a profile into a smooth curve by repeated corner cutting. */
    smooth(input: Node<'Profile'>, params?: ProfileSmoothV1Params): Node<'Profile'>;
  };
  rig: {
    /** Makes this shape a TARGET that ONE bone part turns to look at (a head, an eye, a turret, a searchlight), in play and in the viewer. `bone` is the OUTPUT name of a part that carries rig.bone: the bone's own direction, from its head to its tail at rest, is what points at the target, and `up` (a direction at rest, default [0, 1, 0]) is the side that stays towards the `pole` so a head does not roll. */
    aim<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigAimV1Params): T;
    /** Marks a part as a bone of a skeleton and records its rest segment, from `head` (the joint it turns about) to `tail`, in world coordinates. Bones are ordinary parts: move them with anim.track, anim.wave and behaviours, and join them with anim.parent (a forearm bone follows an upper-arm bone). */
    bone<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigBoneV1Params): T;
    /** Makes a foot target WALK. Put it after rig.ik on the shape that the leg's bone chain reaches for: instead of you animating that target, it plants on the ground while its body passes over it and swings forward in an arc to the next step (a procedural gait from the body's own motion; Spore did the same for any creature). */
    gait<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigGaitV1Params): T;
    /** Makes this shape a TARGET that a chain of bones reaches for (inverse kinematics): in play and in the viewer, an arm, leg or tentacle built of bone parts bends so its tip follows the target as the target moves (animate it, or make it grabbable and carry it). `bones` lists the OUTPUT names of the bone parts from root to tip; each must carry rig.bone, be anim.parent-ed to the one before, and start where it ends. */
    ik<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigIkV1Params): T;
    /** Adds one Blender-style shape key to a character mesh. Make the target by duplicating the base and sculpting or moving its points; point count and face topology must stay identical. */
    morph(inputs: { base: Node<'Solid' | 'Inst' | 'Poly'>; target: Node<'Solid' | 'Inst' | 'Poly'> }, params: RigMorphV1Params): Node<'Solid' | 'Inst' | 'Poly'>;
    /** Gives a chain of bones secondary motion: a tail, ear, antenna, hair or cloth edge that lags behind the body and springs back, from a spring simulation run every frame in the viewer (inertia, damping, gravity and a pull back to the rest pose). The shape you attach it to only carries it (use the tip bone). */
    secondary<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigSecondaryV1Params): T;
    /** Makes a shape bend with moving bones: each vertex follows the bones nearest to it (a blend, so a tube bends smoothly at a joint). `bones` lists the OUTPUT names of parts that carry rig.bone. */
    skin<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: RigSkinV1Params): T;
  };
  river: {
    /** Cuts a river channel into a terrain.tile along a path: a bed below the ground, banks that blend back into the land, and the data the water needs (flow direction and speed, water height, channel and bank layers). The path is a list of world x,z points written as text, in the direction the water flows, for example path="10,64; 60,70; 118,64"; give every tile of a world the same path and the river continues across tiles. */
    carve(input: Node<'Poly'>, params?: RiverCarveV1Params): Node<'Poly'>;
    /** The water surface of the rivers carved into a terrain.tile by river.carve: a smooth sheet at the water height with flow direction, depth and shore foam carried per vertex, drawn with a flowing, depth-tinted, foamy water look that needs no texture. It fades to nothing where it meets the bank, so it has no hard edge. */
    water(input: Node<'Poly'>, params?: RiverWaterV1Params): Node<'Poly'>;
  };
  rooms: {
    /** A building from a list of rooms and the links between them: it lays the rooms out around a corridor (every room gets a door onto it, every link is a short walk), then makes a floor slab under every room and corridor, a wall along every edge, and an opening where each door is. There is no roof, so the inside shows and can be lit. */
    plan(params?: RoomsPlanV1Params): Node<'Solid'>;
  };
  scatter: {
    /** Plants copies of a shape (a tree, a rock, a tuft of grass) over a terrain.tile: blue-noise spacing so they never clump or line up, the same plants in the same places whichever tile you build first, and each copy stands on the ground at its own height. Say where they grow with a mask layer made by terrain.mask (density follows the layer), a height window and a steepest slope; vary size and facing with scale and spin. */
    place(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; surface: Node<'Poly'>; where?: Node<'Pattern'> }, params?: ScatterPlaceV1Params): Node<'Inst'>;
  };
  scene: {
    /** Sky and ground ambient irradiance, approximated as a low-order hemisphere; set gi=true for traced light probes (global illumination) instead. */
    environment(params?: SceneEnvironmentV1Params): Node<'Environment'>;
    /** Procedural analytic sky with a sun, atmosphere gradient and optional deterministic clouds. Supplies an approximate ambient hemisphere and sun light; a separate scene.environment overrides its ambient lighting while keeping this background. */
    sky(params?: SceneSkyV1Params): Node<'Environment'>;
  };
  script: {
    /** A script is behaviour that runs against the live world and is saved with the document. `source` is the body of an async function with `world`, `on`, `every`, `log`, `time` and `random`; `name` is a unique lowercase identifier (empty uses this node's label), and `enabled` controls whether it runs. */
    js(params: ScriptJsV1Params): Node<'Check'>;
  };
  sign: {
    /** Lit text as geometry, for neon and LED signs: the letters (a 5 by 7 pixel font) become small boxes, centred on x = 0, standing on y = 0 (the bottom row at y = 0), reaching from z = 0 to z = `depth`, facing +z. The node sets a dark housing colour and an emissive glow in `color` (emissive = color x glow, so glow 0 is unlit); a later solid.material or material.preset overrides it. */
    text(params?: SignTextV1Params): Node<'Solid'>;
  };
  sim: {
    /** A simulation you write, run live: cloth, hair, ropes, flocks, swarms, crowds, soft bodies, fire, anything with state that changes by rules. The `source` declares `param` settable numbers, `state` arrays with one value per element (`state pos: vec3[256]`), `kernel`s that run for each element of an array (`kernel move each i in pos { pos[i] += vel[i] * dt }`; a kernel reads the state as it was when it began and writes only its own element) or once, a `schedule` listing the kernels to run each step (`schedule flock; move`, with `repeat 6 { ... */
    program(input?: Node<'Poly'>, params?: SimProgramV1Params): Node<'Poly'>;
  };
  solid: {
    /** Axis-aligned box centred on the origin. */
    box(params: SolidBoxV1Params): Node<'Solid'>;
    /** A cylinder with rounded ends standing along Y, centred on the origin. */
    capsule(params: SolidCapsuleV1Params): Node<'Solid'>;
    /** Breaks convex edges with a flat 45-degree facet by eroding then expanding the solid with an octahedron; bounded to 20,000 input and 250,000 output triangles. Concave edges stay sharp. */
    chamfer<T extends Node<'Solid'>>(input: T, params: SolidChamferV1Params): T;
    /** Replaces a solid by the simple shape that fits it best, for collision and level-of-detail: the axis-aligned box, the oriented box (from the solid's principal axes), or a capsule along its longest axis. Emits a warning naming the chosen shape and its volume ratio to the solid. */
    collision_boxes(input: Node<'Solid'>, params?: SolidCollisionBoxesV1Params): Node<'Solid'>;
    /** A cone or truncated cone standing along Y, centred on the origin. */
    cone(params: SolidConeV1Params): Node<'Solid'>;
    /** Cylinder standing along Y, centred on the origin. */
    cylinder(params: SolidCylinderV1Params): Node<'Solid'>;
    /** The first solid minus all the others. */
    difference(input: Node<'Solid'>[]): Node<'Solid'>;
    /** Stands a profile along Y from 0 to height (profile Y maps to -Z). */
    extrude(input: Node<'Profile'>, params: SolidExtrudeV1Params): Node<'Solid'>;
    /** Adds layers on top of a surface that the physically based renderer draws: a glossy clear coat (car paint, wet skin, glazed ceramic, scales, eyes), highlights stretched along the surface (brushed metal, silk, hair) and translucency, light passing through a thin body (leaves, wax, ears, wings). Colours, roughness and textures are kept: put it after solid.material and solid.texture. */
    finish<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: SolidFinishV1Params): T;
    /** Makes a surface behave like leaves, grass, a fence or any thin cut-out card. It is cut away wherever its albedo map's alpha is under alpha_cutoff, so a leaf shape drawn into a texture (a texture program or an expr node that ends in a vec4 whose last number is the alpha) becomes a leaf-shaped silhouette in the picture, in its shadow and in the exported glTF (alphaMode MASK). */
    foliage<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: SolidFoliageV1Params): T;
    /** Replaces a solid by its convex hull: the smallest convex shape that contains every one of its points, as a closed solid. Refused when the solid has more points than max_vertices (decimate it first); the hull is at most 2 * max_vertices triangles. */
    hull(input: Node<'Solid'>, params?: SolidHullV1Params): Node<'Solid'>;
    /** The volume shared by all solids. */
    intersect(input: Node<'Solid'>[]): Node<'Solid'>;
    /** Lofts matching convex profile loops at even spacing along Y; steps above 1 blend smoothly between them. */
    loft(input: Node<'Profile'>[], params?: SolidLoftV1Params): Node<'Solid'>;
    /** Gives a solid, a set of placements or a polygon mesh a surface: colour, metal, roughness and glow. The shape is unchanged. */
    material<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: SolidMaterialV1Params): T;
    /** Spins a profile about the Y axis. */
    revolve(input: Node<'Profile'>, params?: SolidRevolveV1Params): Node<'Solid'>;
    /** Rounds a closed solid by eroding then expanding it with a small sphere; bounded to 20,000 input and 250,000 output triangles. */
    round<T extends Node<'Solid'>>(input: T, params: SolidRoundV1Params): T;
    /** Sphere centred on the origin. */
    sphere(params: SolidSphereV1Params): Node<'Solid'>;
    /** Sets the whole look of a surface with one word, keeping its colours, roughness, textures and shape. `anime`: two-band cel shading with a tinted shadow, rim light and an outline. */
    style<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: SolidStyleV1Params): T;
    /** Sweeps a single closed profile along a curve (a pipe, rope, rail, tentacle, horn). scale_end tapers the tube to a different size at the far end of an open curve (0.2 gives a pointed horn or tail) and twist turns the profile along the way (a twisted rope or ribbon). */
    sweep(inputs: { curve: Node<'Curve'>; profile: Node<'Profile'> }, params?: SolidSweepV1Params): Node<'Solid'>;
    /** Attaches a procedural Material Maker texture program to a solid. The base material and geometry are preserved. */
    texture<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: SolidTextureV1Params): T;
    /** Gives a surface an anime / cel-shaded look. Light falls into flat colour bands (with a tinted shadow colour) instead of smooth shading. */
    toon<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params?: SolidToonV1Params): T;
    /** A ring (doughnut) lying in the XZ plane, centred on the origin. */
    torus(params: SolidTorusV1Params): Node<'Solid'>;
    /** Joins solids into one (a single solid joins to itself). */
    union(input: Node<'Solid'>[]): Node<'Solid'>;
  };
  street: {
    /** A deterministic, placeable street prop kit. Build matching kind/position/axis nodes for body, accents and (for lights) lenses to assign materials. */
    prop(params?: StreetPropV1Params): Node<'Solid'>;
    /** A straight, metre-scaled road kit module: asphalt, lane paint, crosswalks, paired raised kerbs and pavements. Match length, width, axis and centre when making its material-separated parts; join segments to build streets and intersections. */
    road(params?: StreetRoadV1Params): Node<'Solid'>;
  };
  terrain: {
    /** Sorts the ground of a terrain.tile into biomes that blend into each other: forest where it is low, damp and gentle, meadow where it is drier, scree where it is steep, snow above the tree line. Each biome is written as soft windows over the ground's height (metres), steepness (degrees from flat) and moisture (a slow noise, 0 dry to 1 wet): `forest: height 0..80, slope 0..30, wet 0.45..1; meadow: slope 0..20, wet 0..0.5; rock: slope 35..` (either end of a window may be left out). */
    biomes(input: Node<'Poly'>, params?: TerrainBiomesV1Params): Node<'Poly'>;
    /** Weathers a terrain.tile like real mountains: rivers cut valleys (large scale, the same for every tile), loose rock slides down steep slopes, and rain moves soil downhill (fine detail). Neighbouring tiles eroded with the same settings meet without a seam. */
    erode(input: Node<'Poly'>, params?: TerrainErodeV1Params): Node<'Poly'>;
    /** Levels a round pad in the ground of a terrain.tile, with a soft shoulder that blends back into the land: the place for a house, a camp, a road end or a landing. By default the pad keeps the height of the ground at its centre; set `to` to "height" to level it to a height you choose (for a shared floor level across several pads). */
    flatten(input: Node<'Poly'>, params?: TerrainFlattenV1Params): Node<'Poly'>;
    /** A landscape: a seeded fractal ground (rolling hills, ridged mountains, an island) as one closed solid slab, centred on the origin with its lowest ground at y=0 or above and a flat base under it. It is an ordinary solid, so it takes solid.material, can be cut with booleans, and can carry copies placed by inst.drop. */
    heightfield(params?: TerrainHeightfieldV1Params): Node<'Solid'>;
    /** Measures the ground of a terrain.tile into a named 0 to 1 layer you can paint materials and scatter plants by: steepness, bumpiness, height, which way the slope faces, or how enclosed a spot is. Look-around masks use the tile's margin, so neighbouring tiles agree at their edge. */
    mask(input: Node<'Poly'>, params?: TerrainMaskV1Params): Node<'Poly'>;
    /** Dresses a landscape: grass on gentle ground, bare rock on steep faces, sand on the lowest ground and snow above a snow line, blended per pixel from the ground's own slope and height with large-scale variation so it never repeats. No texture is baked, so it looks sharp at any distance and every tile of a world matches. */
    material(input: Node<'Poly'>, params?: TerrainMaterialV1Params): Node<'Poly'>;
    /** Stands copies of any shape on the ground of a terrain.tile at x,z positions you list: a hut here, a well there, a lone tree. Each copy sits at the ground height under it (after terrain.flatten, on the pad), so you never need to know the height. */
    place(inputs: { in: Node<'Solid' | 'Inst' | 'Poly'>; surface: Node<'Poly'> }, params: TerrainPlaceV1Params): Node<'Inst'>;
    /** One tile of a world's ground as an editable polygon mesh with a height grid behind it. Heights come from fractal gradient noise evaluated at WORLD coordinates, so neighbouring tiles (tile_x, tile_z) line up exactly and the same seed always gives the same land, whichever tile you build first. */
    tile(params?: TerrainTileV1Params): Node<'Poly'>;
  };
  value: {
    /** Arithmetic on two numbers. a and b are number parameters, so either can be a literal or the output of another value node (a=width, b=2). */
    math(params?: ValueMathV1Params): Node<'Number'>;
    /** Measures how far a clip moves a part, from its position tracks (anim.track property=position): the straight horizontal `travel` from the first key to the last, the horizontal `path` length, the `duration` in seconds, the `speed` (travel over duration) and the vertical `lift` (highest minus lowest). Use it to read the real speed of a walk or a dash instead of guessing: link `speed` into a state machine's walk threshold or a game unit's speed, or `travel` into how far a part should be placed. */
    motion(input: Node<'Solid' | 'Poly' | 'Inst'>, params?: ValueMotionV1Params): Node<'Number'>;
    /** A number, to link into any number parameter of another node (a port named like the parameter, or like size.y for one component of a vector). Use it to name a dimension once and reuse it. */
    number(params: ValueNumberV1Params): Node<'Number'>;
    /** Reads a place on a part as a number: the head (the joint it turns about) or the tail of a bone (body.bone, rig.bone), or the min, center or max of any solid or polygon mesh's bounding box, along x, y or z. Link it into a number parameter, one component at a time (for example `by.y=hy` or `position.x=hx`), so one part is placed from where another is. */
    point(input: Node<'Solid' | 'Poly'>, params?: ValuePointV1Params): Node<'Number'>;
    /** Measures a solid or polygon mesh: the size of its bounding box along x (width), y (height) or z (depth), the longest or shortest of those, its enclosed volume (solids) or its surface area. Link the result into another node's parameter to size one part from another. */
    size(input: Node<'Solid' | 'Poly'>, params?: ValueSizeV1Params): Node<'Number'>;
  };
  volume: {
    /** Joins volumes: union (everything in any of them), subtract (the first minus all the others) or intersect (only what is in all of them), optionally with a smooth blend where the shapes meet, like clay run together. Every volume must have the same voxel_size. */
    combine(input: Node<'Volume'>[], params?: VolumeCombineV1Params): Node<'Volume'>;
    /** Samples a closed mesh into a grid of distances to its surface (negative inside, positive outside), the starting point of the volume nodes: combine volumes like clay (volume.combine), blur them smooth (volume.smooth), grow or shrink them (volume.offset), and turn the result back into a clean all-quad mesh (volume.to_mesh). The mesh must be closed (a solid, or a polygon mesh with no open edges; poly.fill_holes closes one). */
    from_mesh(input: Node<'Solid' | 'Poly'>, params?: VolumeFromMeshV1Params): Node<'Volume'>;
    /** Grows (positive distance) or shrinks (negative) a volume by moving its surface, in metres: a thicker or thinner shape, a coat around it, a hole made wider. Limited to about 0.9 of band times voxel_size. */
    offset(input: Node<'Volume'>, params?: VolumeOffsetV1Params): Node<'Volume'>;
    /** Smooths a volume: the surface relaxes like clay, sharp creases and small lumps round off and thin bridges thin out. Each pass moves every voxel's distance toward the average of its neighbours, so more passes round more and shrink thin parts a little. */
    smooth(input: Node<'Volume'>, params?: VolumeSmoothV1Params): Node<'Volume'>;
    /** Turns a volume back into a mesh: the surface where the distance is zero, as a closed mesh of clean quads about one voxel across, facing outward (surface nets). Smoothed volumes give smooth meshes. */
    to_mesh(input: Node<'Volume'>, params?: VolumeToMeshV1Params): Node<'Poly'>;
  };
  water: {
    /** Fills the connected hollow below level at a world x,z seed on a terrain.tile. Makes a flat open water sheet with depth tint and shore foam, a one-cell skirt and no side walls. */
    fill(input: Node<'Poly'>, params: WaterFillV1Params): Node<'Poly'>;
    /** For lakes on a landscape use water.fill, which follows the hollow and shoreline. A closed, transparent water body with a seeded wavy rest mesh and a lightweight animated viewer ripple. */
    surface(params?: WaterSurfaceV1Params): Node<'Solid'>;
  };
  xform: {
    /** Rotates a direction onto another about a fixed pivot; zero directions are refused. */
    align(input: Node<'Solid'>, params: XformAlignV1Params): Node<'Solid'>;
    /** Copies a solid into a grid; the first copy stays where it is. */
    array<T extends Node<'Solid'>>(input: T, params: XformArrayV1Params): T;
    /** Moves and rotates the source so its named socket coincides with the target socket. */
    attach(inputs: { in: Node<'Solid'>; to: Node<'Solid'> }, params: XformAttachV1Params): Node<'Solid'>;
    /** Bends a solid around the Z axis along its local Y extent. */
    bend<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformBendV1Params): T;
    /** Free-form deformation: a grid of control points over the solid's bounding box, some of which you move; the solid follows smoothly (a cartoon squash, a bulge, a tapered bottle). Control point (i j k) runs 0 to divisions-1 along X, Y, Z; (0 0 0) is the minimum corner. */
    lattice<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformLatticeV1Params): T;
    /** Adds the mirror image of a solid across the plane through the origin perpendicular to an axis. */
    mirror<T extends Node<'Solid'>>(input: T, params: XformMirrorV1Params): T;
    /** Moves its input. */
    move<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: XformMoveV1Params): T;
    /** Roughens a solid with smooth random displacement (rocks, hills, lumpy clay): every point is pushed by smooth noise. The same seed always gives the same shape. */
    noise<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformNoiseV1Params): T;
    /** Copies a solid evenly around the Y axis; each copy is first moved out along X. */
    radial<T extends Node<'Solid'>>(input: T, params: XformRadialV1Params): T;
    /** Copies a solid along a path: copy i is turned by i times rotate_step, then moved by i times step. */
    repeat<T extends Node<'Solid'>>(input: T, params: XformRepeatV1Params): T;
    /** Rotates its input about the origin (X, then Y, then Z). */
    rotate<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: XformRotateV1Params): T;
    /** Scales its input about the origin. */
    scale<T extends Node<'Solid' | 'Inst' | 'Poly'>>(input: T, params: XformScaleV1Params): T;
    /** Slants a solid: the top moves sideways by x_shift and z_shift (metres) relative to the bottom, in proportion to height, like a leaning tower or a parallelogram. */
    shear<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformShearV1Params): T;
    /** Tapers a solid along its local Y extent. */
    taper<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformTaperV1Params): T;
    /** Twists a solid around its local Y extent. */
    twist<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformTwistV1Params): T;
    /** Ripples a solid like a flag or a wavy wall: points move sideways by a sine wave that runs up the solid's height (Y). Subdivide the shape (or let the tolerance refine it) so there are vertices to ripple. */
    wave<T extends Node<'Solid' | 'Poly'>>(input: T, params?: XformWaveV1Params): T;
  };
}

declare const ve: Ve;
export default ve;
