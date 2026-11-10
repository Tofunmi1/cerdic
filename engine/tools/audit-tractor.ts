// Dimension audit for the farm tractor (web/tests/scenes/tractor/tractor2.txt): reads the OBJ parts `dump_obj` wrote and checks the measured mesh against what a 150 hp row-crop tractor is:
// about 5.3 m long, 2.6 m over the rear tyres, 2.8 m wheelbase, rear tyres 1.96 m by 0.65 m (650/85 R38 class), front tyres 1.30 m by 0.54 m (540/65 R28 class), about 3.1 m to the cab roof, 0.4 to 0.55 m
// under the belly. +z is forward, x across, y up, the origin on the ground at the middle of the wheelbase (the same convention as the car).
//
//   ./target/release/examples/dump_obj web/tests/scenes/tractor/tractor2.txt /tmp/tr_obj
//   bun engine/tools/audit-tractor.ts /tmp/tr_obj
import { existsSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun engine/tools/audit-tractor.ts <obj dir>');
  process.exit(2);
}
function read(name: string): V[] {
  const path = `${dir}/${name}.obj`;
  if (!existsSync(path)) throw new Error(`missing part ${name}`);
  const out: V[] = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      out.push([Number(x), Number(y), Number(z)]);
    }
  }
  return out;
}
const bounds = (pts: V[]) => {
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return { lo, hi, mid: lo.map((v, i) => (v + hi[i]) / 2), size: hi.map((v, i) => v - lo[i]) };
};

const tyre = { rl: bounds(read('wrl_tire')), rr: bounds(read('wrr_tire')), fl: bounds(read('wfl_tire')), fr: bounds(read('wfr_tire')) };
const lugs = bounds([...read('wrl_lugs'), ...read('wrr_lugs')]);
const hood = bounds(read('hood'));
const chassis = bounds(read('chassis'));
const glass = bounds(read('tr_glass'));
const paint = read('tr_paint');
const seat = bounds(read('tr_seat'));
const trim = read('tr_trim');
const everything = ['hood', 'chassis', 'tr_black', 'tr_paint', 'tr_glass', 'tr_trim', 'tr_lamps', 'tr_tail', 'wrl_tire', 'wrr_tire', 'wfl_tire', 'wfr_tire', 'wrl_lugs', 'wrr_lugs'].flatMap(read);
const eb = bounds(everything);
const noBeacon = bounds(['hood', 'chassis', 'tr_black', 'tr_paint', 'tr_glass', 'tr_trim'].flatMap(read));
const all = bounds([...everything, ...read('tr_beacon'), ...read('tr_mirror')]);

const frontAxle = (tyre.fl.mid[2] + tyre.fr.mid[2]) / 2;
const rearAxle = (tyre.rl.mid[2] + tyre.rr.mid[2]) / 2;
const widthRear = Math.max(tyre.rl.hi[0], tyre.rr.hi[0]) - Math.min(tyre.rl.lo[0], tyre.rr.lo[0]);
const asym = Math.abs(all.hi[0] + all.lo[0]);
// a steering wheel sits between the seat and the dash; the wheel rim is the highest trim point near the column
const wheelTop = Math.max(...trim.filter((p) => Math.abs(p[0]) < 0.22 && p[2] > -0.2 && p[2] < 0.1).map((p) => p[1]));

