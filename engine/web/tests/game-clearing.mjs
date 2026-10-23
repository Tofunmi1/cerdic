// Acceptance for world brief 1 in miniature (guide section 10): one program builds a wooded clearing and a lantern-collecting game; the lanterns
// stand on the hilly ground, a bot completes the game by walking, it makes sounds, and the scene stays inside the provisional Quest budget.
// The fixture `worlds/clearing.txt` is what a model writes with the tools: plant kits, patterns, inst.vary, game.* nodes, game.spawn from a placed set.
// Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/game-clearing.mjs
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { driverAdvisory, serve } from './support.mjs';

const program = await readFile(new URL('./worlds/clearing.txt', import.meta.url), 'utf8');
const QUEST_TRIANGLES = 300_000;

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=clearing-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'a wooded clearing'), program);
  assert(staged.pending, JSON.stringify(staged).slice(0, 800));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false, 'the world evaluates');
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const status = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()));

  // Within the budget: every copy counted.
  const triangles = await page.evaluate(() => window.__ve.evaluation.parts.reduce((sum, p) => sum + (p.indices.length / 3) * (p.transforms?.length ? Math.floor(p.transforms.length / 16) : 1), 0));
  assert(triangles > 20_000 && triangles <= QUEST_TRIANGLES, `drawn triangles ${triangles} are within ${QUEST_TRIANGLES}`);

  let s = await status();
  const lanterns = s.entities.filter(e => e.kind === 'lantern');
  assert(lanterns.length >= 4, `at least four lanterns to find: ${lanterns.length}`);
  const ys = lanterns.map(e => e.position[1]);
  assert(Math.max(...ys) - Math.min(...ys) > 0.5, `lanterns stand on the hilly ground, at different heights: ${ys.map(y => y.toFixed(1))}`);
  assert(ys.every(y => y > 0.5), 'and above the flat zero plane (the terrain is raised)');
  assert.equal(s.drawn, lanterns.length, 'every lantern is drawn');

  // Play: walk to the nearest lantern until the game is won.
  for (let round = 0; round < 12 && s.status === 'playing'; round++) {
    const target = s.entities.find(e => e.kind === 'lantern');
    assert(target, 'a lantern is left while the game is not won');
    const [x, y, z] = target.position;
    await page.evaluate(([px, py, pz]) => window.__runtime.viewer_look_at(px, py + 1.6, pz, px + 1, py + 1.6, pz), [x, y, z]);
    await page.waitForFunction(n => JSON.parse(window.__runtime.game_status()).vars.score >= n, round + 1, { timeout: 5000 });
    s = await status();
  }
  assert.equal(s.status, 'won', JSON.stringify({ vars: s.vars, status: s.status }));
  assert.equal(s.vars.score, 4);
  const sounds = await page.evaluate(() => window.__ve.gameAudio.played);
  assert.equal(sounds.length, 4, 'one sound per pickup');
  assert(sounds.every(p => p.name === 'coin' && p.position && p.position[1] > 0.5), 'at the lanterns');

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log(`game clearing ok (${Math.round(triangles)} drawn triangles, ${lanterns.length} lanterns)`);
} finally {
  await browser.close();
  server.close();
}
