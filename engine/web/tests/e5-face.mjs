import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// E5: face expressions. The same head with no expression and with face_happy, face_surprised and face_wink from the face_presets kit, close up: the
// pictures must differ from the neutral one (and from each other), and are looked at by eye.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e5/', import.meta.url).pathname;
const prelude = await readFile(new URL('../../crates/ve-modeling/kits/face_presets.txt', import.meta.url), 'utf8');
const scene = face => `${prelude}
sky = scene.sky@1(sun_elevation=40, sun_azimuth=30, sun_intensity=4, clouds=false)
output sky = sky
body = body.human@1(${face ? `face="\${${face}}"` : ''})
skin = solid.material@1(body, color=[0.86, 0.66, 0.52], roughness=0.6)
output person = skin
`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5223', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5223')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  const show = async (face, name) => {
    await page.goto(`http://127.0.0.1:5223/?viewer=1&eval=1&docname=e5-face-${name}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'face'), scene(face));
    assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 300000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
    await page.evaluate(() => window.__runtime.viewer_look_at(0, 1.62, 0.75, 0, 1.6, 0));
    await page.keyboard.press('h');
    await page.waitForTimeout(2500);
    return PNG.sync.read(await page.screenshot({ path: `${reports}face-${name}.png` }));
  };
  const neutral = await show('', 'neutral');
  const happy = await show('face_happy', 'happy');
  const surprised = await show('face_surprised', 'surprised');
  const wink = await show('face_wink', 'wink');
  const diff = (a, b) => {
    let n = 0;
    for (let y = 0; y < 560; y++) for (let x = 0; x < 530; x++) {
      const i = (y * a.width + x) * 4;
      if (Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2])) > 12) n++;
    }
    return n;
  };
  const [h, s, w, hs] = [diff(neutral, happy), diff(neutral, surprised), diff(neutral, wink), diff(happy, surprised)];
  console.log(`E5 face: pixels changed from neutral to happy ${h}, surprised ${s}, wink ${w}; happy to surprised ${hs}`);
  assert(h > 300 && s > 300 && w > 100, 'each expression changes the picture');
  assert(hs > 300, 'happy and surprised look different');
  console.log('E5 face browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
