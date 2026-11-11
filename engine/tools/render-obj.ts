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
// The sidecar written by dump_obj (parts.json) gives each part its material, tracks and parent: with it the picture uses the scene's own colours,
// and `--time T` plays the main clip to T seconds (the same rules as the viewer: tracks act in order about their pivots, a child follows its parent).
type Mat = number[]; // 4x4, row-major
const matMul = (a: Mat, b: Mat): Mat => { const o = new Array(16).fill(0); for (let r = 0; r < 4; r++) for (let c = 0; c < 4; c++) for (let k = 0; k < 4; k++) o[r * 4 + c] += a[r * 4 + k] * b[k * 4 + c]; return o; };
const ident = (): Mat => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
const translate = (t: V): Mat => [1, 0, 0, t[0], 0, 1, 0, t[1], 0, 0, 1, t[2], 0, 0, 0, 1];
const scaling = (s: V): Mat => [s[0], 0, 0, 0, 0, s[1], 0, 0, 0, 0, s[2], 0, 0, 0, 0, 1];
const rotation = (deg: V): Mat => { // Y, then X, then Z (glam YXZ)
  const [x, y, z] = deg.map((d) => (d * Math.PI) / 180);
  const rx: Mat = [1, 0, 0, 0, 0, Math.cos(x), -Math.sin(x), 0, 0, Math.sin(x), Math.cos(x), 0, 0, 0, 0, 1];
  const ry: Mat = [Math.cos(y), 0, Math.sin(y), 0, 0, 1, 0, 0, -Math.sin(y), 0, Math.cos(y), 0, 0, 0, 0, 1];
  const rz: Mat = [Math.cos(z), -Math.sin(z), 0, 0, Math.sin(z), Math.cos(z), 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  return matMul(ry, matMul(rx, rz));
};
const about = (pivot: V, m: Mat): Mat => matMul(translate(pivot), matMul(m, translate([-pivot[0], -pivot[1], -pivot[2]])));
type Track = { property: string; keys: { time: number; value: V }[]; interpolation: string; looped: boolean; pivot: V; clip: string };
const sample = (tr: Track, seconds: number): V => {
  const keys = tr.keys; const first = keys[0]; const last = keys[keys.length - 1]; const span = last.time - first.time;
  const t = tr.looped && span > 0 ? first.time + ((((seconds - first.time) % span) + span) % span) : Math.min(last.time, Math.max(first.time, seconds));
  if (t >= last.time) return last.value;
  let next = keys.findIndex((k) => k.time > t); if (next < 1) next = 1;
  const a = keys[next - 1]; const b = keys[next];
  const f = (t - a.time) / (b.time - a.time || 1);
  const e = tr.interpolation === 'step' ? 0 : tr.interpolation === 'ease_in' ? f * f : tr.interpolation === 'ease_out' ? 1 - (1 - f) * (1 - f) : tr.interpolation === 'linear' ? f : f * f * (3 - 2 * f); // bezier, back, bounce, elastic: smooth stands in
  return [0, 1, 2].map((i) => a.value[i] + (b.value[i] - a.value[i]) * e) as V;
};
const time = args.includes('--time') ? Number(opt('time', '0')) : NaN;
type Side = { file: string; name: string; parent: string | null; material: { color: V; opacity: number; emissive: V; metallic: number } | null; animation: Track[] };
let sidecar: Side[] = [];
try { sidecar = JSON.parse(readFileSync(`${dir}/parts.json`, 'utf8')); } catch { /* no sidecar: palette colours, rest pose */ }
const sideByFile = new Map(sidecar.map((s) => [s.file, s]));
const sideByName = new Map<string, Side>(); for (const s of sidecar) if (!sideByName.has(s.name)) sideByName.set(s.name, s);
const localPose = (s: Side): Mat => {
  let m = ident();
  for (const tr of s.animation) {
    if (tr.clip !== '' || Number.isNaN(time)) continue;
    const v = sample(tr, time);
    const step = tr.property === 'position' ? translate(v) : tr.property === 'rotation' ? about(tr.pivot, rotation(v)) : tr.property === 'scale' ? about(tr.pivot, scaling(v)) : ident();
    m = matMul(step, m);
  }
  return m;
};
const worldPose = (s: Side): Mat => { let m = localPose(s); let at = s; for (let depth = 0; depth < 32 && at.parent; depth++) { const up = sideByName.get(at.parent); if (!up || up === s) break; m = matMul(localPose(up), m); at = up; } return m; };
const shown = (s: Side) => Number.isNaN(time) || s.animation.every((tr) => tr.property !== 'visible' || tr.clip !== '' || sample(tr, time)[0] >= 0.5);
const parts: { name: string; color: V; tris: [V, V, V][]; glow: boolean }[] = [];
for (const file of readdirSync(dir).filter((n) => n.endsWith('.obj')).sort()) {
  const name = file.replace(/\.obj$/, '');
  if (only.length && !only.includes(name)) continue;
  if (skip.includes(name)) continue;
  const side = sideByFile.get(name);
  if (side && !shown(side)) continue;
  if (side?.material && side.material.opacity < 0.4) continue; // glass and haze: see through to what is behind
  const verts: V[] = [];
  const faces: number[][] = [];
  for (const line of readFileSync(`${dir}/${file}`, 'utf8').split('\n')) {
    if (line.startsWith('v ')) { const [, x, y, z] = line.trim().split(/\s+/); verts.push([Number(x), Number(y), Number(z)]); }
    else if (line.startsWith('f ')) faces.push(line.trim().split(/\s+/).slice(1).map((t) => Number(t.split('/')[0]) - 1));
  }
  if (side && !Number.isNaN(time)) {
    const m = worldPose(side);
    for (const v of verts) { const x = v[0], y = v[1], z = v[2]; v[0] = m[0] * x + m[1] * y + m[2] * z + m[3]; v[1] = m[4] * x + m[5] * y + m[6] * z + m[7]; v[2] = m[8] * x + m[9] * y + m[10] * z + m[11]; }
  }
  const tris: [V, V, V][] = [];
  for (const ids of faces) for (let i = 1; i + 1 < ids.length; i++) tris.push([verts[ids[0]], verts[ids[i]], verts[ids[i + 1]]]);
  const mat = side?.material;
  const glow = !!mat && Math.max(...mat.emissive) > 0.8;
  const color: V = mat ? (glow ? [Math.min(1, mat.emissive[0] * 0.5 + 0.5), Math.min(1, mat.emissive[1] * 0.5 + 0.5), Math.min(1, mat.emissive[2] * 0.5 + 0.5)] : mat.color) : palette[parts.length % palette.length];
  parts.push({ name, color, tris, glow });
}

// `--follow prefix`: keep the parts whose names start with it centred (a creature flying a long circle stays in frame)
const followPrefix = opt('follow', '');
if (followPrefix) {
  const mine = parts.filter((p) => p.name.startsWith(followPrefix));
  const unique = new Set<V>();
  for (const part of parts) for (const t of part.tris) for (const v of t) unique.add(v);
  let sx = 0, sy = 0, sz = 0, n = 0;
  for (const part of mine) for (const t of part.tris) for (const v of t) { sx += v[0]; sy += v[1]; sz += v[2]; n++; }
  if (n) for (const v of unique) { v[0] -= sx / n; v[1] -= sy / n; v[2] -= sz / n; }
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
const ss = Math.max(1, Number(opt('ssaa', '1')));
const wire = args.includes('--wire');
const labelsOut = opt('labels', '');
const scale = (size * ss) / Math.max(width, height * (centerArg ? 1 : 1));
const W = size * ss;
const H = centerArg ? size * ss : Math.max(64, Math.round(height * scale));
const img = new PNG({ width: W, height: H });
const depth = new Float32Array(W * H).fill(-Infinity);
// `--bg r,g,b` (each 0 to 1) sets a flat background, for a picture that sits on a dark page
const bgArg = opt('bg', '');
const bg: V = bgArg ? (bgArg.split(',').map(Number) as V) : [0.93, 0.94, 0.96];
const sunArg = opt('sun', '');
const sunDir: V | null = sunArg ? (() => { const [sa, se] = sunArg.split(',').map((x) => (Number(x) * Math.PI) / 180); return norm([Math.cos(se) * Math.sin(sa), Math.sin(se), Math.cos(se) * Math.cos(sa)]); })() : null;
const skyBackground = args.includes('--sky');
for (let y = 0; y < H; y++) {
  // a sky: deep blue at the top, pale at the horizon (flat grey-white without --sky)
  const g = y / H;
  const c: V = skyBackground ? [0.36 + 0.5 * g, 0.55 + 0.35 * g, 0.84 + 0.1 * g] : bg;
  for (let x = 0; x < W; x++) { const i = y * W + x; img.data[i * 4] = c[0] * 255; img.data[i * 4 + 1] = c[1] * 255; img.data[i * 4 + 2] = c[2] * 255; img.data[i * 4 + 3] = 255; }
}
const light = norm([-0.4, 0.8, 0.5]);
// a light that follows the camera so every angle is lit: key from up-left of the viewer, plus ambient
const key = norm([-right[0] * 0.5 + up[0] * 0.7 + d[0] * 0.6, -right[1] * 0.5 + up[1] * 0.7 + d[1] * 0.6, -right[2] * 0.5 + up[2] * 0.7 + d[2] * 0.6]);
void light;
for (const part of parts) {
  for (const t of part.tris) {
    let n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    if (dot(n, d) < 0) n = [-n[0], -n[1], -n[2]]; // two-sided: the face turned toward the viewer
    // a sun fixed in the world when `--sun az,el` is given (so a turntable shows lit and shadowed sides); otherwise a key light that follows the camera
    const shade = part.glow ? 1 : sunDir ? Math.min(1.15, 0.52 + 0.14 * n[1] + 0.62 * Math.max(0, dot(n, sunDir))) : 0.3 + 0.7 * Math.abs(dot(n, key));
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
// Wireframe overlay: every triangle edge, darkened into the picture where it is not hidden behind a nearer surface.
if (wire) {
  const line = (a: number[], b: number[]) => {
    const steps = Math.ceil(Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])));
    for (let i = 0; i <= steps; i++) {
      const u = steps ? i / steps : 0;
      const x = Math.round(a[0] + (b[0] - a[0]) * u);
      const y = Math.round(a[1] + (b[1] - a[1]) * u);
      if (x < 0 || y < 0 || x >= W || y >= H) continue;
      const k = y * W + x;
      if (a[2] + (b[2] - a[2]) * u < depth[k] - 0.02 * (hi[2] - lo[2] + 1e-6)) continue;
      img.data[k * 4] *= 0.45; img.data[k * 4 + 1] *= 0.45; img.data[k * 4 + 2] *= 0.5;
    }
  };
  for (const part of parts) {
    for (const t of part.tris) {
      const s = t.map((p) => { const q = proj(p); return [(q[0] - cx) * scale + W / 2, H / 2 - (q[1] - cy) * scale, q[2]]; });
      line(s[0], s[1]); line(s[1], s[2]); line(s[2], s[0]);
    }
  }
}
// Part positions on the finished picture (pixels at the output size), so a caption tool can place labels.
if (labelsOut) {
  const places = parts.map((p) => {
    let sx = 0, sy = 0, n = 0;
    for (const t of p.tris) for (const v of t) { const q = proj(v); sx += (q[0] - cx) * scale + W / 2; sy += H / 2 - (q[1] - cy) * scale; n++; }
    return { name: p.name, x: Math.round(sx / n / ss), y: Math.round(sy / n / ss), triangles: p.tris.length };
  });
  writeFileSync(labelsOut, JSON.stringify({ width: Math.round(W / ss), height: Math.round(H / ss), metresPerPixel: ss / scale, parts: places }, null, 1));
}
if (ss > 1) {
  // Box-filter the supersampled picture down to the requested size: smooth edges and fine lines.
  const w2 = Math.round(W / ss), h2 = Math.round(H / ss);
  const small = new PNG({ width: w2, height: h2 });
  for (let y = 0; y < h2; y++) {
    for (let x = 0; x < w2; x++) {
      const acc = [0, 0, 0];
      for (let j = 0; j < ss; j++) for (let i = 0; i < ss; i++) { const k = ((y * ss + j) * W + (x * ss + i)) * 4; acc[0] += img.data[k]; acc[1] += img.data[k + 1]; acc[2] += img.data[k + 2]; }
      const o = (y * w2 + x) * 4;
      small.data[o] = acc[0] / (ss * ss); small.data[o + 1] = acc[1] / (ss * ss); small.data[o + 2] = acc[2] / (ss * ss); small.data[o + 3] = 255;
    }
  }
  writeFileSync(out, PNG.sync.write(small));
} else {
  writeFileSync(out, PNG.sync.write(img));
}
console.log(`${out}: ${Math.round(W / ss)}x${Math.round(H / ss)}px, ${parts.length} parts, view az ${opt('az', '30')} el ${opt('el', '25')}, ${(W / scale).toFixed(2)} m wide`);
console.log(parts.map((p) => `${p.name}=rgb(${p.color.map((c) => Math.round(c * 255)).join(',')})`).join('  '));
