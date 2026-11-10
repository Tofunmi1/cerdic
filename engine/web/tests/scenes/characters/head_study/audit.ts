// Proportion audit for a human head: reads the OBJ files `dump_obj` wrote for a character scene, finds the landmarks in the MESH (never from the
// scene's own parameters), and checks them against the proportion rules artists and craniofacial anthropometry use.
//
//   cargo run -p ve-modeling --example dump_obj -- web/tests/scenes/characters/head_study/world.txt /tmp/head_obj
//   bun web/tests/scenes/characters/head_study/audit.ts <obj dir> [--height <metres>]
//
// Parts it reads: head.obj (the skin: head, neck), eye_l.obj and eye_r.obj (the eyeballs). Rules are targets with tolerances, not law: Farkas found most
// real faces deviate from the canons, so a FAIL says "look at this", and each line names the parameter in the scene's table to change.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';

type V = [number, number, number];

function readObj(path: string): V[] {
  const points: V[] = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      points.push([Number(x), Number(y), Number(z)]);
    }
  }
  return points;
}

const args = process.argv.slice(2);
const dir = args.find((a) => !a.startsWith('--'));
if (!dir) {
  console.error('usage: bun web/tests/scenes/characters/head_study/audit.ts <obj dir> [--height <metres>]');
  process.exit(2);
}
type SpecTarget = { id: string; min: number; max: number; unit: string; hint: string };
const spec = JSON.parse(readFileSync(new URL('./spec.json', import.meta.url), 'utf8')) as { body_height: number; targets: SpecTarget[] };
const heightArg = args.indexOf('--height');
const bodyHeight = heightArg >= 0 ? Number(args[heightArg + 1]) : spec.body_height;

const head = readObj(`${dir}/head.obj`);
const eyeL = readObj(`${dir}/eye_l.obj`);
const eyeR = readObj(`${dir}/eye_r.obj`);
const centre = (pts: V[]): V => {
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (const p of pts) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
};

// ---- landmarks ----
const crown = Math.max(...head.map((p) => p[1]));
const midline = head.filter((p) => Math.abs(p[0]) < 0.0015 && p[2] > 0).map((p) => [p[1], p[2]] as [number, number]).sort((a, b) => a[0] - b[0]);
// the chin (menton): the lowest midline point whose front is already well forward of the neck
const chin = midline.find((p) => p[1] > 0.082)?.[0] ?? NaN;
const cl = centre(eyeL);
const cr = centre(eyeR);
const eyeY = (cl[1] + cr[1]) / 2;
const pd = Math.abs(cl[0] - cr[0]);
const eyeSymmetry = Math.hypot(cl[0] + cr[0], cl[1] - cr[1], cl[2] - cr[2]);
const measuredMeshes = readdirSync(dir).filter((name) => /^(head|eye_[lr])(?:__\d+)?\.obj$/.test(name));
const triangles = measuredMeshes.reduce((sum, name) => sum + readFileSync(`${dir}/${name}`, 'utf8').split('\n').filter((line) => line.startsWith('f ')).length, 0);
const socketFront = (eye: V) => Math.max(...head.filter((p) => Math.abs(p[0] - eye[0]) < 0.012 && Math.abs(p[1] - eye[1]) < 0.012).map((p) => p[2]));
const eyeFront = (side: 'l' | 'r') => Math.max(...measuredMeshes.filter((name) => name.startsWith(`eye_${side}`)).flatMap((name) => readObj(`${dir}/${name}`)).map((p) => p[2]));
const eyeProtrusion = Math.max(eyeFront('l') - socketFront(cl), eyeFront('r') - socketFront(cr)) * 1000;

// the front profile above the chin, smoothed, walked from the nose tip down through its extrema: tip, subnasale, upper lip, mouth line, lower lip, sulcus, chin
const smoothed = midline.map((p, i) => {
  const near = midline.slice(Math.max(0, i - 1), i + 2);
  return [p[0], near.reduce((s, q) => s + q[1], 0) / near.length] as [number, number];
});
function refinedExtremum(points: [number, number][], i: number): [number, number] {
  const [x, f] = points[i];
  const [xl, fl] = points[i - 1];
  const [xr, fr] = points[i + 1];
  const left = xl - x, right = xr - x;
  const slopeLeft = (fl - f) / left, slopeRight = (fr - f) / right;
  const a = (slopeRight - slopeLeft) / (right - left);
  const b = slopeRight - a * right;
  const offset = -b / (2 * a);
  if (!Number.isFinite(offset) || offset < left || offset > right) return [x, f];
  return [x + offset, f + b * offset + a * offset * offset];
}
const noseRegion = smoothed.filter((p) => p[0] > chin + 0.03 && p[0] < eyeY - 0.005);
const tipIndex = noseRegion.reduce((best, p, i) => p[1] > noseRegion[best][1] ? i : best, 0);
const tip = tipIndex > 0 && tipIndex < noseRegion.length - 1 ? refinedExtremum(noseRegion, tipIndex) : [NaN, NaN];
const down = smoothed.filter((p) => p[0] <= tip[0] && p[0] >= chin).reverse();
const extrema: { kind: 'max' | 'min'; y: number; z: number }[] = [];
for (let i = 1; i < down.length - 1; i++) {
  const [, a] = down[i - 1];
  const [y, b] = down[i];
  const [, c] = down[i + 1];
  if (b > a && b >= c) {
    const [refinedY, refinedZ] = refinedExtremum(down, i);
    extrema.push({ kind: 'max', y: refinedY, z: refinedZ });
  } else if (b < a && b <= c) {
    const [refinedY, refinedZ] = refinedExtremum(down, i);
    extrema.push({ kind: 'min', y: refinedY, z: refinedZ });
  }
}
// the alternation after the tip: min (subnasale), max (upper lip), min (mouth line), max (lower lip), min (sulcus), max (chin)
const names = ['subnasale', 'upper lip', 'mouth line', 'lower lip', 'sulcus', 'chin point'];
const found: Record<string, number> = {};
const foundDepth: Record<string, number> = {};
let expect: 'min' | 'max' = 'min';
let idx = 0;
for (const e of extrema) {
  if (e.kind === expect && idx < names.length) {
    found[names[idx++]] = e.y;
    foundDepth[names[idx - 1]] = e.z;
    expect = expect === 'min' ? 'max' : 'min';
  }
}
const nosebase = found['subnasale'] ?? NaN;
const mouth = found['mouth line'] ?? NaN;
const subnasaleZ = foundDepth['subnasale'] ?? NaN;

