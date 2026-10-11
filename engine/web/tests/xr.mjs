// WebXR acceptance test against IWER, Meta's emulated Quest 3: a session starts, both eyes render,
// head movement reaches the camera, and ending the session returns the engine to normal rendering.
// Emulation proves the logic and the frame loop; frame time and comfort need the real headset.
// Set UPDATE_GOLDEN=1 to rewrite the XR golden image after an intentional rendering change.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { differingFraction, drawnPixels, driverAdvisory, GOLDEN, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const goldenPath = path.join(here, "golden", "xr-640x360.png");
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
  // The emulator must exist before any page script runs, like a real browser's navigator.xr.
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    // Chromium ships a native navigator.xr with no devices behind it; replace it with the emulator.
    device.installRuntime({ forceInstall: true });
    // IWER emulates a mono view by default (the right eye is zero-width); a headset is stereo.
    device.stereoEnabled = true;
    window.__xr = device;
  });

  await page.goto(`http://127.0.0.1:${port}/?scene=xr&xr=auto`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
  check(await page.evaluate(() => window.__ve.xrSupported), "the emulated runtime was not detected as immersive-vr capable");

  const xr = () => page.evaluate(() => window.__ve.read().xr);
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 8 && window.__ve.read().xr.view, null, { timeout: 60000 });
  const presenting = await xr();
  check(presenting.error === null, `xr reported an error: ${presenting.error}`);

  // Eyes: separated by the device's IPD, the head where the rig puts it, two side-by-side viewports.
  const ipd = await page.evaluate(() => window.__xr.ipd);
  const view = presenting.view;
  check(Math.abs(view.eye_separation_m - ipd) < 0.003, `eye separation ${view.eye_separation_m} != device IPD ${ipd}`);
  check(Math.abs(view.head_position[2] - 2.2) < 0.15, `head z ${view.head_position[2]} is not near the rig at 2.2`);
  check(view.head_position[1] > 1.2 && view.head_position[1] < 2.0, `head height ${view.head_position[1]} is not standing height`);
  const [left, right] = view.viewports;
  check(left[0] === 0 && left[2] + right[2] === 640 && left[3] === 360, `viewports do not tile the canvas: ${JSON.stringify(view.viewports)}`);

  // Pixels: both eyes show the scene, with parallax between them.
  const first = PNG.sync.read(await page.screenshot());
  const half = first.width >> 1;
  check(drawnPixels(first, 0, half) > 2000 && drawnPixels(first, half, first.width) > 2000, "an eye shows nothing");
  let apart = 0;
  for (let y = 0; y < first.height; y++) {
    for (let x = 0; x < half; x++) {
      const a = (first.width * y + x) * 4;
      const b = (first.width * y + x + half) * 4;
      if (Math.max(Math.abs(first.data[a] - first.data[b]), Math.abs(first.data[a + 1] - first.data[b + 1]), Math.abs(first.data[a + 2] - first.data[b + 2])) > 20) apart++;
    }
  }
  check(apart > half * first.height * 0.002, "the eyes are identical: no parallax");

  if (process.env.UPDATE_GOLDEN) {
    fs.writeFileSync(goldenPath, PNG.sync.write(first));
    console.log(`golden written: ${goldenPath}`);
  } else if (!fs.existsSync(goldenPath)) {
    failures.push(`no golden image at ${goldenPath}; run with UPDATE_GOLDEN=1 once and review it`);
  } else {
    const fraction = differingFraction(first, PNG.sync.read(fs.readFileSync(goldenPath)));
    check(fraction <= GOLDEN.maxBadFraction, `XR frame differs from the golden image in ${(fraction * 100).toFixed(3)}% of pixels`);
  }

  // Head movement: step half a metre to the right; the camera follows and the picture changes.
  await page.evaluate(() => window.__xr.position.set(0.5, 1.6, 0));
  const before = presenting.frames;
  await page.waitForFunction((n) => window.__ve.read().xr.frames >= n + 6, before, { timeout: 30000 });
  const moved = (await xr()).view;
  const dx = moved.head_position[0] - view.head_position[0];
  check(Math.abs(dx - 0.5) < 0.05, `head moved ${dx} m in x, expected about 0.5`);
  const second = PNG.sync.read(await page.screenshot());
  check(differingFraction(first, second, 20) > 0.01, "the picture did not change when the head moved");

  // Controllers: a tracked hand puts its wand in the world, moves it with the hand and reads the trigger.
  const nextFrames = async (n) => {
    const target = (await xr()).frames + n;
    await page.waitForFunction((t) => window.__ve.read().xr.frames >= t, target, { timeout: 30000 });
  };
  await page.evaluate(() => {
    const right = window.__xr.controllers.right;
    right.position.set(0.3, 1.2, -0.2);
    right.setButtonValueImmediate("trigger", 0);
  });
  await nextFrames(6);
  const handAt = async () => (await page.evaluate(() => window.__ve.read().xr.controllers)).find((c) => c.hand === "Right");
  const resting = await handAt();
  check(resting?.tracked === true, "the right controller is not tracked");
  check(Array.isArray(resting?.wand_position), "the right wand has no position");
  // The rig sits at z = 2.2, so a controller 0.2 m in front of the origin is about 2.0 in the world.
  check(Math.abs(resting.wand_position[2] - 2.0) < 0.2 && Math.abs(resting.wand_position[0] - 0.3) < 0.2, `wand at ${resting?.wand_position}, expected near (0.3, 1.2, 2.0)`);
  await page.evaluate(() => window.__xr.controllers.right.position.set(0.7, 1.2, -0.2));
  await nextFrames(6);
  const shifted = await handAt();
  check(Math.abs(shifted.wand_position[0] - resting.wand_position[0] - 0.4) < 0.03, `wand moved ${shifted.wand_position[0] - resting.wand_position[0]} m, expected 0.4`);
  check(resting.trigger < 0.05, `trigger reads ${resting.trigger} at rest`);
  await page.evaluate(() => window.__xr.controllers.right.setButtonValueImmediate("trigger", 1));
  await nextFrames(6);
  check((await handAt()).trigger > 0.95, "the trigger press did not reach the app");
  const leftHand = (await page.evaluate(() => window.__ve.read().xr.controllers)).find((c) => c.hand === "Left");
  check(leftHand !== undefined, "the left controller is missing");

  // Ending the session returns the engine to ordinary rendering on the window loop.
  await page.evaluate(() => window.__ve.exitXr());
  await page.waitForFunction(() => window.__ve.read().xr.state === "Ended", null, { timeout: 30000 });
  const framesAtEnd = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction((n) => window.__ve.read().frames >= n + 10, framesAtEnd, { timeout: 30000 })
    .catch(() => failures.push("the app stopped updating after the XR session ended"));
  check((await xr()).view === null, "the headset camera kept its stereo eyes after the session ended");
  const after = PNG.sync.read(await page.screenshot());
  check(drawnPixels(after, 0, after.width) > 2000, "nothing is drawn after the session ended");

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
  console.log(JSON.stringify({ ipd, eyeSeparation: view.eye_separation_m, headStart: view.head_position, headMoved: moved.head_position, xrFrames: (await xr()).frames }, null, 2));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nxr checks passed");
