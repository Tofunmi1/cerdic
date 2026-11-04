import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// F2: a simulated texture. Three spheres carry textures grown by the reaction_diffusion node (spots, stripes, a maze), baked by the engine in the page and
// shown on the surface. Each must show real contrast (a pattern, not a flat colour), and they must differ from each other. Looked at by eye.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/f2/', import.meta.url).pathname;
const sphere = (name, preset, x, a, b) => `${name}_s = solid.sphere@1(radius=0.5, segments=48)
${name}_t = solid.texture@1(${name}_s, program="skin = reaction_diffusion(preset=\\"${preset}\\", size=128, steps=4000, repeat_x=3, repeat_y=2, seed=3)\\nalbedo = colorize(skin, gradient=[0:${a}, 1:${b}])")
${name}_p = xform.move@1(${name}_t, by=[${x}, 0.5, 0])
output ${name} = ${name}_p
`;
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[8, 0.2, 4])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
${sphere('spots', 'spots', -1.2, '#f2e3b3', '#6b3a1e')}${sphere('stripes', 'stripes', 0, '#1a1a1a', '#e8e0c8')}${sphere('maze', 'maze', 1.2, '#1c3b5a', '#e0f0d0')}`;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5226', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5226')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5226/?viewer=1&eval=1&docname=f2-rd');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'rd'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 600)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 300000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  await page.evaluate(() => window.__runtime.viewer_look_at(0.9, 1.1, 5.2, 0.9, 0.5, 0));
  await page.keyboard.press('h');
  await page.waitForTimeout(6000);
  const png = PNG.sync.read(await page.screenshot({ path: `${reports}reaction-diffusion.png` }));
  // Each sphere's middle, as a square of pixels: the share of dark and of light pixels says there is a pattern.
  // The middle of each sphere (the spheres stand at x = -1.2, 0 and 1.2): the spread of brightness there says there is a pattern, not a flat colour.
  const regions = { spots: [75, 320], stripes: [195, 320], maze: [320, 320] };
  const stats = Object.fromEntries(Object.entries(regions).map(([name, [x0, y0]]) => {
    const lums = [];
    for (let y = y0; y < y0 + 60; y++) for (let x = x0; x < x0 + 60; x++) {
      const i = (y * png.width + x) * 4;
      lums.push((png.data[i] + png.data[i + 1] + png.data[i + 2]) / 3);
    }
    const mean = lums.reduce((a, b) => a + b, 0) / lums.length;
    return [name, { mean: Math.round(mean), spread: Math.round(Math.sqrt(lums.reduce((a, b) => a + (b - mean) ** 2, 0) / lums.length)) }];
  }));
  console.log('F2 reaction-diffusion: brightness mean and spread in the middle of each sphere', JSON.stringify(stats));
  for (const [name, s] of Object.entries(stats)) assert(s.spread > 22, `${name} shows a pattern with contrast (spread ${s.spread})`);
  console.log('F2 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
