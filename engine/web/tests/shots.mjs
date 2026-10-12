// Takes screenshots of the engine pages for comparison: node/bun web/tests/shots.mjs <url> <out.png>
import { chromium } from 'playwright';
const [url, out] = process.argv.slice(2);
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
await page.goto(url);
await page.waitForTimeout(6000);
await page.screenshot({ path: out });
await browser.close();
console.log('saved', out);
