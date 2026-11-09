// Dimension audit for a passenger car: reads the OBJ files `dump_obj` wrote for a car scene, measures the real geometry, and checks it against what a
// mid-size sedan is (the class of a Toyota Camry or Honda Accord: about 4.9 m long, 1.85 m wide, 1.45 m tall, 2.8 m wheelbase, 17 inch wheels).
//
//   ./target/release/examples/dump_obj web/tests/scenes/characters/car.txt /tmp/car_obj
//   bun engine/tools/audit-car.ts /tmp/car_obj
//
// Parts it reads: body, cabin_paint, glass (the shell), tire_fl/fr/rl/rr (the wheels). Instanced parts (lamps, handles) are written once untransformed by
// dump_obj, so they are not used. Each failing line names the value in the scene's parameter table to change.
import { readFileSync, existsSync } from 'node:fs';

type V = [number, number, number];
const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun engine/tools/audit-car.ts <obj dir>');
  process.exit(2);
}

function readObj(name: string): V[] {
  const path = `${dir}/${name}.obj`;
  if (!existsSync(path)) throw new Error(`missing part ${name} in ${dir}`);
  const points: V[] = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      points.push([Number(x), Number(y), Number(z)]);
    }
  }
  return points;
}

const bounds = (pts: V[]) => {
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return { lo, hi, mid: lo.map((v, i) => (v + hi[i]) / 2) };
};

const body = readObj('body');
const cabin = readObj('cabin_paint');
const glass = readObj('glass');
const shell = [...body, ...cabin, ...glass];
const tires = Object.fromEntries(['fl', 'fr', 'rl', 'rr'].map((t) => [t, bounds(readObj(`tire_${t}`))]));

const sb = bounds(shell);
const bb = bounds(body);
const length = sb.hi[2] - sb.lo[2];
const width = bb.hi[0] - bb.lo[0];
const height = sb.hi[1];
const frontAxle = (tires.fl.mid[2] + tires.fr.mid[2]) / 2;
const rearAxle = (tires.rl.mid[2] + tires.rr.mid[2]) / 2;
const wheelbase = frontAxle - rearAxle;
const track = (Math.abs(tires.fl.mid[0] - tires.fr.mid[0]) + Math.abs(tires.rl.mid[0] - tires.rr.mid[0])) / 2;
const tyreDiameter = tires.fl.hi[1] - tires.fl.lo[1];
const tyreWidth = tires.fl.hi[0] - tires.fl.lo[0];
const tyreBottom = Math.min(...Object.values(tires).map((t) => t.lo[1]));
const clearance = bb.lo[1];
const tyreOuter = Math.max(...Object.values(tires).map((t) => Math.max(Math.abs(t.lo[0]), Math.abs(t.hi[0]))));
const asym = Math.abs(bb.hi[0] + bb.lo[0]);

// the centre-line top profile: highest point in each 2 cm slice of z along the middle (|x| < 1 cm)
const top = new Map<number, number>();
for (const p of glass) {
  if (Math.abs(p[0]) > 0.01) continue;
  const key = Math.round(p[2] / 0.02);
  top.set(key, Math.max(top.get(key) ?? -Infinity, p[1]));
}
const slices = [...top.entries()].sort((a, b) => a[0] - b[0]).map(([k, y]) => [k * 0.02, y] as [number, number]);
const peak = Math.max(...slices.map((s) => s[1]));
const roofSlices = slices.filter((s) => s[1] > peak - 0.01);
const roofFront = Math.max(...roofSlices.map((s) => s[0]));
const roofRear = Math.min(...roofSlices.map((s) => s[0]));
// the windshield rake: forward of the roof, the slope between the points 8 cm and 32 cm below the peak (away from the rounded corners)
const front = slices.filter((s) => s[0] >= roofFront);
const at = (drop: number) => front.find((s) => s[1] <= peak - drop) ?? [NaN, NaN];
const w1 = at(0.08);
const w2 = at(0.32);
const rake = (Math.atan2(w1[1] - w2[1], w2[0] - w1[0]) * 180) / Math.PI;

