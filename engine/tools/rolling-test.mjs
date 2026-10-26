// Rolling things and hand-held tools in a real page (here a golf-like example): node tools/rolling-test.mjs [outdir]
// worlds/rolling_range.txt (a flat course, a green slab, a bunker, a cup) plus game text loaded with game_load. A simulated hand swings a tool
// frame by frame (so the head has a real velocity): a putt at the right speed drops into the pocket; the ground field is built from the parts;
// the tool is drawn turned with the hand.
import { chromium } from "../web/node_modules/playwright/index.mjs";
import { mkdirSync } from "node:fs";
const outDir = process.argv[2] ?? "/tmp/rolling-test";
mkdirSync(outDir, { recursive: true });
const GAME = `game name=rolling_test
var strokes=0
var holes=0
var outs=0
kind ball size=0.03 roll=1 bounce=0.3 part=ball_mesh
kind putter size=0.05 tags=hand_r swing=1 reach=0.9 power=1.0 part=club_mesh
kind cup size=0.08 pocket=1 part=cup_mesh
surface green parts=green decel=0.55 bounce=0.5
surface sand parts=bunker decel=8 bounce=0.1
terrain parts=course,green cell=0.2
input club follow=right_hand target=tag:hand_r
spawn putter at=0,1,0.9
spawn ball at=-1,0.03,0
spawn cup at=4,0.02,0
rule on=swing do=add:strokes:1
rule on=pocket do=add:holes:1;destroy:b
rule on=out do=add:outs:1
hud text="Strokes {strokes}   Holes {holes}" anchor=gaze size=0.004
`;
const browser = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
const logs = [];
page.on("pageerror", (e) => logs.push(String(e).slice(0, 300)));
await page.goto("http://localhost:8801/?viewer=1&eval=1&scene=open&quality=low&world=worlds/rolling_range.txt");
await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 120000 });
await page.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
await page.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final", null, { timeout: 300000 });
await page.keyboard.press("h");
const diagnostics = await page.evaluate((text) => window.__rt.game_load(text), GAME);
let failed = 0;
const check = (name, condition, detail) => { console.log(`${condition ? "ok  " : "FAIL"} ${name}${condition ? "" : `: ${JSON.stringify(detail).slice(0, 500)}`}`); if (!condition) failed++; };
check("the golf text parses", diagnostics === "", diagnostics);
await page.waitForFunction(() => JSON.parse(window.__rt.game_status()).present, null, { timeout: 30000 });
await page.waitForTimeout(2500);
let s = await page.evaluate(() => JSON.parse(window.__rt.game_status()));
console.log("ground:", JSON.stringify(s.ground), "vars:", JSON.stringify(s.vars));
check("the ground field was built from the course parts", s.ground?.present && s.ground.samples > 1000, s.ground);
const ballAt = () => page.evaluate(() => JSON.parse(window.__rt.game_status()).entities.filter((e) => e.kind === "ball").map((e) => e.position));
await page.evaluate(() => window.__rt.viewer_look_at(-2.2, 1.6, 2.2, 1.5, 0, 0));
await page.waitForTimeout(500);

// A putt: the club's grip hand moves along +X at `speed` m/s, one frame at a time, 0.9 m behind the ball line (the head is 0.9 m out along -Z).
const putt = (speed, fromX = -1.8, toX = -0.6) => page.evaluate(async ([speed, fromX, toX]) => {
  const rt = window.__rt;
  const frame = () => new Promise((r) => requestAnimationFrame(r));
  const setHand = (p) => rt.game_set_hands(JSON.stringify([{ side: "right", position: p, grip: false, trigger: false, rotation: [0, 0, 0, 1] }]));
  const stepLen = speed / 60;
  let x = fromX;
  setHand([x, 0.03, 0.9]); for (let i = 0; i < 6; i++) await frame();
  while (x < toX) { x += stepLen; setHand([x, 0.03, 0.9]); await frame(); }
  setHand([toX + 0.3, 0.8, 0.9]);
  for (let i = 0; i < 4; i++) await frame();
  return true;
}, [speed, fromX, toX]);
const settle = async (ms) => { await page.waitForTimeout(ms); };

// 5 m to the cup at a decel of 0.55 m/s2: it arrives at 1.0 m/s (under the 1.6 m/s the cup takes) when it leaves at sqrt(1 + 2 * 0.55 * 5) = 2.55 m/s.
await putt(2.55);
await settle(1500);
await page.screenshot({ path: `${outDir}/putt.png` });
await settle(4500);
s = await page.evaluate(() => JSON.parse(window.__rt.game_status()));
console.log("after the gentle putt:", JSON.stringify(s.vars), JSON.stringify(await ballAt()));
check("the swing counted as one stroke", s.vars.strokes === 1, s.vars);
check("the ball rolled and sank in the cup", s.vars.holes === 1, { vars: s.vars, ball: await ballAt() });
await page.screenshot({ path: `${outDir}/sunk.png` });
await browser.close();
check("no page errors", logs.length === 0, logs);
console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
process.exit(failed ? 1 : 0);
