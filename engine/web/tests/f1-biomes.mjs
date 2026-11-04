import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// F1: biome blending. One terrain.tile is sorted into forest (low, gentle, damp), meadow (gentle, dry) and rock (steep) by terrain.biomes; three
// scatter.place nodes plant dark-green cones, yellow tufts and grey boulders by those layers. The picture must show all three kinds, in places that follow the
// ground, and be looked at by eye.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/f1/', import.meta.url).pathname;
const program = `sky = scene.sky@1(sun_elevation=45, sun_azimuth=30, sun_intensity=4, clouds=false)
output sky = sky
land = terrain.tile@1(size=[200, 200], cell=2, seed=11, height=70, scale=100, octaves=4)
b = terrain.biomes@1(land, rules="forest: slope 0..22, wet 0.45..1; meadow: slope 0..22, wet 0..0.55; rock: slope 24..", blend=0.5, wet_scale=90, seed=4)
ground = solid.material@1(b, color=[0.42, 0.5, 0.3], roughness=0.95)
output ground = ground
tree = solid.cone@1(radius=1.2, height=4.5, segments=10)
tree_m = solid.material@1(tree, color=[0.05, 0.3, 0.1], roughness=0.8)
trees = scatter.place@1(tree_m, b, layer="biome_forest", radius=3.2, seed=1)
output trees = trees
tuft = solid.box@1(size=[0.5, 0.6, 0.5])
tuft_m = solid.material@1(tuft, color=[0.85, 0.75, 0.2], roughness=0.9)
tufts = scatter.place@1(tuft_m, b, layer="biome_meadow", radius=1.6, seed=2)
output tufts = tufts
boulder = solid.sphere@1(radius=1.1, segments=10)
boulder_m = solid.material@1(boulder, color=[0.45, 0.45, 0.48], roughness=0.9)
boulders = scatter.place@1(boulder_m, b, layer="biome_rock", radius=3.5, seed=3, sink=0.3)
output boulders = boulders
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5225', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5225')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5225/?viewer=1&eval=1&docname=f1-biomes');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'biomes'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 600)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 300000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const counts = await page.evaluate(() => Object.fromEntries(['trees', 'tufts', 'boulders'].map(n => [n, window.__ve.evaluation.parts.find(p => p.name === n)?.transforms?.length / 16 || 0])));
  console.log('F1 biomes: copies planted', JSON.stringify(counts));
  assert(counts.trees > 40 && counts.tufts > 40 && counts.boulders > 5, 'all three biomes planted something');
  await page.evaluate(() => window.__runtime.viewer_look_at(100, 90, -60, 100, 15, 100));
  await page.keyboard.press('h');
  await page.waitForTimeout(4000);
  const png = PNG.sync.read(await page.screenshot({ path: `${reports}biomes.png` }));
  const seen = { green: 0, yellow: 0, grey: 0 };
  for (let y = 60; y < 520; y += 2) for (let x = 0; x < 530; x += 2) {
    const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
    if (g > r + 25 && g > b + 25 && g < 140) seen.green++;
    if (r > 150 && g > 120 && b < 90) seen.yellow++;
    if (Math.abs(r - g) < 14 && Math.abs(g - b) < 18 && r > 70 && r < 150) seen.grey++;
  }
  console.log('F1 biomes: pixels', JSON.stringify(seen));
  assert(seen.green > 300 && seen.yellow > 30 && seen.grey > 100, 'the picture shows all three kinds');
  console.log('F1 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
