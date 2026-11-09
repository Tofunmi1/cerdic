// Dimension audit for the Cessna 185 Skywagon on EDO 3430 floats: reads the OBJ files `dump_obj` wrote and checks the measured
// mesh against the aircraft's real dimensions.
//
//   ./target/release/examples/dump_obj web/tests/scenes/cessna185/cessna185.txt /tmp/cessna_obj
//   bun engine/tools/audit-cessna.ts /tmp/cessna_obj
//
// Real figures: length 7.85 m, wingspan 10.92 m, wing area 16.2 m2, tail height 2.36 m (3.71 m over the water on floats),
// 2.08 m two-blade prop; EDO 3430 floats 6.4 m long, 0.81 m wide, 2.24 m track, 0.25 m static draft. Float figures come from
// the float maker's table and are approximate, so those rules carry wider tolerances. Each failing line names the value in the
// scene's parameter table or loft to change.
import { existsSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun engine/tools/audit-cessna.ts <obj dir>');
  process.exit(2);
}
const has = (name: string) => existsSync(`${dir}/${name}.obj`);
function readObj(name: string): V[] {
  const out: V[] = [];
  for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      out.push([Number(x), Number(y), Number(z)]);
    }
  }
  return out;
}
function faces(name: string): V[][] {
  const verts: V[] = [];
  const tris: V[][] = [];
  for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      verts.push([Number(x), Number(y), Number(z)]);
    } else if (line.startsWith('f ')) {
      tris.push(line.trim().split(/\s+/).slice(1).map((i) => verts[Number(i.split('/')[0]) - 1]));
    }
  }
  return tris.filter((f) => f.length === 3);
}
const bounds = (pts: V[]) => {
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return { lo, hi };
};
const triangles = (name: string) =>
  readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n').filter((l) => l.startsWith('f ')).length;

const airframe = ['fuselage', 'wings', 'tail', 'float_left', 'float_right', 'canopy', 'glass', 'interior',
  'pilot', 'pilot_face', 'pilot_gear', 'struts', 'propeller', 'prop_tips', 'spinner', 'details', 'trim', 'lights',
  'registration'].flatMap(readObj);
const ab = bounds(airframe);
const fuselage = bounds(readObj('fuselage'));
const wings = bounds(readObj('wings'));
const tail = bounds(readObj('tail'));
const prop = bounds(readObj('propeller'));
const floatL = bounds(readObj('float_left'));
const floatR = bounds(readObj('float_right'));
const centerX = (b: { lo: number[]; hi: number[] }) => (b.lo[0] + b.hi[0]) / 2;

// Planform area: project every wing triangle onto the ground plane; the upper and lower skins each cover it once.
let projected = 0;
for (const [a, b, c] of faces('wings')) {
  projected += Math.abs((b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2])) / 2;
}
const wingArea = projected / 2;

const total = ['fuselage', 'wings', 'tail', 'float_left', 'float_right', 'canopy', 'glass', 'interior',
  'pilot', 'pilot_face', 'pilot_gear', 'struts', 'propeller', 'prop_tips', 'spinner', 'details', 'trim',
  'lights', 'registration'].filter(has).map(triangles).reduce((a, b) => a + b, 0);

const parts = ['fuselage', 'wings', 'tail', 'float_left', 'float_right', 'canopy', 'glass', 'interior',
  'pilot', 'pilot_face', 'pilot_gear', 'struts', 'propeller', 'prop_tips', 'spinner', 'details', 'trim',
  'lights', 'registration'];
const missing = parts.filter((p) => !has(p));

type Rule = { name: string; value: number; lo: number; hi: number; unit: string; hint: string };
const rules: Rule[] = [
  { name: 'length spinner tip to tail', value: ab.hi[2] - ab.lo[2], lo: 7.6, hi: 8.1, unit: 'm', hint: 'the fuselage loft stations and the spinner z' },
  { name: 'wingspan', value: wings.hi[0] - wings.lo[0], lo: 10.7, hi: 11.1, unit: 'm', hint: 'S in wing_code' },
  { name: 'wing planform area', value: wingArea, lo: 15.4, hi: 17.0, unit: 'm2', hint: 'the root and tip chords in wing_code' },
  { name: 'fin top above the waterline', value: tail.hi[1], lo: 3.6, hi: 3.8, unit: 'm', hint: 'fin_top' },
  { name: 'propeller diameter', value: prop.hi[1] - prop.lo[1], lo: 2.0, hi: 2.16, unit: 'm', hint: 'prop_d' },
  { name: 'float length', value: floatL.hi[2] - floatL.lo[2], lo: 6.2, hi: 6.6, unit: 'm', hint: 'float_len and the loft stations (-3.45 to 3.0)' },
  { name: 'float beam', value: floatL.hi[0] - floatL.lo[0], lo: 0.75, hi: 0.88, unit: 'm', hint: 'the maximum half-width in wK' },
  { name: 'float track (centre to centre)', value: centerX(floatL) - centerX(floatR), lo: 2.1, hi: 2.4, unit: 'm', hint: 'the xform.move offsets on fl_place and fr_place' },
  { name: 'static draft at the step', value: -Math.min(...readObj('float_left').map((p) => p[1])), lo: 0.20, hi: 0.30, unit: 'm', hint: 'the keel line kK around the step' },
  { name: 'left-right symmetry error of the airframe', value: Math.abs(ab.hi[0] + ab.lo[0]), lo: 0, hi: 0.02, unit: 'm', hint: 'a part or a float placement is not mirrored' },
  { name: 'wing sits above the fuselage spine', value: wings.lo[1] - fuselage.hi[1], lo: -0.1, hi: 0.6, unit: 'm', hint: 'the wing yc (2.40) against the cabin roof' },
  { name: 'triangles in the whole scene', value: total, lo: 1000, hi: 40000, unit: '', hint: 'the loft station counts' },
  { name: 'outputs missing from the scene', value: missing.length, lo: 0, hi: 0, unit: '', hint: `add ${missing.join(', ') || 'nothing'}` },
];

console.log(`measured airframe: z ${ab.lo[2].toFixed(2)}..${ab.hi[2].toFixed(2)}, x ${ab.lo[0].toFixed(2)}..${ab.hi[0].toFixed(2)}, y ${ab.lo[1].toFixed(2)}..${ab.hi[1].toFixed(2)}`);
let failed = 0;
for (const r of rules) {
  const ok = r.value >= r.lo && r.value <= r.hi;
  if (!ok) failed++;
  const shown = r.unit === '' ? r.value.toFixed(0) : r.value.toFixed(2);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(48)} ${shown.padStart(8)}${r.unit}  target ${r.lo}-${r.hi}${r.unit}${ok ? '' : `  ${r.value < r.lo ? 'too low' : 'too high'}: change ${r.hint}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${rules.length} rules fail`);
process.exit(failed === 0 ? 0 : 1);
