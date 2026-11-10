// A generic mesh audit for any scene, read from the OBJ parts `dump_obj` writes. It catches the faults a model cannot see in a small picture:
// NaN or huge coordinates, zero-area faces, parts that float apart from everything else, and coplanar faces shared by two different parts (z-fighting in the render).
// It also reports open boundary edges per part (info: feathers and leaves are meant to be open) and the triangle count.
//
//   ./target/release/examples/dump_obj web/tests/scenes/creatures/bird.txt /tmp/bird_obj
//   bun engine/tools/audit-mesh.ts /tmp/bird_obj [--tol 0.001] [--float 0.01]
// Exit code 1 when a hard rule fails, so it can sit in a loop or a gate.
import { readdirSync, readFileSync } from 'node:fs';

type V = [number, number, number];
const args = process.argv.slice(2);
const dir = args[0];
if (!dir) {
  console.error('usage: bun engine/tools/audit-mesh.ts <obj dir> [--tol 0.001] [--float 0.01]');
  process.exit(2);
}
const opt = (name: string, fallback: number) => { const i = args.indexOf(`--${name}`); return i >= 0 ? Number(args[i + 1]) : fallback; };
const tol = opt('tol', 0.001); // metres: grid for matching coplanar faces and welding vertices
const floatFraction = opt('float', 0.01); // a part further than this fraction of the whole diagonal from every other part floats

type Part = { name: string; verts: V[]; faces: number[][]; tris: number; zeroArea: number; bad: number; boundary: number; nonManifold: number; lo: V; hi: V };
const parts: Part[] = [];
for (const file of readdirSync(dir).filter((n) => n.endsWith('.obj')).sort()) {
  const verts: V[] = [];
  const faces: number[][] = [];
  for (const line of readFileSync(`${dir}/${file}`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) { const [, x, y, z] = line.trim().split(/\s+/); verts.push([Number(x), Number(y), Number(z)]); }
    else if (line.startsWith('f ')) faces.push(line.trim().split(/\s+/).slice(1).map((t) => Number(t.split('/')[0]) - 1));
  }
  const lo: V = [Infinity, Infinity, Infinity], hi: V = [-Infinity, -Infinity, -Infinity];
  let bad = 0;
  for (const v of verts) for (let i = 0; i < 3; i++) { if (!Number.isFinite(v[i]) || Math.abs(v[i]) > 1e5) bad++; lo[i] = Math.min(lo[i], v[i]); hi[i] = Math.max(hi[i], v[i]); }
  // weld vertices on the tolerance grid so edges match across split vertices
  const weld = new Map<string, number>();
  const id = verts.map((v) => { const k = v.map((c) => Math.round(c / (tol * 0.1))).join(','); if (!weld.has(k)) weld.set(k, weld.size); return weld.get(k)!; });
  const edges = new Map<string, number>();
  let zeroArea = 0, tris = 0;
  for (const f of faces) {
    tris += Math.max(0, f.length - 2);
    for (let i = 0; i < f.length; i++) { const a = id[f[i]], b = id[f[(i + 1) % f.length]]; if (a === b) continue; const k = a < b ? `${a}_${b}` : `${b}_${a}`; edges.set(k, (edges.get(k) ?? 0) + 1); }
    for (let i = 1; i + 1 < f.length; i++) {
      const [p, q, r] = [verts[f[0]], verts[f[i]], verts[f[i + 1]]];
      const u: V = [q[0] - p[0], q[1] - p[1], q[2] - p[2]], w: V = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
      const area = Math.hypot(u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]) / 2;
      if (!(area > 1e-10)) zeroArea++;
    }
  }
  let boundary = 0, nonManifold = 0;
  for (const n of edges.values()) { if (n === 1) boundary++; else if (n > 2) nonManifold++; }
  parts.push({ name: file.replace(/\.obj$/, ''), verts, faces, tris, zeroArea, bad, boundary, nonManifold, lo, hi });
}

