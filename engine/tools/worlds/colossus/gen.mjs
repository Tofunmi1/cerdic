// node tools/worlds/colossus/gen.mjs > web/dist-bevy-gpu/worlds/colossus.txt
// "The Ashen Colossus": a ruined volcanic arena, a 24 m stone titan with lava in its cracks, a player kit and the fight's script (script.js beside this file).
// The boss is nodes only: parts parented in a chain, animation clips per part (idle, two slams, a sweep, a stomp, a roar, a death) and a machine that a script triggers.
// Facing +z; the player starts at z = +34 looking toward -z. Metres.
import { readFileSync } from "node:fs";
const here = new URL(".", import.meta.url).pathname;
const L = [];
const emit = (s) => L.push(s);
const V = (a) => `[${a.map((n) => +n.toFixed(3)).join(", ")}]`;
const outputs = [];
const out = (name, node) => { outputs.push(`output ${name} = ${node}`); };

// ---- texture programs (one per look) ----
const N = "u = p.x; v = p.y;";
const prog = (o) => Object.entries(o).map(([k, v]) => `${k} = ${v}`).join("; ");
const E = (code) => `expr(code=\\"${code.replace(/\s+/g, " ").trim()}\\")`;
const C = (r, g, b) => E(`out = vec4(${r}, ${g}, ${b}, 1.0)`);
const SOFT = (n, lo = "#787878", hi = "#a8a8a8") => `colorize(${n}, gradient=[0:${lo}, 1:${hi}])`;
const rock = prog({
  c: E(`${N} f = fbm(vec2(u*3.0, v*3.0)); g = abs(fbm(vec2(u*5.0 + f*1.5, v*5.0 - f*1.5)) - 0.5); out = vec4(vec3(1.0 - smoothstep(0.0, 0.055, g)), 1.0)`),
  base: E(`${N} f = fbm(vec2(u*6.0, v*6.0)); h = fbm(vec2(u*30.0, v*30.0)); out = vec4(vec3(0.075, 0.068, 0.062) * (0.65 + 0.6*f + 0.25*h), 1.0)`),
  hot: C(1.0, 0.33, 0.04), glow: C(2.6, 0.62, 0.08), none: C(0, 0, 0),
  albedo: "blend(hot, base, c, blend_type=normal, amount=1.0)", emissive: "blend(glow, none, c, blend_type=normal, amount=1.0)",
  roughness: E("out = vec4(vec3(0.88), 1.0)"), hs: SOFT("c"), height: "hs",
});
const ash = prog({
  c: E(`${N} f = fbm(vec2(u*2.0, v*2.0)); g = abs(fbm(vec2(u*4.0 + f*2.0, v*4.0 - f*2.0)) - 0.5); out = vec4(vec3(1.0 - smoothstep(0.0, 0.022, g)), 1.0)`),
  base: E(`${N} f = fbm(vec2(u*8.0, v*8.0)); h = fbm(vec2(u*40.0, v*40.0)); out = vec4(vec3(0.07, 0.065, 0.062) * (0.6 + 0.6*f + 0.3*h), 1.0)`),
  hot: C(0.8, 0.18, 0.02), glow: C(0.8, 0.16, 0.02), none: C(0, 0, 0),
  albedo: "blend(hot, base, c, blend_type=normal, amount=1.0)", emissive: "blend(glow, none, c, blend_type=normal, amount=1.0)",
  roughness: E("out = vec4(vec3(0.92), 1.0)"), hs: SOFT("c", "#808080", "#a0a0a0"), height: "hs",
});
const lava = prog({
  c: E(`${N} f = fbm(vec2(u*4.0, v*4.0)); g = abs(fbm(vec2(u*6.0 + f*2.5, v*6.0 - f*2.5)) - 0.5); out = vec4(vec3(1.0 - smoothstep(0.0, 0.12, g)), 1.0)`),
  hot: C(1.0, 0.5, 0.06), crust: C(0.05, 0.035, 0.03), glow: C(3.0, 0.95, 0.12), dim: C(0.25, 0.04, 0.0),
  albedo: "blend(hot, crust, c, blend_type=normal, amount=1.0)", emissive: "blend(glow, dim, c, blend_type=normal, amount=1.0)", roughness: E("out = vec4(vec3(0.7), 1.0)"),
});
const bone = prog({
  base: E(`${N} f = fbm(vec2(u*5.0, v*5.0)); out = vec4(vec3(0.62, 0.55, 0.44) * (0.7 + 0.5*f), 1.0)`),
  albedo: "base", roughness: E("out = vec4(vec3(0.6), 1.0)"),
});
emit(`let rock_prog = "${rock}"`);
emit(`let ash_prog = "${ash}"`);
emit(`let lava_prog = "${lava}"`);
emit(`let bone_prog = "${bone}"`);

