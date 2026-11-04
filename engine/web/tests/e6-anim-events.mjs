import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// E6: animation events reach the game. A box walks a one-second clip whose state machine has two `step` events per cycle; a game rule
// `on="host:anim.step"` counts them in a game variable. After a few seconds the count must be about twice the seconds, and it must stop
// when the clip is paused.
const root = new URL('../../../', import.meta.url).pathname;
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
box = solid.box@1(size=[0.3, 0.3, 0.3])
walk = anim.track@1(box, property="position", keys="0:0,0.15,0; 0.5:0,0.25,0; 1:0,0.15,0", interpolation="smooth", clip="walk")
body_machine = anim.machine@1(walk, program="entry walk; state walk clip=walk; event walk step at=0.25; event walk step at=0.75")
output body = body_machine
steps = game.var@1(name="steps", value=0)
count = game.rule@1(on="host:anim.step", do="add:steps:1")
all = game.collect@1(steps, count, name="steps")
output rules = all
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5221', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5221')).ok) break; } catch {} await Bun.sleep(100); }
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5221/?viewer=1&eval=1&scene=open&docname=e6-events');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'events'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  await page.waitForFunction(() => JSON.parse(window.__runtime.game_status()).present, null, { timeout: 30000 });
  const steps = () => page.evaluate(() => JSON.parse(window.__runtime.game_status()).vars.steps ?? 0);
  await page.evaluate(() => window.__runtime.viewer_animation(true, 0));
  const t0 = Date.now();
  const before = await steps();
  await page.waitForTimeout(6000);
  const seconds = (Date.now() - t0) / 1000;
  const counted = (await steps()) - before;
  console.log(`E6 events: ${counted} step events counted by the game rule in ${seconds.toFixed(1)} s of a clip with 2 per second`);
  assert(counted >= seconds * 2 * 0.5, `most steps arrived (${counted} in ${seconds.toFixed(1)} s)`);
  assert(counted <= seconds * 2 * 1.2 + 2, `no step arrived twice (${counted})`);
  await page.evaluate(() => window.__runtime.viewer_animation(false, 0.1));
  await page.waitForTimeout(800);
  const paused = await steps();
  await page.waitForTimeout(2000);
  assert.equal(await steps(), paused, 'a paused clip raises no events');
  console.log('E6 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
