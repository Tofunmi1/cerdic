// Interaction acceptance test, mouse part: hovering an object highlights it, dragging moves it,
// the wheel pushes and pulls it along the ray, clicking reports a click, and letting go leaves it
// where it was dropped. Objects are found by sweeping the cursor and reading the status report, so
// the test does not depend on where the camera happens to put them.
import { chromium } from "playwright";
import { driverAdvisory, serve } from "./support.mjs";

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

  await page.goto(`http://127.0.0.1:${port}/?scene=shadows`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);

  const state = () => page.evaluate(() => window.__ve.read().interact);
  const object = async (label) => (await state()).objects.find((o) => o.label === label);
  /** Waits until the engine has run `n` more frames, so input has been processed. */
  const settle = async (n = 4) => {
    const start = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction((target) => window.__ve.read().frames >= target, start + n, { timeout: 20000 });
  };
  /** Moves the cursor over a grid until it hovers `label`; returns the position. */
  const findOnScreen = async (label) => {
    for (let y = 40; y < 340; y += 20) {
      for (let x = 20; x < 620; x += 20) {
        await page.mouse.move(x, y);
        await settle(2);
        if ((await state()).hover.some((h) => h.target === label)) return { x, y };
      }
    }
    return null;
  };

  const start = await state();
  check(start.objects.length === 3, `expected 3 interactable objects, got ${start.objects.length}`);
  check(start.hover.length === 0 && start.held.length === 0, "something was hovered or held before any input");

  // Hover: the cursor over the box hovers it, and moving off clears it.
  const spot = await findOnScreen("box");
  check(spot !== null, "no cursor position hovers the box");
  if (spot) {
    check((await state()).events.some((e) => e.startsWith("hover 0 on box")), "no hover event for the box");
    await page.mouse.move(2, 2);
    await settle();
    check(!(await state()).hover.some((h) => h.target === "box"), "the box is still hovered after moving off it");

    // A click without moving reports a click and moves nothing.
    const before = await object("box");
    await page.mouse.move(spot.x, spot.y);
    await settle();
    await page.mouse.down();
    await settle();
    await page.mouse.up();
    await settle();
    const clicked = await state();
    check(clicked.events.some((e) => e.includes("release 0 on box (click)")), `no click event: ${JSON.stringify(clicked.events.slice(-4))}`);
    const afterClick = await object("box");
    check(afterClick.position.every((v, i) => Math.abs(v - before.position[i]) < 1e-3), "a click moved the box");

    // Drag: press, move 80 px right, release. The box follows and stays where it was dropped.
    await page.mouse.down();
    await settle();
    check((await state()).held.some((h) => h.target === "box"), "pressing on the box did not grab it");
    await page.mouse.move(spot.x + 80, spot.y, { steps: 8 });
    await settle(6);
    const dragged = await object("box");
    check(Math.abs(dragged.position[0] - before.position[0]) > 0.2, `dragging right did not move the box: ${before.position} -> ${dragged.position}`);
    await page.mouse.up();
    await settle();
    check((await state()).held.length === 0, "the box is still held after the button was released");
    const dropped = await object("box");
    await settle(6);
    const later = await object("box");
    check(dropped.position.every((v, i) => Math.abs(v - later.position[i]) < 1e-4), "the box drifted after being dropped");

    // Wheel: holding the box and scrolling pushes it along the ray, away from the camera.
    await page.mouse.move(spot.x + 80, spot.y);
    await settle();
    await page.mouse.down();
    await settle();
    const held = await object("box");
    await page.mouse.wheel(0, -200);
    await settle(6);
    const pushed = await object("box");
    const moved = Math.hypot(...pushed.position.map((v, i) => v - held.position[i]));
    check(moved > 0.1, `the wheel did not move the held box (${moved} m)`);
    await page.mouse.up();
    await settle();
  }

  check(logs.length === 0, `page logged problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length) {
  console.error(`interact test FAILED:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("interact test passed");