// whole-scene bounds and the gap between two boxes
const lo: V = [Infinity, Infinity, Infinity], hi: V = [-Infinity, -Infinity, -Infinity];
for (const p of parts) if (p.verts.length) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p.lo[i]); hi[i] = Math.max(hi[i], p.hi[i]); }
const diag = Math.hypot(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
const gap = (a: Part, b: Part) => Math.hypot(...([0, 1, 2].map((i) => Math.max(0, a.lo[i] - b.hi[i], b.lo[i] - a.hi[i])) as V));

let failed = 0;
const fail = (msg: string) => { failed++; console.log(`FAIL  ${msg}`); };
console.log(`${parts.length} parts, ${parts.reduce((s, p) => s + p.tris, 0)} triangles, scene ${(hi[0] - lo[0]).toFixed(2)} x ${(hi[1] - lo[1]).toFixed(2)} x ${(hi[2] - lo[2]).toFixed(2)} m\n`);
console.log('part'.padEnd(22) + 'tris'.padStart(8) + 'open edges'.padStart(12) + 'non-manifold'.padStart(14) + '  notes');
for (const p of parts) {
  const notes: string[] = [];
  if (p.bad) { notes.push(`${p.bad} NaN/huge coordinates`); fail(`${p.name}: ${p.bad} NaN or huge coordinates`); }
  if (p.zeroArea) { notes.push(`${p.zeroArea} zero-area faces`); fail(`${p.name}: ${p.zeroArea} zero-area faces (collapsed geometry)`); }
  if (p.faces.length === 0) { notes.push('empty'); fail(`${p.name}: no faces`); }
  console.log(p.name.padEnd(22) + String(p.tris).padStart(8) + String(p.boundary).padStart(12) + String(p.nonManifold).padStart(14) + `  ${notes.join('; ')}`);
}

// floating parts
if (parts.length > 1) {
  for (const p of parts) {
    const nearest = Math.min(...parts.filter((q) => q !== p && q.verts.length).map((q) => gap(p, q)));
    if (nearest > floatFraction * diag) fail(`${p.name} floats: its box is ${nearest.toFixed(3)} m from every other part (more than ${(floatFraction * diag).toFixed(3)} m)`);
  }
}

// coplanar faces of two different parts (z-fighting): centroids on the same grid cell with parallel normals
const cell = new Map<string, { part: string; n: V }[]>();
const zfight = new Map<string, number>();
for (const p of parts) {
  for (const f of p.faces) {
    if (f.length < 3) continue;
    const vs = f.map((i) => p.verts[i]);
    const c: V = [0, 0, 0];
    for (const v of vs) for (let i = 0; i < 3; i++) c[i] += v[i] / vs.length;
    const u: V = [vs[1][0] - vs[0][0], vs[1][1] - vs[0][1], vs[1][2] - vs[0][2]], w: V = [vs[2][0] - vs[0][0], vs[2][1] - vs[0][1], vs[2][2] - vs[0][2]];
    const n: V = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const l = Math.hypot(...n) || 1;
    n[0] /= l; n[1] /= l; n[2] /= l;
    const key = c.map((x) => Math.round(x / tol)).join(',');
    const seen = cell.get(key) ?? [];
    for (const o of seen) {
      if (o.part === p.name) continue;
      if (Math.abs(o.n[0] * n[0] + o.n[1] * n[1] + o.n[2] * n[2]) > 0.999) { const k = [o.part, p.name].sort().join(' + '); zfight.set(k, (zfight.get(k) ?? 0) + 1); }
    }
    seen.push({ part: p.name, n });
    cell.set(key, seen);
  }
}
for (const [pair, n] of zfight) fail(`z-fighting: ${n} coplanar face${n > 1 ? 's' : ''} shared by ${pair} (move one part off the other by more than ${tol} m)`);

console.log(failed === 0 ? '\nALL CHECKS PASS' : `\n${failed} problem${failed > 1 ? 's' : ''}`);
process.exit(failed === 0 ? 0 : 1);
