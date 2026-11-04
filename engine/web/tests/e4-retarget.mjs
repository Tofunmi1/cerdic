import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// E4: anim.retarget. A short red arm swings about its shoulder; a tall blue arm (1.5 times the size, on the other side) gets the same clip through
// anim.retarget with scale 1.5 and mirror, so it swings about ITS shoulder the other way. Both must move in the picture, and each moves about its own
// joint (its top end stays where it is while its bottom end swings).
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e4/', import.meta.url).pathname;
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[12, 0.2, 6])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
a_box = solid.box@1(size=[0.1, 0.6, 0.1])
a_red = solid.material@1(a_box, color=[0.95, 0.1, 0.05], roughness=0.6)
a_at = xform.move@1(a_red, by=[-1.0, 1.3, 0])
a_bone = rig.bone@1(a_at, head=[-1.0, 1.6, 0], tail=[-1.0, 1.0, 0])
a_swing = anim.track@1(a_bone, property="rotation", keys="0:0,0,0; 1:0,0,70", interpolation="linear", loop=false, pivot_at="head")
output arm_short = a_swing
b_box = solid.box@1(size=[0.15, 0.9, 0.15])
b_blue = solid.material@1(b_box, color=[0.05, 0.1, 0.95], roughness=0.6)
b_at = xform.move@1(b_blue, by=[0.9, 1.15, 0])
b_bone = rig.bone@1(b_at, head=[0.9, 1.6, 0], tail=[0.9, 0.7, 0])
b_swing = anim.retarget@1(b_bone, a_swing, scale=1.5, mirror=true)
output arm_tall = b_swing
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5217', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5217')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5217/?viewer=1&eval=1&docname=e4-retarget');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'retarget'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const tall = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'arm_tall').animation);
  assert.equal(tall.length, 1);
  assert.deepEqual(tall[0].keys[1].value.map(v => Math.round(v) + 0), [0, 0, -70], 'mirrored rotation');
  assert.deepEqual(tall[0].pivot.map(v => Math.round(v * 100) / 100), [0.9, 1.6, 0], 'about the tall arm\'s own shoulder');

  await page.evaluate(() => window.__runtime.viewer_look_at(0.7, 1.2, 5.5, 0.7, 1.2, 0));
  await page.keyboard.press('h');
  const find = async seconds => {
    await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
    await page.waitForTimeout(1200);
    const png = PNG.sync.read(await page.screenshot({ path: `${reports}retarget-${seconds}.png` }));
    const box = { red: { x0: 1e9, x1: 0, y0: 1e9, y1: 0, n: 0 }, blue: { x0: 1e9, x1: 0, y0: 1e9, y1: 0, n: 0 } };
    for (let y = 0; y < 395; y++) for (let x = 0; x < 530; x++) {
      const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
      const key = r > 200 && g < 130 && b < 130 ? 'red' : b > 200 && r < 130 && g < 130 ? 'blue' : null;
      if (!key) continue;
      const k = box[key]; k.n++; k.x0 = Math.min(k.x0, x); k.x1 = Math.max(k.x1, x); k.y0 = Math.min(k.y0, y); k.y1 = Math.max(k.y1, y);
    }
    return box;
  };
  const rest = await find(0), swung = await find(1);
  console.log('E4 rest', JSON.stringify(rest), 'swung', JSON.stringify(swung));
  for (const colour of ['red', 'blue']) {
    assert(rest[colour].n > 400 && swung[colour].n > 400, `${colour} arm is visible`);
    assert(swung[colour].x1 - swung[colour].x0 > (rest[colour].x1 - rest[colour].x0) + 40, `${colour} arm swings out sideways`);
    assert(Math.abs(swung[colour].y0 - rest[colour].y0) < 14, `${colour} arm's top (its shoulder) stays put`);
  }
  // The blue arm is 1.5 times the size, so it swings out by more.
  assert((swung.blue.x1 - swung.blue.x0) > (swung.red.x1 - swung.red.x0) * 1.15, 'the tall arm swings through a bigger arc');
  // Mirrored: the red arm (on the left) swings one way, the blue one the other way.
  const redSide = Math.sign((swung.red.x0 + swung.red.x1) / 2 - (rest.red.x0 + rest.red.x1) / 2);
  const blueSide = Math.sign((swung.blue.x0 + swung.blue.x1) / 2 - (rest.blue.x0 + rest.blue.x1) / 2);
  assert.notEqual(redSide, blueSide, 'mirror swings the other way');
  console.log('E3 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
