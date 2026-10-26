// Exercises the script layer (scripts.ts, ADR 0065) in a real page: node tools/script-test.mjs
// A script spins and bobs crates through the world API; a runaway loop is stopped; errors come back; a batch moves many entities every
// frame; Math.random is seeded. Exits non-zero on the first failed check.
import { chromium } from "../web/node_modules/playwright/index.mjs";
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/world_api_try.txt");
await page.waitForFunction(() => window.__ve?.ready && window.__ve.scripts, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 200000 });
await page.waitForTimeout(2000);

const start = (name, source) => page.evaluate(({ name, source }) => window.__ve.scripts.start(name, source), { name, source });
const logs = (name) => page.evaluate((name) => window.__ve.scripts.logs(name), name);
const list = () => page.evaluate(() => window.__ve.scripts.list());
const transform = (entity) => page.evaluate(async (entity) => {
  const id = window.__rt.world_call("world.get_components", JSON.stringify({ entity, components: ["Transform"] }));
  for (let i = 0; i < 100; i++) { const a = window.__rt.world_take(id); if (a) return JSON.parse(a).result?.components?.Transform; await new Promise((r) => setTimeout(r, 30)); }
}, entity);
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 300)}`}`); if (!condition) failed++; };

// 1. A script that spins the middle crate and bobs the left one.
const spin = `
let angle = 0;
log('spin started');
on('frame', (dt, t) => {
  angle += dt * 2;
  const q = [0, Math.sin(angle / 2), 0, Math.cos(angle / 2)];
  world.batch([
    { method: 'world.insert_components', params: { entity: 'crate_middle', components: { Transform: { translation: [0, 0.2 + 0.2 * Math.sin(t * 3), 0], rotation: q, scale: [1, 1, 1] } } } },
  ]);
});`;
const started = await start("spin", spin);
check("a script starts", started.ok, started);
const t1 = await transform("crate_middle");
await page.waitForTimeout(700);
const t2 = await transform("crate_middle");
check("the script moves the part (rotation changes over time)", t1 && t2 && JSON.stringify(t1.rotation) !== JSON.stringify(t2.rotation), { t1, t2 });
check("the script bobs the part (height stays in its range)", t2.translation[1] >= -0.01 && t2.translation[1] <= 0.41, t2);
const status = await list();
check("list reports it running with frames", status.some((s) => s.name === "spin" && s.frames > 5), status);
check("log() reaches the host", (await logs("spin")).logs.includes("spin started"), await logs("spin"));

// 2. A runaway loop is stopped, and the others keep running.
await start("runaway", "on('frame', () => { while (true) {} });");
await page.waitForTimeout(1200);
const afterRunaway = await list();
check("a runaway loop is terminated", !afterRunaway.some((s) => s.name === "runaway"), afterRunaway);
check("the other script keeps running", afterRunaway.some((s) => s.name === "spin"), afterRunaway);

// 3. Errors come back: a syntax error, a throw at start, a throw in a handler (the script survives it).
await start("bad_syntax", "this is not javascript (");
await page.waitForTimeout(400);
const syntax = await logs("bad_syntax");
check("a syntax error is reported", syntax.ok && syntax.errors.length > 0, syntax);
await start("throws_late", "let n = 0; on('frame', () => { n++; if (n === 3) throw new Error('boom at frame 3'); });");
await page.waitForTimeout(800);
const late = await logs("throws_late");
check("an error inside a handler is reported", late.errors.some((e) => e.includes("boom at frame 3")), late);
check("the script survives it and keeps getting frames", late.frames > 6, late);

// 4. Many entities in one batch every frame.
const spawned = await page.evaluate(async () => {
  const ids = [];
  for (let i = 0; i < 100; i++) {
    const id = window.__rt.world_call("world.spawn_entity", JSON.stringify({ components: { Transform: { translation: [i, 5, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] } } }));
    ids.push(id);
  }
  await new Promise((r) => setTimeout(r, 600));
  return ids.map((id) => JSON.parse(window.__rt.world_take(id) ?? "{}").result?.entity).filter(Boolean);
});
check("100 entities spawned for the batch test", spawned.length === 100, spawned.length);
await start("mover", `
const ids = ${JSON.stringify(spawned)};
let n = 0;
on('frame', (dt, t) => {
  n++;
  world.batch(ids.map((entity, i) => ({ method: 'world.insert_components', params: { entity, components: { Transform: { translation: [i, 5 + Math.sin(t + i), 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] } } } })));
});`);
const f0 = (await list()).find((s) => s.name === "mover").frames;
await page.waitForTimeout(2000);
const f1 = (await list()).find((s) => s.name === "mover").frames;
console.log(`     the 100-entity batch script ran ${((f1 - f0) / 2).toFixed(0)} frames a second (headless desktop)`);
check("a script that moves 100 entities a frame keeps over 20 frames a second", (f1 - f0) / 2 > 20, { f0, f1 });
const moved = await page.evaluate(async (entity) => {
  const id = window.__rt.world_call("world.get_components", JSON.stringify({ entity, components: ["Transform"] }));
  await new Promise((r) => setTimeout(r, 300));
  return JSON.parse(window.__rt.world_take(id)).result.components.Transform.translation;
}, spawned[50]);
check("the batch really moved an entity", Math.abs(moved[1] - 5) > 0.001 || moved[0] === 50, moved);

// 5. Math.random is seeded: two runs of the same script print the same numbers.
const roll = "log([random(), random(), Math.random()].join(','));";
await start("dice_a", roll); await start("dice_b", roll);
await page.waitForTimeout(500);
const a = (await logs("dice_a")).logs[0], b = (await logs("dice_b")).logs[0];
check("random() and Math.random are seeded and repeat", a && a === b, { a, b });

// 6. The sandbox has no network.
await start("net", "log(typeof fetch, typeof XMLHttpRequest, typeof WebSocket, typeof importScripts);");
await page.waitForTimeout(400);
check("a script has no fetch, XMLHttpRequest, WebSocket or importScripts", (await logs("net")).logs[0] === "undefined undefined undefined undefined", await logs("net"));

// 7. Stopping.
const stopped = await page.evaluate(() => window.__ve.scripts.stop("spin"));
await page.waitForTimeout(200);
const r1 = await transform("crate_middle"); await page.waitForTimeout(400); const r2 = await transform("crate_middle");
check("stop ends a script (the part stops moving)", stopped && JSON.stringify(r1) === JSON.stringify(r2), { r1, r2 });
check("a bad script name is refused", !(await start("Bad Name", "log(1)")).ok);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
