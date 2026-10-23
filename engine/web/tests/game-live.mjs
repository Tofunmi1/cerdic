// The game running live in the viewer (ADR 0062 phase 3): a document with game.* nodes is stepped at 60 Hz by the runtime, the hero follows the
// viewer, kinds are drawn with the scene part they name, and walking into a pickup scores. Written before the bodies; do not weaken.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-live.mjs
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory, serve } from './support.mjs';

const PROGRAM = [
  'ground = solid.box@1(size=[20, 0.2, 20])',
  'gm = xform.move@1(ground, by=[0, -0.1, 0])',
  'coin_mesh = solid.sphere@1(radius=0.2, segments=12)',
  'body = solid.box@1(size=[0.4, 0.4, 0.4])',
  'score = game.var@1(name="score", value=0)',
  'hero = game.kind@1(name="player", size=0.5, tags=["hero"], part="body")',
  'coin = game.kind@1(name="coin", size=0.4, tags=["pickup"], part="coin_mesh")',
  'p0 = game.spawn@1(kind="player", at=[0, 0, 0])',
  'c0 = game.spawn@1(kind="coin", at=[3, 0, 0])',
  'c1 = game.spawn@1(kind="coin", at=[-3, 0, 0])',
  'walk = game.input@1(name="walk", follow="viewer", target="tag:hero")',
  'pick = game.rule@1(on="touch", a="tag:hero", b="tag:pickup", do="add:score:1;destroy:b;effect:sfx:coin@b")',
  'win = game.goal@1(outcome="win", when="score>=2")',
  'panel = game.hud@1(text="Score {score} / 2", anchor="gaze", size=0.03)',
  'all = game.collect@1(score, hero, coin, p0, c0, c1, walk, pick, win, panel, name="coins")',
  'output ground = gm',
  'output coin_mesh = coin_mesh',
  'output body = body',
  'output rules = all',
].join('\n');

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=game-live-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const status = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()));
  const settled = () => page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 120000 });

  assert.equal((await status()).present, false, 'no game before one is written');

  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'a live coin game'), PROGRAM);
  assert(staged.pending, JSON.stringify(staged).slice(0, 600));
  await settled();
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });

  let s = await status();
  assert.equal(s.status, 'playing');
  assert.equal(s.entities.length, 3, JSON.stringify(s));
  assert.deepEqual(s.entities.map(e => e.kind).sort(), ['coin', 'coin', 'player']);
  assert.equal(s.drawn, 3, 'every entity of a kind with a part is drawn with that part');
  assert.equal(s.hidden_prototypes, 2, 'the two authored parts the game binds are prototypes, not scenery');
  // The viewer starts where the hero was spawned, looking at something else in the game (not wherever the camera happened to be).
  const player = s.entities.find(e => e.kind === 'player').position;
  assert.deepEqual(player.map(v => Math.round(v * 10) / 10), [0, 0, 0], `the hero is at its spawn: ${player}`);
  const eye = await page.evaluate(() => JSON.parse(window.__runtime.viewer_pose()).position);
  assert(Math.abs(eye[0]) < 0.2 && Math.abs(eye[1] - 1.6) < 0.2 && Math.abs(eye[2]) < 0.2, `the viewer's eye is at the spawn, 1.6 m up: ${eye}`);
  assert.equal(s.vars.score, 0);
  // The HUD: a panel of plain geometry in front of the viewer, its text bound to the variables.
  assert.deepEqual(s.hud.lines, ['Score 0 / 2'], JSON.stringify(s.hud));
  assert(s.hud.triangles > 100, `the panel is drawn: ${s.hud.triangles} triangles`);
  const before = s.hud.triangles;

  // Walking is the viewer's pose; the hero stands at the viewer's feet (eye height 1.6 m) and touches what is there.
  const stand = (x, z) => page.evaluate(([px, pz]) => window.__runtime.viewer_look_at(px, 1.6, pz, px, 1.6, pz - 5), [x, z]);
  assert(await stand(3, 0));
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).vars.score === 1, null, { timeout: 10000 });
  s = await status();
  assert.equal(s.entities.filter(e => e.kind === 'coin').length, 1, 'the picked coin is gone');
  assert.equal(s.drawn, 2, 'and so is its drawing');
  assert.equal(s.status, 'playing');
  assert.deepEqual(s.hud.lines, ['Score 1 / 2'], 'the panel follows the variable');
  assert(s.hud.triangles > 100, `and is redrawn: ${before} then ${s.hud.triangles} triangles`);

  // Effects come out with where they happened (the picked coin is already gone from the game), each numbered so a host sees every one once.
  const effects = await page.evaluate(() => JSON.parse(window.__runtime.game_effects_since(0)));
  assert.equal(effects.length, 1, JSON.stringify(effects));
  assert.equal(effects[0].kind, 'sfx');
  assert.equal(effects[0].name, 'coin');
  assert(Math.abs(effects[0].position[0] - 3) < 0.01 && Math.abs(effects[0].position[2]) < 0.01, `at the coin: ${JSON.stringify(effects[0])}`);
  assert.deepEqual(await page.evaluate(seq => JSON.parse(window.__runtime.game_effects_since(seq)), effects[0].seq), [], 'nothing newer yet');
  // The page plays it: a log of what the audio layer was asked for, kept even when the browser has not allowed sound yet.
  const played = await page.evaluate(() => window.__ve.gameAudio.played);
  assert.equal(played.length, 1, JSON.stringify(played));
  assert.equal(played[0].name, 'coin');
  assert(Math.abs(played[0].position[0] - 3) < 0.01);

  assert(await stand(-3, 0));
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).status === 'won', null, { timeout: 10000 });
  s = await status();
  assert.equal(s.vars.score, 2);
  assert.equal(s.hud.lines[s.hud.lines.length - 1], 'YOU WON', `a finished game says so: ${JSON.stringify(s.hud.lines)}`);
  assert.deepEqual(s.hud.lines.slice(0, -1), ['Score 2 / 2'], 'above the panels it already had');
  assert.equal(s.hidden_prototypes, 2, 'when the last entity of a kind is gone its prototype stays hidden (it would reappear at the origin otherwise)');
  assert.equal((await page.evaluate(() => window.__ve.gameAudio.played)).length, 2, 'the second pickup sounded too, once');

  // A document without a game stops it and removes its drawings.
  await page.evaluate(() => { window.__ve.doc.open(`empty-${Date.now()}`, 1); });
  const cleared = await page.evaluate(() => window.__ve.doc.sendText('b = solid.box@1(size=[1, 1, 1])\noutput b = b', 'no game'));
  assert(cleared.pending, JSON.stringify(cleared).slice(0, 300));
  await page.waitForFunction(() => !JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  assert.equal((await status()).drawn, 0);

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log('game live ok');
} finally {
  await browser.close();
  server.close();
}
