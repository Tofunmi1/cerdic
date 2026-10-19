import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: sockets (ADR 0029). A gold key with a `hold` behaviour is carried by the mouse. A lock with a `socket`
// behaviour accepts it: let go near the lock and the key snaps onto it and the lock sets the flag `unlocked`; pick it
// out again and the flag clears. Let go far from the lock and nothing snaps.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5218'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `key = solid.box@1(size=[0.2, 0.2, 0.2])
brass = solid.material@1(key, color=[0.9, 0.7, 0.05], roughness=0.5)
key_at = xform.move@1(brass, by=[-0.7, 0.3, 0])
carry = behavior.on@1(key_at, trigger="hold", sets="carried")
output key_out = carry
lock = solid.box@1(size=[0.3, 0.3, 0.05])
steel = solid.material@1(lock, color=[0.1, 0.3, 0.9], roughness=0.4)
lock_at = xform.move@1(steel, by=[0.5, 0.3, 0])
fits = behavior.on@1(lock_at, trigger="socket", accepts="key_out", radius=0.4, sets="unlocked")
output lock_out = fits
`;
const logs = [];
const gold = (r, g, b) => r > 150 && g > 100 && b < 90 && r > b + 100;
const blue = (r, g, b) => b > r + 100 && b > g + 50;
const where = (png, match, what) => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (match(png.data[k], png.data[k + 1], png.data[k + 2])) { x += i; y += j; n++; }
  }
  assert(n > 60, `the ${what} is visible (${n} pixels)`);
  return { x: x / n, y: y / n };
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5218')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5218/?eval=1&docname=g10-socket');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'socket'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const shot = async name => PNG.sync.read(await page.screenshot({ path: `${reports}socket-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
  const flags = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()).flags);
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  await page.waitForTimeout(800);

  const start = await shot('start');
  const key = where(start, gold, 'key'), lock = where(start, blue, 'lock');
  const drag = async (from, to, hold = false) => {
    await page.mouse.move(from.x, from.y); await page.waitForTimeout(500);
    await page.mouse.down(); await page.waitForTimeout(400);
    for (let i = 1; i <= 8; i++) {
      await page.mouse.move(from.x + ((to.x - from.x) * i) / 8, from.y + ((to.y - from.y) * i) / 8, { steps: 2 });
      await page.waitForTimeout(150);
    }
    await page.waitForTimeout(300);
    if (!hold) { await page.mouse.up(); await page.waitForTimeout(700); }
  };

  // Let go far from the lock: nothing snaps and the flag stays off.
  await drag(key, { x: key.x + 40, y: key.y });
  assert.notEqual((await flags()).unlocked, true, 'too far from the lock to dock');
  const nearMiss = where(await shot('miss'), gold, 'key');
  assert(Math.abs(nearMiss.x - key.x - 40) < 14, 'the key stays where it was dropped');

  // Carry it to the lock and let go: it snaps onto it and the lock fires.
  // It is let go 43 px off the lock's centre (inside the socket's radius), so only the snap can put it on the centre.
  await drag(nearMiss, { x: lock.x + 35, y: lock.y + 25 });
  assert.equal((await flags()).unlocked, true, 'docking in the socket sets the flag');
  const docked = where(await shot('docked'), gold, 'key');
  const off = Math.hypot(docked.x - lock.x, docked.y - lock.y);
  console.log(`G10 socket: let go 43 px off, the key ended ${off.toFixed(1)} px from the lock centre`);
  assert(off < 20, 'the key snapped onto the lock');

  // Pick it out again: the flag clears.
  await drag(docked, { x: docked.x - 160, y: docked.y + 40 });
  assert.notEqual((await flags()).unlocked, true, 'taking the key out clears the flag');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
