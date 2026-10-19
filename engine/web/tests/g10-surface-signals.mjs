import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { compileTexture } from '../../../src/engine/texture/compile.ts';
import { loadHelpers, loadLibrary } from '../../../src/engine/texture/library.ts';
import { parseTextureProgram } from '../../../src/engine/texture/program.ts';
import { cornerNormals } from '../../../src/engine/normals.ts';

// G10: surface signals for texture programs. A texture node such as `surface_convex` reads the mean curvature of the surface, worked
// out per texel from how the normal changes. Checked on shapes whose curvature is known: a sphere of radius r has curvature 1 / r
// everywhere, a plane has none, and the inside of a sphere is concave. The signal is 1 at a 5 cm radius, so `gain * 0.05 / r`
// is the grey a program with `surface_convex(gain)` bakes.
const root = new URL('../../third_party/material-maker/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);
const compile = (text) => {
  const program = parseTextureProgram(text, library);
  return compileTexture(library, { nodes: program.nodes, output: program.channels.albedo }, helpers);
};
const convex = compile('m = surface_convex(gain=10); albedo = m');
const cavity = compile('m = surface_cavity(gain=10); albedo = m');
const slope = compile('m = surface_up(power=1); albedo = m');
assert(convex.needs.surface && cavity.needs.surface && slope.needs.surface);

/** A UV sphere (u around, v pole to pole) as triangle corners with UVs, wound outward (or inward). */
function sphere(radius, inward = false, stacks = 48, slices = 96) {
  const uvs = [], positions = [];
  const vertex = (i, j) => {
    const u = i / slices, v = j / stacks, phi = u * Math.PI * 2, theta = v * Math.PI;
    return { uv: [u, v], p: [radius * Math.cos(phi) * Math.sin(theta), radius * Math.cos(theta), radius * Math.sin(phi) * Math.sin(theta)] };
  };
  const push = (a, b, c) => {
    const ab = [b.p[0] - a.p[0], b.p[1] - a.p[1], b.p[2] - a.p[2]], ac = [c.p[0] - a.p[0], c.p[1] - a.p[1], c.p[2] - a.p[2]];
    const n = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]];
    const centre = [(a.p[0] + b.p[0] + c.p[0]) / 3, (a.p[1] + b.p[1] + c.p[1]) / 3, (a.p[2] + b.p[2] + c.p[2]) / 3];
    const outward = n[0] * centre[0] + n[1] * centre[1] + n[2] * centre[2] > 0;
    for (const v of outward !== inward ? [a, b, c] : [a, c, b]) { uvs.push(...v.uv); positions.push(...v.p); }
  };
  for (let j = 0; j < stacks; j++) for (let i = 0; i < slices; i++) {
    const a = vertex(i, j), b = vertex(i + 1, j), c = vertex(i + 1, j + 1), d = vertex(i, j + 1);
    push(a, b, c); push(a, c, d);
  }
  return { uvs, positions };
}

/** A flat square facing up, filling the unit UV square. */
function plane() {
  const corner = (u, v) => ({ uv: [u, v], p: [u - 0.5, 0, v - 0.5] });
  const uvs = [], positions = [];
  for (const t of [[[0, 0], [0, 1], [1, 1]], [[0, 0], [1, 1], [1, 0]]]) for (const [u, v] of t) { const c = corner(u, v); uvs.push(...c.uv); positions.push(...c.p); }
  return { uvs, positions };
}

const bundle = await Bun.build({ entrypoints: [new URL('./g10-texture-bake-entry.ts', import.meta.url).pathname], target: 'browser', format: 'iife' });
assert(bundle.success, 'bundle builds');
const script = await bundle.outputs[0].text();
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage();
  await page.setContent('<canvas></canvas>');
  await page.addScriptTag({ content: script });
  const size = 256;
  /** The grey of a program's first texels around the middle of the picture, 0 to 255. */
  const grey = async (mesh, compiled) => {
    const corners = mesh.positions.length / 3;
    const indices = new Uint32Array(corners).map((_, i) => i);
    const normals = Array.from(cornerNormals(new Float32Array(mesh.positions), indices));
    const pixels = await page.evaluate(({ compiled, uvs, positions, normals, size }) => {
      const baker = new window.Baker(new OffscreenCanvas(size, size));
      const p = baker.positionMap(new Float32Array(uvs), new Float32Array(positions), size, 4);
      const n = baker.positionMap(new Float32Array(uvs), new Float32Array(normals), size, 4);
      const ao = new Float32Array(uvs.length / 2 * 3);
      for (let i = 0; i < ao.length; i += 3) ao[i] = 1;
      const a = baker.positionMap(new Float32Array(uvs), ao, size, 4);
      const surface = baker.surfaceMap(p, n, a, size);
      return Array.from(baker.bake(compiled, size, p, surface));
    }, { compiled: { source: compiled.source, space: compiled.space, needs: compiled.needs }, uvs: mesh.uvs, positions: mesh.positions, normals, size });
    let sum = 0, count = 0;
    for (let y = size / 2 - 6; y <= size / 2 + 6; y++) for (let x = size / 2 - 6; x <= size / 2 + 6; x++) { sum += pixels[(y * size + x) * 4]; count++; }
    return sum / count;
  };

  // Choose each gain so the expected grey is half way (127): a sphere of radius r reads 0.05 / r, so the gain is 0.5 r / 0.05.
  const small = await grey(sphere(0.25), compile('m = surface_convex(gain=2.5); albedo = m'));
  const large = await grey(sphere(1), compile('m = surface_convex(gain=10); albedo = m'));
  const flat = await grey(plane(), convex);
  const inside = await grey(sphere(0.25, true), compile('m = surface_cavity(gain=2.5); albedo = m'));
  const insideConvex = await grey(sphere(0.25, true), convex);
  const top = await grey(plane(), slope);
  console.log(`G10 surface signals: convex r=0.25 -> ${small.toFixed(0)}, r=1 -> ${large.toFixed(0)} (both expect 127); plane -> ${flat.toFixed(0)} (expect 0); inside of a sphere: cavity ${inside.toFixed(0)} (expect 127), convex ${insideConvex.toFixed(0)} (expect 0); plane facing up: ${top.toFixed(0)} (expect 255)`);
  assert(small > 95 && small < 160, `curvature of a 25 cm sphere is within 25% of 1/r (got ${small})`);
  assert(large > 95 && large < 160, `curvature of a 1 m sphere is within 25% of 1/r (got ${large})`);
  assert(flat < 6, `a plane has no curvature (got ${flat})`);
  assert(inside > 95 && inside < 160, `the inside of a sphere has the same curvature, concave (got ${inside})`);
  assert(insideConvex < 6, 'the inside of a sphere is not convex');
  assert(top > 250, 'a plane facing up has slope 1');
} finally {
  await browser.close();
}
