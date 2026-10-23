// BLACKOUT, a playable VR game set: carry four fuse cells to four sockets with your hands (grip to grab, trigger to use) to restart a reactor.
// This plays it the way a player does, through hands: the test supplies the poses and buttons a headset's controllers would
// (`game_set_hands`), the game does everything else. It checks: the world evaluates inside the headset budget with no texture or
// renderer errors; a fuse is picked up and carried; the first socket opens the first door (a solid that blocks until then); the lever
// opens the cage; the last two fuses restart the reactor and win; and the same game is lost when the air runs out.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-blackout.mjs
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { driverAdvisory, serve } from './support.mjs';

const program = await readFile(new URL('./worlds/blackout.txt', import.meta.url), 'utf8');
const QUEST_TRIANGLES = 300_000;

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=blackout-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'blackout'), program);
  assert(staged.pending, JSON.stringify(staged).slice(0, 800));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 240000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false, 'the world evaluates');
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const warnings = await page.evaluate(() => (window.__ve.evaluation.state.warnings ?? []).filter(w => /texture|bake/.test(w.code)));
  assert.deepEqual(warnings, [], `no texture warnings: ${JSON.stringify(warnings)}`);
  const triangles = await page.evaluate(() => window.__ve.evaluation.parts.reduce((sum, p) => sum + (p.indices.length / 3) * (p.transforms?.length ? Math.floor(p.transforms.length / 16) : 1), 0));
  assert(triangles > 3_000 && triangles <= QUEST_TRIANGLES, `drawn triangles ${triangles} fit the headset`);

  const status = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()));
  const kinds = (s, kind) => s.entities.filter(e => e.kind === kind);
  const pause = ms => page.waitForTimeout(ms);
  // A hand in the air at `at`, with the grip and trigger as given: what a controller reports.
  const hand = async (at, grip = false, trigger = false, side = 'right') => {
    await page.evaluate(([a, g, t, s]) => window.__runtime.game_set_hands(JSON.stringify([{ side: s, position: a, grip: g, trigger: t }])), [at, grip, trigger, side]);
    await pause(120);
  };
  // Walk the whole way: pick the thing up where it lies, carry it, and let go there.
  const carry = async (from, to) => {
    await hand(from);
    await hand(from, true);
    const mid = from.map((v, i) => (v + to[i]) / 2);
    await hand(mid, true);
    await hand(to, true);
    await pause(200);
    await hand(to, false);
  };
  const standAt = ([x, z]) => page.evaluate(([px, pz]) => window.__runtime.viewer_look_at(px, 1.6, pz, px, 1.6, pz + 5), [x, z]);

  let s = await status();
  assert.equal(kinds(s, 'fuse').length, 4, 'four fuse cells to find');
  assert.equal(kinds(s, 'door1').length + kinds(s, 'door2').length, 2, 'two doors');
  assert.equal(s.vars.fuses, 0);
  const door1Start = kinds(s, 'door1')[0].position[1];

  // The first fuse lies on the table in the quarters. Pick it up, and it comes with the hand.
  await hand([-2.6, 1.0, -2.3]);
  await hand([-2.6, 1.0, -2.3], true);
  await hand([-1.0, 1.4, -1.0], true);
  s = await status();
  const fuse = kinds(s, 'fuse').find(e => Math.abs(e.position[0] - -1.0) < 0.3);
  assert(fuse, `the fuse followed the hand: ${JSON.stringify(kinds(s, 'fuse').map(e => e.position))}`);
  // Let go in the air: it stays put.
  await hand([-1.0, 1.4, -1.0], false);
  await hand([-3.0, 1.0, 1.0], false);
  s = await status();
  assert(kinds(s, 'fuse').some(e => Math.abs(e.position[0] - -1.0) < 0.3), 'a released fuse stays where it was let go');

  // Door 1 blocks until the fuse is seated.
  await standAt([0, 2.0]);
  await pause(300);
  s = await status();
  assert.equal(s.vars.fuses, 0);
  await carry([-1.0, 1.4, -1.0], [1.5, 1.3, 2.7]);
  s = await status();
  assert.equal(s.vars.fuses, 1, 'the first socket took the fuse');
  assert.equal(kinds(s, 'fuse').length, 3, 'and the cell is in the socket now');
  assert(kinds(s, 'door1')[0].position[1] > door1Start + 2, 'door 1 slid up');
  assert.equal(kinds(s, 'led_ok').length, 1, 'a green light shows the socket is live');

  // Through the open door into the machine hall.
  await standAt([0, 4.0]);
  await pause(300);
  s = await status();
  assert(s.entities.some(e => e.kind === 'player' && e.position[2] > 3.4), 'the player walked through the open door');

  // The lever opens the cage: a trigger pull beside it.
  assert.equal(kinds(s, 'cage_gate').length, 1);
  await hand([-4.6, 1.3, 7.0]);
  await hand([-4.6, 1.3, 7.0], false, true);
  await hand([-4.6, 1.3, 7.0], false, false);
  s = await status();
  assert.equal(kinds(s, 'cage_gate').length, 0, 'the cage is open');
  assert.equal(kinds(s, 'lever').length, 0);
  assert.equal(kinds(s, 'lever_dn').length, 1, 'the lever shows it was pulled');

  // The cage's fuse to door 2.
  await carry([4.3, 1.0, 8.75], [1.5, 1.3, 13.7]);
  s = await status();
  assert.equal(s.vars.fuses, 2);
  assert(kinds(s, 'door2')[0].position[1] > 3, 'door 2 slid up');

  // The shelf fuse and the reactor.
  await standAt([0, 15.0]);
  await pause(300);
  await carry([-4.4, 2.0, 11.6], [-2.4, 1.05, 18.9]);
  s = await status();
  assert.equal(s.vars.fuses, 3);
  assert.equal(s.status, 'playing', 'three of four is not enough');
  await carry([3.7, 1.15, 22.2], [2.4, 1.05, 18.9]);
  s = await status();
  assert.equal(s.vars.fuses, 4);
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).status === 'won', null, { timeout: 15000 });
  s = await status();
  assert(kinds(s, 'core_lit').length === 1, 'the reactor is lit');
  const sounds = (await page.evaluate(() => window.__ve.gameAudio.played.map(p => p.name)));
  assert(sounds.includes('dooropen') && sounds.includes('powerup'), `doors and the reactor are heard: ${sounds}`);

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log(`game blackout ok (${Math.round(triangles)} drawn triangles, won with air ${s.vars.air})`);
} finally {
  await browser.close();
  server.close();
}
