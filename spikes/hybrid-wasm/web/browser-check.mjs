// Loads the page in headless Chromium and prints window.__spike.
import { chromium } from "/Users/mac/Work/victoria-vr-hack/node_modules/playwright/index.mjs";
const server = Bun.spawn(["bun", new URL("./serve.mjs", import.meta.url).pathname], { stdout: "pipe" });
await Bun.sleep(400);
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  page.on("console", (m) => console.log("[console]", m.text()));
  await page.goto("http://127.0.0.1:5199/");
  await page.waitForFunction(() => window.__spike, null, { timeout: 30000 });
  const r = await page.evaluate(() => ({ ...window.__spike, isolated: crossOriginIsolated, ua: navigator.userAgent }));
  delete r.imports;
  console.log(JSON.stringify(r, null, 2));
} finally { await browser.close(); server.kill(); }
