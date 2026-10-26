// Verifies, with REAL mouse clicks, what a model built with a saved script node: node tools/click-try-verify.mjs [world] [part]
// Loads the world (default worlds/click_try.txt), clicks the part (default crate_middle) with the cursor free, samples the part's Transform
// every 100 ms for 1.4 s (a hop should rise and come back), then clicks again (it should become hidden).
import { chromium } from "../web/node_modules/playwright/index.mjs";
const world = process.argv[2] ?? "worlds/click_try.txt";
const part = process.argv[3] ?? "crate_middle";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto(`http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready && window.__ve.scripts, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 200000 });
await page.waitForTimeout(3000);
await page.keyboard.press("h");
await page.evaluate(() => window.__rt.viewer_look_at(0, 2.0, 6, 0, 0.5, 0));
await page.waitForTimeout(900);
console.log("scripts:", JSON.stringify(await page.evaluate(() => window.__ve.scripts.list())));
const call = (method, params = {}) => page.evaluate(async ({ method, params }) => {
  const id = window.__rt.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 100; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a); await new Promise((r) => setTimeout(r, 30)); }
}, { method, params });
const screen = await page.evaluate((partName) => {
  const p = window.__ve.evaluation.parts.find((x) => x.name === partName);
  let sx = 0, sy = 0, sz = 0; const n = p.positions.length / 3;
  for (let i = 0; i < p.positions.length; i += 3) { sx += p.positions[i]; sy += p.positions[i + 1]; sz += p.positions[i + 2]; }
  return JSON.parse(window.__rt.viewer_project(sx / n, sy / n, sz / n, innerWidth, innerHeight));
}, part);
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 300)}`}`); if (!condition) failed++; };
await page.mouse.move(screen.x - 160, screen.y);
await page.waitForTimeout(300);
await page.mouse.move(screen.x, screen.y, { steps: 6 });
await page.waitForTimeout(500);
await page.mouse.down(); await page.waitForTimeout(120); await page.mouse.up();
const heights = [];
for (let i = 0; i < 14; i++) {
  const r = await call("world.get_components", { entity: part, components: ["Transform"] });
  heights.push(r.result.components.Transform.translation[1]);
  await page.waitForTimeout(100);
}
console.log("heights after one click:", heights.map((h) => h.toFixed(2)).join(" "));
check("one click makes the part rise", Math.max(...heights) > 0.3, heights);
check("and it comes back down (a hop, not a jump to a new place)", heights.at(-1) < 0.1, heights);
// Sampled every 100 ms, a half-second hop moves about 0.4 m between samples at its steepest: a teleport would show one jump of the whole height.
check("the rise is an arc (it takes more than one sample to go up and to come down)", heights.filter((h) => h > 0.05 && h < 0.57).length >= 2, heights);
await page.waitForTimeout(600);
await page.mouse.down(); await page.waitForTimeout(120); await page.mouse.up();
await page.waitForTimeout(1500);
const visible = await call("world.get_components", { entity: part, components: ["Visibility"] });
check("a second click hides the part", visible.ok && visible.result.components.Visibility === "Hidden", visible);
console.log("script logs:", JSON.stringify((await page.evaluate(() => window.__ve.scripts.list()))));
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
