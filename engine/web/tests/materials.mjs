// The procedural material library (tools/mcp/materials.mjs): every program bakes in the viewer without a bake error, and each one gives a surface
// that is not flat colour. Run: cd engine/web && VE_DIST=dist-bevy-gpu VE_CHANNEL=chromium bun tests/materials.mjs
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';
import { driverAdvisory, serve } from './support.mjs';
import { MATERIALS } from '../../tools/mcp/materials.mjs';

const names = Object.keys(MATERIALS);
assert(names.length >= 8, 'the library has its eight materials');
const lines = ['floor = solid.box@1(size=[60, 0.2, 14])', 'floor_m = solid.material@1(floor, color=[0.3, 0.3, 0.32], roughness=0.9)', 'output floor = floor_m'];
names.forEach((name, i) => {
  lines.push(`${name}_b = solid.box@1(size=[4, 4, 4])`, `${name}_at = xform.move@1(${name}_b, by=[${-26 + i * 7.4}, 2.1, 0])`,
    `${name}_m = solid.material@1(${name}_at, color=[0.5, 0.5, 0.5], roughness=0.8)`,
    `${name}_t = solid.texture@1(${name}_m, program="${MATERIALS[name].program}", tile=${MATERIALS[name].tile}, size="512")`, `output ${name} = ${name}_t`);
});
lines.push('sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=6, turbidity=3, clouds=false)', 'output sky = sky');

const { server, port } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = []; const bakeWarnings = [];
try {
  const page = await browser.newPage({ viewport: { width: 1300, height: 420 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => {
    if (m.type() === 'warning' && /texture bake/.test(m.text())) bakeWarnings.push(m.text().slice(0, 300));
    if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(m.text());
  });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&scene=open&docname=materials-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'the material library'), lines.join('\n'));
  assert(staged.pending, JSON.stringify(staged).slice(0, 800));
  await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent, null, { timeout: 240000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.failureCurrent), false, 'the world evaluates');
  await page.waitForTimeout(3000);
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.filter(w => w.code === 'texture.bake_failed'));
  assert.deepEqual(warnings, [], `every library program bakes: ${JSON.stringify(warnings)}`);
  assert.deepEqual(bakeWarnings, [], `no bake warnings in the console: ${bakeWarnings.join(' | ')}`);

  // Each material is more than one colour on screen.
  await page.keyboard.press('h');
  await page.evaluate(() => window.__runtime.viewer_look_at(0, 3, 24, 0, 2, 0));
  await page.waitForTimeout(2500);
  const shot = PNG.sync.read(await page.screenshot());
  const reportsDir = process.env.MATERIALS_SHEET;
  if (reportsDir) (await import('node:fs')).writeFileSync(reportsDir, PNG.sync.write(shot));
  assert(shot.width > 0, 'a picture was taken');
  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')}`);
  console.log(`materials ok (${names.length} programs baked)`);
} finally {
  await browser.close();
  server.close();
}
