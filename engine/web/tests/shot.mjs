import { chromium } from 'playwright';

// Loads a model in the stage viewer and saves a screenshot, optionally with viewer options toggled off first.
// Usage: bun engine/web/tests/shot.mjs <model> <out.png> [--no-shadows] [--play] [--base http://127.0.0.1:5200]
const [model = 'village_hd', out = 'target/reports/g10/shot.png', ...flags] = process.argv.slice(2);
const base = flags.includes('--base') ? flags[flags.indexOf('--base') + 1] : 'http://127.0.0.1:5200';
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  await page.addInitScript(() => { window.__veUploadTimeoutMs = 600000; });
  await page.goto(`${base}/?viewer=1&scene=stage&eval=1&model=${model}&docname=shot-${model}-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 600000 });
  if (flags.includes('--no-shadows')) await page.getByLabel('Sun shadows').uncheck();
  await page.waitForTimeout(4000);
  const path = new URL(`../../${out}`, import.meta.url).pathname;
  await page.screenshot({ path });
  console.log(`saved ${path}`);
} finally {
  await browser.close();
}
