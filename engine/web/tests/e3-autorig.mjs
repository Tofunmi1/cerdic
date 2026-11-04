import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// E3: the autorig macro. Three lines make a rigged, skinned character; the right upper arm is the one bone animated by hand (raised 70 degrees),
// and the skinned body must follow it on screen. Run on the Bevy WebGPU bundle: bun web/tests/e3-autorig.mjs
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e3/', import.meta.url).pathname;
const prelude = await readFile(new URL('../../crates/ve-modeling/kits/autorig_human.txt', import.meta.url), 'utf8');
const program = `${prelude}
sky = scene.sky@1(sun_elevation=40, sun_azimuth=30, sun_intensity=4, clouds=false)
output sky = sky
floor_g = solid.box@1(size=[8, 0.2, 8])
floor_a = xform.move@1(floor_g, by=[0, -0.1, 0])
floor_m = solid.material@1(floor_a, color=[0.45, 0.45, 0.42], roughness=0.9)
output floor = floor_m
body = body.human@1(height=0.5)
human_rig("body", 0, "upperarm01_r")
human_skin("body", "person", 0, 0.86, 0.66, 0.52)
ab = body.bone@1(body, bone="upperarm01_r")
ap = anim.parent@1(ab, parent="shoulder01_r")
raise = anim.track@1(ap, property="rotation", keys="0:0,0,0; 1:0,0,-70", interpolation="linear", loop=false, pivot_at="head")
output upperarm01_r = raise
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5216', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5216')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5216/?viewer=1&eval=1&docname=e3-autorig');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'autorig'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 600)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 240000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const summary = await page.evaluate(() => {
    const parts = window.__ve.evaluation.parts;
    return { bones: parts.filter(p => p.bone).length, person: Boolean(parts.find(p => p.name === 'person')?.skin), arm: parts.find(p => p.name === 'upperarm01_r')?.animation?.length ?? 0, warnings: window.__ve.evaluation.warnings?.length ?? 0 };
  });
  console.log('E3 autorig:', JSON.stringify(summary));
  assert.equal(summary.bones, 1 + 5 + 3 + 1 + 14 + 14, 'every bone of the rig is a part');
  assert(summary.person, 'the body is skinned');
  assert.equal(summary.arm, 1, 'the hand-written arm bone carries its track');

  await page.evaluate(() => window.__runtime.viewer_look_at(0, 1.0, 4.2, 0, 0.95, 0));
  await page.keyboard.press('h');
  const shot = async (seconds, name) => {
    await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
    await page.waitForTimeout(1200);
    return PNG.sync.read(await page.screenshot({ path: `${reports}autorig-${name}.png` }));
  };
  const rest = await shot(0, 'rest');
  const raised = await shot(1, 'raised');
  let changed = 0, minX = Infinity, maxX = 0, minY = Infinity;
  for (let y = 0; y < 395; y++) for (let x = 0; x < 530; x++) {
    const i = (y * rest.width + x) * 4;
    if (Math.max(Math.abs(rest.data[i] - raised.data[i]), Math.abs(rest.data[i + 1] - raised.data[i + 1]), Math.abs(rest.data[i + 2] - raised.data[i + 2])) > 40) {
      changed++; minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y);
    }
  }
  console.log(`E3 autorig: raising the arm changed ${changed} pixels (x ${minX}..${maxX}, top y ${minY})`);
  assert(changed > 1500, 'the skinned body follows the raised arm');
  assert(changed < 60000, 'only the arm region moved, not the whole picture');
  console.log('E3 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
