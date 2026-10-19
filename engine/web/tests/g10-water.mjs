import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// A deterministic lake slab is evaluated over a flat_below terrain basin. Paused viewer time lets
// the transparent and opaque captures be compared without animation noise.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const port = 5231;
const server = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
const program = `land = terrain.heightfield@1(size=[14,8], height=1.4, scale=4, octaves=3, roughness=0.5, seed=12, resolution=48, base=0.5, flat_below=0.35)
grass = solid.material@1(land, color=[0.22,0.48,0.18], roughness=0.9)
output basin = grass
raw_transparent = water.surface@1(size=[5,6], level=0.65, depth=0.55, color=[0.04,0.32,0.48], opacity=0.55, wave_height=0.025, wave_length=2.5, speed=0.7, seed=6, resolution=48)
transparent_lake = xform.move@1(raw_transparent, by=[-3,0,0])
output transparent_lake = transparent_lake
raw_opaque = water.surface@1(size=[5,6], level=0.65, depth=0.55, color=[0.04,0.32,0.48], opacity=1, wave_height=0.025, wave_length=2.5, speed=0.7, seed=6, resolution=48)
opaque_lake = xform.move@1(raw_opaque, by=[3,0,0])
output opaque_lake = opaque_lake
sky = scene.sky@1(sun_elevation=28, sun_azimuth=-35, clouds=false)
output sky = sky`;
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1000, height: 760 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=stage&docname=g10-water`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const result = await page.evaluate(text => window.__ve.doc.sendText(text, 'water'), program);
  assert(result.pending, `staged: ${JSON.stringify(result)}`);
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'lake basin evaluates');
  assert(await page.evaluate(() => window.__runtime.viewer_look_at(0, 4.4, 7.5, 0, 0.55, 0)), 'angled lake review camera accepted');
  await page.evaluate(() => window.__runtime.viewer_animation(false, 0));
  await page.waitForTimeout(400);
  const canvas = page.locator('canvas').first();
  const image = PNG.sync.read(await canvas.screenshot({ path: `${reports}water-lake-pair.png` }));
  let transparentBed = 0, opaqueBed = 0;
  for (let y = Math.floor(image.height * 0.1); y < image.height * 0.9; y++) {
    for (let x = Math.floor(image.width * 0.1); x < image.width * 0.9; x++) {
      const i = (y * image.width + x) * 4;
      const green = image.data[i + 1] > image.data[i] * 1.22 && image.data[i + 1] > image.data[i + 2] * 1.05;
      if (green && x < image.width * 0.5) transparentBed++;
      if (green && x >= image.width * 0.5) opaqueBed++;
    }
  }
  const info = await page.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.filter(p => p.name.includes('lake')).map(p => [p.name,{ triangles:p.indices.length/3, opacity:p.material?.opacity }])));
  console.log(`G10 water: transparent ${info.transparent_lake.triangles} triangles opacity ${info.transparent_lake.opacity}; opaque opacity ${info.opaque_lake.opacity}; grassy pixels ${transparentBed} vs ${opaqueBed}`);
  assert(info.transparent_lake.triangles > 4000, 'water carries a regular high-detail surface grid');
  assert(Math.abs(info.transparent_lake.opacity - 0.55) < 1e-5, 'transparent lake carries its water material');
  assert.equal(info.opaque_lake.opacity, 1, 'opaque lake comparison is actually opaque');
  assert(transparentBed > opaqueBed + 1000, `green basin pixels are visible through water (${transparentBed} vs ${opaqueBed})`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); server.kill(); await server.exited; }
