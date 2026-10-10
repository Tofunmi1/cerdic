import { chromium } from "/Users/mac/Work/victoria-vr-hack/node_modules/playwright/index.mjs";
import http from "node:http"; import fs from "node:fs"; import path from "node:path";
const root = new URL("./web/", import.meta.url).pathname;
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const srv = http.createServer((q, s) => { const f = path.join(root, q.url === "/" ? "index.html" : q.url.split("?")[0]); try { s.writeHead(200, { "content-type": types[path.extname(f)] || "text/plain" }); s.end(fs.readFileSync(f)); } catch { s.writeHead(404); s.end(); } }).listen(5301);
const b = await chromium.launch();
try {
  const p = await b.newPage(); p.on("console", m => console.log("[console]", m.text())); p.on("pageerror", e => console.log("[pageerror]", e.message));
  await p.goto("http://127.0.0.1:5301/"); await p.waitForFunction(() => window.__spike, null, { timeout: 20000 });
  console.log(JSON.stringify(await p.evaluate(() => window.__spike)));
} finally { await b.close(); srv.close(); }
