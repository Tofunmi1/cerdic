import { chromium } from 'playwright';

// Loads a served page and prints what the browser says: console errors, page errors and failed requests.
// Usage: bun engine/web/tests/smoke.mjs [url]
const url = process.argv[2] ?? 'http://127.0.0.1:5200/?viewer=1&scene=stage&eval=1&model=village_hd&docname=smoke';
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  page.on('pageerror', error => console.log(`pageerror: ${error.message}`.slice(0, 400)));
  page.on('console', message => { if (['error', 'warning'].includes(message.type())) console.log(`console ${message.type()}: ${message.text()}`.slice(0, 400)); });
  page.on('requestfailed', request => console.log(`request failed: ${request.url()} ${request.failure()?.errorText}`));
  page.on('response', response => { if (response.status() >= 400) console.log(`http ${response.status()}: ${response.url()}`); });
  await page.goto(url);
  const ready = await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 60000 }).then(() => true, () => false);
  await page.waitForTimeout(3000);
  console.log(`ready: ${ready}; title: ${await page.title()}; body text length: ${(await page.evaluate(() => document.body.innerText)).length}`);
  await page.screenshot({ path: new URL('../../target/reports/g10/smoke.png', import.meta.url).pathname });
} finally {
  await browser.close();
}
