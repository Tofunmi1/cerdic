import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// G10: a cottage written by a model (Sonnet, through the MCP helper) from a prose request: rounded beige
// walls with a cut door and a separate dark red pyramid roof, as two outputs. It must evaluate, render
// and be visible from a three-quarter view; the image lands in target/reports/g10/.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5205'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const program = await readFile(new URL('./fixtures/g10-cottage.txt', import.meta.url), 'utf8');
const lights = `
sun_main = light.sun@1(direction=[-0.4, -1, -0.3], color=[1, 0.92, 0.82], intensity=3)
output main_light = sun_main
`;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5205')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1100, height: 800 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text());
  });
  await page.goto('http://127.0.0.1:5205/?viewer=1&eval=1&fov=45&docname=g10-cottage');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(([text, name]) => window.__ve.doc.sendText(text, name), [`${program}${lights}`, 'G10 cottage']);
  assert(staged.pending, `staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
    null, { timeout: 180000 });
  // Frame the whole model the way the MCP frame_view tool does (a fixed camera can end up outside the
  // studio room's walls for a model this size).
  const framed = await page.evaluate(() => {
    const bounds = window.__ve.evaluation.bounds;
    return bounds ? window.__runtime.viewer_frame(...bounds.min, ...bounds.max) : false;
  });
  assert(framed, 'the model is framed');
  const frameStart = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction(start => window.__ve.read().frames >= start + 12, frameStart, { timeout: 30000 });
  await page.screenshot({ path: `${reports}cottage.png` });
  const triangles = await page.evaluate(() => window.__ve.read().render.main.triangles);
  console.log(`G10 cottage: ${triangles} rendered triangles`);
  assert(triangles > 200, 'the walls and the roof render');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
