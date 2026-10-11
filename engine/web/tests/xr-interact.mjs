// Interaction acceptance test, controller part, against IWER (Meta's emulated Quest 3): a controller
// pointing at the cube hovers it, the squeeze grabs it and carries it with the hand, and two hands
// squeezing it scale it. Emulation proves the logic; how it feels needs the headset.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { driverAdvisory, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => {
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    window.__xr = device;
  });

  await page.goto(`http://127.0.0.1:${port}/?scene=xr&xr=auto`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 8, null, { timeout: 60000 });

  const state = () => page.evaluate(() => window.__ve.read().interact);
  const cube = async () => (await state()).objects.find((o) => o.label === "cube");
  const settle = async (n = 6) => {
    const target = (await page.evaluate(() => window.__ve.read().xr.frames)) + n;
    await page.waitForFunction((t) => window.__ve.read().xr.frames >= t, target, { timeout: 30000 });
  };
  /** Places a controller (tracking space, metres) and sets its squeeze. */
  const hand = (side, x, y, z, squeeze) => page.evaluate(([s, px, py, pz, q]) => {
    const c = window.__xr.controllers[s];
    c.position.set(px, py, pz);
    c.setButtonValueImmediate("squeeze", q);
  }, [side, x, y, z, squeeze]);

  // The rig sits 2.2 m in front of the cube, which floats at 1.4 m: a controller at (0, 1.4, -0.2)
  // pointing straight ahead (-z) is aimed at it.
  const home = await cube();
  check(home !== undefined, "the cube is not interactable");
  await hand("right", 0, 1.4, -0.2, 0);
  await settle();
  const hovered = await state();
  check(hovered.hover.some((h) => h.pointer === 2 && h.target === "cube"), `the right controller does not hover the cube: ${JSON.stringify(hovered.hover)}`);

  // Grab with the squeeze, then carry the cube 0.3 m to the right with the hand.
  await hand("right", 0, 1.4, -0.2, 1);
  await settle();
  check((await state()).held.some((h) => h.target === "cube" && h.holders.includes(2)), "the squeeze did not grab the cube");
  await hand("right", 0.3, 1.4, -0.2, 1);
  await settle();
  const carried = await cube();
  check(Math.abs(carried.position[0] - home.position[0] - 0.3) < 0.05, `the cube moved ${carried.position[0] - home.position[0]} m in x, expected 0.3`);
  check(Math.abs(carried.scale - 1) < 1e-3, `one hand changed the scale to ${carried.scale}`);

  // Release: it stays where it was dropped.
  await hand("right", 0.3, 1.4, -0.2, 0);
  await settle();
  check((await state()).held.length === 0, "the cube is still held after the squeeze ended");
  const dropped = await cube();
  await hand("right", 0.6, 1.4, -0.2, 0);
  await settle();
  const afterDrop = await cube();
  check(afterDrop.position.every((v, i) => Math.abs(v - dropped.position[i]) < 1e-4), "the cube followed a hand that was not holding it");

  // Two hands: both aim at the cube and squeeze, then spread apart; the cube grows.
  const x0 = dropped.position[0];
  await hand("left", x0 - 0.1, 1.4, -0.2, 0);
  await hand("right", x0 + 0.1, 1.4, -0.2, 0);
  await settle();
  await hand("left", x0 - 0.1, 1.4, -0.2, 1);
  await hand("right", x0 + 0.1, 1.4, -0.2, 1);
  await settle();
  const both = (await state()).held.find((h) => h.target === "cube");
  check(both?.holders.length === 2, `two hands did not both hold the cube: ${JSON.stringify(both)}`);
  await hand("left", x0 - 0.3, 1.4, -0.2, 1);
  await hand("right", x0 + 0.3, 1.4, -0.2, 1);
  await settle();
  const grown = await cube();
  check(grown.scale > 2.5 && grown.scale < 3.5, `spreading the hands to three times the distance gave scale ${grown.scale}`);
  await hand("left", x0 - 0.3, 1.4, -0.2, 0);
  await hand("right", x0 + 0.3, 1.4, -0.2, 0);
  await settle();
  check((await state()).held.length === 0, "the cube is still held after both squeezes ended");

  // A controller that disappears mid-grab drops the cube once and cannot take it back by returning.
  await hand("right", x0, 1.4, -0.2, 0);
  await settle();
  await hand("right", x0, 1.4, -0.2, 1);
  await settle();
  check((await state()).held.some((h) => h.target === "cube"), "the cube was not held before the controller vanished");
  const eventsBefore = (await state()).events.filter((e) => e.startsWith("drop 2")).length;
  await page.evaluate(() => { window.__xr.controllers.right.connected = false; });
  await settle();
  const gone = await state();
  check(gone.held.length === 0, "the cube is still held after its controller disappeared");
  check(gone.events.filter((e) => e.startsWith("drop 2")).length === eventsBefore + 1, "the drop was not reported exactly once");
  const before = await cube();
  await hand("right", 1.5, 3.0, -0.2, 1);
  await page.evaluate(() => { window.__xr.controllers.right.connected = true; });
  await settle();
  check((await state()).held.length === 0, "a returning controller took the cube back");
  const after = await cube();
  check(after.position.every((v, i) => Math.abs(v - before.position[i]) < 1e-4), "the cube moved while nobody held it");

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`xr-interact test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("xr-interact test passed");
