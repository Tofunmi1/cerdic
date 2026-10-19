// Automatic exposure in a real browser: the manual exposure slider is the disturbance. With auto exposure off a quarter and
// four times the exposure give a dark and a blown-out picture; with it on both settle on about the same brightness.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5251;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 640, height: 420 } });
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
  });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=low&scene=stage`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await mkdir(reports, { recursive: true });
  const scene = `slab = solid.box@1(size=[6,0.2,6])
wall = solid.box@1(size=[6,3,0.2])
wall_up = xform.move@1(wall, by=[0,1.6,-2])
all = solid.union@1(slab, wall_up)
paint = solid.material@1(all, color=[0.6,0.6,0.6], roughness=0.9)
output room = paint`;
  const staged = await page.evaluate(source => window.__ve.doc.sendText(source, 'auto exposure test scene'), scene);
  assert(staged.pending, `test scene staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
      || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 90000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'test scene evaluates');
  await page.evaluate(() => window.__runtime.viewer_look_at(4.8, 3.2, 6.5, 0, 1, -1));
  await page.waitForTimeout(1500);
  // The panels sit over the canvas; hide them so a screenshot is the render alone.
  await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    document.querySelectorAll('body *').forEach(element => {
      if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
    });
  });

  // Mean sRGB brightness of the middle of the picture.
  const brightness = async () => {
    const picture = PNG.sync.read(await page.screenshot());
    let sum = 0;
    let count = 0;
    for (let y = Math.floor(picture.height * 0.45); y < picture.height * 0.7; y += 2) {
      for (let x = Math.floor(picture.width * 0.4); x < picture.width * 0.6; x += 2) {
        const at = (y * picture.width + x) * 4;
        sum += 0.2126 * picture.data[at] + 0.7152 * picture.data[at + 1] + 0.0722 * picture.data[at + 2];
        count++;
      }
    }
    return sum / count;
  };
  // Waits until two readings in a row agree (the software renderer is slow, so adaptation spans several seconds).
  const settled = async () => {
    let previous = -1000;
    for (let i = 0; i < 40; i++) {
      await page.waitForTimeout(2500);
      const now = await brightness();
      if (Math.abs(now - previous) < 1.5) return now;
      previous = now;
    }
    throw new Error('the picture never settled');
  };
  const setExposure = value => page.evaluate(v => {
    const slider = document.querySelector('#v-exposure');
    slider.value = String(v);
    slider.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
  const setAuto = on => page.evaluate(enabled => window.__runtime.set_auto_exposure(enabled), on);

  await setAuto(false);
  await setExposure(0.25);
  const darkOff = await settled();
  await setExposure(4);
  const brightOff = await settled();
  assert(brightOff - darkOff > 100, `without auto exposure the pictures differ a lot (${darkOff.toFixed(0)} and ${brightOff.toFixed(0)})`);

  await setAuto(true);
  await setExposure(0.25);
  const darkOn = await settled();
  await page.screenshot({ path: `${reports}auto-exposure-dark-start.png` });
  await setExposure(4);
  const brightOn = await settled();
  await page.screenshot({ path: `${reports}auto-exposure-bright-start.png` });
  assert(Math.abs(brightOn - darkOn) < 12, `with auto exposure both settle together (${darkOn.toFixed(0)} and ${brightOn.toFixed(0)})`);
  // About 18% grey in linear light is sRGB 118; the middle of the picture is not the whole frame, so allow a band.
  for (const value of [darkOn, brightOn]) {
    assert(value > 80 && value < 180, `the settled brightness is a mid grey (${value.toFixed(0)})`);
  }

  // Switching it off puts the exposure back at the manual value.
  await setAuto(false);
  const backOff = await settled();
  assert(Math.abs(backOff - brightOff) < 8, `off again restores the manual look (${backOff.toFixed(0)} against ${brightOff.toFixed(0)})`);
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 auto exposure: off ${darkOff.toFixed(0)}/${brightOff.toFixed(0)}, on ${darkOn.toFixed(0)}/${brightOn.toFixed(0)}, off again ${backOff.toFixed(0)}`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
