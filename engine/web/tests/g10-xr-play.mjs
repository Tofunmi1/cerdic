// G10: play mode in a headset. The Y button on the left controller (B on the right) turns play mode on and off on
// the press, against IWER's emulated Quest 3. Behaviours themselves are covered by g10-behavior and g10-logic;
// this checks the headset can reach play mode at all.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { driverAdvisory, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };
const program = `door = solid.box@1(size=[0.9, 2, 0.05])
swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 1:0,90,0", pivot=[-0.45, 0, 0], loop=false)
latch = behavior.on@1(swing, trigger="select", action="toggle")
output front_door = latch
`;
const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => { if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`); });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    window.__xr = device;
  });
  await page.goto(`http://127.0.0.1:${port}/?eval=1&xr=auto&docname=g10-xr-play`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import("./ve_runtime.js"); });
  const staged = await page.evaluate((text) => window.__ve.doc.sendText(text, "xr play"), program);
  check(Boolean(staged.pending), `staged: ${JSON.stringify(staged).slice(0, 200)}`);
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === "final", null, { timeout: 180000 });
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 8, null, { timeout: 60000 });
  const frames = async (n) => {
    const target = (await page.evaluate(() => window.__ve.read().xr.frames)) + n;
    await page.waitForFunction((t) => window.__ve.read().xr.frames >= t, target, { timeout: 30000 });
  };
  const playing = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  const hold = async (button, value) => { await page.evaluate(([b, v]) => window.__xr.controllers.left.setButtonValueImmediate(b, v), [button, value]); await frames(6); };

  check((await playing()) === false, "play mode starts off");
  await hold("y-button", 1);
  check((await playing()) === true, "pressing Y did not turn play mode on");
  await hold("y-button", 0);
  check((await playing()) === true, "releasing Y changed play mode");
  await hold("y-button", 1);
  check((await playing()) === false, "pressing Y again did not turn play mode off");
  await hold("y-button", 0);
  // A held button toggles once, not every frame.
  await hold("y-button", 1);
  await frames(20);
  check((await playing()) === true, "a held Y button kept toggling");
  check(logs.length === 0, `browser logs: ${logs.join("; ")}`);
} finally {
  await browser.close();
  server.close();
}
if (failures.length) {
  console.error(`G10 xr play FAILED:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("G10 xr play: Y toggles play mode once per press in the emulated headset");