type Rule = { name: string; value: number; lo: number; hi: number; unit: string; hint: string };
const rules: Rule[] = [
  { name: 'overall length', value: length, lo: 4.6, hi: 5.0, unit: 'm', hint: 'car_len' },
  { name: 'body width (without mirrors)', value: width, lo: 1.78, hi: 1.95, unit: 'm', hint: 'body_w' },
  { name: 'overall height', value: height, lo: 1.4, hi: 1.5, unit: 'm', hint: 'roof_h' },
  { name: 'wheelbase', value: wheelbase, lo: 2.75, hi: 2.95, unit: 'm', hint: 'wheelbase and the wheel z positions' },
  { name: 'front overhang (bumper to front axle)', value: sb.hi[2] - frontAxle, lo: 0.85, hi: 1.05, unit: 'm', hint: 'front_over / car_len (and the front wheel z)' },
  { name: 'rear overhang (rear axle to bumper)', value: rearAxle - sb.lo[2], lo: 0.95, hi: 1.2, unit: 'm', hint: 'car_len, wheelbase, front_over' },
  { name: 'track (wheel centre to wheel centre)', value: track, lo: 1.5, hi: 1.65, unit: 'm', hint: 'track and the wheel x positions' },
  { name: 'tyre diameter', value: tyreDiameter, lo: 0.64, hi: 0.7, unit: 'm', hint: 'tyre_r and the tyre profile' },
  { name: 'tyre width', value: tyreWidth, lo: 0.19, hi: 0.24, unit: 'm', hint: 'tyre_w and the tyre profile' },
  { name: 'tyres touch the ground (lowest point)', value: tyreBottom, lo: -0.004, hi: 0.004, unit: 'm', hint: 'wheel y = tyre radius' },
  { name: 'ground clearance (underbody)', value: clearance, lo: 0.12, hi: 0.17, unit: 'm', hint: 'clear' },
  { name: 'tyre outside the body line', value: bb.hi[0] - tyreOuter, lo: 0.005, hi: 0.1, unit: 'm', hint: 'track / body_w' },
  { name: 'left-right symmetry error', value: asym, lo: 0, hi: 0.003, unit: 'm', hint: 'a loft or a placement is not mirrored' },
  { name: 'windshield rake from horizontal', value: rake, lo: 25, hi: 40, unit: 'deg', hint: 'cowl_z, roof_front_z, roof_h, belt_h' },
  { name: 'roof flat between the axles (centre z over wheelbase)', value: ((roofFront + roofRear) / 2 - rearAxle) / wheelbase, lo: 0.3, hi: 0.7, unit: '', hint: 'roof_rear_z, roof_front_z' },
];

// ---- attachment: every part that stands proud of the body is ray-cast against the shell. proud = how far its outer face sticks out past the surface;
// float = how far its INNER face is outside the surface (a gap you could see daylight through). ----
const shellTris: [V, V, V][] = [];
for (const name of ['body', 'cabin_paint', 'glass']) {
  const pts: V[] = [];
  for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      pts.push([Number(x), Number(y), Number(z)]);
    } else if (line.startsWith('f ')) {
      const [a, b, c] = line.trim().split(/\s+/).slice(1).map((i) => Number(i) - 1);
      shellTris.push([pts[a], pts[b], pts[c]]);
    }
  }
}
function readTris(names: string[]): [V, V, V][] {
  const out: [V, V, V][] = [];
  for (const name of names) {
    const pts: V[] = [];
    for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
      if (line.startsWith('v ')) {
        const [, x, y, z] = line.trim().split(/\s+/);
        pts.push([Number(x), Number(y), Number(z)]);
      } else if (line.startsWith('f ')) {
        const [a, b, c] = line.trim().split(/\s+/).slice(1).map((i) => Number(i) - 1);
        out.push([pts[a], pts[b], pts[c]]);
      }
    }
  }
  return out;
}
function castDistance(o: V, d: V, tris: [V, V, V][] = shellTris): number {
  let best = Infinity;
  for (const [a, b, c] of tris) {
    const e1: V = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const e2: V = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    const p: V = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
    const det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
    if (Math.abs(det) < 1e-12) continue;
    const s: V = [o[0] - a[0], o[1] - a[1], o[2] - a[2]];
    const u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) / det;
    if (u < 0 || u > 1) continue;
    const q: V = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
    const v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) / det;
    if (v < 0 || u + v > 1) continue;
    const t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) / det;
    if (t > 1e-9 && t < best) best = t;
  }
  return best;
}
// part, axis (0 x, 1 y, 2 z), outward sign
const proudParts: [string, number, number][] = [
  ['trim_l', 0, 1], ['trim_r', 0, -1], ['mirror_l', 0, 1], ['mirror_r', 0, -1], ['handle_fl', 0, 1], ['handle_fr', 0, -1], ['handle_rl', 0, 1], ['handle_rr', 0, -1],
  ['plate_front', 2, 1], ['plate_rear', 2, -1], ['antenna', 1, 1],
];
const attach: { name: string; proud: number; float: number }[] = [];
for (const [name, axis, sign] of proudParts) {
  const b = bounds(readObj(name));
  const outer = sign > 0 ? b.hi[axis] : b.lo[axis];
  const inner = sign > 0 ? b.lo[axis] : b.hi[axis];
  const origin = [...b.mid] as V;
  origin[axis] = outer + sign * 0.5;
  const dir: V = [0, 0, 0];
  dir[axis] = -sign;
  const dist = castDistance(origin, dir);
  const hit = origin[axis] - sign * dist;
  attach.push({ name, proud: sign * (outer - hit), float: sign * (inner - hit) });
}
for (const a of attach) {
  rules.push({ name: `${a.name} sits on the surface (inner face outside it)`, value: a.float, lo: -0.03, hi: 0.008, unit: 'm', hint: `${a.name}'s position in the scene (the probe gives the surface)` });
  rules.push({ name: `${a.name} stands proud of the surface`, value: a.proud, lo: 0.002, hi: a.name.startsWith('mirror') ? 0.18 : 0.06, unit: 'm', hint: `${a.name}'s position or size` });
}

