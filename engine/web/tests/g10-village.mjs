import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// G10: the village demo loads with no warnings, shows textured scenery from the editor camera, and in play mode the
// viewer stands on the plaza and sees it from eye level. Screenshots go to target/reports/g10 for a look.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5222'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
const model = process.argv[2] ?? 'village';
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5222')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  // The grass texture bakes on the test browser's software GPU: allow it ten minutes to upload.
  const mapSize = Number(process.env.VE_MAP_SIZE ?? 1024);
  await page.addInitScript(size => { window.__veUploadTimeoutMs = 600000; window.__veMapSize = size; }, mapSize);
  await page.goto(`http://127.0.0.1:5222/?viewer=1&scene=stage&eval=1&model=${model}&docname=g10-${model}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 }).catch(error => {
    console.log(`page never became ready; browser said: ${JSON.stringify(logs).slice(0, 1500)}`);
    throw error;
  });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 560000 });
  const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  assert.equal(failure, null, `evaluates: ${JSON.stringify(failure)}`);
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => `${w.code}: ${w.message}`));
  assert.deepEqual(warnings, [], 'no warnings');
  await page.waitForTimeout(2500);
  await page.screenshot({ path: `${reports}${model}-overview.png` });
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).walker?.grounded === true, null, { timeout: 90000, polling: 250 }).catch(async error => {
    const world = await page.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
    console.log(`not grounded: active=${world.active} steps=${world.steps} bodies=${world.bodies.length} walker=${JSON.stringify(world.walker)} refused=${JSON.stringify(world.refused)}`);
    throw error;
  });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${reports}${model}-walk.png` });
  const world = await page.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  // The hinged door must be at rest, not jittering against the wall, lintel or floor.
  const doorNode = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'door').node);
  const doorAt = async () => (await page.evaluate(() => JSON.parse(window.__runtime.viewer_physics()))).bodies.find(b => b.node === doorNode);
  const before = await doorAt();
  await page.waitForTimeout(1500);
  const after = await doorAt();
  const drift = Math.hypot(...after.rotation.map((v, i) => v - before.rotation[i]));
  assert(Math.hypot(...after.speed) < 0.05 && drift < 0.01, `the door is at rest (speed ${Math.hypot(...after.speed).toFixed(3)}, turned ${drift.toFixed(4)})`);
  console.log(`G10 village: ${world.bodies.length} bodies, walker at ${world.walker.position.map(v => v.toFixed(2))}`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
