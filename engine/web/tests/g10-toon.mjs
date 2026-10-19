import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: cel shading. In the toon gallery the physically shaded sphere shows a smooth gradient, and a toon sphere with N bands
// shows N + 1 flat tones (the shadow colour and N lit steps). Each sphere is found on screen by projecting its centre.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const port = 5234;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text())) logs.push(m.text().slice(0, 300)); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&scene=stage&eval=1&model=toon_gallery&docname=g10-toon-${Date.now()}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 200000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'the gallery evaluates');
  assert.deepEqual(await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => w.message)), [], 'no warnings');
  await page.waitForTimeout(3000);
  const png = PNG.sync.read(await page.screenshot());
  const project = (x, y, z) => page.evaluate(([px, py, pz]) => JSON.parse(window.__runtime.viewer_project(px, py, pz, 1200, 700)), [x, y, z]);
  const luma = (i) => 0.2126 * png.data[i] + 0.7152 * png.data[i + 1] + 0.0722 * png.data[i + 2];
  // Distinct tones inside a sphere: luminance bucketed in steps of 7, keeping buckets holding at least 3% of the pixels.
  const tones = async (x) => {
    const centre = await project(x, 0.5, 0);
    const edge = await project(x, 1.0, 0);
    const cx = centre.x ?? centre[0], cy = centre.y ?? centre[1];
    const radius = Math.hypot(cx - (edge.x ?? edge[0]), cy - (edge.y ?? edge[1]));
    const buckets = new Map();
    let total = 0;
    for (let y = Math.floor(cy - radius); y <= Math.ceil(cy + radius); y++) {
      for (let px = Math.floor(cx - radius); px <= Math.ceil(cx + radius); px++) {
        if (Math.hypot(px - cx, y - cy) > radius * 0.8) continue;
        const bucket = Math.round(luma((png.width * y + px) * 4) / 7);
        buckets.set(bucket, (buckets.get(bucket) ?? 0) + 1);
        total++;
      }
    }
    return { radius, levels: [...buckets.values()].filter(n => n >= total * 0.03).length };
  };
  // Mean luminance of a thin ring just outside a sphere's silhouette: dark where an outline is drawn.
  const ring = async (x) => {
    const centre = await project(x, 0.5, 0);
    const edge = await project(x, 1.0, 0);
    const cx = centre.x ?? centre[0], cy = centre.y ?? centre[1];
    const radius = Math.hypot(cx - (edge.x ?? edge[0]), cy - (edge.y ?? edge[1]));
    let sum = 0, count = 0;
    for (let angle = 0; angle < Math.PI * 2; angle += 0.05) {
      for (const offset of [0.5, 1.0, 1.5]) {
        const px = Math.round(cx + Math.cos(angle) * (radius + offset)), py = Math.round(cy + Math.sin(angle) * (radius + offset));
        sum += luma((png.width * py + px) * 4); count++;
      }
    }
    return sum / count;
  };
  const outlinedRing = await ring(-0.6);
  const plainRing = await ring(-3);
  console.log(`ring luminance: toon ${outlinedRing.toFixed(0)}, physically shaded ${plainRing.toFixed(0)}`);
  assert(outlinedRing < plainRing - 25, 'a toon sphere with outline=2.5 has a dark ring around it, the physical one does not');
  const result = {};
  for (const [name, x] of [['pbr', -3], ['bands_1', -1.8], ['bands_2', -0.6], ['bands_3', 0.6], ['bands_4', 1.8]]) result[name] = await tones(x);
  console.log('tones per sphere:', JSON.stringify(Object.fromEntries(Object.entries(result).map(([k, v]) => [k, v.levels]))), `radius ~${result.pbr.radius.toFixed(0)} px`);
  assert(result.pbr.radius > 12, 'the spheres are big enough on screen to measure');
  assert(result.pbr.levels >= 6, `the physically shaded sphere is a smooth gradient: ${result.pbr.levels} tones`);
  for (const n of [1, 2, 3, 4]) {
    const levels = result[`bands_${n}`].levels;
    assert(levels >= n + 1 - 1 && levels <= n + 2, `bands=${n} gives flat tones, found ${levels}`);
    assert(levels < result.pbr.levels, `bands=${n} has fewer tones than the smooth sphere`);
  }
  assert(result.bands_4.levels > result.bands_1.levels, 'more bands give more tones');
  await page.screenshot({ path: `${reports}toon-gallery.png` });
  assert.deepEqual(logs, [], 'no browser errors');
  console.log('G10 toon: flat bands with tinted shadows; the physically shaded sphere stays smooth');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