// ---- sky, light, ground ----
emit(`sky = scene.sky@1(sun_elevation=3, sun_azimuth=-60, sun_intensity=1.6, turbidity=6, ground_color=[0.07, 0.055, 0.055], clouds=true, cloud_cover=0.85, seed=3, haze=0.05)`);
emit(`env = scene.environment@1(intensity=0.35, reflection=0.4, bounce=0.5, haze=0.03, haze_color=[0.28, 0.13, 0.1], gi=false)`);
out("sky", "sky"); out("env", "env");
emit(`core_light = light.point@1(position=[0, 13.5, 7], color=[1.0, 0.45, 0.14], intensity=5500, range=60)`);
emit(`pit_light = light.point@1(position=[0, 2.5, 4], color=[1.0, 0.4, 0.1], intensity=6000, range=40)`);
out("core_light", "core_light"); out("pit_light", "pit_light");

function material(name, src, { color = [0.5, 0.5, 0.5], rough = 0.8, metallic = 0, program, tile = 6, size = 512 }) {
  emit(`${name}_m = solid.material@1(${src}, color=${V(color)}, roughness=${rough}, metallic=${metallic})`);
  if (!program) return `${name}_m`;
  emit(`${name}_t = solid.texture@1(${name}_m, program="\${${program}}", tile=${tile}, size="${size}")`);
  return `${name}_t`;
}
const ellipsoid = (n, c, r, o) => { emit(`${n}_g = solid.sphere@1(radius=1)`); emit(`${n}_k = xform.scale@1(${n}_g, by=${V(r)})`); emit(`${n}_a = xform.move@1(${n}_k, by=${V(c)})`); return material(n, `${n}_a`, o); };
const cylinder = (n, c, radius, height, o, top) => {
  if (top === undefined) emit(`${n}_g = solid.cylinder@1(radius=${radius}, height=${height}, segments=28)`);
  else emit(`${n}_g = solid.cone@1(radius=${radius}, top_radius=${top}, height=${height}, segments=28)`);
  emit(`${n}_a = xform.move@1(${n}_g, by=${V(c)})`);
  return material(n, `${n}_a`, o);
};
const box = (n, c, size, o, rot) => {
  emit(`${n}_g = solid.box@1(size=${V(size)})`);
  let cur = `${n}_g`;
  if (rot) { emit(`${n}_r = xform.rotate@1(${cur}, angles=${V(rot)})`); cur = `${n}_r`; }
  emit(`${n}_a = xform.move@1(${cur}, by=${V(c)})`);
  return material(n, `${n}_a`, o);
};
const cone = (n, c, radius, height, rot, o) => {
  emit(`${n}_g = solid.cone@1(radius=${radius}, height=${height}, segments=16)`);
  emit(`${n}_r = xform.rotate@1(${n}_g, angles=${V(rot)})`);
  emit(`${n}_a = xform.move@1(${n}_r, by=${V(c)})`);
  return material(n, `${n}_a`, o);
};
const body = (n, src, kind = "static", shape = "box") => { emit(`${n}_b = phys.body@1(${src}, kind="${kind}", shape="${shape}")`); return `${n}_b`; };

