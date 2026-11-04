import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// D5: a live liquid. A 0.4 x 0.5 x 0.36 m column of water (about 500 particles) collapses into a 1 m wide tank. The runtime's read-out shows the particles
// stay in the tank, settle, and keep a surface of triangles; the picture shows a blue body of water that has spread across the tank.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/d5/', import.meta.url).pathname;
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[12, 0.2, 6])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
water = fluid.sim@1(min=[0.02, 0.0, 0.02], max=[0.42, 0.5, 0.38], spacing=0.05, bounds_min=[0, 0, 0], bounds_max=[1, 1, 0.4], max_triangles=4000)
sea = solid.material@1(water, color=[0.1, 0.35, 0.95], roughness=0.15)
output water = sea
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5219', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5219')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5219/?viewer=1&eval=1&docname=d5-fluid');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'fluid'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  assert(await page.evaluate(() => Boolean(window.__ve.evaluation.parts.find(p => p.name === 'water')?.fluid)), 'the part is a liquid');
  await page.evaluate(() => window.__runtime.viewer_look_at(0.5, 0.5, 2.6, 0.5, 0.35, 0));
  await page.keyboard.press('h');
  const read = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_fluid()).fluids[0]);
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_fluid()).fluids.length === 1, null, { timeout: 30000 });
  const start = await read();
  await page.screenshot({ path: `${reports}water-start.png` });
  await page.waitForTimeout(10000);
  const end = await read();
  const png = PNG.sync.read(await page.screenshot({ path: `${reports}water-settled.png` }));
  let blue = 0, x0 = 1e9, x1 = 0;
  for (let y = 300; y < 530; y++) for (let x = 0; x < 530; x++) {
    const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
    if (b > 150 && r < 120 && b > g + 40) { blue++; x0 = Math.min(x0, x); x1 = Math.max(x1, x); }
  }
  console.log('D5 fluid start:', JSON.stringify(start));
  console.log('D5 fluid after 10 s:', JSON.stringify(end), `blue pixels ${blue} (x ${x0}..${x1})`);
  assert(start.particles > 400 && end.particles === start.particles, `the particles are kept (${start.particles} then ${end.particles})`);
  assert(end.steps > 40, 'the solver has run many frames');
  assert(end.min[0] >= -0.001 && end.max[0] <= 1.001 && end.min[1] >= -0.001 && end.max[1] <= 1.001 && end.min[2] >= -0.001 && end.max[2] <= 0.401, 'the liquid stays in the tank');
  assert(end.triangles > 300 && end.triangles <= 4000, `the surface is meshed (${end.triangles} triangles)`);
  assert(end.max[1] < start.max[1] - 0.05, `the column has collapsed (top ${start.max[1].toFixed(2)} to ${end.max[1].toFixed(2)})`);
  assert(end.fastest < 3.0, `it has mostly come to rest (${end.fastest.toFixed(2)} m/s)`);
  assert(blue > 1500, 'the picture shows a body of blue water');
  console.log('D5 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
