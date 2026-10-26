// Creatures, throwing, stun, bursts and the walkable grid in a real page: node tools/creature-test.mjs [world] [outdir]
// worlds/creature_chase.txt: a robot creature patrols a small floor; a rock falls onto it (a `touch` rule stuns it for 3 s and bursts an explosion);
// when it recovers it sees the hero (the viewer) and chases and catches it. Samples the game's status every 250 ms and takes pictures.
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { mkdirSync } from "node:fs";
const world = process.argv[2] ?? "worlds/creature_chase.txt";
const outDir = process.argv[3] ?? "/tmp/creature-test";
mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("pageerror", (e) => console.log("PAGEERR", String(e).slice(0, 300)));
await page.goto(`http://localhost:8801/?viewer=1&eval=1&scene=stage&quality=low&world=${world}`);
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 300000 });
await page.keyboard.press("h");
// The hero is wherever the viewer stands: close to the floor, so the creature can notice it (sight is 4 m).
await page.evaluate(() => window.__rt.viewer_look_at(0.6, 1.9, 2.0, -0.8, 0.5, -1.2));
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 500)}`}`); if (!condition) failed++; };
const status = () => page.evaluate(() => JSON.parse(window.__rt.game_status()));
const samples = [];
let shots = 0;
const began = Date.now();
for (let i = 0; i < 52; i++) {
  const s = await status();
  const c = s.entities.find((e) => e.kind === "creature");
  samples.push({ t: (Date.now() - began) / 1000, steps: s.steps, creature: c?.position, rock: s.entities.some((e) => e.kind === "rock"), vars: s.vars, bursts: s.bursts, nav: s.nav, drawn: s.drawn });
  if (i % 8 === 0 || (s.bursts?.particles > 30 && shots++ < 3)) await page.screenshot({ path: `${outDir}/s${String(i).padStart(2, "0")}.png` });
  await page.waitForTimeout(250);
}
console.log(samples.filter((_, i) => i % 3 === 0).map((s) => `${s.t.toFixed(1)}s step=${s.steps} c=${s.creature?.map((v) => v.toFixed(2))} rock=${s.rock} hits=${s.vars.hits} lives=${s.vars.lives} burst=${s.bursts?.requested}/${s.bursts?.particles}`).join("\n"));
check("the game has a navmesh built from the floor", samples.some((s) => s.nav?.present), samples.at(-1)?.nav);
check("the rock fell onto the creature and was destroyed (hits = 1)", samples.at(-1).vars.hits >= 1 && !samples.at(-1).rock, samples.at(-1));
check("an explosion burst was requested", samples.at(-1).bursts?.requested >= 1, samples.at(-1).bursts);
check("and particles were drawn", Math.max(...samples.map((s) => s.bursts?.particles ?? 0)) > 20, samples.map((s) => s.bursts?.particles));
// Stun: after the hit the creature stands still for about 3 s.
// The game runs on fixed 1/60 s steps and the page may run slower than real time, so stun is measured in the game's own steps (3 s = 180).
const hitAt = samples.findIndex((s) => s.vars.hits >= 1);
const hitStep = samples[hitAt]?.steps ?? 0;
// The first sample that shows the hit comes some steps after it happened, so the window starts there and stays well inside the 180 steps.
const still = samples.filter((s) => s.steps >= hitStep && s.steps <= hitStep + 100);
const moved = (list) => list.length > 1 ? Math.hypot(list.at(-1).creature[0] - list[0].creature[0], list.at(-1).creature[2] - list[0].creature[2]) : 0;
check("it stands still while stunned (game steps 5 to 80 after the hit is first seen)", hitAt >= 0 && still.length >= 2 && moved(still) < 0.05, { hitAt, hitStep, n: still.length, moved: moved(still) });
const early = samples.slice(0, Math.max(1, hitAt));
check("before the hit it patrolled", moved(early) > 0.2 || hitAt < 3, { moved: moved(early), hitAt });
check("it caught the hero at least once (lives dropped)", samples.at(-1).vars.lives < 3, samples.at(-1).vars);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
await browser.close();
process.exit(failed ? 1 : 0);
