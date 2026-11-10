import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { compileTexture } from '../../../src/engine/texture/compile.ts';
import { loadHelpers, loadLibrary } from '../../../src/engine/texture/library.ts';

// G10: baking a 3D (object-space) texture onto a mesh through its UV layout. A sphere is drawn into UV
// space with its positions as colour, the 3D shader reads those positions, and the picture is read back.
// A 3D pattern is continuous across the UV seam, which is what a 2D pattern cannot do; the test checks it.
const root = new URL('../../lib/material-maker/', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const library = loadLibrary(root);
const helpers = loadHelpers(root);

const ramp = (...stops) => ({ type: 'Gradient', interpolation: 1, points: stops.map(([pos, r, g, b]) => ({ pos, r, g, b, a: 1 })) });
const graph = {
  nodes: [
    { id: 'n', type: 'tex3d_fbm', params: { scale_x: 3, scale_y: 3, scale_z: 3 } },
    { id: 'c', type: 'tex3d_colorize', params: { g: ramp([0, 0.1, 0.05, 0.02], [1, 0.9, 0.6, 0.3]) }, inputs: { in: { node: 'n' } } },
  ],
  output: { node: 'c' },
};
const compiled = compileTexture(library, graph, helpers);
assert.equal(compiled.space, '3d');

// A UV sphere: u runs around, v from pole to pole, so u = 0 and u = 1 are the same line in space.
const stacks = 32, slices = 64;
const uvs = [], positions = [];
const vertex = (i, j) => {
  const u = i / slices, v = j / stacks, phi = u * Math.PI * 2, theta = v * Math.PI;
  return { uv: [u, v], p: [Math.cos(phi) * Math.sin(theta), Math.cos(theta), Math.sin(phi) * Math.sin(theta)] };
};
for (let j = 0; j < stacks; j++) for (let i = 0; i < slices; i++) {
  const a = vertex(i, j), b = vertex(i + 1, j), c = vertex(i + 1, j + 1), d = vertex(i, j + 1);
  for (const t of [[a, b, c], [a, c, d]]) for (const v of t) { uvs.push(...v.uv); positions.push(...v.p); }
}

const bundleEntry = new URL('./g10-texture-bake-entry.ts', import.meta.url).pathname;
const bundle = await Bun.build({ entrypoints: [bundleEntry], target: 'browser', format: 'iife' });
assert(bundle.success, 'bundle builds');
const script = await bundle.outputs[0].text();

const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage();
  await page.setContent('<canvas></canvas>');
  await page.addScriptTag({ content: script });
  const size = 256;
  const run = () => page.evaluate(({ source, space, uvs, positions, size }) => {
    const baker = new window.Baker(new OffscreenCanvas(size, size));
    const map = baker.positionMap(new Float32Array(uvs), new Float32Array(positions), size, 8);
    const pixels = baker.bake({ source, space }, size, map);
    return Array.from(pixels);
  }, { source: compiled.source, space: compiled.space, uvs, positions, size });
  const first = Uint8Array.from(await run());
  const second = Uint8Array.from(await run());
  assert.deepEqual(second, first, 'bakes the same twice');

  const at = (x, y) => { const o = (y * size + x) * 4; return [first[o], first[o + 1], first[o + 2], first[o + 3]]; };
  let covered = 0;
  for (let i = 0; i < size * size; i++) if (first[i * 4] + first[i * 4 + 1] + first[i * 4 + 2] > 0) covered++;
  const share = covered / (size * size);
  assert(share > 0.9, `most texels are covered by the sphere (${(share * 100).toFixed(0)}%)`);
  // Seam: the texels either side of the u = 0 / u = 1 edge are the same place on the sphere, so a 3D
  // pattern must be nearly equal there. Compare with the typical step between neighbouring texels.
  let seam = 0, step = 0;
  const rows = [size >> 2, size >> 1, (3 * size) >> 2];
  for (const y of rows) {
    seam += Math.abs(at(0, y)[0] - at(size - 1, y)[0]);
    step += Math.abs(at(size >> 1, y)[0] - at((size >> 1) + 1, y)[0]);
  }
  console.log(`G10 texture bake: sphere covered ${(share * 100).toFixed(1)}%, seam step ${(seam / rows.length).toFixed(1)}, interior step ${(step / rows.length).toFixed(1)}`);
  assert(seam / rows.length < 12, 'the pattern is continuous across the UV seam');
  const png = new PNG({ width: size, height: size });
  png.data = Buffer.from(first);
  await writeFile(`${reports}texture-bake-sphere.png`, PNG.sync.write(png));
} finally {
  await browser.close();
}
