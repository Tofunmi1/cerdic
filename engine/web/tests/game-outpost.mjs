// A playable game set: "Water for the outpost". The desert outpost world (huts, ruins, a well, a camp) plus five water jars to find and bring
// to the well before time runs out. The program is `dist-bevy-gpu/worlds/outpost-game.txt` (a model's outpost with a game.* tail).
// Checks: it evaluates inside the provisional Quest budget, the jars are real entities on the ground, walking to them collects them, the well
// accepts the water only once all five are held (a visit with fewer wins nothing), and winning plays its sounds. Nothing here is faked: the
// viewer moves the hero the way a walking player does.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-outpost.mjs
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { driverAdvisory, serve } from './support.mjs';

const program = await readFile(new URL('../dist-bevy-gpu/worlds/outpost-game.txt', import.meta.url), 'utf8');
const QUEST_TRIANGLES = 300_000;

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=outpost-game-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'water for the outpost'), program);
  assert(staged.pending, JSON.stringify(staged).slice(0, 800));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final', null, { timeout: 240000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false, `the world evaluates: ${await page.evaluate(() => JSON.stringify(window.__ve.evaluation.state.lastFailure)?.slice(0, 600))}`);
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const status = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()));

  const triangles = await page.evaluate(() => window.__ve.evaluation.parts.reduce((sum, p) => sum + (p.indices.length / 3) * (p.transforms?.length ? Math.floor(p.transforms.length / 16) : 1), 0));
  assert(triangles > 100_000 && triangles <= QUEST_TRIANGLES, `drawn triangles ${triangles} are within ${QUEST_TRIANGLES}`);

  let s = await status();
  const jars = s.entities.filter(e => e.kind === 'jar');
  assert.equal(jars.length, 5, 'five jars to find');
  assert(jars.every(e => Math.abs(e.position[1] - 1.6) < 0.05), 'the jars stand on the outpost floor');
  assert(s.drawn >= 5, `the jars are drawn (${s.drawn} things drawn)`);
  const well = s.entities.find(e => e.kind === 'well');
  assert(well, 'the well is an entity');
  const walkTo = async ([x, y, z]) => page.evaluate(([px, py, pz]) => window.__runtime.viewer_look_at(px, py + 1.6, pz, px + 1, py + 1.6, pz), [x, y, z]);

  // Visiting the well empty-handed does nothing.
  await walkTo(well.position);
  await page.waitForTimeout(800);
  s = await status();
  assert.equal(s.status, 'playing', 'the well does not take an empty visit');
  assert.equal(s.vars.jars, 0);

  // Collect the jars one by one.
  for (let round = 0; round < 5; round++) {
    const target = s.entities.find(e => e.kind === 'jar');
    assert(target, 'a jar is left');
    await walkTo(target.position);
    await page.waitForFunction(n => JSON.parse(window.__runtime.game_status()).vars.jars >= n, round + 1, { timeout: 5000 });
    s = await status();
    assert.equal(s.status, 'playing', `still playing with ${s.vars.jars} jars`);
  }
  assert.equal(s.vars.jars, 5);
  assert.equal(s.entities.filter(e => e.kind === 'jar').length, 0, 'every jar was taken');
  assert(s.vars.time < 300, 'time is still on the clock');

  // Bring the water to the well: that wins.
  await walkTo(well.position);
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).status === 'won', null, { timeout: 5000 });
  const sounds = await page.evaluate(() => window.__ve.gameAudio.played.map(p => p.name));
  assert.equal(sounds.filter(n => n === 'coin').length, 5, 'one sound per jar');
  assert(sounds.includes('powerup'), 'the well answers');

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log(`game outpost ok (${Math.round(triangles)} drawn triangles, won with ${s.vars.jars} jars at ${s.vars.time}s)`);
} finally {
  await browser.close();
  server.close();
}
