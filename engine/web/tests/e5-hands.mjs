import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// E5: hand poses. The autorig gives every finger bone the clips "fist" and "point". The same character is shown with its hands open and then with
// an animation machine playing "fist" (and "point"); the pictures of the left hand must differ, and be looked at by eye.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e5/', import.meta.url).pathname;
const prelude = await readFile(new URL('../../crates/ve-modeling/kits/autorig_human.txt', import.meta.url), 'utf8');
const scene = clip => `${prelude}
sky = scene.sky@1(sun_elevation=40, sun_azimuth=30, sun_intensity=4, clouds=false)
output sky = sky
floor_g = solid.box@1(size=[8, 0.2, 8])
floor_a = xform.move@1(floor_g, by=[0, -0.1, 0])
floor_m = solid.material@1(floor_a, color=[0.45, 0.45, 0.42], roughness=0.9)
output floor = floor_m
body = body.human@1(height=0.5)
human_rig("body", 1, "root")
human_skin("body", "person", 1, 0.86, 0.66, 0.52)
root_b = body.bone@1(body, bone="root")
${clip ? `root_m = anim.machine@1(root_b, program="entry pose; state pose clip=${clip}")\noutput root = root_m` : 'output root = root_b'}
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5222', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5222')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  const show = async (clip, name) => {
    // A fresh document for each pose (sending text again would add to the one before).
    await page.goto(`http://127.0.0.1:5222/?viewer=1&eval=1&docname=e5-hands-${name}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'hands'), scene(clip));
    assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 300000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
    // The left hand hangs at the figure's left (+x), about 0.7 m from the middle and 0.85 m up; look at it from the front and a little to the side.
    // VE_HAND_VIEW=front (default), side or oblique. The fingers curl toward the body, so a view from the outside (side) hides the curl.
    const views = { front: [0.65, 0.9, 1.3], side: [1.5, 0.9, 0.3], oblique: [1.1, 1.0, 1.0] };
    const [ex, ey, ez] = views[process.env.VE_HAND_VIEW ?? 'front'];
    await page.evaluate(([x, y, z]) => window.__runtime.viewer_look_at(x, y, z, 0.65, 0.85, 0.1), [ex, ey, ez]);
    await page.keyboard.press('h');
    await page.waitForTimeout(3000);
    return PNG.sync.read(await page.screenshot({ path: `${reports}hand-${name}.png` }));
  };
  const open = await show('', 'open');
  const fist = await show('fist', 'fist');
  const point = await show('point', 'point');
  const diff = (a, b) => {
    let n = 0;
    for (let y = 0; y < 560; y++) for (let x = 0; x < 530; x++) {
      const i = (y * a.width + x) * 4;
      if (Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2])) > 40) n++;
    }
    return n;
  };
  const [openFist, openPoint, fistPoint] = [diff(open, fist), diff(open, point), diff(fist, point)];
  console.log(`E5 hands: pixels changed open to fist ${openFist}, open to point ${openPoint}, fist to point ${fistPoint}`);
  assert(openFist > 600, 'the fist pose changes the hand');
  assert(openPoint > 300, 'the point pose changes the hand');
  assert(fistPoint > 100, 'fist and point differ (the index finger)');
  console.log('E5 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
