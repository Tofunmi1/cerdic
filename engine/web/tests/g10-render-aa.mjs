import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const changedPixels = async (beforePath, afterPath) => {
  const before = PNG.sync.read(await readFile(beforePath));
  const after = PNG.sync.read(await readFile(afterPath));
  let changed = 0;
  for (let y = 40; y < Math.min(before.height, 750); y++) {
    for (let x = 320; x < Math.min(before.width, 900); x++) {
      const index = (y * before.width + x) * 4;
      const difference = Math.abs(before.data[index] - after.data[index])
        + Math.abs(before.data[index + 1] - after.data[index + 1])
        + Math.abs(before.data[index + 2] - after.data[index + 2]);
      if (difference > 6) changed++;
    }
  }
  return changed;
};
const port = 5250;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
  });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=low&specular-aa=off&ssao=on`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await mkdir(reports, { recursive: true });
  const toggle = page.locator('#v-specular-aa');
  assert.equal(await toggle.isVisible(), true, 'the specular AA control is present');
  assert.equal(await toggle.isChecked(), false, 'the effect starts off');
  const ssao = page.locator('#v-ssao');
  assert.equal(await ssao.isChecked(), true, 'URL setting enables AO at startup');
  const scene = `floor = solid.box@1(size=[8,0.2,8])
floor_low = xform.move@1(floor, by=[0,-0.1,0])
floor_mat = solid.material@1(floor_low, color=[0.55,0.55,0.55], roughness=0.9)
output floor = floor_mat
wall = solid.box@1(size=[8,4,0.2])
wall_back = xform.move@1(wall, by=[0,2,-3.9])
wall_mat = solid.material@1(wall_back, color=[0.58,0.58,0.58], roughness=0.9)
output wall = wall_mat
side = solid.box@1(size=[0.2,4,8])
side_back = xform.move@1(side, by=[-3.9,2,0])
side_mat = solid.material@1(side_back, color=[0.58,0.58,0.58], roughness=0.9)
output side = side_mat
block = solid.box@1(size=[1,1,1])
block_up = xform.move@1(block, by=[-1,0.5,-1])
block_mat = solid.material@1(block_up, color=[0.72,0.28,0.12], roughness=0.85)
output block = block_mat
glow = solid.sphere@1(radius=0.3)
glow_up = xform.move@1(glow, by=[1.4,0.3,-1])
glow_mat = solid.material@1(glow_up, color=[0.7,0.48,0.18], roughness=0.12, metallic=0.15, emissive=[5,2.5,0.7], emits_light=true)
output glow = glow_mat`;
  const staged = await page.evaluate(source => window.__ve.doc.sendText(source, 'render quality test scene'), scene);
  assert(staged.pending, `render test scene staged: ${JSON.stringify(staged)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
      || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 90000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'test scene evaluates');
  await page.evaluate(() => window.__runtime.viewer_look_at(4.8, 3.6, 7.5, 0, 1.2, -1));
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
    null, { timeout: 60000 });
  await page.waitForTimeout(700);
  await ssao.uncheck();
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${reports}render-effects-off.png` });
  await ssao.check();
  await page.waitForFunction(() => window.__ve.read().quality.ssao === true);
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${reports}render-ao-on.png` });
  const aoPixels = await changedPixels(`${reports}render-effects-off.png`, `${reports}render-ao-on.png`);
  assert(aoPixels > 20, `AO changes contact shading (${aoPixels} viewport pixels)`);
  await ssao.uncheck();
  await page.waitForTimeout(400);

  await page.locator('#v-bloom').check();
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${reports}render-bloom-on.png` });
  const bloomPixels = await changedPixels(`${reports}render-effects-off.png`, `${reports}render-bloom-on.png`);
  assert(bloomPixels > 20, `emissive bloom halo changes the viewport (${bloomPixels} pixels)`);
  await toggle.check();
  await ssao.check();
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${reports}render-effects-on.png` });
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  const exposure = page.locator('#v-exposure');
  await exposure.evaluate(element => {
    element.value = '1.5';
    element.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.waitForFunction(() => JSON.parse(window.__runtime.grading_status()).exposure === 1.5);
  assert.equal((await page.evaluate(() => JSON.parse(window.__runtime.grading_status()).exposure)), 1.5,
    'exposure slider reaches the filmic forward path');
  console.log(`G10 render effects: AO ${aoPixels}px, bloom ${bloomPixels}px; no browser/WebGPU errors; exposure works`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