// ---- the arena ----
let f = cylinder("floor", [0, -0.5, 0], 37, 1, { program: "ash_prog", tile: 8, color: [0.12, 0.11, 0.1] });
f = body("floor", f, "static", "box");
// play mode walks the player: WASD, Space to jump, Shift to run
emit(`floor_w = phys.world@1(${f}, gravity=[0, -9.81, 0], walker=true, height=1.7, radius=0.35, speed=5.5, jump=6)`);
out("floor", "floor_w");
// a few crates and barrels to knock about (they also make the scene playable, so the Play button shows)
for (let i = 0; i < 5; i++) {
  const a = 1.0 + i * 0.55, x = Math.cos(a) * 17, z = Math.sin(a) * 17, n = `crate${i}`;
  emit(`${n}_g = solid.box@1(size=[1.1, 1.1, 1.1])`);
  emit(`${n}_a = xform.move@1(${n}_g, by=${V([x, 0.56, z])})`);
  emit(`${n}_m = solid.material@1(${n}_a, color=[0.2, 0.12, 0.07], roughness=0.8)`);
  out(n, `${n}_b`);
  emit(`${n}_b = phys.body@1(${n}_m, kind="dynamic", shape="box", density=0.6)`);
}
let rim = (() => { emit(`rim_g = solid.torus@1(major=37, minor=2.4, segments=96, tube_segments=14)`); emit(`rim_a = xform.move@1(rim_g, by=[0, 0.4, 0])`); return material("rim", "rim_a", { program: "rock_prog", tile: 7 }); })();
rim = body("rim", rim, "static", "mesh");
out("rim", rim);
// broken pillars round the arena
const pillars = [];
for (let i = 0; i < 12; i++) {
  const a = (i / 12) * Math.PI * 2 + 0.2, r = 31 + (i % 3) * 1.4, h = 7 + ((i * 5) % 4) * 2.2;
  const name = `pillar${i}`;
  pillars.push(cylinder(name, [Math.cos(a) * r, h / 2, Math.sin(a) * r], 1.5, h, { program: "rock_prog", tile: 5 }, 1.2));
}
emit(`pillars_u = solid.union@1(${pillars.join(", ")})`);
out("pillars", "pillars_u");
// lava pool round the legs (decor, no collision) and a glowing ring
emit(`pool_g = solid.cylinder@1(radius=10.5, height=0.06, segments=48)`);
emit(`pool_a = xform.move@1(pool_g, by=[0, 0.03, 0])`);
out("lava_pool", material("pool", "pool_a", { program: "lava_prog", tile: 7 }));
// braziers with fire and light
const braziers = [];
for (let i = 0; i < 6; i++) {
  const a = (i / 6) * Math.PI * 2 + 0.5, x = Math.cos(a) * 21, z = Math.sin(a) * 21, n = `brazier${i}`;
  const stand = cylinder(`${n}_stand`, [x, 0.9, z], 0.55, 1.8, { color: [0.07, 0.065, 0.06], rough: 0.7 }, 0.3);
  const bowl = cylinder(`${n}_bowl`, [x, 1.95, z], 0.95, 0.45, { color: [0.1, 0.08, 0.07], rough: 0.5, metallic: 0.5 }, 1.25);
  emit(`${n}_u = solid.union@1(${stand}, ${bowl})`);
  emit(`${n}_flame_s = solid.sphere@1(radius=0.3)`);
  emit(`${n}_flame_a = xform.move@1(${n}_flame_s, by=${V([x, 2.3, z])})`);
  emit(`${n}_flame_m = solid.material@1(${n}_flame_a, color=[1, 0.5, 0.1], emissive=[2.5, 0.9, 0.15], roughness=0.5)`);
  emit(`${n}_fire = fx.emitter@1(${n}_flame_m, name="fire", rate=70, lifetime=[0.5, 1.0], speed=[0.8, 1.8], cone=14, shape="disc", extent=[0.4, 0, 0], size=[0.55, 0.1], colour=[1, 0.62, 0.2], colour_end=[0.7, 0.08, 0], alpha=[1, 0], glow=3, blend="additive")`);
  emit(`${n}_fire_f = fx.force@1(${n}_fire, kind="turbulence", strength=2.2, scale=1.3)`);
  emit(`${n}_smoke = fx.emitter@1(${n}_fire_f, name="smoke", rate=14, lifetime=[2, 3.4], speed=[0.5, 1.2], cone=18, shape="disc", extent=[0.3, 0, 0], size=[0.4, 1.8], colour=[0.2, 0.19, 0.19], alpha=[0.5, 0], blend="alpha")`);
  braziers.push(`${n}_u`);
  out(`${n}_flame`, `${n}_smoke`);
  emit(`${n}_light = light.point@1(position=${V([x, 3.2, z])}, color=[1, 0.5, 0.18], intensity=1800, range=22)`);
  out(`${n}_light`, `${n}_light`);
}
emit(`braziers_u = solid.union@1(${braziers.join(", ")})`);
out("braziers", "braziers_u");
// the lava pool throws embers
emit(`ember_s = solid.sphere@1(radius=0.2)`);
emit(`ember_a = xform.move@1(ember_s, by=[0, 0.4, 0])`);
emit(`ember_m = solid.material@1(ember_a, color=[1, 0.4, 0.1], emissive=[2, 0.6, 0.1])`);
emit(`ember_fx = fx.emitter@1(ember_m, name="embers", rate=60, lifetime=[2, 4], speed=[1, 3], direction=[0, 1, 0], cone=40, shape="disc", extent=[9, 0, 0], size=[0.12, 0.02], colour=[1, 0.55, 0.12], colour_end=[0.8, 0.1, 0], alpha=[1, 0], glow=5, blend="additive")`);
emit(`ember_fx_f = fx.force@1(ember_fx, kind="turbulence", strength=1.5, scale=1.2)`);
out("embers", "ember_fx_f");

