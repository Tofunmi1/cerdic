// Developer probe: does the picture change from frame to frame when nothing should change?
// Builds the vase, then captures consecutive canvas screenshots in three situations (still, pointer
// moving over the model, after a zoom) and counts pixels that differ between neighbours.
//   bun web/tests/flicker.mjs [program]
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { serve } from './support.mjs';

const name = process.argv[2] ?? 'vase';
const text = await readFile(new URL(`../../crates/ve-modeling/tests/gallery/${name}.txt`, import.meta.url), 'utf8');
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=flicker-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 60000 });
  await page.evaluate((t) => window.__ve.doc.sendText(t, 'flicker'), text);
  await page.waitForFunction(() => window.__ve.evaluation.state.revision === 1, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  const canvas = page.locator('canvas#ve');
  const grab = async () => PNG.sync.read(await canvas.screenshot());
  const differing = (a, b) => {
    let n = 0, biggest = 0;
    for (let i = 0; i < a.data.length; i += 4) {
      const d = Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]);
      if (d > 6) n++;
      biggest = Math.max(biggest, d);
    }
    return { pixels: n, biggest };
  };
  const run = async (label, action, count = 14) => {
    const shots = [];
    for (let i = 0; i < count; i++) { await action?.(i); shots.push(await grab()); }
    const diffs = shots.slice(1).map((s, i) => differing(shots[i], s));
    const worst = diffs.reduce((m, d) => (d.pixels > m.pixels ? d : m), { pixels: 0, biggest: 0 });
    console.log(`${label.padEnd(34)} frames=${count} changed pixels between neighbours: worst ${worst.pixels}, largest step ${worst.biggest}, of ${shots[0].width * shots[0].height}`);
    return diffs;
  };
  await run('still (nothing moves)');
  await run('pointer sweeping across the model', (i) => page.mouse.move(250 + i * 25, 330 + (i % 3) * 15));
  await page.mouse.move(450, 350);
  await run('wheel zoom steps', (i) => page.mouse.wheel(0, i % 2 ? 120 : -120));
  await page.evaluate(() => window.__ve.read());
  const status = await page.evaluate(() => { const s = window.__ve.read(); return { frames: s.frames, quality: s.quality, perf: s.perf }; });
  console.log('status', JSON.stringify(status).slice(0, 300));
} finally {
  await browser.close(); server.close();
}
