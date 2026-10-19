import { chromium } from 'playwright';

// Prints every console message the page emits after the fisheye lens is chosen, plus what the runtime reports.
const model = process.argv[2] ?? 'viaduct';
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  const lines = [];
  let armed = !!process.env.FROM_START; // FROM_START=1 also prints what the page logs while loading
  page.on('console', m => { if (armed && !/GPU stall/.test(m.text())) lines.push(`${m.type()}: ${m.text().slice(0, 300)}`); });
  page.on('pageerror', e => lines.push(`pageerror: ${e.message.slice(0, 300)}`));
  await page.goto(`http://127.0.0.1:5200/?viewer=1&scene=stage&eval=1&model=${model}&docname=lc-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForTimeout(Number(process.env.WAIT_MS ?? 8000));
  armed = true;
  await page.evaluate(() => { window.__runtime.viewer_configure(true, 130, false, 4); window.__runtime.viewer_set_lens('fisheye'); });
  await page.waitForTimeout(3000);
  console.log(await page.evaluate(() => JSON.stringify(JSON.parse(window.__runtime.eval_projection_status()).viewer ?? Object.keys(JSON.parse(window.__runtime.eval_projection_status())))));
  console.log(lines.join('\n') || '(no console output after choosing the lens)');
} finally {
  await browser.close();
}