// ---- the interior: the cabin must be a real, closed room with seats in it ----
const roomShell = readTris(['body', 'cabin_paint', 'glass', 'window_seals', 'interior_floor', 'interior_doors', 'interior_headliner']);
const inside: V = [0, 0.8, -0.35];
const rayDirs: [string, V][] = [['+x', [1, 0, 0]], ['-x', [-1, 0, 0]], ['+y', [0, 1, 0]], ['-y', [0, -1, 0]], ['+z', [0, 0, 1]], ['-z', [0, 0, -1]]];
const leaks = rayDirs.filter(([, d]) => !Number.isFinite(castDistance(inside, d, roomShell))).length;
const widthAt = (y: number) => castDistance([0, y, -0.35], [1, 0, 0], roomShell) + castDistance([0, y, -0.35], [-1, 0, 0], roomShell);
const roomUp = castDistance(inside, [0, 1, 0], roomShell);
const roomDown = castDistance(inside, [0, -1, 0], roomShell);
const headroom = castDistance([0.37, 0.56, -0.05], [0, 1, 0], roomShell);
const seatBox = bounds([...readObj('seat_fl'), ...readObj('seat_fr'), ...readObj('seat_rear')]);
const under = bounds(readObj('underbody'));
const interior: Rule[] = [
  { name: 'cabin is a closed room (rays from inside that escape)', value: leaks, lo: 0, hi: 0, unit: '', hint: 'room_a / room_b lofts: a gap in the cavity or a missing wall' },
  { name: 'interior width at shoulder height (y 1.2)', value: widthAt(1.2), lo: 1.3, hi: 1.55, unit: 'm', hint: 'greenhouse widths (wr) and the room inset' },
  { name: 'interior width at elbow height (y 0.8)', value: widthAt(0.8), lo: 1.4, hi: 1.65, unit: 'm', hint: 'door thickness in the room loft (xi)' },
  { name: 'floor to roof lining', value: roomUp + roomDown, lo: 1.0, hi: 1.2, unit: 'm', hint: 'floor_y and roof_h' },
  { name: 'headroom above the driver cushion', value: headroom, lo: 0.8, hi: 1.0, unit: 'm', hint: 'seat height (cushion y) or roof_h' },
  { name: 'seats inside the room (lowest point above the floor)', value: seatBox.lo[1] - Number(0.3), lo: -0.02, hi: 0.25, unit: 'm', hint: 'seat cushion y against floor_y' },
  { name: 'seats inside the room (widest x)', value: Math.max(Math.abs(seatBox.lo[0]), Math.abs(seatBox.hi[0])), lo: 0.3, hi: 0.78, unit: 'm', hint: 'seat x positions' },
  { name: 'underbody parts clear the ground (lowest point)', value: under.lo[1], lo: 0.04, hi: 0.14, unit: 'm', hint: 'underbody part heights against clear' },
  { name: 'underbody stays under the belly (highest point)', value: under.hi[1], lo: 0.05, hi: 0.15, unit: 'm', hint: 'underbody part heights against clear' },
];
rules.push(...interior);

console.log(`measured: length ${length.toFixed(3)}, width ${width.toFixed(3)}, height ${height.toFixed(3)}, front axle z ${frontAxle.toFixed(3)}, rear axle z ${rearAxle.toFixed(3)}, roof flat z ${roofRear.toFixed(2)}..${roofFront.toFixed(2)} at y ${peak.toFixed(3)}`);
let failed = 0;
for (const r of rules) {
  const ok = r.value >= r.lo && r.value <= r.hi;
  if (!ok) failed++;
  const shown = r.unit === 'deg' ? r.value.toFixed(1) : r.value.toFixed(3);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(54)} ${shown.padStart(8)}${r.unit}  target ${r.lo}-${r.hi}${r.unit}${ok ? '' : `  ${r.value < r.lo ? 'too low' : 'too high'}: change ${r.hint}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${rules.length} rules fail`);
if (process.env.DEBUG_PROFILE) console.log(JSON.stringify(slices.filter((_, i) => i % 6 === 0).map((s) => [+s[0].toFixed(2), +s[1].toFixed(3)])), JSON.stringify({ w1, w2 }));
process.exit(failed === 0 ? 0 : 1);
