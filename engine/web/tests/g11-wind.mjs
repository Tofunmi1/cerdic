// Wind in a real browser: a tall green card with `solid.foliage` sway bends downwind as the shared animation clock runs, while the
// same card without sway stays put. The card's top and bottom are measured at two clock times.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5254;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
const scene = (tail, strength) => `slab = solid.box@1(size=[0.8, 2.5, 0.05])
card = xform.move@1(slab, by=[0, 1.25, 0])
paint = solid.material@1(card, color=[0.1, 0.8, 0.1], roughness=0.8)
${tail}
env = scene.environment@1(sky_color=[0.7, 0.8, 1.0], ground_color=[0.4, 0.4, 0.4], intensity=1.5, wind_direction=0, wind_strength=${strength})
output env = env`;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  // Green pixels' mean x in the top and bottom thirds of the picture, at the given clock time.
  const photograph = async (name, source, times) => {
    const page = await browser.newPage({ viewport: { width: 640, height: 420 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=high&scene=stage`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(([text, label]) => window.__ve.doc.sendText(text, label), [source, name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${name} evaluates`);
    await page.evaluate(() => window.__runtime.viewer_look_at(0, 1.25, 3.8, 0, 1.25, 0));
    await page.waitForTimeout(2500);
    await page.evaluate(() => {
      const canvas = document.querySelector('canvas');
      document.querySelectorAll('body *').forEach(element => {
        if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
      });
    });
    const results = [];
    for (const seconds of times) {
      await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
      await page.waitForTimeout(700);
      const image = PNG.sync.read(await page.screenshot({ path: `${reports}wind-${name}-${seconds}.png` }));
      const band = (from, to) => {
        let sum = 0; let count = 0;
        for (let y = Math.floor(image.height * from); y < image.height * to; y++) {
          for (let x = 0; x < image.width; x++) {
            const at = (y * image.width + x) * 4;
            if (image.data[at + 1] > image.data[at] + 40 && image.data[at + 1] > image.data[at + 2] + 40) { sum += x; count++; }
          }
        }
        return count ? sum / count : NaN;
      };
      results.push({ seconds, top: band(0.0, 0.3), bottom: band(0.7, 1.0) });
    }
    await page.close();
    return results;
  };

  const still = await photograph('still', scene('output card = paint', 2), [1, 4]);
  const swaying = await photograph('sway', scene('leaf = solid.foliage@1(paint, sway=1)\noutput card = leaf', 2), [1, 4]);
  for (const r of [...still, ...swaying]) assert(Number.isFinite(r.top) && Number.isFinite(r.bottom), `the card is on screen: ${JSON.stringify(r)}`);
  const move = rows => Math.abs(rows[0].top - rows[1].top);
  assert(move(still) < 0.5, `a card without sway does not move (${move(still).toFixed(2)}px)`);
  assert(move(swaying) > 8, `a swaying card's top moves with the clock (${move(swaying).toFixed(2)}px)`);
  const bottomMove = Math.abs(swaying[0].bottom - swaying[1].bottom);
  assert(bottomMove < move(swaying) / 3, `the base stays put while the top moves (${bottomMove.toFixed(2)}px at the base)`);
  // Downwind is +x, to the right of the picture, for the bend relative to the still card's top.
  const leaned = Math.max(swaying[0].top, swaying[1].top) - still[0].top;
  assert(leaned > 4, `it leans downwind at the worst moment (${leaned.toFixed(2)}px right of rest)`);
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 wind: still top moves ${move(still).toFixed(2)}px, swaying top ${move(swaying).toFixed(2)}px, its base ${bottomMove.toFixed(2)}px, leans ${leaned.toFixed(1)}px downwind`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
