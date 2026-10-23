// The game in a headset (IWER, Meta's emulated Quest 3): the hero is the player's head. When the game loads in a presenting session the tracking
// origin is moved so the player stands at the hero's spawn; walking (the emulated head moving in the room) moves the hero, whose feet are on the
// floor of the tracking space, and walking into a pickup scores. Emulation proves the logic; comfort and real tracking need the headset.
// Written before the bodies; do not weaken.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-xr.mjs
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';
import { driverAdvisory, here, serve } from './support.mjs';

const iwerBundle = fs.readFileSync(path.join(here, '..', 'node_modules', 'iwer', 'build', 'iwer.min.js'), 'utf8');
const program = [
  'coin_mesh = solid.sphere@1(radius=0.2, segments=12)',
  'score = game.var@1(name="score", value=0)',
  'hero = game.kind@1(name="player", size=0.5, tags=["hero"])',
  'coin = game.kind@1(name="coin", size=0.5, tags=["pickup"], part="coin_mesh")',
  'wall = game.kind@1(name="wall", size=0.3, tags=["wall"], solid=[0.3, 2, 4])',
  'p0 = game.spawn@1(kind="player", at=[10, 0, 5])',
  'w0 = game.spawn@1(kind="wall", at=[11, 1, 5])',
  'c0 = game.spawn@1(kind="coin", at=[12, 0, 5])',
  'walk = game.input@1(name="walk", follow="viewer", target="tag:hero")',
  'pick = game.rule@1(on="touch", a="tag:hero", b="tag:pickup", do="add:score:1;destroy:b")',
  'win = game.goal@1(outcome="win", when="score>=1")',
  'panel = game.hud@1(text="Coins {score}", size=0.005)',
  'all = game.collect@1(score, hero, coin, wall, p0, w0, c0, walk, pick, win, panel, name="xr")',
  'output coin_mesh = coin_mesh',
  'output rules = all',
].join('\n');

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
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
  await page.goto(`http://127.0.0.1:${port}/?scene=xr&eval=1&xr=auto&docname=game-xr-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'an xr game'), program);
  assert(staged.pending, JSON.stringify(staged).slice(0, 600));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 180000 });
  await page.waitForFunction(() => window.__ve.read().xr.state === 'Presenting' && window.__ve.read().xr.frames >= 8 && window.__ve.read().xr.view, null, { timeout: 60000 });
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const head = () => page.evaluate(() => window.__ve.read().xr.view.head_position);
  const player = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()).entities.find(e => e.kind === 'player').position);

  // The tracking origin is moved so the player stands at the spawn.
  await page.waitForFunction(() => {
    const h = window.__ve.read().xr.view.head_position;
    return Math.abs(h[0] - 10) < 0.3 && Math.abs(h[2] - 5) < 0.3;
  }, null, { timeout: 15000 });
  let h = await head();
  assert(Math.abs(h[0] - 10) < 0.3 && Math.abs(h[2] - 5) < 0.3, `the head is over the spawn: ${h}`);
  let p = await player();
  assert(Math.abs(p[0] - h[0]) < 0.3 && Math.abs(p[2] - h[2]) < 0.3, `the hero is under the head: ${p} against ${h}`);
  assert(Math.abs(p[1]) < 0.3, `and its feet are on the floor, not at head height: ${p[1]}`);

  // The HUD hangs in front of the player's head, not wherever the desktop camera is.
  const panel = (await page.evaluate(() => JSON.parse(window.__runtime.game_status()).hud.centers))[0];
  const away = Math.hypot(panel[0] - h[0], panel[1] - h[1], panel[2] - h[2]);
  assert(away > 1.0 && away < 1.7, `the panel is about 1.3 m from the head: ${away.toFixed(2)} m (panel ${panel}, head ${h})`);

  // A wall stands between the player and the coin. Walking at it in the room moves the hero up to it and no further, and the tracking origin is
  // shoved back so the player's head is held in front of the wall too.
  await page.evaluate(() => window.__xr.position.set(2, 1.6, 0));
  await page.waitForTimeout(600);
  h = await head();
  p = await player();
  assert(p[0] < 10.5 && p[0] > 10.2, `the hero is held in front of the wall: ${p}`);
  assert(h[0] < 10.6, `and so is the head: ${h}`);
  assert.equal((await page.evaluate(() => JSON.parse(window.__runtime.game_status()).vars.score)), 0, 'no coin through the wall');
  // Walk around it: the tracking origin moved, so aim by world position (head = origin + tracked head).
  const walkToWorld = async (x, z) => {
    const now = await head();
    const tracked = await page.evaluate(() => [window.__xr.position.x, window.__xr.position.z]);
    const origin = [now[0] - tracked[0], now[2] - tracked[1]];
    await page.evaluate(([tx, tz]) => window.__xr.position.set(tx, 1.6, tz), [x - origin[0], z - origin[1]]);
    await page.waitForTimeout(500);
  };
  await walkToWorld(10.3, 8.8);
  await walkToWorld(12, 8.8);
  await walkToWorld(12, 5);
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).vars.score === 1, null, { timeout: 15000 });
  const s = await page.evaluate(() => JSON.parse(window.__runtime.game_status()));
  assert.equal(s.status, 'won');
  assert.equal(s.entities.filter(e => e.kind === 'coin').length, 0);

  assert.deepEqual(logs, [], `browser logs: ${logs.join('; ')}`);
  console.log('game xr ok');
} finally {
  await browser.close();
  server.close();
}
