import { chromium } from 'playwright';

// Loads a demo or model by name in the stage viewer and prints the evaluation failure (if any), the warnings and each part's triangles.
// Usage: bun engine/web/tests/probe.mjs <model> [base url]
const model = process.argv[2] ?? 'village_hd';
const base = process.argv[3] ?? 'http://127.0.0.1:5200';
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  await page.addInitScript(() => { window.__veUploadTimeoutMs = 600000; });
  await page.goto(`${base}/?viewer=1&scene=stage&eval=1&model=${model}&docname=probe-${model}-${Date.now()}`);
  const done = await page.waitForFunction(
    () => window.__ve?.evaluation?.state?.failureCurrent || (window.__ve?.evaluation?.parts?.length > 0),
    null, { timeout: 120000 }).then(() => true, () => false);
  const report = await page.evaluate(() => {
    const evaluation = window.__ve?.evaluation;
    return {
      failure: evaluation?.state?.lastFailure ?? null,
      warnings: (evaluation?.state?.warnings ?? []).map(w => `${w.code}: ${w.message}`.slice(0, 200)),
      parts: (evaluation?.parts ?? []).map(p => `${p.name}${p.transforms ? ` x${p.transforms.length / 16}` : ''}: ${p.indices.length / 3}`),
    };
  });
  console.log(JSON.stringify({ done, ...report }, null, 1));
} finally {
  await browser.close();
}
