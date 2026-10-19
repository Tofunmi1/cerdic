import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: grab triggers (ADR 0029). A cube with a `hold` behaviour cannot be picked up in edit mode; in play mode a
// mouse drag carries it, sets a flag while it is held and clears the flag when it is let go. It stays where it was
// dropped, and leaving play mode puts it back where it was authored.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5215'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
const program = `cube = solid.box@1(size=[0.3, 0.3, 0.3])
gold = solid.material@1(cube, color=[0.9, 0.7, 0.05], roughness=0.5)
lifted = xform.move@1(gold, by=[0, 0.4, 0])
pick = behavior.on@1(lifted, trigger="hold", sets="holding")
output gold_cube = pick
`;
const gold = (r, g, b) => r > 150 && g > 100 && b < 90 && r > b + 100;
const where = png => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (gold(png.data[k], png.data[k + 1], png.data[k + 2])) { x += i; y += j; n++; }
  }
  assert(n > 100, `the cube is visible (${n} gold pixels)`);
  return { x: x / n, y: y / n };
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5215')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5215/?eval=1&docname=g10-grab');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'grab'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 300)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const shot = async name => PNG.sync.read(await page.screenshot({ path: `${reports}grab-${name}.png` }));
  const flags = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()).flags);
  await page.waitForTimeout(800);
  const start = where(await shot('start'));

  // Edit mode: a drag does not carry the part.
  await page.mouse.move(start.x, start.y); await page.waitForTimeout(500);
  await page.mouse.down(); await page.waitForTimeout(300);
  for (let i = 1; i <= 6; i++) { await page.mouse.move(start.x + i * 12, start.y, { steps: 2 }); await page.waitForTimeout(150); }
  await page.mouse.up(); await page.waitForTimeout(500);
  const dragged = where(await shot('edit-drag'));
  assert(Math.hypot(dragged.x - start.x, dragged.y - start.y) < 4, 'authored parts cannot be dragged around in edit mode');

  // Play mode: the drag carries it and the flag follows the hand.
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  await page.waitForTimeout(500);
  await page.mouse.move(start.x, start.y); await page.waitForTimeout(500);
  await page.mouse.down(); await page.waitForTimeout(400);
  for (let i = 1; i <= 8; i++) { await page.mouse.move(start.x + i * 12, start.y, { steps: 2 }); await page.waitForTimeout(150); }
  await page.waitForTimeout(500);
  assert.equal((await flags()).holding, true, 'the flag is on while the cube is held');
  const carried = where(await shot('carried'));
  console.log(`G10 grab: the cube moved ${(carried.x - start.x).toFixed(0)} px while held`);
  assert(carried.x - start.x > 30, 'the cube follows the pointer');
  await page.mouse.up(); await page.waitForTimeout(700);
  assert.equal((await flags()).holding, false, 'letting go clears the flag');
  const dropped = where(await shot('dropped'));
  assert(Math.hypot(dropped.x - carried.x, dropped.y - carried.y) < 12, 'it stays where it was dropped');

  // Stop: everything goes back.
  await page.evaluate(() => window.__runtime.viewer_play_mode(false));
  await page.waitForFunction(() => !JSON.parse(window.__runtime.viewer_behaviors()).play);
  await page.waitForTimeout(700);
  const home = where(await shot('home'));
  assert(Math.hypot(home.x - start.x, home.y - start.y) < 4, 'leaving play mode puts the cube back');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