type Rule = { name: string; value: number; lo: number; hi: number; unit: string; hint: string };
const rules: Rule[] = [
  { name: 'overall length (front weight to the rear lugs)', value: eb.size[2], lo: 5.0, hi: 5.6, unit: 'm', hint: 'the front weight z, the hitch length or the wheel positions' },
  { name: 'wheelbase', value: frontAxle - rearAxle, lo: 2.7, hi: 2.9, unit: 'm', hint: 'the wheel z positions' },
  { name: 'width over the rear tyres', value: widthRear, lo: 2.4, hi: 2.65, unit: 'm', hint: 'the rear wheel x (track) or the tyre width' },
  { name: 'rear tyre diameter', value: tyre.rl.size[1], lo: 1.85, hi: 2.05, unit: 'm', hint: 'the rear tyre profile radius' },
  { name: 'rear tyre width', value: tyre.rl.size[0], lo: 0.58, hi: 0.72, unit: 'm', hint: 'the rear tyre profile width' },
  { name: 'front tyre diameter', value: tyre.fl.size[1], lo: 1.2, hi: 1.4, unit: 'm', hint: 'the front tyre profile radius' },
  { name: 'front tyre width', value: tyre.fl.size[0], lo: 0.46, hi: 0.6, unit: 'm', hint: 'the front tyre profile width' },
  { name: 'rear tyres touch the ground', value: tyre.rl.lo[1], lo: -0.01, hi: 0.01, unit: 'm', hint: 'the rear wheel y = its radius' },
  { name: 'front tyres touch the ground', value: tyre.fl.lo[1], lo: -0.02, hi: 0.01, unit: 'm', hint: 'the front wheel y = its radius' },
  { name: 'lugs stay above the ground', value: lugs.lo[1], lo: -0.03, hi: 0.1, unit: 'm', hint: 'the lug ring radius in the rear wheel macro' },
  { name: 'height to the roof equipment (no beacon)', value: noBeacon.hi[1], lo: 3.0, hi: 3.4, unit: 'm', hint: 'the roof y in the cab frame and the roof slab' },
  { name: 'glass height (belt to roof line)', value: glass.size[1], lo: 1.3, hi: 1.6, unit: 'm', hint: 'the cab belt and roof y' },
  { name: 'cab width over the glass', value: glass.size[0], lo: 1.5, hi: 1.75, unit: 'm', hint: 'the post x positions (0.80)' },
  { name: 'cab length over the glass', value: glass.size[2], lo: 1.5, hi: 1.8, unit: 'm', hint: 'the rear and front glass z' },
  { name: 'hood length', value: hood.size[2], lo: 1.8, hi: 2.0, unit: 'm', hint: 'the hood ring z range' },
  { name: 'hood width', value: hood.size[0], lo: 0.75, hi: 0.95, unit: 'm', hint: 'the hood ring half widths' },
  { name: 'hood top height', value: hood.hi[1], lo: 1.9, hi: 2.05, unit: 'm', hint: 'the hood ring tops' },
  { name: 'hood starts in front of the windshield', value: hood.lo[2] - glass.hi[2], lo: 0.0, hi: 0.3, unit: 'm', hint: 'the hood rear z against the windshield z' },
  { name: 'ground clearance under the belly', value: chassis.lo[1], lo: 0.35, hi: 0.55, unit: 'm', hint: 'the sump and axle field heights' },
  { name: 'driver seat inside the cab (x)', value: Math.max(Math.abs(seat.lo[0]), Math.abs(seat.hi[0])), lo: 0.2, hi: 0.4, unit: 'm', hint: 'the seat x half width' },
  { name: 'seat behind the windshield', value: glass.hi[2] - seat.hi[2], lo: 0.5, hi: 1.0, unit: 'm', hint: 'the seat z against the windshield z' },
  // the seat mesh starts at its suspension base; the cushion sits about 0.25 m above that, and a wheel rim is 0.35 to 0.55 m over the cushion
  { name: 'steering wheel above the seat cushion', value: wheelTop - (seat.lo[1] + 0.25), lo: 0.35, hi: 0.55, unit: 'm', hint: 'the steering wheel centre y' },
  { name: 'rear fender clears the tyre (gap above the tyre top)', value: Math.max(...paint.filter((p) => p[0] > 0.6 && p[2] < -0.5 && p[2] > -2.0 && p[1] > 1.2 && p[1] < 2.3).map((p) => p[1])) - tyre.rl.hi[1], lo: 0.04, hi: 0.2, unit: 'm', hint: 'the rear fender arc radius (1.08) against the tyre radius' },
  { name: 'left-right symmetry', value: asym, lo: 0, hi: 0.01, unit: 'm', hint: 'a part is not mirrored' },
];
console.log(`measured: z ${eb.lo[2].toFixed(2)}..${eb.hi[2].toFixed(2)}, x ${eb.lo[0].toFixed(2)}..${eb.hi[0].toFixed(2)}, y 0..${all.hi[1].toFixed(2)} (with beacon)`);
let failed = 0;
for (const r of rules) {
  const ok = Number.isFinite(r.value) && r.value >= r.lo && r.value <= r.hi;
  if (!ok) failed++;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(56)} ${r.value.toFixed(3).padStart(8)}${r.unit}  target ${r.lo}-${r.hi}${r.unit}${ok ? '' : `  ${r.value < r.lo ? 'too low' : 'too high'}: change ${r.hint}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${rules.length} rules fail`);
process.exit(failed === 0 ? 0 : 1);
