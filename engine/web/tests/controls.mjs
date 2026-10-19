import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// The viewer's side-panel controls and mouse/keyboard navigation work: each control changes the picture, and nothing throws.
// Usage: bun engine/web/tests/controls.mjs [model] [base url]
const model = process.argv[2] ?? 'mannequin';
const base = process.argv[3] ?? 'http://127.0.0.1:5200';
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  page.on('pageerror', e => errors.push(`pageerror: ${e.message}`));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text())) errors.push(`console: ${m.text()}`); });
  await page.addInitScript(() => { window.__veUploadTimeoutMs = 600000; });
  await page.goto(`${base}/?viewer=1&scene=stage&eval=1&model=${model}&docname=controls-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 300000 });
  await page.waitForTimeout(1500);
  const shot = () => page.screenshot({ clip: { x: 320, y: 70, width: 520, height: 420 } });
  const results = {};
  const changed = async (name, act) => {
    const before = await shot();
    await act();
    await page.waitForTimeout(900);
    const after = await shot();
    results[name] = !before.equals(after);
  };
  await changed('wheel zoom', async () => { await page.mouse.move(580, 300); await page.mouse.wheel(0, -400); });
  await changed('drag orbit', async () => {
    await page.getByLabel('Drag to orbit (O)').check();
    await page.mouse.move(580, 300); await page.mouse.down(); await page.mouse.move(700, 330, { steps: 6 }); await page.mouse.up();
  });
  // Headless Chromium refuses pointer lock, so the lock is set through the runtime the way the pointerlockchange handler does.
  assert(page.url().includes(`model=${model}`), 'a control navigated away from the model');
  await page.evaluate(() => { document.activeElement?.blur(); window.__runtime.viewer_set_locked(true); });
  await changed('WASD moves the camera while locked', async () => {
    await page.keyboard.down('w'); await page.waitForTimeout(700); await page.keyboard.up('w');
    await page.keyboard.down('a'); await page.waitForTimeout(500); await page.keyboard.up('a');
  });
  await changed('mouse look turns the camera while locked', async () => { await page.evaluate(() => window.__runtime.viewer_look(220, 40)); });
  await page.evaluate(() => window.__runtime.viewer_set_locked(false));
  await changed('reset view (R)', async () => { await page.evaluate(() => document.activeElement?.blur()); await page.keyboard.press('r'); });
  await changed('lens select', () => page.locator('select', { has: page.locator('option', { hasText: 'Flat perspective' }) }).selectOption({ index: 1 }));
  await changed('field of view slider', async () => {
    const slider = page.locator('input[type=range]').first();
    await slider.evaluate(el => { el.value = String(Number(el.max)); el.dispatchEvent(new Event('input', { bubbles: true })); });
  });
  console.log(JSON.stringify(results, null, 1));
  console.log(errors.length ? `errors:\n${errors.join('\n')}` : 'no browser errors');
  assert(Object.values(results).every(Boolean), `controls that changed nothing: ${Object.keys(results).filter(k => !results[k]).join(', ')}`);
  assert.deepEqual(errors, []);
} finally {
  await browser.close();
}
