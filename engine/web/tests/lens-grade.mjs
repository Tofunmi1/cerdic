import { chromium } from 'playwright';

// Does each Lens option, each Grade option and the split-screen toggle change the picture?
// Usage: bun engine/web/tests/lens-grade.mjs [model] [base url]
const model = process.argv[2] ?? 'village_hd';
const base = process.argv[3] ?? 'http://127.0.0.1:5200';
// VE_GPU=1 runs a visible browser on the machine's real GPU instead of the software renderer.
const browser = process.env.VE_GPU
  ? await chromium.launch({ headless: false, channel: 'chromium', args: ['--ignore-gpu-blocklist'] })
  : await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text().slice(0, 200)); });
  await page.addInitScript(() => { window.__veUploadTimeoutMs = 600000; });
  await page.goto(`${base}/?viewer=1&scene=stage&eval=1&model=${model}&docname=lg-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 400000 });
  await page.waitForTimeout(2000);
  const shot = () => page.screenshot({ clip: { x: 320, y: 70, width: 520, height: 420 } });
  const select = name => page.locator('select', { has: page.locator('option', { hasText: name }) });
  const options = async sel => sel.locator('option').allInnerTexts();
  const lens = select('Flat perspective');
  const grade = select('Neutral');
  console.log('lens options:', await options(lens));
  console.log('grade options:', await options(grade));
  const base0 = await shot();
  for (const [name, sel] of [['lens', lens], ['grade', grade]]) {
    const list = await options(sel);
    for (let i = 1; i < list.length; i++) {
      await sel.selectOption({ index: i });
      await page.waitForTimeout(1200);
      const s = await shot();
      console.log(`${name} = ${list[i]}: picture ${s.equals(base0) ? 'UNCHANGED' : 'changed'}`);
      await page.screenshot({ path: new URL(`../../target/reports/g10/${name}-${i}.png`, import.meta.url).pathname });
    }
    await sel.selectOption({ index: 0 });
    await page.waitForTimeout(800);
  }
  const box = page.getByLabel('Split-screen mock headset (V)');
  console.log('split checked before:', await box.isChecked(), 'class:', JSON.stringify(await page.evaluate(() => document.body.className)));
  await box.setChecked(true);
  await page.waitForTimeout(1500);
  const split = await page.screenshot({ path: new URL('../../target/reports/g10/split.png', import.meta.url).pathname });
  console.log('split screen body class:', await page.evaluate(() => document.body.className));
  console.log(errors.length ? `errors: ${errors.join(' | ')}` : 'no browser errors');
} finally {
  await browser.close();
}
