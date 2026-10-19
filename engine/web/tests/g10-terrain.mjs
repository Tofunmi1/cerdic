import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: terrain and placement. A seeded hilly ground carries trees dropped onto it. The trees are the same
// prototype scattered flat and then lowered onto the ground, so their feet follow the hills; the screenshot must show
// green ground, brown trunks and leaves, and the evaluated copies must sit at ground height (not all at one level).
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5217'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `land = terrain.heightfield@1(size=[6, 6], height=1.2, scale=3, octaves=3, seed=3, resolution=48, base=0.3)
grass = solid.material@1(land, color=[0.25, 0.55, 0.2], roughness=0.9)
output ground = grass
trunk = solid.cylinder@1(radius=0.05, height=0.3, segments=8)
bark = solid.material@1(trunk, color=[0.45, 0.28, 0.12])
crowd = inst.scatter@1(bark, count=30, width=5.6, depth=5.6, min_spacing=0.5, seed=7)
trees = inst.drop@1(crowd, land, offset=-0.03)
output forest = trees
`;
const logs = [];
const grassy = (r, g, b) => g > r + 40 && g > b + 50;
const brown = (r, g, b) => r > g + 20 && g > b + 8 && r < 200 && r > 70;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5217')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5217/?eval=1&docname=g10-terrain');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'terrain'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const forest = await page.evaluate(() => {
    const part = window.__ve.evaluation.parts.find(p => p.name === 'forest');
    return { copies: part?.transforms ? part.transforms.length / 16 : 0, heights: part?.transforms ? Array.from({ length: part.transforms.length / 16 }, (_, i) => part.transforms[i * 16 + 13]) : [] };
  });
  assert(forest.copies >= 20 && forest.copies <= 30, `most of the 30 trees are on the ground: ${forest.copies}`);
  const spread = Math.max(...forest.heights) - Math.min(...forest.heights);
  assert(spread > 0.15, `the trees stand at different heights, following the hills (spread ${spread.toFixed(2)} m)`);
  await page.waitForTimeout(1500);
  const png = PNG.sync.read(await page.screenshot({ path: `${reports}terrain.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
  let green = 0, wood = 0;
  for (let k = 0; k < png.data.length; k += 4) {
    if (grassy(png.data[k], png.data[k + 1], png.data[k + 2])) green++;
    else if (brown(png.data[k], png.data[k + 1], png.data[k + 2])) wood++;
  }
  console.log(`G10 terrain: ${forest.copies} trees, height spread ${spread.toFixed(2)} m, ${green} green and ${wood} bark pixels`);
  assert(green > 6000, `the ground is visible (${green} green pixels)`);
  assert(wood > 40, `the trunks are visible (${wood} bark pixels)`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
