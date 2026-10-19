import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: the exported GLB's normal-mapped shading agrees with three.js's own reference path. A brick box
// (strong bevels in the height map) is exported by the engine page, then rendered by three.js with the
// file's MikkTSpace TANGENT data and again with three.js's derivative tangent frame (which, for glTF, flips
// the normal's y). If the exporter's tangent sign or the baked normal's green channel were inverted, the two
// pictures would light the bevels from opposite sides. The viewport shader uses the same derivative frame.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5210'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `box = solid.box@1(size=[1, 1, 1])
paint = solid.material@1(box, color=[1, 1, 1], roughness=0.8)
cage = poly.from_solid@1(paint)
flat = poly.merge_coplanar@1(cage)
seams = poly.select@1(flat, element="edge", by="sharp", angle=0.5)
uv = poly.unwrap_uv@1(flat, seams, method="angle", rotate=true)
solid = poly.to_solid@1(uv)
pattern = solid.texture@1(solid, program="b = bricks(columns=3, rows=6, mortar=0.12, bevel=0.1); albedo = uniform_greyscale(color=0.8); height = b")
output textured = pattern
`;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5210')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5210/?viewer=1&eval=1&docname=g10-normal-reference');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'normal reference'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const bytes = await page.evaluate(async () => Array.from(await window.__ve.doc.exportGlb()));
  await writeFile(`${reports}normal-reference.glb`, Uint8Array.from(bytes));

  const bundle = await Bun.build({ entrypoints: [new URL('./g10-normal-reference-entry.ts', import.meta.url).pathname], target: 'browser', format: 'iife' });
  assert(bundle.success, 'the three.js entry bundles');
  const viewer = await browser.newPage({ viewport: { width: 600, height: 600 } });
  viewer.on('pageerror', error => logs.push(error.message));
  await viewer.setContent('<body></body>');
  await viewer.addScriptTag({ content: await bundle.outputs[0].text() });
  const shots = {};
  for (const withTangents of [true, false]) {
    const url = await viewer.evaluate(([data, flag]) => window.renderGlb(new Uint8Array(data), flag), [bytes, withTangents]);
    shots[withTangents ? 'tangents' : 'derivatives'] = PNG.sync.read(Buffer.from(url.split(',')[1], 'base64'));
    await writeFile(`${reports}normal-reference-${withTangents ? 'tangents' : 'derivatives'}.png`, PNG.sync.write(shots[withTangents ? 'tangents' : 'derivatives']));
  }
  const a = shots.tangents, b = shots.derivatives;
  // Pixels the model covers (not the dark background), and how far the two renders differ there.
  let covered = 0, total = 0, large = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if (a.data[i] > 40 || b.data[i] > 40) {
      covered++;
      const d = Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]);
      total += d / 3;
      if (d / 3 > 12) large++;
    }
  }
  const mean = total / covered;
  console.log(`G10 normal reference: ${covered} model pixels, mean difference ${mean.toFixed(2)} of 255, ${((large / covered) * 100).toFixed(1)}% differ by more than 12`);
  assert(covered > 20000, 'the box is in view');
  assert(mean < 3, 'the file tangents and the derivative frame light the bevels the same way');
  assert(large / covered < 0.05, 'few pixels disagree strongly');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
