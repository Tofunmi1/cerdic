// Builds nothing; serves ./web and reports what the page found. Throwaway spike runner.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "web");
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const requested = req.url.split("?")[0];
  const p = requested === "/" ? "/index.html" : requested;
  let body;
  try { body = fs.readFileSync(path.join(root, p)); } catch { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { "content-type": types[path.extname(p)] ?? "application/octet-stream" });
  res.end(body);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const page = await browser.newPage();
const logs = [];
page.on("console", (m) => { if (!/GL Driver Message/.test(m.text())) logs.push(`[${m.type()}] ${m.text().slice(0, 300)}`); });
page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message.slice(0, 300)}`));
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => window.__spike?.done, null, { timeout: 60000 });
console.log(JSON.stringify(await page.evaluate(() => window.__spike)), logs.length ? logs : "");
await browser.close();
server.close();
