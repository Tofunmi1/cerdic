// Throwaway spike: evaluate a compiled geometry program with Manifold (wasm) + our own SDF mesher.
import Module from 'manifold-3d';
import { compile } from './lang.mjs';

const wasm = await Module(); wasm.setup();
const { Manifold, CrossSection, Mesh } = wasm;
const D2R = Math.PI / 180;

// ---------- helpers
const rotY = (m, a) => m.rotate([0, a, 0]);
const upright = (m) => m.rotate([-90, 0, 0]);
const orient = (m, a) => a === 'x' ? m.rotate([0, 0, -90]) : a === 'z' ? m.rotate([90, 0, 0]) : m;
const chk = (a) => { if (!['x', 'y', 'z'].includes(a)) throw new Error(`axis must be x|y|z, got ${a}`); return a; }; // manifold Z-axis shapes -> Y-up
function bboxOf(m) { const b = m.boundingBox(); return { min: [...b.min], max: [...b.max] }; }

function rng(seed) { let s = seed >>> 0 || 1; return () => (s = (s * 1664525 + 1013904223) >>> 0) / 4294967296; }

// value noise for sd_noise
function hash3(x, y, z, seed) { let h = (x * 374761393 + y * 668265263 + z * 2147483647 + seed * 1274126177) | 0; h = (h ^ (h >>> 13)) * 1274126177 | 0; return ((h ^ (h >>> 16)) >>> 0) / 4294967296; }
function vnoise(x, y, z, seed) {
  const xi = Math.floor(x), yi = Math.floor(y), zi = Math.floor(z);
  const f = (t) => t * t * (3 - 2 * t);
  const xf = f(x - xi), yf = f(y - yi), zf = f(z - zi);
  const l = (a, b, t) => a + (b - a) * t;
  const c = (dx, dy, dz) => hash3(xi + dx, yi + dy, zi + dz, seed) * 2 - 1;
  return l(l(l(c(0, 0, 0), c(1, 0, 0), xf), l(c(0, 1, 0), c(1, 1, 0), xf), yf), l(l(c(0, 0, 1), c(1, 0, 1), xf), l(c(0, 1, 1), c(1, 1, 1), xf), yf), zf);
}
const smin = (a, b, k) => { if (k <= 0) return Math.min(a, b); const h = Math.max(k - Math.abs(a - b), 0) / k; return Math.min(a, b) - h * h * k * 0.25; };
const smax = (a, b, k) => -smin(-a, -b, k);

// ---------- surface nets mesher for SDF
function meshSdf(f, bounds, res) {
  const [bx, by, bz] = bounds; const longest = Math.max(bx, by, bz); const h = longest / res;
  const nx = Math.ceil(bx / h) + 2, ny = Math.ceil(by / h) + 2, nz = Math.ceil(bz / h) + 2;
  const ox = -bx / 2 - h, oy = -by / 2 - h, oz = -bz / 2 - h;
  const idx = (i, j, k) => (k * ny + j) * nx + i;
  const val = new Float32Array(nx * ny * nz);
  for (let k = 0; k < nz; k++) for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) val[idx(i, j, k)] = f(ox + i * h, oy + j * h, oz + k * h);
  const vid = new Int32Array((nx - 1) * (ny - 1) * (nz - 1)).fill(-1);
  const cid = (i, j, k) => (k * (ny - 1) + j) * (nx - 1) + i;
  const verts = [];
  const corners = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1]];
  const edges = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
  for (let k = 0; k < nz - 1; k++) for (let j = 0; j < ny - 1; j++) for (let i = 0; i < nx - 1; i++) {
    const v = corners.map(([a, b, c]) => val[idx(i + a, j + b, k + c)]);
    let neg = 0; for (const x of v) if (x < 0) neg++;
    if (neg === 0 || neg === 8) continue;
    let sx = 0, sy = 0, sz = 0, n = 0;
    for (const [a, b] of edges) {
      if ((v[a] < 0) === (v[b] < 0)) continue;
      const t = v[a] / (v[a] - v[b]);
      sx += corners[a][0] + t * (corners[b][0] - corners[a][0]);
      sy += corners[a][1] + t * (corners[b][1] - corners[a][1]);
      sz += corners[a][2] + t * (corners[b][2] - corners[a][2]); n++;
    }
    vid[cid(i, j, k)] = verts.length / 3;
    verts.push(ox + (i + sx / n) * h, oy + (j + sy / n) * h, oz + (k + sz / n) * h);
  }
  const tris = [];
  const quad = (a, b, c, d, flip) => { if (a < 0 || b < 0 || c < 0 || d < 0) return; flip ? tris.push(a, c, b, a, d, c) : tris.push(a, b, c, a, c, d); };
  for (let k = 1; k < nz - 1; k++) for (let j = 1; j < ny - 1; j++) for (let i = 1; i < nx - 1; i++) {
    const s0 = val[idx(i, j, k)] < 0;
    // edge along x
    if (s0 !== (val[idx(i + 1, j, k)] < 0)) quad(vid[cid(i, j - 1, k - 1)], vid[cid(i, j, k - 1)], vid[cid(i, j, k)], vid[cid(i, j - 1, k)], s0);
    if (s0 !== (val[idx(i, j + 1, k)] < 0)) quad(vid[cid(i - 1, j, k - 1)], vid[cid(i - 1, j, k)], vid[cid(i, j, k)], vid[cid(i, j, k - 1)], s0);
    if (s0 !== (val[idx(i, j, k + 1)] < 0)) quad(vid[cid(i - 1, j - 1, k)], vid[cid(i, j - 1, k)], vid[cid(i, j, k)], vid[cid(i - 1, j, k)], s0);
  }
  return { verts: new Float32Array(verts), tris: new Uint32Array(tris) };
}

