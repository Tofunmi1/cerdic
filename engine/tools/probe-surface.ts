// Ray-casts against the OBJ parts `dump_obj` wrote, to find where a surface really is before something is attached to it.
//
//   bun engine/tools/probe-surface.ts <obj dir> <parts, comma separated> <ox,oy,oz,dx,dy,dz> [more rays ...]
//   bun engine/tools/probe-surface.ts /tmp/car_obj body,cabin_paint 0,0.55,5,0,0,-1     # where is the front face at x=0, y=0.55
//
// Prints the nearest hit point (x y z) and the surface normal for each ray, or "miss".
import { readFileSync, existsSync } from 'node:fs';

const [dir, partList, ...rays] = process.argv.slice(2);
if (!dir || !partList || rays.length === 0) {
  console.error('usage: bun engine/tools/probe-surface.ts <obj dir> <part,part> <ox,oy,oz,dx,dy,dz> ...');
  process.exit(2);
}

type V = [number, number, number];
const triangles: [V, V, V][] = [];
for (const name of partList.split(',')) {
  const path = `${dir}/${name}.obj`;
  if (!existsSync(path)) throw new Error(`missing part ${name}`);
  const points: V[] = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      points.push([Number(x), Number(y), Number(z)]);
    } else if (line.startsWith('f ')) {
      const [a, b, c] = line.trim().split(/\s+/).slice(1).map((i) => Number(i) - 1);
      triangles.push([points[a], points[b], points[c]]);
    }
  }
}

const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a: V, b: V): V => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const dot = (a: V, b: V): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

for (const spec of rays) {
  const [ox, oy, oz, dx, dy, dz] = spec.split(',').map(Number);
  const o: V = [ox, oy, oz];
  const d: V = [dx, dy, dz];
  let best = Infinity;
  let normal: V = [0, 0, 0];
  for (const [a, b, c] of triangles) {
    const e1 = sub(b, a);
    const e2 = sub(c, a);
    const p = cross(d, e2);
    const det = dot(e1, p);
    if (Math.abs(det) < 1e-12) continue;
    const s = sub(o, a);
    const u = dot(s, p) / det;
    if (u < 0 || u > 1) continue;
    const q = cross(s, e1);
    const v = dot(d, q) / det;
    if (v < 0 || u + v > 1) continue;
    const t = dot(e2, q) / det;
    if (t > 1e-9 && t < best) {
      best = t;
      normal = cross(e1, e2);
    }
  }
  if (best === Infinity) console.log(`${spec}  miss`);
  else {
    const len = Math.hypot(...normal) || 1;
    console.log(`${spec}  hit ${(ox + dx * best).toFixed(4)} ${(oy + dy * best).toFixed(4)} ${(oz + dz * best).toFixed(4)}  n ${normal.map((v) => (v / len).toFixed(2)).join(' ')}`);
  }
}
