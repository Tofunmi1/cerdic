import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// G10: the PBR material kit. Stages a set of surfaces (brick wall, stone, planks, plaster, roof tile) as texture
// programs and takes a screenshot, so the look of the kit can be judged by eye. Fails if any program is refused.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5223'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
const file = process.argv[2] ?? 'engine/web/demos/materials.txt';
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5223')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const text = await Bun.file(`${root}${file}`).text();
  const page = await browser.newPage({ viewport: { width: 1400, height: 800 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  // Baking texture maps on the software GPU of the test browser is slow: allow ten minutes for the upload.
  const mapSize = Number(process.env.VE_MAP_SIZE ?? 1024);
  await page.addInitScript(size => { window.__veUploadTimeoutMs = 600000; window.__veMapSize = size; }, mapSize);
  // A fresh document each run: a saved one with the same name would be loaded again instead of the file.
  await page.goto(`http://127.0.0.1:5223/?eval=1&docname=g10-pbr-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(t => window.__ve.doc.sendText(t, 'pbr'), text);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 600)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 540000 });
  const failure = await page.evaluate(() => window.__ve.evaluation.state.lastFailure);
  assert.equal(failure, null, `evaluates: ${JSON.stringify(failure)}`);
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => `${w.code}: ${w.message}`));
  console.log(`G10 pbr: warnings ${JSON.stringify(warnings)}`);
  console.log(`G10 pbr: state ${await page.evaluate(() => JSON.stringify(Object.fromEntries(Object.entries(window.__ve.evaluation.state).filter(([key]) => !['parts', 'warnings'].includes(key)))).slice(0, 1500))}`);
  await page.evaluate(() => {
    window.__runtime.viewer_configure(true, 45, false, 2);
    window.__runtime.viewer_frame(-2.7, 0, -0.2, 2.7, 3.4, 0.2);
  });
  await page.waitForTimeout(8000);
  await page.screenshot({ path: `${reports}pbr.png` });
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
