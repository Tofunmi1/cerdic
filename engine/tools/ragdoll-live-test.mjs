// Live check of the physics queries and ragdolls (plan W5): loads `worlds/ragdoll_test.txt` in the real viewer, starts play mode, ray-casts and overlaps, makes
// the puppet go limp with a shove, watches it fall to the floor, and checks that stopping play stands it up again.
//   bun tools/ragdoll-live-test.mjs
// Needs the static server on 127.0.0.1:8801 serving web/dist-bevy-gpu with a built viewer.
import assert from "node:assert/strict";
import { chromium } from "../web/node_modules/playwright/index.mjs";

const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 640, height: 400 } });
const problems = [];
page.on("pageerror", (e) => problems.push(String(e).slice(0, 300)));
page.on("console", (m) => { if (m.type() === "error" && !/404/.test(m.text())) problems.push(m.text().slice(0, 300)); });
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=worlds/ragdoll_test.txt");
await page.waitForFunction(() => globalThis.__ve?.evaluation?.parts?.length > 5, null, { timeout: 120000 });
// The page's own runtime module (the browser returns the instance it already loaded).
await page.evaluate(async () => { globalThis.__runtime = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(globalThis.__runtime.eval_projection_status()).committed?.quality === "final", null, { timeout: 120000 });
await page.waitForTimeout(1000);

const call = (method, params = {}) => page.evaluate(async ([method, params]) => {
  const id = globalThis.__runtime.world_call(method, JSON.stringify(params));
  for (let i = 0; i < 200; i++) {
    const text = globalThis.__runtime.world_take(id);
    if (text) return JSON.parse(text);
    await new Promise((r) => setTimeout(r, 25));
  }
  return { error: "no answer" };
}, [method, params]);
const result = (answer) => answer.result ?? answer;
const centre = async (name) => result(await call("world.bounds", { entity: name })).center;

const parts = await page.evaluate(() => globalThis.__ve.evaluation.parts.map((p) => ({ name: p.name, ragdoll: p.ragdoll?.name ?? null })));
assert.ok(parts.some((p) => p.ragdoll === "hero"), `the marker carries the ragdoll: ${JSON.stringify(parts)}`);

// Not in play mode: refused with the way out.
let answer = await call("physics.raycast", { origin: [0, 1, 3], direction: [0, 0, -1] });
assert.match(JSON.stringify(answer), /play mode/, `a query before play says so: ${JSON.stringify(answer)}`);
const standing = await centre("head");

await page.evaluate(() => globalThis.__runtime.viewer_play_mode(true));
await page.waitForTimeout(1500);

// A ray at the wall (the puppet has no bodies yet, so it passes through the standing figure).
answer = result(await call("physics.raycast", { origin: [0, 1.2, 3], direction: [0, 0, -1] }));
assert.equal(answer.hit, true, JSON.stringify(answer));
assert.equal(answer.part, "wall");
assert.ok(Math.abs(answer.distance - 5.8) < 0.05, `distance to the wall face: ${answer.distance}`);
assert.ok(answer.normal[2] > 0.99, `the wall faces the shooter: ${answer.normal}`);
answer = result(await call("physics.raycast", { origin: [5, 5, 0], direction: [0, -1, 0], max_distance: 3 }));
assert.equal(answer.hit, false, "too short to reach the floor");
answer = result(await call("physics.raycast", { origin: [5, 5, 0], direction: [0, -1, 0] }));
assert.equal(answer.part, "ground");
assert.ok(Math.abs(answer.distance - 5) < 0.05, `${answer.distance}`);
answer = result(await call("physics.overlap", { center: [5, 0, 0], radius: 0.5 }));
assert.ok(answer.parts.some((p) => p.part === "ground"), JSON.stringify(answer));
answer = await call("physics.raycast", { origin: [0, 1, 3], direction: [0, 0, 0] });
assert.ok(JSON.stringify(answer).includes("hit") || JSON.stringify(answer).includes("error"), "a zero direction does not crash");

// Go limp with a shove towards the wall.
answer = await call("physics.ragdoll", { name: "nobody" });
assert.match(JSON.stringify(answer), /no phys.ragdoll is called/, JSON.stringify(answer));
answer = result(await call("physics.ragdoll", { name: "hero", impulse: [0, 0, -40] }));
assert.equal(answer.bones, 5, JSON.stringify(answer));
answer = await call("physics.ragdoll", { name: "hero" });
assert.match(JSON.stringify(answer), /already limp/, JSON.stringify(answer));
await page.waitForTimeout(600);
answer = result(await call("physics.raycast", { origin: [0, 1.2, 3], direction: [0, 0, -1] }));
assert.ok(["pelvis", "spine", "head", "thigh", "shin", "wall"].includes(answer.part), JSON.stringify(answer));
await page.waitForTimeout(5000);
const fallen = await centre("head");
assert.ok(fallen[1] < 0.6, `the head came down to the floor: y = ${fallen[1]} (standing it was ${standing[1]})`);
const moved = Math.hypot(fallen[0] - standing[0], fallen[2] - standing[2]);
assert.ok(moved > 0.3, `the shove carried it: moved ${moved}`);
// Bones stayed together: head and spine are close.
const spineAt = await centre("spine");
assert.ok(Math.hypot(...fallen.map((v, i) => v - spineAt[i])) < 0.9, "the head is still joined to the spine");
await page.screenshot({ path: "/tmp/ve-look/ragdoll_live.png" });

// Stop play: the figure stands again.
await page.evaluate(() => globalThis.__runtime.viewer_play_mode(false));
await page.waitForTimeout(1500);
const back = await centre("head");
assert.ok(Math.abs(back[1] - standing[1]) < 0.05, `back on its feet: ${back[1]} vs ${standing[1]}`);
assert.deepEqual(problems, [], "no page errors");
console.log("ok: queries, ragdoll, and restore all behave");
await browser.close();
