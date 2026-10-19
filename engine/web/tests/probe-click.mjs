import { chromium } from 'playwright';

// What element receives a click at the middle of the viewport, and does it request pointer lock?
// Usage: bun engine/web/tests/probe-click.mjs <model> [base url]
const model = process.argv[2] ?? 'mannequin';
const base = process.argv[3] ?? 'http://127.0.0.1:5200';
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  await page.addInitScript(() => {
    window.__lockRequests = 0;
    const original = HTMLCanvasElement.prototype.requestPointerLock;
    HTMLCanvasElement.prototype.requestPointerLock = function (...args) { window.__lockRequests++; return original?.apply(this, args); };
  });
  await page.goto(`${base}/?viewer=1&scene=stage&eval=1&model=${model}&docname=click-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.waitForTimeout(5000);
  for (const [x, y] of [[580, 300], [600, 450], [500, 200]]) {
    const hit = await page.evaluate(([px, py]) => {
      const el = document.elementFromPoint(px, py);
      return `${el?.tagName}#${el?.id}.${el?.className}`;
    }, [x, y]);
    await page.mouse.click(x, y);
    console.log(`(${x},${y}) -> ${hit}; pointer-lock requests so far: ${await page.evaluate(() => window.__lockRequests)}`);
  }
} finally {
  await browser.close();
}
