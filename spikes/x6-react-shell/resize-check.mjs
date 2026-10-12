// Does the engine's own page survive a window resize? (Throwaway check against engine/web/dist.)
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const dist = "/Users/mac/Work/victoria-vr-hack/engine/web/dist";
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const file = path.join(dist, req.url.split("?")[0] === "/" ? "index.html" : req.url.split("?")[0]);
  try { res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream" }); res.end(fs.readFileSync(file)); }
  catch { res.writeHead(404); res.end(); }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
const t0 = Date.now();
const stamp = () => `${((Date.now() - t0) / 1000).toFixed(1)}s`;
page.on("console", (m) => { if (/Quitting the application|Caught rendering error|differing sizes/.test(m.text())) console.log(stamp(), "ENGINE:", m.text().replace(/\s+/g, " ").slice(0, 220)); });
await page.goto(`http://127.0.0.1:${server.address().port}/?scene=xr`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
const frames = () => page.evaluate(() => window.__ve.read().frames);
const sample = async (label) => { const a = await frames(); await new Promise((r) => setTimeout(r, 1500)); const b = await frames(); console.log(stamp(), label, "frames", a, "->", b); };
await sample("idle at boot");
for (const [w, h] of [[800, 450], [500, 300]]) {
  await page.setViewportSize({ width: w, height: h });
  await new Promise((r) => setTimeout(r, 500));
  await sample(`after ${w}x${h}`);
}
await browser.close();
server.close();
