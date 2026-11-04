import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// D4: hair strips on the cloth solver. One hair_cards line hangs 24 strips from a ring pinned to a head bone that slides 0.6 m in 0.3 s; the strips
// hang below the roots, trail the head while it moves and hang under it again after. Read from the runtime's own cloth read-out (viewer_cloth).
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/d4/', import.meta.url).pathname;
const prelude = await readFile(new URL('../../crates/ve-modeling/kits/hair_cards.txt', import.meta.url), 'utf8');
const program = `${prelude}
sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[12, 0.2, 6])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
box = solid.box@1(size=[0.2, 0.25, 0.2])
blue = solid.material@1(box, color=[0.05, 0.1, 0.95], roughness=0.5)
head_at = xform.move@1(blue, by=[0, 1.7, 0])
head_bone = rig.bone@1(head_at, head=[0, 1.6, 0], tail=[0, 1.8, 0])
head = anim.track@1(head_bone, property="position", keys="0:0,0,0; 0.3:0.6,0,0", interpolation="linear", loop=false)
output head = head
hair_cards("hair", "head", 0, 1.78, 0, 0.1, 0.35, 24, 0.05, 3)
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5218', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5218')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5218/?viewer=1&eval=1&docname=d4-hair');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'hair'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  await page.evaluate(() => window.__runtime.viewer_look_at(0.3, 1.5, 3.2, 0.3, 1.5, 0));
  await page.keyboard.press('h');
  await page.evaluate(() => window.__runtime.viewer_animation(false, 0));
  await page.waitForTimeout(5000);
  const read = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_cloth()).cloths[0]);
  const rest = await read();
  console.log('D4 hair at rest:', JSON.stringify({ points: rest.points, min: rest.min, max: rest.max, steps: rest.steps }));
  assert.equal(rest.points, 24 * 2 * 11, 'the strips are one cloth');
  assert(rest.max[1] <= 1.78 + 0.001 && rest.min[1] < 1.78 - 0.25, 'the strips hang below their roots');
  await page.screenshot({ path: `${reports}hair-rest.png` });
  await page.evaluate(() => window.__runtime.viewer_animation(true, 0));
  let worstLag = 0;
  for (let i = 0; i < 40; i++) {
    await page.waitForTimeout(40);
    const c = await read();
    worstLag = Math.max(worstLag, c.pin_matrix[12] - (c.min[0] + c.max[0]) / 2);
  }
  await page.screenshot({ path: `${reports}hair-moving.png` });
  await page.waitForTimeout(6000);
  const end = await read();
  const endLag = end.pin_matrix[12] - (end.min[0] + end.max[0]) / 2;
  console.log(`D4 hair: worst lag ${worstLag.toFixed(3)} m, after settling ${endLag.toFixed(3)} m, head moved ${end.pin_matrix[12].toFixed(2)} m`);
  assert(end.pin_matrix[12] > 0.55, 'the head moved 0.6 m');
  assert(worstLag > 0.02, `the hair trailed behind the moving head (${worstLag.toFixed(3)} m)`);
  assert(Math.abs(endLag) < 0.05, 'it hangs under the head again');
  assert(end.max[1] <= 1.78 + 0.001 && end.min[1] < 1.78 - 0.25, 'still hanging below the roots');
  console.log('D4 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
