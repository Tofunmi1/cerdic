// Measurement helpers used by the task checks. All lengths are metres, Y is up.

export const near = (a, b, tol) => Math.abs(a - b) <= tol;
export const within = (v, lo, hi) => v >= lo && v <= hi;
export const out = (res, name) => res.outputs.find((o) => o.part === name || o.node === name);
export const lum = (c) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
export const dist3 = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

export const isMetal = (o) => !!o?.material && o.material.metallic >= 0.8;
export const isDielectric = (o) => !!o?.material && o.material.metallic <= 0.2;
export const isGlass = (o) => !!o?.material && o.material.opacity <= 0.5 && o.material.roughness <= 0.2 && o.material.metallic <= 0.2;
export const isEmissive = (o) => !!o?.material && Math.max(...o.material.emissive) >= 0.8;
export const matte = (o, r = 0.6) => !!o?.material && o.material.roughness >= r;
export const glossy = (o, r = 0.4) => !!o?.material && o.material.roughness <= r;

// union bounding box of several outputs
export function bboxAll(outs) {
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  for (const o of outs) for (let k = 0; k < 3; k++) { min[k] = Math.min(min[k], o.bbox.min[k]); max[k] = Math.max(max[k], o.bbox.max[k]); }
  return { min, max, size: max.map((v, k) => v - min[k]) };
}

export const center = (b) => b.min.map((v, k) => (v + b.max[k]) / 2);

// horizontal section at height y: the solid's Y axis is turned onto Z so slice() cuts a level plane
export function hslice(o, y) { const r = o.m.rotate([90, 0, 0]); const cs = r.slice(y); r.delete(); return cs; }
// section through the XY plane (z = 0): the profile view of an axisymmetric part
export function vslice(o, z = 0) { return o.m.slice(z); }

export function sectionBounds(cs) {
  const polys = cs.toPolygons(); const min = [Infinity, Infinity], max = [-Infinity, -Infinity];
  for (const l of polys) for (const p of l) { min[0] = Math.min(min[0], p[0]); min[1] = Math.min(min[1], p[1]); max[0] = Math.max(max[0], p[0]); max[1] = Math.max(max[1], p[1]); }
  return polys.length ? { min, max, w: max[0] - min[0], h: max[1] - min[1] } : null;
}

// width (largest horizontal extent) of the part at height y
export function widthAt(o, y) {
  const cs = hslice(o, y); const b = sectionBounds(cs); cs.delete();
  return b ? Math.max(b.w, b.h) : 0;
}

export function loopArea(l) { let a = 0; for (let i = 0; i < l.length; i++) { const p = l[i], q = l[(i + 1) % l.length]; a += p[0] * q[1] - q[0] * p[1]; } return a / 2; }

// number of teeth/lobes: runs of outline vertices lying above the mid radius
export function lobes(o, y) {
  const cs = hslice(o, y); const polys = cs.toPolygons(); cs.delete();
  const outer = polys.map((l) => ({ l, a: Math.abs(loopArea(l)) })).sort((a, b) => b.a - a.a)[0]?.l;
  if (!outer) return 0;
  const c = outer.reduce((s, p) => [s[0] + p[0] / outer.length, s[1] + p[1] / outer.length], [0, 0]);
  const r = outer.map((p) => Math.hypot(p[0] - c[0], p[1] - c[1]));
  const mid = (Math.max(...r) + Math.min(...r)) / 2;
  const hi = r.map((v) => v > mid);
  let runs = 0; for (let i = 0; i < hi.length; i++) if (hi[i] && !hi[(i + hi.length - 1) % hi.length]) runs++;
  return runs;
}

// radius of the inner (bore) loop of the section at height y, or null
export function boreRadius(o, y) {
  const cs = hslice(o, y); const polys = cs.toPolygons(); cs.delete();
  const holes = polys.filter((l) => loopArea(l) < 0); if (!holes.length) return null;
  const h = holes[0]; return Math.sqrt(Math.abs(loopArea(h)) / Math.PI);
}

// fraction of the section lost by opening with a disc of radius w/2 (0 = every part is >= w thick)
export function openingLoss(cs, w) {
  if (cs.isEmpty()) return 0;
  const opened = cs.offset(-w / 2, 'Miter', 2).offset(w / 2, 'Miter', 2);
  const lost = 1 - opened.area() / cs.area(); opened.delete(); return lost;
}

// areas of the holes in a section (positive numbers)
export function holeAreas(cs) { return cs.toPolygons().map(loopArea).filter((a) => a < 0).map(Math.abs); }

// centres of the connected components of an output
export function componentBoxes(o) { return o.boxes.map((b) => ({ ...b, c: center(b), size: b.max.map((v, k) => v - b.min[k]) })); }

export function stddev(xs) { const m = xs.reduce((s, x) => s + x, 0) / xs.length; return Math.sqrt(xs.reduce((s, x) => s + (x - m) ** 2, 0) / xs.length); }
