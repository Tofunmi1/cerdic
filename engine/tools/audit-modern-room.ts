// Geometry audit for the modern-room scene. Usage:
//   (cd engine && target/release/examples/dump_obj web/demos/modern_room.txt /tmp/modern-room)
//   bun engine/tools/audit-modern-room.ts /tmp/modern-room
import { existsSync, readFileSync, readdirSync } from 'node:fs';

type V = [number, number, number];
const dir = process.argv[2];
if (!dir || !existsSync(dir)) {
  console.error('usage: bun engine/tools/audit-modern-room.ts <dump_obj output directory>');
  process.exit(2);
}
function readObj(name: string): { vertices: V[]; triangles: number } {
  const path = `${dir}/${name}.obj`;
  if (!existsSync(path)) return { vertices: [], triangles: 0 };
  const vertices: V[] = [];
  let triangles = 0;
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (line.startsWith('v ')) {
      const [, x, y, z] = line.trim().split(/\s+/);
      vertices.push([Number(x), Number(y), Number(z)]);
    } else if (line.startsWith('f ')) triangles++;
  }
  return { vertices, triangles };
}
const size = (name: string, axis: number) => {
  const { vertices } = readObj(name);
  if (!vertices.length) return 0;
  return Math.max(...vertices.map(p => p[axis])) - Math.min(...vertices.map(p => p[axis]));
};
const files = readdirSync(dir).filter(f => f.endsWith('.obj'));
const triangles = files.reduce((n, f) => n + readObj(f.slice(0, -4)).triangles, 0);
const rules: { id: string; value: number; min: number; max: number; unit: string; hint: string }[] = [
  { id: 'room-width', value: size('floor', 0), min: 9.9, max: 10.1, unit: 'm', hint: 'floor_src.size.x' },
  { id: 'room-depth', value: size('floor', 2), min: 7.9, max: 8.1, unit: 'm', hint: 'floor_src.size.z' },
  { id: 'picture-window-width', value: size('picture_glass', 2), min: 6.3, max: 6.5, unit: 'm', hint: 'glass_src.size.x after it is turned onto the side wall' },
  { id: 'sofa-width', value: size('sofa_seats', 0), min: 3.0, max: 3.2, unit: 'm', hint: 'seat spacing and seat_src.size.x' },
  { id: 'named-outputs', value: files.length, min: 60, max: Infinity, unit: '', hint: 'retain separate named room, furniture and decor outputs' },
  { id: 'mesh-detail', value: triangles, min: 40000, max: Infinity, unit: ' triangles', hint: 'retain rounded furniture and detailed foliage meshes' },
];
console.log(`measured modern room: ${files.length} mesh outputs, ${triangles} triangles`);
let failed = 0;
for (const r of rules) {
  const ok = r.value >= r.min && r.value <= r.max;
  if (!ok) failed++;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${r.id.padEnd(24)} ${r.value.toFixed(r.unit === 'm' ? 2 : 0)}${r.unit}  target ${r.min}-${r.max}${r.unit}${ok ? '' : `  change ${r.hint}`}`);
}
console.log(failed ? `${failed} of ${rules.length} rules fail` : 'ALL RULES PASS');
process.exit(failed ? 1 : 0);