// ---- the Colossus ----
const ROCK = { program: "rock_prog", tile: 7 };
// legs and hips are static colliders (a part with animation cannot be a body)
for (const s of [1, -1]) {
  let leg = cylinder(`leg${s > 0 ? "p" : "n"}`, [s * 4.6, 4.2, 0], 2.9, 8.4, ROCK, 2.5);
  leg = body(`leg${s > 0 ? "p" : "n"}`, leg, "static", "cylinder");
  out(`leg_${s > 0 ? "p" : "n"}`, leg);
  let foot = box(`foot${s > 0 ? "p" : "n"}`, [s * 4.6, 0.7, 1.2], [6.4, 1.4, 8.2], ROCK);
  foot = body(`foot${s > 0 ? "p" : "n"}`, foot, "static", "box");
  out(`foot_${s > 0 ? "p" : "n"}`, foot);
}
let hips = box("hips", [0, 9.0, 0], [14, 2.6, 7.5], ROCK);
hips = body("hips", hips, "static", "box");
out("hips", hips);

const keysText = (frames) => frames.map(([t, v]) => `${t}:${v.join(",")}`).join("; ");
/** Chains one anim.track per clip that the part has onto a node; `data` maps clip -> [[time, [x, y, z]], ...]. */
function animate(label, node, prop, pivot, data) {
  let cur = node;
  for (const [clip, frames] of Object.entries(data)) {
    const looping = clip === "idle";
    emit(`${label}_${prop}_${clip} = anim.track@1(${cur}, property="${prop}", keys="${keysText(frames)}", interpolation="smooth", ${looping ? "" : "loop=false, "}clip="${clip}"${pivot ? `, pivot=${V(pivot)}` : ""})`);
    cur = `${label}_${prop}_${clip}`;
  }
  return cur;
}
const parentTo = (label, node, parent) => { emit(`${label}_p = anim.parent@1(${node}, parent="${parent}")`); return `${label}_p`; };
const HIPS = [0, 9.0, 0];

