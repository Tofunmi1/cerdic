// The desktop viewer: with ?viewer=1 the camera can be turned with the mouse, moved with the keys,
// widened, split into a mock headset's two eyes and reset; while the pointer is held the crosshair
// picks and grabs. Scenes are untouched without ?viewer=1 (the other tests prove that).
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { differingFraction, drawnPixels, driverAdvisory, serve } from "./support.mjs";

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

  await page.goto(`http://127.0.0.1:${port}/?scene=xr&viewer=1&fov=70`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
  const view = () => page.evaluate(() => window.__ve.read().viewer);
  const settle = async (n = 6) => {
    const start = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction((t) => window.__ve.read().frames >= t, start + n, { timeout: 30000 });
  };
  const api = (call, ...args) => page.evaluate(([name, rest]) => window.__ve.viewer.api[name](...rest), [call, args]);
  await page.waitForFunction(() => window.__ve.read().viewer.started, null, { timeout: 30000 });
  check(await page.evaluate(() => !!document.getElementById("viewer-hud")), "the viewer panel is missing");

  // The viewer starts where the scene's own camera was: standing back from the cube, looking at it.
  const home = await view();
  check(Math.abs(home.position[2] - 2.2) < 0.01 && home.position[1] > 1.5, `start position ${home.position}`);

  // Looking: moving the mouse right turns the view right (yaw decreases), moving it down looks down.
  await api("viewer_look", 200, 0);
  await settle();
  const turned = await view();
  check(Math.abs(turned.yaw - home.yaw + 200 * 0.0022) < 0.01, `yaw ${home.yaw} -> ${turned.yaw}`);
  await api("viewer_look", 0, 100000);
  await settle();
  const clamped = await view();
  check(clamped.pitch > -1.6 && clamped.pitch < -1.5, `pitch is not clamped short of straight down: ${clamped.pitch}`);
  await api("viewer_reset");
  await settle();
  const back = await view();
  check(back.position.every((v, i) => Math.abs(v - home.position[i]) < 1e-4) && Math.abs(back.yaw - home.yaw) < 1e-4, "reset did not restore the start view");

  // Moving: holding W flies forward, which is toward -z from the start; releasing stops.
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(700);
  await page.keyboard.up("KeyW");
  await settle();
  const flown = await view();
  check(home.position[2] - flown.position[2] > 0.3, `holding W moved z ${home.position[2]} -> ${flown.position[2]}`);
  await settle(10);
  const stopped = await view();
  check(Math.abs(stopped.position[2] - flown.position[2]) < 1e-4, "the viewer kept moving after the key was released");
  await api("viewer_reset");
  await settle();

  // Field of view: a much wider view shows the scene smaller, so the picture changes a lot.
  await api("viewer_configure", true, 50, false, 3);
  await settle(8);
  const narrow = PNG.sync.read(await page.screenshot());
  await api("viewer_configure", true, 140, false, 3);
  await settle(8);
  const wide = PNG.sync.read(await page.screenshot());
  check(differingFraction(narrow, wide, 20) > 0.05, "changing the field of view did not change the picture");
  check(drawnPixels(wide, 0, wide.width) > 2000, "the wide view drew nothing");

  // Split screen: two eyes side by side, both drawn, with parallax between them; back to one view.
  await api("viewer_configure", true, 90, true, 3);
  await settle(10);
  check((await view()).split_screen === true, "split screen was not reported");
  const split = PNG.sync.read(await page.screenshot());
  const half = split.width >> 1;
  check(drawnPixels(split, 0, half) > 2000 && drawnPixels(split, half, split.width) > 2000, "an eye of the split view shows nothing");
  await api("viewer_look", -150, 0);
  await api("viewer_move", 0, 0, 0, false);
  await settle(8);
  check((await view()).yaw !== turned.yaw, "the split-screen view did not follow the mouse");
  await api("viewer_configure", true, 90, false, 3);
  await settle(8);
  check((await view()).split_screen === false, "split screen did not switch off");
  const mono = PNG.sync.read(await page.screenshot());
  check(drawnPixels(mono, 0, mono.width) > 2000, "the single view drew nothing after leaving split screen");
  await api("viewer_reset");
  await settle();

  // Lenses: a fisheye bends the whole frame and a headset lens bends each eye; both change the
  // picture, both still draw, and a lens that does not fit the layout is ignored rather than wrong.
  const lens = async (name, fov, split) => {
    await api("viewer_configure", true, fov, split, 3);
    await api("viewer_set_lens", name);
    await settle(10);
    return PNG.sync.read(await page.screenshot());
  };
  check((await api("viewer_set_lens", "bogus")) === false, "an unknown lens name was accepted");
  const flat = await lens("off", 130, false);
  const fisheye = await lens("fisheye", 130, false);
  check((await view()).lens === "fisheye", `the lens was not reported: ${(await view()).lens}`);
  check(differingFraction(flat, fisheye, 20) > 0.1, "the fisheye did not change the picture");
  check(drawnPixels(fisheye, 0, fisheye.width) > 2000, "the fisheye drew nothing");
  const corner = (png) => { const i = 4 * (3 * png.width + 3); return png.data[i] + png.data[i + 1] + png.data[i + 2]; };
  check(corner(fisheye) === 0, "the fisheye corner is not black: the source cannot fill it");
  const mismatched = await lens("headset", 130, false);
  check(differingFraction(flat, mismatched, 8) < 0.01, "a headset lens changed a single (not split) view");
  const plainSplit = await lens("off", 100, true);
  const headset = await lens("headset", 100, true);
  check(differingFraction(plainSplit, headset, 20) > 0.1, "the headset lens did not change the split view");
  check(drawnPixels(headset, 0, headset.width >> 1) > 2000 && drawnPixels(headset, headset.width >> 1, headset.width) > 2000, "an eye vanished under the headset lens");
  await lens("off", 90, false);
  await api("viewer_reset");
  await settle();

  // A click on the view asks for the pointer; a refusal (headless browsers refuse) must not error.
  await page.mouse.click(320, 180);
  await page.waitForTimeout(300);
  const granted = await page.evaluate(() => document.pointerLockElement !== null);

  // Pointer held: the crosshair at the centre hovers and grabs the cube it points at. A headless
  // browser cannot hold the pointer, so tell the engine directly; the DOM side is a few lines.
  await api("viewer_set_locked", true);
  await page.mouse.move(5, 5); // the cursor is elsewhere: only the crosshair may count
  await settle(6);
  check((await view()).locked === true, "the engine was not told the pointer is held");
  const hover = await page.evaluate(() => window.__ve.read().interact.hover);
  check(hover.some((h) => h.target === "cube" && h.pointer === 0), `the crosshair does not hover the cube: ${JSON.stringify(hover)}`);
  await page.mouse.down();
  await settle(6);
  check((await page.evaluate(() => window.__ve.read().interact.held)).some((h) => h.target === "cube"), "pressing with the crosshair on the cube did not grab it");
  await page.mouse.up();
  await settle();
  await api("viewer_set_locked", false);
  await settle(6);
  check((await page.evaluate(() => window.__ve.read().interact.hover)).every((h) => h.target !== "cube"), "the cube stayed hovered after the pointer was released and the cursor is elsewhere");

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
  console.log(JSON.stringify({ pointerLockGrantedByBrowser: granted }));
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`viewer test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("viewer test passed");
