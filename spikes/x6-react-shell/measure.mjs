// X6 measurements: the engine canvas inside a React 19 (StrictMode) shell served by Vite.
import { spawn } from "node:child_process";
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";

const port = 5191;
const base = `http://127.0.0.1:${port}/`;
const vite = spawn("node", ["/Users/mac/Work/victoria-vr-hack/node_modules/vite/bin/vite.js", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { cwd: process.cwd(), stdio: "ignore" });
const ready = async () => { for (let i = 0; i < 100; i++) { try { if ((await fetch(base)).ok) return; } catch {} await new Promise((r) => setTimeout(r, 200)); } throw new Error("vite did not start"); };
await ready();

const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const out = {};
const logs = [];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  const open = async (query = "") => {
    const page = await browser.newPage({ viewport: { width: 1100, height: 600 } });
    page.on("console", (m) => { if (["error", "warning"].includes(m.type()) && !/GL Driver|software rendering|Download the React DevTools/.test(m.text())) logs.push(`${m.type()}: ${m.text()}`); });
    page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
    const t0 = Date.now();
    await page.goto(base + query);
    await page.waitForFunction(() => window.__x6?.starts >= 1, null, { timeout: 180000 });
    await page.waitForFunction(() => JSON.parse(window.__x6.api.status()).frames > 5, null, { timeout: 180000 });
    return { page, bootMs: Date.now() - t0 };
  };
  const status = (page) => page.evaluate(() => JSON.parse(window.__x6.api.status()));
  const frames = async (page) => (await status(page)).frames;
  const advancing = async (page) => { const a = await frames(page); await wait(600); return (await frames(page)) > a + 3; };
  const canvasInfo = (page) => page.evaluate(() => {
    const c = document.getElementById("ve");
    const r = c.getBoundingClientRect();
    return { connected: c.isConnected, client: [c.clientWidth, c.clientHeight], buffer: [c.width, c.height], x: Math.round(r.x), y: Math.round(r.y), dpr: devicePixelRatio };
  });
  const hoverScan = async (page) => {
    const c = await canvasInfo(page);
    for (let fy = 0.3; fy <= 0.8; fy += 0.1) for (let fx = 0.3; fx <= 0.7; fx += 0.1) {
      await page.mouse.move(c.x + c.client[0] * fx, c.y + c.client[1] * fy);
      await wait(60);
      const s = await status(page);
      if (s.interact.hover.length) return { found: true, at: [Math.round(fx * 10) / 10, Math.round(fy * 10) / 10], target: s.interact.hover[0].target };
    }
    return { found: false };
  };

  // Persistent canvas pattern.
  const { page, bootMs } = await open();
  out.persistent = { bootMs, starts: await page.evaluate(() => window.__x6.starts), errors: await page.evaluate(() => window.__x6.errors) };
  out.persistent.advancing = await advancing(page);
  out.persistent.canvas = await canvasInfo(page);
  out.persistent.hover = await hoverScan(page);
  await page.mouse.move(40, 300);
  await wait(100);
  out.persistent.hoverOverPanel = (await status(page)).interact.hover.length;

  await page.click("#wide");
  await wait(800);
  out.persistent.afterWide = await canvasInfo(page);
  out.persistent.hoverAfterWide = await hoverScan(page);
  await page.click("#hide");
  await wait(600);
  out.persistent.afterHide = { canvas: await canvasInfo(page), advancing: await advancing(page), errors: await page.evaluate(() => window.__x6.errors) };
  await page.click("#narrow");
  await wait(800);
  out.persistent.afterNarrow = await canvasInfo(page);

  await page.click("#remount");
  await wait(800);
  out.persistent.afterRemount = { canvas: await canvasInfo(page), advancing: await advancing(page), starts: await page.evaluate(() => window.__x6.starts), hover: await hoverScan(page) };

  // The canvas out of the document for a second (a panel hidden or detached), then back.
  await page.evaluate(() => { window.__held = document.getElementById("ve"); window.__held.remove(); });
  await wait(1000);
  out.persistent.detached = { advancing: await advancing(page), errors: await page.evaluate(() => window.__x6.errors) };
  await page.evaluate(() => document.getElementById("host").appendChild(window.__held));
  await wait(800);
  out.persistent.reattached = { canvas: await canvasInfo(page), advancing: await advancing(page) };

  // Typing in a React field.
  await page.click("#field");
  await page.keyboard.type("wasd");
  out.persistent.typed = await page.textContent("#typed");
  out.persistent.engineErrors = (await status(page)).errors;
  await page.close();

  // Naive pattern: React renders its own canvas and re-creates it on remount.
  const naive = await open("?naive=1");
  const original = await naive.page.evaluate(() => { window.__first = document.getElementById("ve"); return true; });
  out.naive = { starts: await naive.page.evaluate(() => window.__x6.starts), advancing: await advancing(naive.page) };
  await naive.page.click("#remount");
  await wait(1000);
  out.naive.afterRemount = await naive.page.evaluate(() => {
    const now = document.getElementById("ve");
    return { sameElement: now === window.__first, oldConnected: window.__first.isConnected, newBuffer: [now.width, now.height], newClient: [now.clientWidth, now.clientHeight], starts: window.__x6.starts };
  });
  out.naive.engineStillRunning = await advancing(naive.page);
  await naive.page.close();
  out.consoleProblems = logs.slice(0, 10);
} finally {
  await browser.close();
  vite.kill();
}
console.log(JSON.stringify(out, null, 1));
