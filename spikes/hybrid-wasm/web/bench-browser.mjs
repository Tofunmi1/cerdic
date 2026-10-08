import { chromium } from "/Users/mac/Work/victoria-vr-hack/node_modules/playwright/index.mjs";
const server = Bun.spawn(["bun", new URL("./serve.mjs", import.meta.url).pathname], { stdout: "pipe" });
await Bun.sleep(400);
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.goto("http://127.0.0.1:5199/bench.html");
  await page.waitForFunction(() => window.__bench, null, { timeout: 300000 });
  console.log(JSON.stringify(await page.evaluate(() => window.__bench)));
} finally { await browser.close(); server.kill(); }
