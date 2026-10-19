// Global illumination in a real browser: a closed room with one window. With constant sky ambient the far wall is as bright as
// anywhere; with light probes (`gi=true` on scene.environment) the sky only reaches the room through the window, so the far wall
// goes darker, and the scene brightens on the lit side once the probes have been traced.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5253;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
const room = gi => `outer = solid.box@1(size=[6, 3, 6])
inner = solid.box@1(size=[5.6, 2.8, 5.6])
shell = solid.difference@1(outer, inner)
shell_up = xform.move@1(shell, by=[0, 1.5, 0])
window = solid.box@1(size=[1, 1, 1])
window_at = xform.move@1(window, by=[0, 1.5, 3])
open_shell = solid.difference@1(shell_up, window_at)
walls = solid.material@1(open_shell, color=[0.8, 0.8, 0.8], roughness=0.9)
output walls = walls
env = scene.environment@1(sky_color=[0.7, 0.8, 1.0], ground_color=[0.3, 0.3, 0.3], intensity=1.0, gi=${gi}, gi_spacing=0.5)
output env = env`;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const measure = async (gi) => {
    const page = await browser.newPage({ viewport: { width: 640, height: 420 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=high&scene=stage`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([source, label]) => window.__ve.doc.sendText(source, label), [room(gi), `room-${gi}`]);
    assert(staged.pending, `staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'room evaluates');
    if (gi) {
      await page.waitForFunction(
        () => { const s = JSON.parse(window.__runtime.eval_projection_status()); return s.live_environment?.gi && s.gi_progress == null; },
        null, { timeout: 120000 });
    }
    await page.evaluate(() => window.__runtime.viewer_look_at(0, 1.5, 2.4, 0, 1.5, -2.8));
    await page.waitForTimeout(3500);
    await page.evaluate(() => {
      const canvas = document.querySelector('canvas');
      document.querySelectorAll('body *').forEach(element => {
        if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
      });
    });
    await page.waitForTimeout(400);
    const image = PNG.sync.read(await page.screenshot({ path: `${reports}gi-${gi}.png` }));
    await page.close();
    // The far wall: the middle of the picture.
    let sum = 0; let count = 0;
    for (let y = Math.floor(image.height * 0.4); y < image.height * 0.6; y++) {
      for (let x = Math.floor(image.width * 0.4); x < image.width * 0.6; x++) {
        const at = (y * image.width + x) * 4;
        sum += (image.data[at] + image.data[at + 1] + image.data[at + 2]) / 3;
        count++;
      }
    }
    return sum / count;
  };
  const flat = await measure(false);
  const probes = await measure(true);
  assert(flat > 30, `the room is visible with the constant ambient (${flat.toFixed(1)})`);
  assert(probes < 0.8 * flat, `light probes darken the far wall away from the window (${probes.toFixed(1)} against ${flat.toFixed(1)})`);
  assert(probes > 3, `...but it is not black (${probes.toFixed(1)})`);
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 gi: far wall brightness ${flat.toFixed(1)} with constant ambient, ${probes.toFixed(1)} with light probes`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