// torso: the root of the animation, with the machine
let torso = ellipsoid("torso", [0, 13.2, 0], [7.0, 5.8, 4.7], ROCK);
let t = animate("torso", torso, "rotation", HIPS, {
  idle: [[0, [0, 0, 0]], [2, [1.4, 0, 0]], [4, [0, 0, 0]]],
  slam_p: [[0, [0, 0, 0]], [1.15, [-10, 0, -3]], [1.75, [17, 0, 4]], [2.3, [12, 0, 3]], [3, [0, 0, 0]]],
  slam_n: [[0, [0, 0, 0]], [1.15, [-10, 0, 3]], [1.75, [17, 0, -4]], [2.3, [12, 0, -3]], [3, [0, 0, 0]]],
  sweep: [[0, [0, 0, 0]], [0.9, [0, -24, 0]], [1.7, [4, 30, 0]], [2.6, [0, 0, 0]]],
  stomp: [[0, [0, 0, 0]], [1.0, [-14, 0, 0]], [1.55, [22, 0, 0]], [2.1, [15, 0, 0]], [3, [0, 0, 0]]],
  roar: [[0, [0, 0, 0]], [0.8, [-18, 0, 0]], [2.4, [-18, 0, 0]], [3.4, [0, 0, 0]]],
  death: [[0, [0, 0, 0]], [1.2, [-7, 0, 0]], [6.5, [74, 0, 0]]],
});
t = animate("torso", t, "position", null, { death: [[0, [0, 0, 0]], [6.5, [0, -4.5, 4]]] });
emit(`torso_machine = anim.machine@1(${t}, program="entry idle;state idle clip=idle;state slam_p clip=slam_p once;state slam_n clip=slam_n once;state sweep clip=sweep once;state stomp clip=stomp once;state roar clip=roar once;state death clip=death once;transition any -> slam_p when trigger:slam_p fade=0.2;transition any -> slam_n when trigger:slam_n fade=0.2;transition any -> sweep when trigger:sweep fade=0.2;transition any -> stomp when trigger:stomp fade=0.2;transition any -> roar when trigger:roar fade=0.2;transition any -> death when trigger:death fade=0.3;transition any -> idle when trigger:idle fade=0.3;transition slam_p -> idle exit=0.98;transition slam_n -> idle exit=0.98;transition sweep -> idle exit=0.98;transition stomp -> idle exit=0.98;transition roar -> idle exit=0.98")`);
out("colossus", "torso_machine");

// everything else hangs off the torso or the part above it
const onTorso = (label, node, data, pivot) => { let cur = animate(label, node, "rotation", pivot, data); return parentTo(label, cur, "colossus"); };
// head, jaw, horns, eyes
const NECK = [0, 17.6, 0.5];
const head = ellipsoid("head", [0, 19.9, 1.9], [3.6, 3.7, 4.0], ROCK);
out("head", onTorso("head", head, {
  idle: [[0, [0, 0, 0]], [2, [0, 5, 0]], [4, [0, 0, 0]]],
  slam_p: [[0, [0, 0, 0]], [1.75, [-14, 0, 0]], [3, [0, 0, 0]]],
  slam_n: [[0, [0, 0, 0]], [1.75, [-14, 0, 0]], [3, [0, 0, 0]]],
  sweep: [[0, [0, 0, 0]], [0.9, [0, 18, 0]], [1.7, [0, -22, 0]], [2.6, [0, 0, 0]]],
  stomp: [[0, [0, 0, 0]], [1.55, [-16, 0, 0]], [3, [0, 0, 0]]],
  roar: [[0, [0, 0, 0]], [0.8, [-34, 0, 0]], [2.4, [-34, 0, 0]], [3.4, [0, 0, 0]]],
  death: [[0, [0, 0, 0]], [6.5, [44, 0, 0]]],
}, NECK));
const headChild = (label, node, data, pivot) => parentTo(label, animate(label, node, "rotation", pivot, data), "head");
const JAW = [0, 18.7, 0.9];
const jawClips = {
  roar: [[0, [0, 0, 0]], [0.8, [32, 0, 0]], [2.4, [32, 0, 0]], [3.4, [0, 0, 0]]],
  death: [[0, [0, 0, 0]], [6.5, [34, 0, 0]]],
};
const jaw = box("jaw", [0, 17.6, 3.0], [5.6, 1.8, 5.6], { program: "rock_prog", tile: 5 });
out("jaw", headChild("jaw", jaw, jawClips, JAW));
// teeth: a row of bone cones up from the jaw and another down from the skull
const lowTeeth = [], upTeeth = [];
for (let i = 0; i < 9; i++) {
  const x = (i - 4) * 0.62;
  lowTeeth.push(cone(`tooth_l${i}`, [x, 19.0, 5.45 - Math.abs(i - 4) * 0.12], 0.3, 1.5 + (i % 2) * 0.5, [0, 0, 0], { program: "bone_prog", tile: 2 }));
  upTeeth.push(cone(`tooth_u${i}`, [x, 18.2, 5.7 - Math.abs(i - 4) * 0.12], 0.28, 1.3 + ((i + 1) % 2) * 0.5, [180, 0, 0], { program: "bone_prog", tile: 2 }));
}
emit(`teeth_low_u = solid.union@1(${lowTeeth.join(", ")})`);
emit(`teeth_up_u = solid.union@1(${upTeeth.join(", ")})`);
out("teeth_low", headChild("teeth_low", "teeth_low_u", jawClips, JAW));
out("teeth_up", headChild("teeth_up", "teeth_up_u", {}, NECK));
// a heavy brow, long swept horns and small cheek horns
const hornGroup = [ellipsoid("brow", [0, 21.7, 4.5], [3.6, 0.7, 1.5], ROCK)];
for (const s of [1, -1]) {
  const k = s > 0 ? "p" : "n";
  hornGroup.push(cone(`horn_big_${k}`, [s * 4.3, 25.0, 0.2], 1.35, 10.5, [-14, 0, -s * 30], { program: "bone_prog", tile: 3 }));
  hornGroup.push(cone(`horn_mid_${k}`, [s * 4.6, 22.0, 0.0], 0.85, 6.0, [-10, 0, -s * 64], { program: "bone_prog", tile: 3 }));
  hornGroup.push(cone(`horn_cheek_${k}`, [s * 3.6, 18.9, 2.6], 0.55, 3.2, [10, 0, -s * 100], { program: "bone_prog", tile: 3 }));
}
emit(`horns_u = solid.union@1(${hornGroup.join(", ")})`);
out("horns", headChild("horns", "horns_u", {}, NECK));
// eyes: slanted, narrow and white-hot, so the face looks angry
const eyeNodes = [];
for (const s of [1, -1]) {
  const e = `eye_${s > 0 ? "p" : "n"}`;
  emit(`${e}_s = solid.sphere@1(radius=1)`);
  emit(`${e}_k = xform.scale@1(${e}_s, by=[0.95, 0.38, 0.5])`);
  emit(`${e}_r = xform.rotate@1(${e}_k, angles=[0, 0, ${-s * 20}])`);
  emit(`${e}_a = xform.move@1(${e}_r, by=${V([s * 1.7, 20.5, 5.55])})`);
  emit(`${e}_m = solid.material@1(${e}_a, color=[1, 0.8, 0.4], emissive=[9, 5, 1.2], roughness=0.3)`);
  eyeNodes.push(`${e}_m`);
}
emit(`eyes_u = solid.union@1(${eyeNodes.join(", ")})`);
out("eyes", headChild("eyes", "eyes_u", {}, NECK));

