import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// F3: a realtime texture graph. A panel carries a texture program that reads the clock (an expr node using t): the page compiles its albedo once and draws it on the GPU every frame, sending the
// picture to the renderer. A sphere with a still program stands beside it as the control. Pictures at different times must differ over the panel and stay the same over the sphere,
// and the renderer must report accepting live pictures.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/f3/', import.meta.url).pathname;
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[8, 0.2, 4])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
panel_b = solid.box@1(size=[1.6, 1.0, 0.1])
panel_t = solid.texture@1(panel_b, program="n = expr(code=\\"out = vec3(0.5 + 0.5 * sin(p.x * 18.0 + t * 3.0), 0.5 + 0.5 * sin(p.y * 12.0 - t * 2.0), 0.5 + 0.5 * sin(t))\\")\\nalbedo = n", size="256")
panel_p = xform.move@1(panel_t, by=[0.2, 0.7, 0])
output panel = panel_p
ball_s = solid.sphere@1(radius=0.5, segments=40)
ball_t = solid.texture@1(ball_s, program="n = expr(code=\\"out = vec3(0.5 + 0.5 * sin(p.x * 18.0), 0.5 + 0.5 * sin(p.y * 12.0), 0.3)\\")\\nalbedo = n", size="256")
ball_p = xform.move@1(ball_t, by=[-1.3, 0.5, 0])
output ball = ball_p
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5227', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5227')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5227/?viewer=1&eval=1&docname=f3-live');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'live'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 600)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 300000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  await page.evaluate(() => window.__runtime.viewer_look_at(0, 0.8, 5.0, 0, 0.6, 0));
  await page.keyboard.press('h');
  await page.waitForTimeout(1500);
  const stats0 = await page.evaluate(() => window.__ve.liveTextures());
  console.log('F3 live textures at start:', JSON.stringify(stats0));
  assert.equal(stats0.count, 1, 'one part (the panel) is drawn live; the still sphere is not');
  const shoot = async name => PNG.sync.read(await page.screenshot({ path: `${reports}live-${name}.png` }));
  const a = await shoot('a');
  await page.waitForTimeout(700);
  const b = await shoot('b');
  await page.waitForTimeout(700);
  const c = await shoot('c');
  const stats1 = await page.evaluate(() => window.__ve.liveTextures());
  console.log('F3 live textures after 3 s:', JSON.stringify(stats1));
  // The ball stands at the left of the picture and the panel to its right (ball x -1.3, panel x 0.2; the page's side panel hides everything past x = 530).
  const diff = (p, q, [x0, y0, x1, y1]) => {
    let n = 0, total = 0;
    for (let y = y0; y < y1; y += 2) for (let x = x0; x < x1; x += 2) {
      const i = (y * p.width + x) * 4;
      total++;
      if (Math.max(Math.abs(p.data[i] - q.data[i]), Math.abs(p.data[i + 1] - q.data[i + 1]), Math.abs(p.data[i + 2] - q.data[i + 2])) > 20) n++;
    }
    return n / total;
  };
  const ball = [160, 270, 300, 400], panel = [360, 220, 520, 380];
  const [pab, pbc] = [diff(a, b, panel), diff(b, c, panel)];
  const [bab, bbc] = [diff(a, b, ball), diff(b, c, ball)];
  console.log(`F3 live textures: share of pixels changed over the panel ${pab.toFixed(2)} and ${pbc.toFixed(2)}, over the still ball ${bab.toFixed(3)} and ${bbc.toFixed(3)}`);
  assert(stats1.accepted > 30, `the renderer accepted live pictures (${stats1.accepted})`);
  assert(stats1.accepted > stats0.accepted + 20, 'they keep coming');
  assert(pab > 0.2 && pbc > 0.2, 'the panel shows a moving pattern');
  assert(bab < 0.02 && bbc < 0.02, 'the control ball does not change');
  console.log('F3 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
