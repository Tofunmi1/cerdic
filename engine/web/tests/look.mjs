// Developer helper: opens the viewer with evaluation on, builds a gallery program from text and saves
// a screenshot of the whole page.   bun web/tests/look.mjs [program] [out.png]
//   program: vase (default), twisted_tower or offset_plate
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { serve } from './support.mjs';

const [name = 'vase', out = `${name}-viewer.png`] = process.argv.slice(2);
const text = await readFile(new URL(`../../crates/ve-modeling/tests/gallery/${name}.txt`, import.meta.url), 'utf8');
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=look-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 60000 });
  await page.evaluate((t) => window.__ve.doc.sendText(t, 'look'), text);
  await page.waitForFunction(() => window.__ve.evaluation.state.revision === 1, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: out });
  console.log('saved', out);
} finally {
  await browser.close(); server.close();
}
