// V0 acceptance runs against the root Vite editor and the real Rust runtime.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';
import { PNG } from 'pngjs';
import { mkdir } from 'node:fs/promises';

const process = Bun.spawn(['bun', 'x', 'vite', '--host', '127.0.0.1', '--port', '5187'], {
  cwd: new URL('../../../', import.meta.url).pathname, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
try {
  let available = false;
  for (let i = 0; i < 100; i++) {
    try { available = (await fetch('http://127.0.0.1:5187')).ok; } catch {}
    if (available) break;
    await Bun.sleep(100);
  }
  assert(available, 'Vite started');
  const context = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const page = await context.newPage();
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => {
    if (m.type() === 'error' && !driverAdvisory.test(m.text())) logs.push(m.text());
  });
  await page.goto('http://127.0.0.1:5187/?docname=v0-first');
  await page.waitForFunction(() => window.__spatial?.ready, null, { timeout: 90000 });
  assert.equal(await page.locator('#ve').count(), 1, 'v0_01 one engine canvas');
  assert.equal(await page.evaluate(() => window.__spatial.boots), 1, 'v0_01 boot once in StrictMode');
  const pixels = PNG.sync.read(await page.locator('#ve').screenshot());
  const colors = new Set();
  for (let i = 0; i < pixels.data.length; i += 4) colors.add(`${pixels.data[i]},${pixels.data[i+1]},${pixels.data[i+2]}`);
  assert(colors.size > 100, 'v0_01 gallery actually draws');
  await page.getByRole('button', { name: 'Add box' }).click();
  await page.waitForFunction(() => window.__spatial.doc.state().nodes === 1);
  assert.match(await page.getByTestId('document-view').innerText(), /solid.box/, 'v0_02 live document');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  for (const [width, height] of [[800, 600], [1400, 900]]) {
    await page.setViewportSize({ width, height });
    await page.waitForFunction(() => {
      const c = document.querySelector('#ve');
      return Math.abs(c.width - c.clientWidth * devicePixelRatio) <= 1;
    });
  }
  await page.getByLabel('Command JSON').fill('wasd');
  assert.equal(await page.getByLabel('Command JSON').inputValue(), 'wasd', 'v0_04 field owns keys');
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  assert.equal(await page.evaluate(() => document.pointerLockElement), null, 'v0_04 lock released');
  const other = await context.newPage();
  await other.goto('http://127.0.0.1:5187/?docname=v0-second');
  await other.waitForFunction(() => window.__spatial?.ready, null, { timeout: 90000 });
  assert.equal(await other.evaluate(() => window.__spatial.doc.state().nodes), 0, 'v0_05 separate document');
  await other.close();
  const highDpi = await browser.newContext({ deviceScaleFactor: 2, viewport: { width: 1000, height: 700 } });
  const dpiPage = await highDpi.newPage();
  await dpiPage.goto('http://127.0.0.1:5187/?docname=v0-dpi');
  await dpiPage.waitForFunction(() => window.__spatial?.ready, null, { timeout: 90000 });
  assert(await dpiPage.evaluate(() => {
    const c = document.querySelector('#ve'); return Math.abs(c.width - c.clientWidth * 2) <= 1;
  }), 'v0_03 device pixel ratio');
  await highDpi.close();
  const resources = await page.evaluate(() => {
    window.dispatchEvent(new PageTransitionEvent('pagehide'));
    return window.__spatial.resources;
  });
  assert.deepEqual(resources, { inputBindings: 0, workers: 0, polling: 0 }, 'v0_07 owned resources released');
  await page.reload();
  await page.waitForFunction(() => window.__spatial?.ready, null, { timeout: 90000 });
  assert.equal(await page.evaluate(() => window.__spatial.boots), 1, 'v0_07 fresh engine after reload');
  assert.equal(await page.evaluate(() => window.__spatial.doc.state().nodes), 1, 'v0_05 saved document restored');
  const reports = new URL('../../target/reports/', import.meta.url).pathname;
  await mkdir(reports, { recursive: true });
  await page.screenshot({ path: `${reports}v0-editor.png` });
  const legacy = await context.newPage();
  await legacy.route('**/api/status', route => route.fulfill({ json: { mode: 'mock' } }));
  await legacy.goto('http://127.0.0.1:5187/?legacy=1');
  await legacy.locator('canvas').waitFor({ timeout: 90000 });
  await legacy.getByText('Lavender block', { exact: true }).first().waitFor();
  await legacy.screenshot({ path: `${reports}v0-legacy.png` });
  await legacy.close();
  assert.deepEqual(logs, [], 'v0_01 console clean');
  console.log('V0 editor acceptance passed');
} finally {
  await browser.close();
  process.kill();
  await process.exited;
}
