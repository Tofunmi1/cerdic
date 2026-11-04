import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// D6 (spike): fracture. A wall 1.6 x 1 x 0.3 m is broken into 8 Voronoi pieces (poly.fracture, one node each), lifted 1 m and given dynamic convex
// physics bodies. In play mode the pieces fall onto the floor, collide, and come to rest; the picture shows several separate pieces lying about.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/d6/', import.meta.url).pathname;
let program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[8, 0.2, 8])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
floor_b = phys.body@1(floor_p, kind="static", shape="box")
output floor = floor_b
wall = solid.box@1(size=[1.6, 1.0, 0.3])
lifted = xform.move@1(wall, by=[0, 1.5, 0])
`;
const colours = [[0.9, 0.2, 0.15], [0.2, 0.5, 0.9], [0.9, 0.7, 0.1], [0.3, 0.75, 0.3], [0.7, 0.3, 0.8], [0.95, 0.5, 0.2], [0.2, 0.8, 0.8], [0.8, 0.8, 0.8]];
for (let k = 0; k < 8; k++) {
  program += `f${k} = poly.fracture@1(lifted, pieces=8, piece=${k}, voxel_size=0.05, gap=0.02)
c${k} = solid.material@1(f${k}, color=[${colours[k].join(', ')}], roughness=0.6)
b${k} = phys.body@1(c${k}, kind="dynamic", shape="convex", density=900)
output piece${k} = b${k}
`;
}
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5220', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5220')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5220/?viewer=1&eval=1&docname=d6-fracture');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'fracture'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 240000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const pieces = await page.evaluate(() => window.__ve.evaluation.parts.filter(p => p.name.startsWith('piece') && p.body).length);
  console.log(`D6 fracture: ${pieces} pieces reached the page with physics bodies`);
  assert(pieces >= 6, 'most pieces exist');
  await page.evaluate(() => window.__runtime.viewer_look_at(0, 1.2, 4.2, 0, 0.6, 0));
  await page.keyboard.press('h');
  await page.screenshot({ path: `${reports}fracture-before.png` });
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  await page.waitForFunction(() => {
    const w = JSON.parse(window.__runtime.viewer_physics());
    return w.active && w.steps > 200 && w.bodies.filter(b => b.kind !== 'Static').every(b => Math.hypot(...b.speed) < 0.05);
  }, null, { timeout: 60000 });
  const world = await page.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const dynamic = world.bodies.filter(b => b.kind !== 'Static');
  const heights = dynamic.map(b => b.position[1]);
  const spread = Math.max(...dynamic.map(b => b.position[0])) - Math.min(...dynamic.map(b => b.position[0]));
  console.log(`D6 fracture: ${dynamic.length} dynamic pieces at rest, heights ${Math.min(...heights).toFixed(2)}..${Math.max(...heights).toFixed(2)} m, spread ${spread.toFixed(2)} m, steps ${world.steps}`);
  assert.equal(dynamic.length, pieces, 'every piece is a body in the world');
  assert(Math.max(...heights) < 0.9, 'the pieces came down from 1.5 m to the floor');
  assert(spread > 0.5, 'the wall has spread out into separate pieces');
  const png = PNG.sync.read(await page.screenshot({ path: `${reports}fracture-after.png` }));
  const seen = new Set();
  for (let y = 250; y < 560; y += 2) for (let x = 0; x < 530; x += 2) {
    const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
    colours.forEach((c, k) => { if (Math.abs(r - c[0] * 255) < 70 && Math.abs(g - c[1] * 255) < 70 && Math.abs(b - c[2] * 255) < 70 && Math.abs(r - g) + Math.abs(g - b) > 60) seen.add(k); });
  }
  console.log(`D6 fracture: ${seen.size} differently coloured pieces visible`);
  assert(seen.size >= 4, 'several coloured pieces are visible lying on the floor');
  console.log('D6 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
