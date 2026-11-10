// A generic reference-dimension audit: a JSON rules file says what a real thing measures, this checks the evaluated parts against it (the tractor audit, made reusable).
//
//   ./target/release/examples/dump_obj web/tests/scenes/creatures/bird.txt /tmp/bird_obj
//   bun engine/tools/audit-dims.ts /tmp/bird_obj web/tests/scenes/creatures/bird.dims.json
//
// Rules file: { "title": "...", "rules": [ { "name": "wingspan", "parts": ["wing_*"], "measure": "size.x", "lo": 1.9, "hi": 2.3, "hint": "what to change" },
//   { "name": "beak / head+beak", "parts": ["beak"], "measure": "size.z", "per": { "parts": ["head", "beak"], "measure": "size.z" }, "lo": 0.3, "hi": 0.5 } ] }
// parts: part names, `*` matches any run of characters. measure: size.x|y|z, lo.x|y|z, hi.x|y|z, mid.x|y|z, tris, or `max.x` (largest |coordinate|).
// "per" divides by a second measurement, so proportions (the thing that makes a head look wrong) can be bounded as well as sizes.
import { readdirSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const [dir, rulesFile] = process.argv.slice(2);
if (!dir || !rulesFile) {
  console.error('usage: bun engine/tools/audit-dims.ts <obj dir> <rules.json>');
  process.exit(2);
}
const names = readdirSync(dir).filter((n) => n.endsWith('.obj')).map((n) => n.replace(/\.obj$/, ''));
const cache = new Map<string, { verts: V[]; tris: number }>();
function load(name: string) {
  let part = cache.get(name);
  if (!part) {
    const verts: V[] = [];
    let tris = 0;
    for (const line of readFileSync(`${dir}/${name}.obj`, 'utf8').split('\n')) {
      if (line.startsWith('v ')) { const [, x, y, z] = line.trim().split(/\s+/); verts.push([Number(x), Number(y), Number(z)]); }
      else if (line.startsWith('f ')) tris += line.trim().split(/\s+/).length - 3;
    }
    part = { verts, tris };
    cache.set(name, part);
  }
  return part;
}
const match = (pattern: string) => {
  const re = new RegExp(`^${pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*')}$`);
  return names.filter((n) => re.test(n));
};
type Measure = { parts: string[]; measure: string };
function measure(m: Measure): number {
  const selected = [...new Set(m.parts.flatMap(match))];
  if (selected.length === 0) return NaN;
  if (m.measure === 'tris') return selected.reduce((s, n) => s + load(n).tris, 0);
  const [kind, axisName] = m.measure.split('.');
  const axis = { x: 0, y: 1, z: 2 }[axisName as 'x' | 'y' | 'z'];
  let lo = Infinity, hi = -Infinity, big = 0;
  for (const n of selected) for (const v of load(n).verts) { lo = Math.min(lo, v[axis]); hi = Math.max(hi, v[axis]); big = Math.max(big, Math.abs(v[axis])); }
  return { size: hi - lo, lo, hi, mid: (lo + hi) / 2, max: big }[kind as 'size' | 'lo' | 'hi' | 'mid' | 'max'] ?? NaN;
}

const spec = JSON.parse(readFileSync(rulesFile, 'utf8')) as { title?: string; rules: (Measure & { name: string; lo: number; hi: number; hint?: string; per?: Measure })[] };
console.log(spec.title ?? rulesFile);
let failed = 0;
for (const r of spec.rules) {
  let value = measure(r);
  if (r.per) value /= measure(r.per);
  const ok = Number.isFinite(value) && value >= r.lo && value <= r.hi;
  if (!ok) failed++;
  const missing = Number.isNaN(measure(r)) ? '  (no part matches)' : '';
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(44)} ${value.toFixed(3).padStart(8)}  target ${r.lo}-${r.hi}${ok ? '' : `  ${value < r.lo ? 'too low' : 'too high'}${r.hint ? `: change ${r.hint}` : ''}${missing}`}`);
}
console.log(failed === 0 ? 'ALL RULES PASS' : `${failed} of ${spec.rules.length} rules fail`);
process.exit(failed === 0 ? 0 : 1);