// the glowing core in the chest, and the spikes
emit(`core_s = solid.sphere@1(radius=1.55)`);
emit(`core_a = xform.move@1(core_s, by=[0, 13.8, 4.3])`);
emit(`core_m = solid.material@1(core_a, color=[1, 0.6, 0.25], emissive=[3.2, 1.1, 0.18], roughness=0.4)`);
emit(`core_fx = fx.emitter@1(core_m, name="core_embers", rate=30, lifetime=[1, 2], speed=[0.6, 1.6], cone=70, shape="sphere", extent=[1.9, 0, 0], size=[0.18, 0.02], colour=[1, 0.7, 0.2], colour_end=[0.8, 0.1, 0], alpha=[1, 0], glow=5, blend="additive")`);
out("core", onTorso("core", "core_fx", {}, HIPS));
const spikes = [];
for (let i = 0; i < 6; i++) spikes.push(cone(`spike${i}`, [((i % 2) - 0.5) * 6, 14.5 + (i >> 1) * 2.4, -4.5], 0.9, 3.4, [-80, 0, 0], { program: "bone_prog", tile: 3 }));
emit(`spikes_u = solid.union@1(${spikes.join(", ")})`);
out("spikes", onTorso("spikes", "spikes_u", {}, HIPS));

// arms: shoulder -> upper arm -> forearm -> fist, per side (p = +x, n = -x)
for (const s of [1, -1]) {
  const k = s > 0 ? "p" : "n";
  const SH = [s * 8.6, 16.8, 0.2];
  const shoulder = ellipsoid(`shoulder_${k}`, SH, [3.9, 3.2, 3.9], ROCK);
  const pauldron = [shoulder];
  for (let i = 0; i < 3; i++) pauldron.push(cone(`pauldron_spike_${k}${i}`, [s * (8.8 + i * 0.5), 19.6 + i * 0.6, -1.8 + i * 1.8], 0.8, 4.2 + i * 0.4, [-20 + i * 18, 0, -s * (60 - i * 14)], { program: "bone_prog", tile: 3 }));
  emit(`pauldron_${k}_u = solid.union@1(${pauldron.join(", ")})`);
  out(`shoulder_${k}`, onTorso(`shoulder_${k}`, `pauldron_${k}_u`, {}, HIPS));
  // each arm's own clips (the p arm slams in slam_p, the n arm in slam_n; the sweep is the p arm's; stomp and roar use both)
  const mirror = (frames) => frames.map(([t0, v]) => [t0, [v[0], v[1] * s, v[2] * s]]);
  const own = (slam) => ({
    idle: mirror([[0, [-8, 0, 4]], [2, [-11, 0, 6]], [4, [-8, 0, 4]]]),
    slam_p: s > 0 ? slam : mirror([[0, [-8, 0, 4]], [3, [-8, 0, 4]]]),
    slam_n: s < 0 ? mirror(slam) : mirror([[0, [-8, 0, 4]], [3, [-8, 0, 4]]]),
    sweep: s > 0 ? [[0, [-8, 0, 4]], [0.7, [-72, 0, 78]], [1.55, [-88, 0, -62]], [2.6, [-8, 0, 4]]] : mirror([[0, [-8, 0, 4]], [1.0, [-30, 0, 20]], [2.6, [-8, 0, 4]]]),
    stomp: mirror([[0, [-8, 0, 4]], [1.0, [-150, 0, 12]], [1.55, [-60, 0, 6]], [2.2, [-58, 0, 6]], [3, [-8, 0, 4]]]),
    roar: mirror([[0, [-8, 0, 4]], [0.8, [-35, 0, 52]], [2.4, [-35, 0, 52]], [3.4, [-8, 0, 4]]]),
    death: mirror([[0, [-8, 0, 4]], [6.5, [10, 0, 16]]]),
  });
  const slamArm = [[0, [-8, 0, 4]], [0.6, [-62, 0, 4]], [1.3, [-172, 0, 7]], [1.6, [-172, 0, 7]], [1.78, [-64, 0, 3]], [2.3, [-62, 0, 3]], [3, [-8, 0, 4]]];
  const upper = cylinder(`upper_${k}`, [s * 8.8, 12.8, 0.2], 1.8, 8, ROCK, 2.4);
  const upperNode = parentTo(`upper_${k}`, animate(`upper_${k}`, upper, "rotation", SH, own(slamArm)), "colossus");
  out(`upper_${k}`, upperNode);
  const EL = [s * 8.8, 8.8, 0.2];
  const slamFore = [[0, [-40, 0, 0]], [0.6, [-30, 0, 0]], [1.3, [-20, 0, 0]], [1.6, [-20, 0, 0]], [1.78, [58, 0, 0]], [2.3, [56, 0, 0]], [3, [-40, 0, 0]]];
  const foreData = {
    idle: [[0, [-40, 0, 0]], [2, [-46, 0, 0]], [4, [-40, 0, 0]]],
    slam_p: s > 0 ? slamFore : [[0, [-40, 0, 0]], [3, [-40, 0, 0]]],
    slam_n: s < 0 ? slamFore : [[0, [-40, 0, 0]], [3, [-40, 0, 0]]],
    sweep: [[0, [-40, 0, 0]], [0.7, [-20, 0, 0]], [1.55, [-10, 0, 0]], [2.6, [-40, 0, 0]]],
    stomp: [[0, [-40, 0, 0]], [1.0, [-20, 0, 0]], [1.55, [52, 0, 0]], [2.2, [50, 0, 0]], [3, [-40, 0, 0]]],
    roar: [[0, [-40, 0, 0]], [0.8, [-70, 0, 0]], [2.4, [-70, 0, 0]], [3.4, [-40, 0, 0]]],
    death: [[0, [-40, 0, 0]], [6.5, [-20, 0, 0]]],
  };
  const fore = cylinder(`fore_${k}`, [s * 8.8, 4.8, 0.2], 1.6, 8, ROCK, 2.1);
  const foreNode = parentTo(`fore_${k}`, animate(`fore_${k}`, fore, "rotation", EL, foreData), `upper_${k}`);
  out(`fore_${k}`, foreNode);
  const fist = box(`fist_${k}`, [s * 8.8, -0.2, 0.7], [4.0, 3.4, 4.2], ROCK);
  const knuckles = [];
  for (let i = 0; i < 4; i++) knuckles.push(cone(`knuckle_${k}${i}`, [s * 8.8 + (i - 1.5) * 0.95, -0.4, 3.0], 0.42, 1.5, [90, 0, 0], { program: "bone_prog", tile: 2 }));
  emit(`fist_${k}_u = solid.union@1(${fist}, ${knuckles.join(", ")})`);
  out(`fist_${k}`, parentTo(`fist_${k}`, `fist_${k}_u`, `fore_${k}`));
}

