import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';

// E1/E2 look-see: runs the unit world on the Bevy WebGPU bundle and saves pictures of the moving unit, so the skinning lag, the left
// toes and the toon outline can be judged by eye. Usage: bun web/tests/e1-unit-look.mjs [world] [seconds-between-shots]
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e1/', import.meta.url).pathname;
const world = process.argv[2] || 'unit_test';
const program = await readFile(new URL(`../dist-bevy-gpu/worlds/${world}.txt`, import.meta.url), 'utf8');
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5215', '--dir', 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5215')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('console', m => { if (m.type() === 'error') console.log('[console error]', m.text().slice(0, 200)); });
  await page.goto('http://127.0.0.1:5215/?viewer=1&eval=1&scene=open&docname=e1');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'unit'), program);
  console.log('staged', JSON.stringify(staged).slice(0, 200));
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  await page.evaluate(() => window.__runtime.viewer_look_at(0, 5, 9, 0, 0.5, 0));
  await page.keyboard.press('h');
  for (let i = 0; i < 4; i++) {
    await page.waitForTimeout(Number(process.argv[3] || 1.2) * 1000);
    await page.screenshot({ path: `${reports}${world}-${i}.png` });
    console.log('shot', i, await page.evaluate(() => window.__runtime.game_status().slice(0, 240)));
  }
} finally { await browser.close(); child.kill(); await child.exited; }