function manifoldFromSoup({ verts, tris }) {
  const mesh = new Mesh({ numProp: 3, vertProperties: verts, triVerts: tris });
  mesh.merge();
  return Manifold.ofMesh(mesh);
}

// ---------- ring loft / sweep (own code; manifold has no sweep)
function polyOf(cs) { const p = cs.toPolygons(); return p[0].map(v => [v[0], v[1]]); }
function resample(poly, n) {
  const d = [0]; for (let i = 1; i <= poly.length; i++) { const a = poly[i - 1], b = poly[i % poly.length]; d.push(d[i - 1] + Math.hypot(b[0] - a[0], b[1] - a[1])); }
  const L = d[d.length - 1]; const out = [];
  for (let k = 0; k < n; k++) { const t = (k / n) * L; let i = 1; while (d[i] < t) i++; const a = poly[i - 1], b = poly[i % poly.length]; const u = (t - d[i - 1]) / (d[i] - d[i - 1] || 1); out.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]); }
  return out;
}
function ringsToManifold(rings) {
  let last;
  for (const [f0, f1, flipAll] of [[false, true, false], [true, false, false], [false, true, true], [true, false, true]]) {
    try { const m = ringsBuild(rings, f0, f1, flipAll); if (m.volume() > 0 && !m.isEmpty()) return m; } catch (e) { last = e; }
  }
  throw last || new Error('sweep/loft: could not build a manifold (self-intersecting path?)');
}
function ringsBuild(rings, f0, f1, flipAll) { // rings: array of arrays of [x,y,z], same length
  const n = rings[0].length, verts = [], tris = [];
  for (const r of rings) for (const p of r) verts.push(...p);
  for (let s = 0; s < rings.length - 1; s++) for (let i = 0; i < n; i++) {
    const a = s * n + i, b = s * n + (i + 1) % n, c = (s + 1) * n + (i + 1) % n, d = (s + 1) * n + i;
    tris.push(a, b, c, a, c, d);
  }
  const cap = (ri, flip) => { const cx = rings[ri].reduce((s, p) => [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n], [0, 0, 0]); const ci = verts.length / 3; verts.push(...cx); for (let i = 0; i < n; i++) { const a = ri * n + i, b = ri * n + (i + 1) % n; flip ? tris.push(ci, b, a) : tris.push(ci, a, b); } };
  cap(0, f0); cap(rings.length - 1, f1);
  if (flipAll) for (let i = 0; i < tris.length; i += 3) { const t = tris[i + 1]; tris[i + 1] = tris[i + 2]; tris[i + 2] = t; }
  return manifoldFromSoup({ verts: new Float32Array(verts), tris: new Uint32Array(tris) });
}
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => { const l = Math.hypot(...a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };

function sweep(profile, curve, twist, scaleEnd) {
  const poly = resample(polyOf(profile), 24);
  const pts = curve; const rings = []; let N0 = null;
  for (let i = 0; i < pts.length; i++) {
    const T = norm(sub(pts[Math.min(i + 1, pts.length - 1)], pts[Math.max(i - 1, 0)]));
    if (!N0) { const ref = Math.abs(T[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0]; N0 = norm(cross(cross(T, ref), T)); }
    else { N0 = norm(sub(N0, T.map(x => x * (N0[0] * T[0] + N0[1] * T[1] + N0[2] * T[2])))); }
    const B = norm(cross(T, N0)); const u = i / (pts.length - 1);
    const s = 1 + (scaleEnd - 1) * u, ang = twist * D2R * u, ca = Math.cos(ang), sa = Math.sin(ang);
    rings.push(poly.map(([x, y]) => { const xr = (x * ca - y * sa) * s, yr = (x * sa + y * ca) * s; return [pts[i][0] + N0[0] * xr + B[0] * yr, pts[i][1] + N0[1] * xr + B[1] * yr, pts[i][2] + N0[2] * xr + B[2] * yr]; }));
  }
  return ringsToManifold(rings);
}
function catmull(pts, samples) {
  const out = []; const P = (i) => pts[Math.max(0, Math.min(pts.length - 1, i))];
  for (let i = 0; i < pts.length - 1; i++) for (let s = 0; s < samples / (pts.length - 1); s++) {
    const t = s / (samples / (pts.length - 1)), t2 = t * t, t3 = t2 * t, p0 = P(i - 1), p1 = P(i), p2 = P(i + 1), p3 = P(i + 2);
    out.push([0, 1, 2].map(k => 0.5 * ((2 * p1[k]) + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3)));
  }
  out.push(pts[pts.length - 1]); return out;
}

// ---------- sdf transforms
function sdfMove(f, by) { return (x, y, z) => f(x - by[0], y - by[1], z - by[2]); }
function sdfRotate(f, deg) {
  const [ax, ay, az] = deg.map(d => -d * D2R); // inverse
  return (x, y, z) => { // apply inverse of Rz*Ry*Rx (manifold order x,y,z) => Rx^-1 Ry^-1 Rz^-1
    let c = Math.cos(az), s = Math.sin(az); [x, y] = [x * c - y * s, x * s + y * c];
    c = Math.cos(ay); s = Math.sin(ay); [x, z] = [x * c + z * s, -x * s + z * c];
    c = Math.cos(ax); s = Math.sin(ax); [y, z] = [y * c - z * s, y * s + z * c];
    return f(x, y, z);
  };
}
function sdfScale(f, by) { const m = Math.min(...by); return (x, y, z) => f(x / by[0], y / by[1], z / by[2]) * m; }

// ---------- node evaluators
const unsupported = (what) => { throw new Error(`NOT IMPLEMENTED: ${what}`); };
const E = {
  box: (n, p) => Manifold.cube(p.size, true),
  sphere: (n, p) => Manifold.sphere(p.radius, p.segments),
  cylinder: (n, p) => orient(upright(Manifold.cylinder(p.height, p.radius, p.radius, p.segments, true)), chk(p.axis)),
  cone: (n, p) => orient(upright(Manifold.cylinder(p.height, p.radius, p.top_radius, p.segments, true)), chk(p.axis)),
  torus: (n, p) => orient(upright(CrossSection.circle(p.minor, p.segments).translate([p.major, 0]).revolve(p.segments)), chk(p.axis)),
  capsule: (n, p) => { const c = Manifold.cylinder(Math.max(p.height - 2 * p.radius, 1e-3), p.radius, p.radius, 24, true); const s = Manifold.sphere(p.radius, 24); return orient(upright(Manifold.union([c, s.translate([0, 0, (p.height - 2 * p.radius) / 2]), s.translate([0, 0, -(p.height - 2 * p.radius) / 2])])), chk(p.axis)); },
  rect: (n, p) => CrossSection.square(p.size, true),
  circle: (n, p) => CrossSection.circle(p.radius, p.segments),
  polygon: (n, p) => new CrossSection([p.points]),
  line: (n, p) => p.points,
  spline: (n, p) => catmull(p.points, p.samples),
  extrude: (n, p, [a]) => upright(a.extrude(p.height)),
  revolve: (n, p, [a]) => upright(a.revolve(p.segments, p.angle)),
  sweep: (n, p, [a, c]) => sweep(a, c, p.twist, p.scale_end),
  loft: (n, p, ins) => { const polys = ins.map(a => resample(polyOf(a), 48)); return ringsToManifold(polys.map((poly, i) => poly.map(([x, y]) => [x, i * p.spacing, y]))); },
  union: (n, p, ins) => Manifold.union(ins),
  difference: (n, p, ins) => Manifold.difference([ins[0], Manifold.union(ins.slice(1))]),
  intersect: (n, p, ins) => Manifold.intersection(ins),
  move: (n, p, [a], t) => t === 'sdf' ? sdfMove(a, p.by) : t === 'inst' ? a.map(m => m.translate(p.by)) : a.translate(p.by),
  rotate: (n, p, [a], t) => t === 'sdf' ? sdfRotate(a, p.degrees) : t === 'inst' ? a.map(m => m.rotate(p.degrees)) : a.rotate(p.degrees),
  scale: (n, p, [a], t) => t === 'sdf' ? sdfScale(a, p.by) : t === 'inst' ? a.map(m => m.scale(p.by)) : a.scale(p.by),
  mirror: (n, p, [a]) => { const nm = { x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] }[p.axis]; if (!nm) throw new Error(`mirror axis must be x|y|z, got ${p.axis}`); return Manifold.union([a, a.mirror(nm)]); },
  repeat: (n, p, [a]) => Manifold.union(Array.from({ length: p.count }, (_, i) => a.rotate(p.rotate_step.map(v => v * i)).translate(p.step.map(v => v * i)))),
  array: (n, p, [a]) => { const out = []; for (let i = 0; i < p.count[0]; i++) for (let j = 0; j < p.count[1]; j++) for (let k = 0; k < p.count[2]; k++) out.push(a.translate([i * p.spacing[0], j * p.spacing[1], k * p.spacing[2]])); return Manifold.union(out); },
  radial: (n, p, [a]) => Manifold.union(Array.from({ length: p.count }, (_, i) => rotY(a.translate([p.radius, 0, 0]), (360 / p.count) * i))),
  scatter: (n, p, [a]) => { const r = rng(p.seed); return Array.from({ length: p.count }, () => a.translate([(r() - 0.5) * p.area[0], 0, (r() - 0.5) * p.area[1]]).rotate([0, r() * 360, 0])); },
  merge: (n, p, [a]) => Manifold.union(a),
  bevel: () => unsupported('bevel (Manifold has no offset/bevel)'),
  smooth: (n, p, [a]) => a.smoothOut(60, 0).refine(Math.max(1, p.iterations)),
  simplify: (n, p, [a]) => { const b = bboxOf(a); const d = Math.hypot(...b.max.map((v, i) => v - b.min[i])); return a.simplify(d * (1 - p.ratio) * 0.02); },
  hollow: (n, p, [a]) => { const b = bboxOf(a); const c = b.min.map((v, i) => (v + b.max[i]) / 2); const sz = b.max.map((v, i) => v - b.min[i]); const s = sz.map(x => Math.max(0.05, 1 - (2 * p.thickness) / x)); return Manifold.difference([a, a.translate(c.map(x => -x)).scale(s).translate(c)]); },
  sd_sphere: (n, p) => (x, y, z) => Math.hypot(x, y, z) - p.radius,
  sd_box: (n, p) => { const h = p.size.map(v => v / 2 - p.round); return (x, y, z) => { const q = [Math.abs(x) - h[0], Math.abs(y) - h[1], Math.abs(z) - h[2]]; return Math.hypot(Math.max(q[0], 0), Math.max(q[1], 0), Math.max(q[2], 0)) + Math.min(Math.max(q[0], q[1], q[2]), 0) - p.round; }; },
  sd_capsule: (n, p) => { const [a, b] = [p.a, p.b]; const ba = sub(b, a); const bb = ba[0] ** 2 + ba[1] ** 2 + ba[2] ** 2 || 1; return (x, y, z) => { const pa = [x - a[0], y - a[1], z - a[2]]; const h = Math.max(0, Math.min(1, (pa[0] * ba[0] + pa[1] * ba[1] + pa[2] * ba[2]) / bb)); return Math.hypot(pa[0] - ba[0] * h, pa[1] - ba[1] * h, pa[2] - ba[2] * h) - p.radius; }; },
  sd_union: (n, p, fs) => (x, y, z) => fs.reduce((acc, f, i) => i === 0 ? f(x, y, z) : smin(acc, f(x, y, z), p.smooth), 0),
  sd_subtract: (n, p, [a, b]) => (x, y, z) => smax(a(x, y, z), -b(x, y, z), p.smooth),
  sd_noise: (n, p, [a]) => (x, y, z) => a(x, y, z) + p.amount * vnoise(x * p.frequency, y * p.frequency, z * p.frequency, p.seed),
  sd_expr: (n, p) => { if (!/^[\w\s+\-*/().,<>=?:%^]*$/.test(p.expr)) throw new Error('sd_expr: illegal characters'); const fn = new Function('x', 'y', 'z', 'sin', 'cos', 'abs', 'min', 'max', 'sqrt', 'length', `return (${p.expr.replace(/\^/g, '**')});`); return (x, y, z) => fn(x, y, z, Math.sin, Math.cos, Math.abs, Math.min, Math.max, Math.sqrt, Math.hypot); },
  mesh_sdf: (n, p, [f]) => { const h = Math.max(...p.bounds) / p.resolution; const m = Manifold.levelSet((q) => -f(q[0], q[1], q[2]), { min: p.bounds.map(v => -v / 2), max: p.bounds.map(v => v / 2) }, h); if (m.isEmpty()) throw new Error('mesh_sdf: empty (surface not inside bounds)'); return m; },
  part: (n, p, [a], t, ctx) => { ctx.parts.push({ name: p.name, bbox: a.boundingBox ? bboxOf(a) : null }); return a; },
  socket: (n, p, [a], t, ctx) => { ctx.sockets.push({ name: p.name, at: p.at, normal: p.normal }); return a; },
  material: (n, p, [a], t, ctx) => { ctx.materials.push(p.slot); return a; },
};

export function evaluate(src) {
  const c = compile(src);
  if (!c.ok) return { ok: false, stage: 'compile', errors: c.errors };
  const vals = new Map(), types = new Map();
  const ctx = { parts: [], sockets: [], materials: [] };
  const t0 = performance.now();
  try {
    for (const n of c.nodes) {
      const params = {};
      for (const [k, [, d]] of Object.entries(n.spec.params)) params[k] = k in n.params ? n.params[k] : (typeof d === 'symbol' ? undefined : d);
      const ins = n.inputs.map(r => vals.get(r));
      const t = n.spec.out === '=0' ? types.get(n.inputs[0]) : n.spec.out;
      const fn = E[n.kind]; if (!fn) throw new Error(`no evaluator for ${n.kind}`);
      vals.set(n.name, fn(n, params, ins, t, ctx)); types.set(n.name, t);
    }
  } catch (e) { return { ok: false, stage: 'eval', errors: [{ ln: 0, msg: String(e.message || e) }] }; }
  const m = vals.get(c.output);
  const ms = performance.now() - t0;
  const parts = m.decompose();
  const voids = parts.filter(p => p.volume() < 0).length;
  const solids = parts.filter(p => p.volume() >= 0);
  const boxes = solids.map(p => bboxOf(p)).map((b, i) => ({ i, ...b, vol: solids[i].volume() }));
  const gaps = [];
  if (boxes.length > 1) for (const a of boxes) { // nearest other component by bbox gap
    let best = null;
    for (const o of boxes) { if (o === a) continue; const g = Math.hypot(...[0, 1, 2].map(k => Math.max(0, a.min[k] - o.max[k], o.min[k] - a.max[k]))); if (!best || g < best.gap) best = { to: o.i, gap: g }; }
    gaps.push({ component: a.i, size: a.max.map((v, k) => +(v - a.min[k]).toFixed(3)), center: a.min.map((v, k) => +((v + a.max[k]) / 2).toFixed(3)), nearest: best.to, gap: +best.gap.toFixed(4) });
  }
  const b = bboxOf(m); const mesh = m.getMesh();
  const res = {
    ok: true, ms, tris: mesh.triVerts.length / 3, volume: m.volume(), area: m.surfaceArea(), components: solids.length, voids, gaps, genus: m.genus(),
    bbox: b, size: b.max.map((v, i) => +(v - b.min[i]).toFixed(3)), parts: ctx.parts.map(p => p.name), sockets: ctx.sockets,
    verts: mesh.vertProperties, numProp: mesh.numProp, tri: mesh.triVerts,
  };
  parts.forEach(p => p.delete());
  return res;
}
