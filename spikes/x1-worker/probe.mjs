// Finds which X1 step hangs: each call gets a timeout and is reported as it finishes.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "web");
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const file = path.join(root, req.url.split("?")[0] === "/" ? "index.html" : req.url.split("?")[0]);
  try { res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream" }); res.end(fs.readFileSync(file)); }
  catch { res.writeHead(404); res.end(); }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader"] });
const page = await browser.newPage();
page.on("console", (m) => console.log("console:", m.text()));
page.on("pageerror", (e) => console.log("pageerror:", e.message));
page.on("worker", (w) => console.log("worker started", w.url()));
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => window.x1);
const step = async (label, type, args) => {
  const t0 = Date.now();
  const r = await page.evaluate(async ([type, args]) => {
    if (!window.w) { window.w = window.x1.spawn(); }
    const timeout = new Promise((res) => setTimeout(() => res({ timeout: true }), 20000));
    return Promise.race([window.w.call(type, args), timeout]);
  }, [type, args]);
  console.log(label, JSON.stringify({ ...r, ms: Math.round(r.ms ?? -1), wall: Date.now() - t0 }).slice(0, 300));
};
await step("init", "init", []);
await step("memory", "memory_bytes", []);
await step("graph", "graph", [20]);
await step("heavy 1 sphere", "heavy", [1, 16]);
await step("heavy 2x32", "heavy", [2, 32]);
await step("heavy 6x48", "heavy", [6, 48]);
await browser.close();
server.close();
