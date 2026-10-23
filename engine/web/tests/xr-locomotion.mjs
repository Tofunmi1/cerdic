// VR locomotion v1 against IWER (emulated Quest 3): the left thumbstick moves the player where the head faces, the right thumbstick snap-turns in fixed steps once per flick.
// The tracking origin (XrRig) is what moves; the player's head follows. Emulation proves the logic; comfort needs the headset. Written before the bodies; do not weaken.
// Run: cd engine/web && VE_DIST=dist-xr bun tests/xr-locomotion.mjs
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';
import { driverAdvisory, here, serve } from './support.mjs';

const iwerBundle = fs.readFileSync(path.join(here, '..', 'node_modules', 'iwer', 'build', 'iwer.min.js'), 'utf8');
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  page.on('console', m => { if (['error', 'warning'].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`); });
  page.on('pageerror', e => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    window.__xr = device;
  });
  await page.goto(`http://127.0.0.1:${port}/?scene=xr&xr=auto`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  await page.waitForFunction(() => window.__ve.read().xr.state === 'Presenting' && window.__ve.read().xr.frames >= 8 && window.__ve.read().xr.view, null, { timeout: 60000 });
  const view = () => page.evaluate(() => window.__ve.read().xr.view);
  const stick = (hand, x, y) => page.evaluate(([h, a, b]) => window.__xr.controllers[h].updateAxes('thumbstick', a, b), [hand, x, y]);
  const settle = ms => page.waitForTimeout(ms);

  // Nothing moves while the sticks rest.
  const start = await view();
  await settle(400);
  const rest = await view();
  assert(Math.hypot(rest.head_position[0] - start.head_position[0], rest.head_position[2] - start.head_position[2]) < 0.01, 'the player stands still');

  // Pushing the left stick forward walks where the head faces (the head faces -z at the start).
  await stick('left', 0, -1);
  await settle(1000);
  await stick('left', 0, 0);
  await settle(200);
  const walked = await view();
  const dz = walked.head_position[2] - start.head_position[2];
  const dx = walked.head_position[0] - start.head_position[0];
  assert(dz < -1.0 && dz > -4.0, `about a second of walking forward moved the player 1 to 4 m toward -z: dz ${dz.toFixed(2)}`);
  assert(Math.abs(dx) < 0.3, `and not sideways: dx ${dx.toFixed(2)}`);
  assert(Math.abs(walked.head_position[1] - start.head_position[1]) < 0.05, 'and not up or down');
  await settle(400);
  const stopped = await view();
  assert(Math.hypot(stopped.head_position[0] - walked.head_position[0], stopped.head_position[2] - walked.head_position[2]) < 0.02, 'releasing the stick stops the player');

  // Pushing it sideways strafes.
  await stick('left', 1, 0);
  await settle(700);
  await stick('left', 0, 0);
  await settle(200);
  const strafed = await view();
  assert(strafed.head_position[0] - stopped.head_position[0] > 0.6, `the right push moved the player to its right (+x): ${(strafed.head_position[0] - stopped.head_position[0]).toFixed(2)}`);

  // A flick of the right stick turns by one step (30 degrees) and holding it does not keep turning; centring and flicking again turns once more.
  const yaw = async () => (await view()).head_yaw;
  const y0 = await yaw();
  await stick('right', 1, 0);
  await settle(600);
  const y1 = await yaw();
  const step = Math.abs(((y1 - y0 + Math.PI) % (2 * Math.PI)) - Math.PI) * 180 / Math.PI;
  assert(Math.abs(step - 30) < 3, `one flick is one 30 degree snap turn: ${step.toFixed(1)}`);
  await settle(600);
  assert(Math.abs((await yaw()) - y1) < 0.01, 'holding the stick does not keep turning');
  await stick('right', 0, 0);
  await settle(200);
  await stick('right', -1, 0);
  await settle(300);
  await stick('right', 0, 0);
  await settle(200);
  const y2 = await yaw();
  assert(Math.abs(((y2 - y0 + Math.PI) % (2 * Math.PI)) - Math.PI) < 0.05 || Math.abs(y2 - y0) < 0.05, `a flick the other way turns back: ${(y2 - y0).toFixed(3)}`);

  // After a turn, forward is where the head now faces: turn right, walk, and the player's heading follows the turn.
  await stick('right', 1, 0);
  await settle(300);
  await stick('right', 0, 0);
  await settle(200);
  const before = await view();
  await stick('left', 0, -1);
  await settle(800);
  await stick('left', 0, 0);
  await settle(200);
  const after = await view();
  const moveX = after.head_position[0] - before.head_position[0];
  const moveZ = after.head_position[2] - before.head_position[2];
  assert(Math.hypot(moveX, moveZ) > 0.8, 'it walked');
  const heading = Math.atan2(-moveX, -moveZ);
  const diff = Math.abs(((heading - after.head_yaw + Math.PI) % (2 * Math.PI)) - Math.PI);
  assert(diff < 0.2, `forward follows the head after a turn: movement heading ${heading.toFixed(2)} against head yaw ${after.head_yaw.toFixed(2)}`);

  assert.deepEqual(logs, [], `browser logs: ${logs.join('; ')}`);
  console.log('xr locomotion ok');
} finally {
  await browser.close();
  server.close();
}
