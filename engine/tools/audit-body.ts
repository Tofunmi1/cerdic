// Proportion audit for the human body blockout: reads the OBJ parts `dump_obj` wrote for web/tests/scenes/characters/body.txt and checks the measured mesh
// against the standard landmark heights as fractions of stature (Drillis and Contini, as used in figure and ergonomics references) and typical adult female breadths.
//
//   ./target/release/examples/dump_obj web/tests/scenes/characters/body.txt /tmp/body_obj
//   bun engine/tools/audit-body.ts /tmp/body_obj [--height 1.75]
//
// Landmarks are FOUND in the mesh (narrowest or widest cross-section in a height window), never read from the scene's station tables.
import { existsSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const args = process.argv.slice(2);
const dir = args.find((a) => !a.startsWith('--'));
if (!dir) {
  console.error('usage: bun engine/tools/audit-body.ts <obj dir> [--height 1.75]');
  process.exit(2);
}
const hi = args.indexOf('--height');
const H = hi >= 0 ? Number(args[hi + 1]) : 1.75;

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
// Two layouts: the blockout's separate parts (body_torso, body_legs, body_arms, body_feet), or ONE skin mesh (body_skin, from poly.skin), which is split by region:
// legs are the left side below the hip, arms are the left side outside the torso, the torso is what is near the middle, the feet are the lowest 10 cm.
const single = existsSync(`${dir}/body_skin.obj`);
const skin = single ? readObj('body_skin') : [];
const torso = single ? skin.filter((p) => Math.abs(p[0]) < 0.215 && p[1] > 0.78) : readObj('body_torso');
const legs = single ? skin.filter((p) => p[0] > 0 && p[0] < 0.19 && p[1] < 0.86) : readObj('body_legs').filter((p) => p[0] > 0);
const arms = single ? skin.filter((p) => p[0] > 0.2 && p[1] > 0.6 && p[1] < 1.38) : readObj('body_arms').filter((p) => p[0] > 0);
const feet = single ? skin.filter((p) => p[1] < 0.1) : readObj('body_feet');
const all = single ? skin : [...torso, ...readObj('body_legs'), ...readObj('body_arms'), ...feet];

const slab = (pts: V[], y: number, tol = 0.004) => pts.filter((p) => Math.abs(p[1] - y) <= tol);
const span = (pts: V[], axis: 0 | 2) => (pts.length ? Math.max(...pts.map((p) => p[axis])) - Math.min(...pts.map((p) => p[axis])) : NaN);
const widthAt = (pts: V[], y: number) => span(slab(pts, y), 0);
// the height of the narrowest cross-section (area proxy: width x depth) between y0 and y1
function narrowest(pts: V[], y0: number, y1: number, byWidth = false): number {
  let best = Infinity;
  let at = NaN;
  for (let y = y0; y <= y1; y += 0.005) {
    const s = slab(pts, y);
    if (s.length < 4) continue;
    const a = byWidth ? span(s, 0) : span(s, 0) * span(s, 2);
    if (a < best) { best = a; at = y; }
  }
  return at;
}

// the crotch: the lowest point of the surface on the midline between the legs (the torso's own lowest point in the blockout)
const crotch = single ? Math.min(...skin.filter((p) => Math.abs(p[0]) < 0.01 && p[1] > 0.6 && p[1] < 1.0).map((p) => p[1])) : Math.min(...torso.map((p) => p[1]));
const shoulderTop = Math.max(...torso.filter((p) => Math.abs(p[0]) > 0.16).map((p) => p[1]));
const elbow = narrowest(arms, 1.0, 1.25);
const wrist = narrowest(arms, 0.76, 0.95, true);
const fingertip = Math.min(...arms.map((p) => p[1]));
const knee = narrowest(legs, 0.40, 0.62);
const neckTop = Math.max(...torso.map((p) => p[1]));
const floor = Math.min(...feet.map((p) => p[1]));
const waistY = narrowest(torso, 1.0, 1.25);
const hipY = (() => { let best = 0; let at = NaN; for (let y = 0.85; y <= 1.05; y += 0.005) { const w = widthAt(torso, y); if (w > best) { best = w; at = y; } } return at; })();
const bustY = (() => { let best = -Infinity; let at = NaN; for (let y = 1.2; y <= 1.36; y += 0.005) { const s = slab(torso, y); const f = Math.max(...s.map((p) => p[2])); if (f > best) { best = f; at = y; } } return at; })();
const bustDepth = span(slab(torso, bustY), 2);
const waistDepth = span(slab(torso, waistY), 2);

type Rule = { name: string; value: number; lo: number; hi: number; unit: string; hint: string };
const frac = (v: number) => v / H;
const rules: Rule[] = [
  { name: 'crotch height / stature (0.470)', value: frac(crotch), lo: 0.455, hi: 0.485, unit: '', hint: 'the first torso and leg station y (0.82 is 0.469 of 1.75)' },
  { name: 'waist (narrowest torso) / stature (navel 0.600)', value: frac(waistY), lo: 0.585, hi: 0.66, unit: '', hint: 'the torso waist station y' },
  { name: 'elbow / stature (0.630)', value: frac(elbow), lo: 0.605, hi: 0.655, unit: '', hint: 'the arm elbow station y' },
  { name: 'shoulder (acromion) / stature (0.818)', value: frac(shoulderTop), lo: 0.795, hi: 0.84, unit: '', hint: 'the torso shoulder station y' },
  { name: 'wrist / stature (0.485)', value: frac(wrist), lo: 0.465, hi: 0.505, unit: '', hint: 'the arm wrist station y' },
  { name: 'fingertip / stature (0.377)', value: frac(fingertip), lo: 0.36, hi: 0.395, unit: '', hint: 'the last arm station y' },
  { name: 'knee / stature (0.285)', value: frac(knee), lo: 0.27, hi: 0.30, unit: '', hint: 'the leg knee station y' },
  { name: 'neck top (chin line) / stature (0.870)', value: frac(neckTop), lo: 0.855, hi: 0.885, unit: '', hint: 'the last torso station y' },
  { name: 'feet on the floor', value: floor, lo: -0.004, hi: 0.004, unit: 'm', hint: 'foot y centre = its y radius' },
  { name: 'biacromial (shoulder) breadth', value: widthAt(torso, shoulderTop - 0.01), lo: 0.34, hi: 0.42, unit: 'm', hint: 'the shoulder station half width' },
  { name: 'hip breadth (widest)', value: widthAt(torso, hipY), lo: 0.33, hi: 0.40, unit: 'm', hint: 'the hip station half widths' },
  { name: 'waist breadth', value: widthAt(torso, waistY), lo: 0.23, hi: 0.30, unit: 'm', hint: 'the waist station half width' },
  { name: 'waist to hip breadth ratio', value: widthAt(torso, waistY) / widthAt(torso, hipY), lo: 0.68, hi: 0.82, unit: '', hint: 'waist or hip half widths' },
  { name: 'bust depth over waist depth', value: bustDepth / waistDepth, lo: 1.05, hi: 1.4, unit: '', hint: 'the bust station front z' },
  { name: 'thigh width at the top', value: widthAt(legs, 0.80), lo: 0.15, hi: 0.2, unit: 'm', hint: 'the first leg station half width' },
  { name: 'thigh gap at the crotch (between the legs)', value: 2 * Math.min(...legs.filter((p) => p[1] > 0.75 && p[1] < 0.8).map((p) => p[0])), lo: 0.0, hi: 0.05, unit: 'm', hint: 'leg centre x or width at the top' },
  { name: 'ankle width', value: widthAt(legs, 0.10), lo: 0.055, hi: 0.075, unit: 'm', hint: 'the leg ankle station half width' },
  { name: 'upper arm width', value: widthAt(arms, 1.25), lo: 0.075, hi: 0.1, unit: 'm', hint: 'the arm 1.25 station half width' },
  { name: 'hand length (wrist to fingertip)', value: wrist - fingertip, lo: 0.17, hi: 0.21, unit: 'm', hint: 'the arm stations below the wrist' },
  { name: 'arm span half (shoulder joint to fingertip x)', value: Math.max(...arms.map((p) => p[0])), lo: 0.24, hi: 0.3, unit: 'm', hint: 'the arm centre x stations' },
  { name: 'left-right symmetry (whole body)', value: Math.abs(Math.max(...all.map((p) => p[0])) + Math.min(...all.map((p) => p[0]))), lo: 0, hi: 0.003, unit: 'm', hint: 'the right side is not the mirror of the left' },
  { name: 'total stature, floor to chin line', value: neckTop - floor, lo: 1.5, hi: 1.55, unit: 'm', hint: 'matches head.txt chin 1.5202' },
];
let failed = 0;
console.log(`landmarks (m): floor ${floor.toFixed(3)}, knee ${knee.toFixed(3)}, crotch ${crotch.toFixed(3)}, hip ${hipY.toFixed(3)}, waist ${waistY.toFixed(3)}, bust ${bustY.toFixed(3)}, elbow ${elbow.toFixed(3)}, wrist ${wrist.toFixed(3)}, fingertip ${fingertip.toFixed(3)}, shoulder ${shoulderTop.toFixed(3)}, neck top ${neckTop.toFixed(3)}`);
for (const r of rules) {
  const ok = r.value >= r.lo && r.value <= r.hi;
  if (!ok) failed++;
  const shown = r.unit === '' ? r.value.toFixed(3) : r.value.toFixed(3);
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(52)} ${shown.padStart(8)}${r.unit}  target ${r.lo}-${r.hi}${r.unit}${ok ? '' : `  ${r.value < r.lo ? 'too low' : 'too high'}: change ${r.hint}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${rules.length} rules fail`);
process.exit(failed === 0 ? 0 : 1);
