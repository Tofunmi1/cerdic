// Metals and glossy paint reflect the studio sky in the viewer's stage scene: the steel gear is lit
// and shows a gradient across its top (a dark disc before the reflection existed), the red vase shows
// bright highlights, and a part's colour still decides what it looks like.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { serve } from './support.mjs';

const gallery = (name) => readFile(new URL(`../../crates/ve-modeling/tests/gallery/${name}.txt`, import.meta.url), 'utf8');
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });

// Luminance statistics of the middle of the canvas, where the model sits.
const centre = (png) => {
  const values = [];
  let whitest = 0; // the most colourless pixel: a highlight on red paint has green and blue in it
  for (let y = Math.floor(png.height * 0.2); y < png.height * 0.7; y++) {
    for (let x = Math.floor(png.width * 0.35); x < png.width * 0.65; x++) {
      const p = (y * png.width + x) * 4;
      values.push(0.2126 * png.data[p] + 0.7152 * png.data[p + 1] + 0.0722 * png.data[p + 2]);
      whitest = Math.max(whitest, Math.min(png.data[p + 1], png.data[p + 2]));
    }
  }
  const mean = values.reduce((a, b) => a + b, 0) / values.length;
  return { mean, whitest, spread: Math.sqrt(values.reduce((a, b) => a + (b - mean) ** 2, 0) / values.length) };
};

try {
  const shoot = async (name) => {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=reflection-${name}-${Date.now()}`);
    await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
    await page.evaluate((t) => window.__ve.doc.sendText(t, 'reflection'), await gallery(name));
    await page.waitForFunction(() => window.__ve.evaluation.state.revision === 1, null, { timeout: 60000 });
    await page.waitForTimeout(2000);
    const png = PNG.sync.read(await page.locator('canvas#ve').screenshot());
    await page.close();
    assert.deepEqual(errors, [], `${name}: no page errors`);
    return centre(png);
  };

  const gear = await shoot('gear');
  assert(gear.mean > 70, `the steel gear is lit, not a black disc (mean ${gear.mean.toFixed(1)})`);
  assert(gear.spread > 12, `the steel gear shows the sky across it (spread ${gear.spread.toFixed(1)})`);
  const vase = await shoot('ceramic_vase');
  assert(vase.whitest > 170, `the glossy vase shows a bright highlight (whitest ${vase.whitest})`);
  console.log('reflection test passed', JSON.stringify({ gear, vase }));
} finally {
  await browser.close();
  server.close();
}
