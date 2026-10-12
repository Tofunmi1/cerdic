import { spawn } from "node:child_process";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const port = 5193;
const base = `http://127.0.0.1:${port}/`;
const vite = spawn("node", ["/Users/mac/Work/victoria-vr-hack/node_modules/vite/bin/vite.js", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { cwd: process.cwd(), stdio: "ignore" });
for (let i = 0; i < 100; i++) { try { if ((await fetch(base)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 200)); }
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 600 } });
page.on("console", (m) => console.log("console:", m.type(), m.text().slice(0, 400)));
page.on("pageerror", (e) => console.log("pageerror:", e.message.slice(0, 400)));
await page.goto(base);
await page.waitForFunction(() => window.__x6?.starts >= 1 && JSON.parse(window.__x6.api.status()).frames > 5, null, { timeout: 120000 });
const probe = async (label) => {
  const a = await page.evaluate(() => JSON.parse(window.__x6.api.status()).frames);
  await new Promise((r) => setTimeout(r, 500));
  const s = await page.evaluate(() => { const st = JSON.parse(window.__x6.api.status()); const c = document.getElementById("ve"); return { frames: st.frames, errors: st.errors, buffer: [c.width, c.height], client: [c.clientWidth, c.clientHeight] }; });
  console.log(label, JSON.stringify({ before: a, ...s }));
};
await probe("start");
await page.click("#hide");
await probe("after hide");
await page.click("#narrow");
await probe("after narrow");
await new Promise((r) => setTimeout(r, 1500));
await probe("1.5s later");
await browser.close();
vite.kill();
