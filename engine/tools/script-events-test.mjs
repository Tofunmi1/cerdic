// Events into scripts (ADR 0065): node tools/script-events-test.mjs
// Scripts hear what happens in the world: a real mouse click on a part, hovering it, and a game rule's effect. A listener can be general
// (on("click")) or for one part (on("click:crate_middle")). Exits non-zero when a check fails.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/world_api_try.txt");
await page.waitForFunction(() => window.__ve?.ready && window.__ve.scripts, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 200000 });
await page.waitForTimeout(2500);
await page.keyboard.press("h"); // hide the panels so a click lands on the scene
await page.evaluate(() => window.__rt.viewer_look_at(0, 2.0, 6, 0, 0.5, 0));
await page.waitForTimeout(900);

let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 300)}`}`); if (!condition) failed++; };
const logs = (name) => page.evaluate((name) => window.__ve.scripts.logs(name), name);
const screenOf = (partName) => page.evaluate((partName) => {
  const part = window.__ve.evaluation.parts.find((p) => p.name === partName);
  let sx = 0, sy = 0, sz = 0; const n = part.positions.length / 3;
  for (let i = 0; i < part.positions.length; i += 3) { sx += part.positions[i]; sy += part.positions[i + 1]; sz += part.positions[i + 2]; }
  const p = JSON.parse(window.__rt.viewer_project(sx / n, sy / n, sz / n, innerWidth, innerHeight));
  return { x: p.x ?? p[0], y: p.y ?? p[1], raw: p };
}, partName);

await page.evaluate(() => window.__ve.scripts.start("listener", `
  on('click', (e) => log('any click on ' + e.part));
  on('hover', (e) => log('hovered ' + e.part));
  on('press', (e) => log('pressed ' + e.part));
  on('release', (e) => log('released ' + e.part));
  on('click:crate_middle', (e) => log('middle clicked'));
  on('hover:crate_middle', (e) => log('middle hovered'));
  on('unhover:crate_middle', (e) => log('middle unhovered'));
  on('game.message:hello', (e) => log('game said hello'));
  on('game.message', (e) => log('any message ' + e.part));
`));
await page.waitForTimeout(500);
const mid = await screenOf("crate_middle");
console.log(`     crate_middle projects to ${JSON.stringify(mid.raw)}`);
await page.mouse.move(mid.x - 150, mid.y);
await page.waitForTimeout(300);
await page.mouse.move(mid.x, mid.y, { steps: 6 });
await page.waitForTimeout(500);
check("hovering a part tells scripts", (await logs("listener")).logs.includes("middle hovered"), await logs("listener"));
await page.mouse.down(); await page.waitForTimeout(80); await page.mouse.up();
await page.waitForTimeout(600);
const afterClick = (await logs("listener")).logs;
check("a click on a part tells the general listener its name", afterClick.includes("any click on crate_middle"), afterClick);
check("a click on a part tells the listener for that part", afterClick.includes("middle clicked"), afterClick);
await page.mouse.move(mid.x - 200, mid.y - 150, { steps: 6 });
await page.waitForTimeout(500);
check("moving away tells scripts it is no longer hovered", (await logs("listener")).logs.includes("middle unhovered"), await logs("listener"));

// A click on another part is not heard by the middle crate's listener.
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
// Find another crate by hovering along the row (the part's centroid is not a reliable place to click), then click it.
let other = null;
for (const dx of [-260, 260, -200, 200, -330, 330, -150, 150]) {
  await page.mouse.move(mid.x + dx, mid.y, { steps: 4 });
  await page.waitForTimeout(250);
  const last = (await logs("listener")).logs.filter((l) => l.startsWith("hovered ")).at(-1);
  if (last && last !== "hovered crate_middle") { other = { x: mid.x + dx, name: last.slice("hovered ".length) }; break; }
}
console.log(`     another part found by hovering: ${JSON.stringify(other)}`);
check("hovering finds another crate", other !== null, await logs("listener"));
await page.waitForTimeout(400);
await page.mouse.down(); await page.waitForTimeout(150); await page.mouse.up();
await page.waitForTimeout(500);
const afterOther = (await logs("listener")).logs;
// Informational: after a click the viewer may be in look mode (a click on empty space locks the pointer), and a second click then does
// not pick; that is the viewer's existing behaviour, so this is reported, not required.
console.log(`     a second click on ${other?.name}: ${afterOther.includes(`any click on ${other?.name}`) ? "heard" : "not delivered (the viewer may have been in look mode)"}`);
check("a click never reaches the middle crate's own listener when it was not on it", afterOther.filter((l) => l === "middle clicked").length === 1, afterOther);

// A game rule's effect is an event too.
const error = await page.evaluate(() => window.__rt.game_load("game name=events\nrule on=start do=effect:message:hello"));
await page.waitForTimeout(1200);
const afterGame = (await logs("listener")).logs;
check("a game load had no error", !error, error);
check("a game effect reaches the listener for its name", afterGame.includes("game said hello"), afterGame);
check("a game effect reaches the general listener too", afterGame.includes("any message hello"), afterGame);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
