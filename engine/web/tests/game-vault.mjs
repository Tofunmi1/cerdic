// A key-and-door puzzle written entirely as nodes (fixture `worlds/vault.txt`): the player starts at the spawn, the door stays shut without the key
// (a rule with a condition), the key opens it, the exit wins. Also guards two bugs found building it: a destroyed door's prototype must stay hidden, and the viewer starts at the hero's spawn.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-vault.mjs
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { driverAdvisory, serve } from './support.mjs';

const program = await readFile(new URL('./worlds/vault.txt', import.meta.url), 'utf8');
const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=vault-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'a vault'), program);
  assert(staged.pending, JSON.stringify(staged).slice(0, 800));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false);
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const status = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()));
  const goTo = async (x, y, z) => {
    await page.evaluate(([a, b, c]) => window.__runtime.viewer_look_at(a, b + 1.6, c, a + 1, b + 1.6, c), [x, y, z]);
    await page.waitForTimeout(500);
    return status();
  };
  const kinds = s => s.entities.map(e => e.kind).sort().join(',');

  let s = await status();
  assert.equal(kinds(s), 'door,exit,key,player');
  assert.equal(s.hidden_prototypes, 3, 'door, key and exit meshes are prototypes');
  assert.equal(s.drawn, 3);
  assert.deepEqual(s.entities.find(e => e.kind === 'player').position.map(v => Math.round(v)), [-12, 0, 0], 'the hero starts at its spawn');
  const eye = await page.evaluate(() => JSON.parse(window.__runtime.viewer_pose()).position);
  assert(Math.abs(eye[0] + 12) < 0.3 && Math.abs(eye[2]) < 0.3, `the viewer starts at the spawn: ${eye}`);
  assert.deepEqual(s.hud.lines, ['Key 0']);

  s = await goTo(4, 0, 0);
  assert.equal(kinds(s), 'door,exit,key,player', 'without the key the door does not open');
  assert(s.entities.find(e => e.kind === 'player').position[0] < 3.75, 'and the hero is held in front of it, not inside');
  s = await goTo(12, 0, 0);
  assert.notEqual(s.status, 'won', 'walking the viewer to the exit without the key does not win: the hero cannot pass the shut door');
  assert(s.entities.find(e => e.kind === 'player').position[0] < 3.75);
  // The player's body is held too: the viewer is moved back out of the door, not left standing in it.
  const held = await page.evaluate(() => JSON.parse(window.__runtime.viewer_pose()).position);
  assert(held[0] < 3.8 && held[0] > 2.5, `the viewer is shoved back in front of the shut door, not at the exit: ${held}`);
  s = await goTo(-4, 0.5, 2);
  assert.equal(s.vars.has_key, 1);
  assert.deepEqual(s.hud.lines, ['Key 1']);
  s = await goTo(4, 0, 0);
  assert.equal(kinds(s), 'exit,player', 'with the key it opens');
  assert.equal(s.hidden_prototypes, 3, 'and the door mesh does not reappear at the origin');
  assert.equal(s.status, 'playing');
  s = await goTo(12, 0, 0);
  assert.equal(s.status, 'won');
  const sounds = await page.evaluate(() => window.__ve.gameAudio.played.map(p => p.name));
  assert.deepEqual(sounds, ['coin', 'dooropen'], 'the key and the door each sounded once');

  // The playtest bots tell the same story: straight for the exit fails, key then door then exit wins.
  const bots = page.evaluate(() => window.__ve.evaluation.game.text);
  const text = await bots;
  const play = policy => page.evaluate(([t, p]) => window.__ve.evaluation.playtest(t, p, 1, 3600).then(JSON.parse), [text, policy]);
  assert.notEqual((await play('seek:hero:exit')).status, 'won', 'a bot that skips the key cannot win');
  const full = await play('seek:hero:key,gate,exit');
  assert.equal(full.status, 'won', JSON.stringify(full));

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log('game vault ok');
} finally {
  await browser.close();
  server.close();
}
