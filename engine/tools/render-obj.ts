// A fast, high-resolution look at the natively evaluated parts, for the model's edit-and-look loop (no browser, no 512 px limit, any angle, any zoom).
//
//   ./target/release/examples/dump_obj web/tests/scenes/creatures/bird.txt /tmp/bird_obj
//   bun engine/tools/render-obj.ts /tmp/bird_obj /tmp/bird_top.png --az 0 --el 90
//   bun engine/tools/render-obj.ts /tmp/bird_obj /tmp/bird_head.png --az 90 --el 10 --center 0,0.09,0.4 --extent 0.3
//
// Options: --az degrees around +y (0 looks from +z toward -z, 90 from +x), --el degrees above the horizon (90 is straight down), --size pixels (default 1100),
// --center x,y,z and --extent metres (the width of the view; default: fit everything), --only name,name (render just those parts), --skip name,name.
// Orthographic, flat shaded, two-sided, a z-buffer; every part gets its own colour from a fixed palette and the legend is printed.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { PNG } from '../web/node_modules/pngjs/lib/png.js';

type V = [number, number, number];
const args = process.argv.slice(2);
const dir = args[0];
const out = args[1];
if (!dir || !out) {
  console.error('usage: bun engine/tools/render-obj.ts <obj dir> <out.png> [--az 30] [--el 25] [--size 1100] [--center x,y,z] [--extent m] [--only a,b] [--skip a,b]');
  process.exit(2);
}
const opt = (name: string, fallback: string) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback; };
const az = (Number(opt('az', '30')) * Math.PI) / 180;
const el = (Number(opt('el', '25')) * Math.PI) / 180;
const size = Number(opt('size', '1100'));
const only = opt('only', '').split(',').filter(Boolean);
const skip = opt('skip', '').split(',').filter(Boolean);