// widths
const band = (lo: number, hi: number) => head.filter((p) => p[1] >= lo && p[1] <= hi);
const breadth = 2 * Math.max(...band(eyeY + 0.045, eyeY + 0.07).map((p) => Math.abs(p[0])));
const foreheadFront = Math.max(...band(eyeY + 0.01, eyeY + 0.06).map((p) => p[2]));
const backMost = Math.min(...band(eyeY + 0.01, eyeY + 0.06).map((p) => p[2]));
const length = foreheadFront - backMost;
// ---- rules ----
type Rule = { id: string; name: string; value: number };
const headHeight = crown - chin;
const rules: Rule[] = [
  { id: 'head-height-in-heads', name: 'head height in heads (body ' + bodyHeight.toFixed(2) + ' m)', value: bodyHeight / headHeight },
  { id: 'eye-line-from-crown', name: 'eye line: fraction of head from crown down', value: (crown - eyeY) / headHeight },
  { id: 'nose-base-eye-to-chin', name: 'nose base: fraction of eye-to-chin', value: (eyeY - nosebase) / (eyeY - chin) },
  { id: 'mouth-line-nose-to-chin', name: 'mouth line: fraction of nose-base-to-chin', value: (nosebase - mouth) / (nosebase - chin) },
  { id: 'pupillary-distance', name: 'pupillary distance', value: pd * 1000 },
  { id: 'eye-centre-mismatch', name: 'left/right eye centre mismatch', value: eyeSymmetry * 1000 },
  { id: 'eye-protrusion', name: 'eye projection beyond socket surface', value: eyeProtrusion },
  { id: 'head-breadth', name: 'head breadth above the ears', value: breadth * 1000 },
  { id: 'head-length', name: 'head length (brow to back)', value: length * 1000 },
  { id: 'cephalic-index', name: 'cephalic index (breadth / length)', value: breadth / length },
  { id: 'nose-projection', name: 'nose projection (tip beyond subnasale)', value: (tip[1] - subnasaleZ) * 1000 },
  { id: 'head-and-eye-triangles', name: 'head and eye triangles', value: triangles },
];

const targets = new Map(spec.targets.map((target) => [target.id, target]));
if (targets.size !== spec.targets.length || rules.some((rule) => !targets.has(rule.id)) || targets.size !== rules.length) {
  throw new Error('spec targets must uniquely match every audit rule');
}

console.log(`landmarks (m): crown ${crown.toFixed(4)}, eye line ${eyeY.toFixed(4)}, nose tip ${tip[0].toFixed(4)}, ${names.map((n) => `${n} ${found[n]?.toFixed(4) ?? '?'}`).join(', ')}, chin ${chin.toFixed(4)}`);
let failed = 0;
const results: { id: string; name: string; value: number; min: number; max: number; unit: string; hint: string; passed: boolean }[] = [];
for (const r of rules) {
  const target = targets.get(r.id)!;
  const ok = r.value >= target.min && r.value <= target.max;
  if (!ok) failed++;
  results.push({ id: r.id, name: r.name, value: r.value, min: target.min, max: target.max, unit: target.unit, hint: target.hint, passed: ok });
  const shown = target.unit === 'mm' ? r.value.toFixed(1) : target.unit === 'triangles' ? r.value.toFixed(0) : r.value.toFixed(3);
  const unit = target.unit === 'ratio' ? '' : target.unit === 'triangles' ? ' tris' : target.unit;
  const direction = r.value < target.min ? 'too low' : 'too high';
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(46)} ${shown.padStart(7)}${unit}  target ${target.min}-${target.max}${unit}${ok ? '' : `  ${direction}: change ${target.hint}`}`);
}
console.log(failed === 0 ? 'ALL MEASURED RULES PASS' : `${failed} of ${rules.length} rules fail`);
// Not measured yet: they need the skull WITHOUT the landmarks to subtract (a reference part), because the nose edge and the lip corners cannot be told from
// the skull's own curvature. Until then they are judged by eye.
const notMeasured = ['nose width', 'mouth width', 'eye opening width', 'ear height', 'expression deformation', 'UV and texture budgets', 'game export round trip'];
console.log(`NOT MEASURED YET: ${notMeasured.join(', ')}`);
const jsonOutIndex = args.indexOf('--json-out');
if (jsonOutIndex >= 0) {
  const path = args[jsonOutIndex + 1];
  if (!path) throw new Error('--json-out requires a path');
  writeFileSync(path, `${JSON.stringify({ scene: spec.scene, bodyHeight, passed: failed === 0, measuredRuleCount: results.length, failedRuleCount: failed, landmarks: { crown, eyeLine: eyeY, noseTip: tip[0], subnasale: nosebase, mouthLine: mouth, chin }, rules: results, notMeasured }, null, 2)}\n`);
}
process.exit(failed === 0 ? 0 : 1);
