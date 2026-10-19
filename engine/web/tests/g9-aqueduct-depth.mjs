import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// The stone aqueduct's base slab has its top at y = 0, the height the studio floor used to have. Two
// coplanar faces tie in the depth test, so large floor patches flipped between two looks for camera moves
// of a few millimetres. With the floor 5 mm lower, moving the camera 4 mm changes only edge pixels.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5203'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
const poses = [[0.5, 1.2, -0.6, -1.5, 0.3, 0.9], [0.504, 1.2, -0.6, -1.5, 0.3, 0.9]];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5203')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto('http://127.0.0.1:5203/?viewer=1&eval=1&model=viaduct&docname=g9-aqueduct-depth');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
    null, { timeout: 300000 });
  assert(await page.evaluate(() => document.querySelector('#v-shadows')?.checked), 'the aqueduct loads with sun shadows on');
  const frames = [];
  for (const pose of poses) {
    assert(await page.evaluate(p => window.__runtime.viewer_look_at(...p), pose), 'pose accepted');
    const start = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction(s => window.__ve.read().frames >= s + 6, start, { timeout: 60000 });
    frames.push((await page.screenshot()).toString('base64'));
  }
  const changed = await page.evaluate(async ([a, b]) => {
    const load = async data => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const canvas = new OffscreenCanvas(image.width, image.height);
      const context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      return context.getImageData(0, 0, image.width, image.height).data;
    };
    const [x, y] = [await load(a), await load(b)];
    let count = 0;
    for (let i = 0; i < x.length; i += 4) {
      if (Math.abs(x[i] - y[i]) + Math.abs(x[i + 1] - y[i + 1]) + Math.abs(x[i + 2] - y[i + 2]) > 75) count++;
    }
    return count;
  }, frames);
  console.log(`g9 aqueduct depth: ${changed} pixels differ for a 4 mm camera move`);
  assert(changed < 6000, `coplanar surfaces are flickering: ${changed} pixels changed (about 23,000 before the floor was lowered)`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