const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V, b: V) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V, b: V): V => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V): V => { const l = Math.hypot(...a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };

// the camera: looks at the origin of the view from direction d
const d: V = [Math.sin(az) * Math.cos(el), Math.sin(el), Math.cos(az) * Math.cos(el)];
const f: V = [-d[0], -d[1], -d[2]];
const up0: V = Math.abs(d[1]) > 0.999 ? [0, 0, 1] : [0, 1, 0];
const right = norm(cross(f, up0));
const up = cross(right, f);

const palette: V[] = [[0.85, 0.35, 0.3], [0.3, 0.6, 0.85], [0.35, 0.75, 0.4], [0.9, 0.75, 0.25], [0.7, 0.45, 0.85], [0.3, 0.8, 0.8], [0.9, 0.55, 0.3], [0.6, 0.6, 0.65]];
const parts: { name: string; color: V; tris: [V, V, V][] }[] = [];
for (const file of readdirSync(dir).filter((n) => n.endsWith('.obj')).sort()) {
  const name = file.replace(/\.obj$/, '');
  if (only.length && !only.includes(name)) continue;
  if (skip.includes(name)) continue;
  const verts: V[] = [];
  const tris: [V, V, V][] = [];
  for (const line of readFileSync(`${dir}/${file}`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) { const [, x, y, z] = line.trim().split(/\s+/); verts.push([Number(x), Number(y), Number(z)]); }
    else if (line.startsWith('f ')) {
      const ids = line.trim().split(/\s+/).slice(1).map((t) => Number(t.split('/')[0]) - 1);
      for (let i = 1; i + 1 < ids.length; i++) tris.push([verts[ids[0]], verts[ids[i]], verts[ids[i + 1]]]);
    }
  }
  parts.push({ name, color: palette[parts.length % palette.length], tris });
}

const proj = (p: V): V => [dot(p, right), dot(p, up), dot(p, d)];
let lo = [Infinity, Infinity, Infinity];
let hi = [-Infinity, -Infinity, -Infinity];
for (const part of parts) for (const t of part.tris) for (const p of t) { const q = proj(p); for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], q[i]); hi[i] = Math.max(hi[i], q[i]); } }
const centerArg = opt('center', '');
let cx = (lo[0] + hi[0]) / 2;
let cy = (lo[1] + hi[1]) / 2;
let width = (hi[0] - lo[0]) * 1.08;
let height = (hi[1] - lo[1]) * 1.08;
if (centerArg) { const c = centerArg.split(',').map(Number) as V; const q = proj(c); cx = q[0]; cy = q[1]; width = Number(opt('extent', '1')); height = width; }
const scale = size / Math.max(width, height * (centerArg ? 1 : 1));
const W = size;
const H = centerArg ? size : Math.max(64, Math.round(height * scale));
const img = new PNG({ width: W, height: H });
const depth = new Float32Array(W * H).fill(-Infinity);
const bg: V = [0.93, 0.94, 0.96];
for (let i = 0; i < W * H; i++) { img.data[i * 4] = bg[0] * 255; img.data[i * 4 + 1] = bg[1] * 255; img.data[i * 4 + 2] = bg[2] * 255; img.data[i * 4 + 3] = 255; }
const light = norm([-0.4, 0.8, 0.5]);
// a light that follows the camera so every angle is lit: key from up-left of the viewer, plus ambient
const key = norm([-right[0] * 0.5 + up[0] * 0.7 + d[0] * 0.6, -right[1] * 0.5 + up[1] * 0.7 + d[1] * 0.6, -right[2] * 0.5 + up[2] * 0.7 + d[2] * 0.6]);
void light;
for (const part of parts) {
  for (const t of part.tris) {
    const n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    const lam = Math.abs(dot(n, key));
    const shade = 0.3 + 0.7 * lam;
    const col = part.color.map((c) => Math.min(255, c * shade * 255));
    const s = t.map((p) => { const q = proj(p); return [(q[0] - cx) * scale + W / 2, H / 2 - (q[1] - cy) * scale, q[2]]; });
    const minX = Math.max(0, Math.floor(Math.min(s[0][0], s[1][0], s[2][0])));
    const maxX = Math.min(W - 1, Math.ceil(Math.max(s[0][0], s[1][0], s[2][0])));
    const minY = Math.max(0, Math.floor(Math.min(s[0][1], s[1][1], s[2][1])));
    const maxY = Math.min(H - 1, Math.ceil(Math.max(s[0][1], s[1][1], s[2][1])));
    const den = (s[1][1] - s[2][1]) * (s[0][0] - s[2][0]) + (s[2][0] - s[1][0]) * (s[0][1] - s[2][1]);
    if (Math.abs(den) < 1e-9) continue;
    for (let y = minY; y <= maxY; y++) {
      for (let x = minX; x <= maxX; x++) {
        const px = x + 0.5, py = y + 0.5;
        const a = ((s[1][1] - s[2][1]) * (px - s[2][0]) + (s[2][0] - s[1][0]) * (py - s[2][1])) / den;
        const b = ((s[2][1] - s[0][1]) * (px - s[2][0]) + (s[0][0] - s[2][0]) * (py - s[2][1])) / den;
        const c = 1 - a - b;
        if (a < -0.001 || b < -0.001 || c < -0.001) continue;
        const z = a * s[0][2] + b * s[1][2] + c * s[2][2];
        const k = y * W + x;
        if (z <= depth[k]) continue;
        depth[k] = z;
        img.data[k * 4] = col[0]; img.data[k * 4 + 1] = col[1]; img.data[k * 4 + 2] = col[2];
      }
    }
  }
}
writeFileSync(out, PNG.sync.write(img));
console.log(`${out}: ${W}x${H}px, ${parts.length} parts, view az ${opt('az', '30')} el ${opt('el', '25')}, ${(W / scale).toFixed(2)} m wide`);
console.log(parts.map((p) => `${p.name}=rgb(${p.color.map((c) => Math.round(c * 255)).join(',')})`).join('  '));
