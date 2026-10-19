// The viewer's texture memory is a byte budget over three map sizes. Forty distinct textured parts at `size="256"` all fit and show
// their own colours (the old limit was 32 maps of 1024). Forty distinct parts at the default 1024 are over the budget, so the
// viewer shrinks the last ones instead of dropping their maps, and says so.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5255;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
const BUDGET = 192 * 1024 * 1024;
const mapBytes = side => { let t = 0; for (let e = side; ; e = Math.floor(e / 2)) { t += e * e; if (e <= 1) break; } return t * 4; };

const hsv = (h, s, v) => {
  const i = Math.floor(h * 6), f = h * 6 - i, p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s);
  return [[v, t, p], [q, v, p], [p, v, t], [p, q, v], [t, p, v], [v, p, q]][i % 6];
};
const scene = (count, size) => {
  const lines = [];
  for (let i = 0; i < count; i++) {
    const [r, g, b] = hsv(i / count, 0.9, 0.9);
    const x = ((i % 8) - 3.5) * 0.55, z = -Math.floor(i / 8) * 0.55;
    lines.push(
      `b${i} = solid.box@1(size=[0.4, 0.4, 0.4])`,
      `m${i} = xform.move@1(b${i}, by=[${x.toFixed(3)}, 0.2, ${z.toFixed(3)}])`,
      `c${i} = solid.material@1(m${i}, color=[1, 1, 1], roughness=0.8)`,
      `t${i} = solid.texture@1(c${i}, program="n = expr(code=\\"out = vec4(${r.toFixed(3)}, ${g.toFixed(3)}, ${b.toFixed(3)}, 1.0)\\"); albedo = n"${size ? `, size="${size}"` : ''})`,
      `output p${i} = t${i}`);
  }
  return lines.join('\n');
};

try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const run = async (name, count, size) => {
    const page = await browser.newPage({ viewport: { width: 800, height: 520 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=high&scene=stage`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); window.__veUploadTimeoutMs = 900000; });
    const staged = await page.evaluate(([text, label]) => window.__ve.doc.sendText(text, label), [scene(count, size), name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 600000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${name} evaluates`);
    const status = JSON.parse(await page.evaluate(() => window.__runtime.eval_projection_status()));
    const notes = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => w.code));
    await page.evaluate(() => window.__runtime.viewer_look_at(0, 3.2, 3.4, 0, 0, -1.0));
    await page.waitForTimeout(3500);
    await page.evaluate(() => {
      const canvas = document.querySelector('canvas');
      document.querySelectorAll('body *').forEach(element => {
        if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
      });
    });
    await page.waitForTimeout(400);
    const image = PNG.sync.read(await page.screenshot({ path: `${reports}texture-budget-${name}.png` }));
    await page.close();
    // How many of 24 hue buckets hold a real amount of strongly coloured pixels: the parts' own colours, not the default grey-blue.
    const buckets = new Array(24).fill(0);
    for (let at = 0; at < image.data.length; at += 4) {
      const r = image.data[at] / 255, g = image.data[at + 1] / 255, b = image.data[at + 2] / 255;
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      if (max < 0.25 || (max - min) / max < 0.55) continue;
      let h = max === r ? ((g - b) / (max - min)) % 6 : max === g ? (b - r) / (max - min) + 2 : (r - g) / (max - min) + 4;
      h = (h * 60 + 360) % 360;
      buckets[Math.floor(h / 15)]++;
    }
    return { status, notes, hues: buckets.filter(n => n > 150).length };
  };

  const small = await run('small', 40, 256);
  assert.equal(small.status.texture_maps, 40, 'forty distinct maps are all held');
  assert.equal(small.status.texture_bytes, 40 * mapBytes(256), 'at 256 they cost 40 small chains');
  assert(!small.notes.some(code => code.startsWith('texture.viewport')), `no budget notes (${small.notes})`);
  assert(small.hues >= 14, `the parts show their own colours (${small.hues} of 24 hue buckets)`);

  assert(40 * mapBytes(1024) > BUDGET, 'forty large maps really are over budget at full size');
  const large = await run('large', 40, null);
  assert(large.status.texture_bytes <= BUDGET, `the budget holds (${large.status.texture_bytes} of ${BUDGET})`);
  assert(large.status.texture_bytes > 0.7 * BUDGET, 'and most of it is used: only the last maps shrank');
  assert.equal(large.status.texture_maps, 40, 'every part keeps its map: shrunk, not dropped');
  assert(large.notes.includes('texture.viewport_shrunk'), `the shrinking is reported (${large.notes})`);
  assert(!large.notes.includes('texture.viewport_budget'), 'nothing was dropped');
  assert(large.hues >= 14, `the parts still show their own colours (${large.hues} of 24 hue buckets)`);
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 texture budget: 40 maps at 256 = ${(small.status.texture_bytes / 1048576).toFixed(1)} MiB (${small.hues} hues); 40 maps at 1024 fit ${(large.status.texture_bytes / 1048576).toFixed(1)} MiB by shrinking (${large.hues} hues)`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