// ---- the player's weapon: a sword that follows the desktop hand ----
emit(`blade_g = solid.cone@1(radius=0.055, top_radius=0.0, height=1.1, segments=4)`);
emit(`blade_r = xform.rotate@1(blade_g, angles=[90, 0, 0])`);
emit(`blade_k = xform.scale@1(blade_r, by=[1.0, 0.32, 1.0])`);
emit(`blade_a = xform.move@1(blade_k, by=[0, 0, 0.65])`);
emit(`blade_m = solid.material@1(blade_a, color=[0.8, 0.82, 0.9], metallic=0.95, roughness=0.2, emissive=[0.5, 0.18, 0.03])`);
emit(`guard_g = solid.box@1(size=[0.36, 0.05, 0.07])`);
emit(`guard_a = xform.move@1(guard_g, by=[0, 0, 0.08])`);
emit(`guard_m = solid.material@1(guard_a, color=[0.7, 0.5, 0.12], metallic=0.9, roughness=0.3)`);
emit(`grip_g = solid.cylinder@1(radius=0.024, height=0.3, segments=12)`);
emit(`grip_r = xform.rotate@1(grip_g, angles=[90, 0, 0])`);
emit(`grip_a = xform.move@1(grip_r, by=[0, 0, -0.1])`);
emit(`grip_m = solid.material@1(grip_a, color=[0.14, 0.07, 0.04], roughness=0.8)`);
emit(`pommel_g = solid.sphere@1(radius=0.04)`);
emit(`pommel_a = xform.move@1(pommel_g, by=[0, 0, -0.26])`);
emit(`pommel_m = solid.material@1(pommel_a, color=[0.7, 0.5, 0.12], metallic=0.9, roughness=0.3)`);
emit(`sword_u = solid.union@1(blade_m, guard_m, grip_m, pommel_m)`);
emit(`sword_fx = fx.emitter@1(sword_u, name="sword_embers", rate=22, lifetime=[0.4, 0.9], speed=[0.1, 0.5], cone=80, shape="sphere", extent=[0.05, 0, 0], size=[0.05, 0.0], colour=[1, 0.6, 0.2], colour_end=[0.8, 0.1, 0], alpha=[1, 0], glow=6, blend="additive")`);
out("sword_mesh", "sword_fx");
emit(`k_sword = game.kind@1(name="sword", size=0.15, tags=["hand_r"], part="sword_mesh")`);
emit(`s_sword = game.spawn@1(kind="sword", at=[0, 1.5, 27])`);
emit(`i_sword = game.input@1(name="rhand", follow="right_hand", target="tag:hand_r")`);
emit(`game_all = game.collect@1(k_sword, s_sword, i_sword, name="colossus")`);
outputs.push("output rules = game_all");

// ---- the fight itself ----
const source = readFileSync(`${here}script.js`, "utf8");
emit(`fight = script.js@1(name="fight", source=${JSON.stringify(source)})`);

console.log(["# about: THE ASHEN COLOSSUS: a boss fight in a ruined volcanic arena. A 24 m stone titan with lava in its cracks (animated parts, nine clips driven by a script), a first-person player with a sword, fire bolts, a dodge with invulnerability frames and healing potions, three phases, telegraphed attacks, effects and a screen UI. Keys: WASD walk, Space jump, Shift run, mouse look (click the view to lock the mouse), left click or F slash, right click or Q fire bolt, E dodge, X potion, Enter to start or retry. No sound.", "# kind: world", ...L, ...outputs].join("\n"));
