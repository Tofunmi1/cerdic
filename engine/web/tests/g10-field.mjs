import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: animated placed sets. A row of five reeds shares one sway track. Without a phase every reed leans the same way
// at the same moment; with `phase_step` they lean differently (a wave along the row). The file exports each copy with
// its own channels and validates.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5219'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const field = phase => `reed = solid.box@1(size=[0.06, 0.8, 0.06])
green = solid.material@1(reed, color=[0.1, 0.8, 0.2], roughness=0.6)
standing = xform.move@1(green, by=[0, 0.4, 0])
row = inst.array@1(standing, count_x=5, spacing=[0.22, 0, 0])
sway = anim.wave@1(row, property="rotation", amplitude=[0, 0, 25], period=2, pivot=[0, 0, 0]${phase})
output reeds = sway
`;
const logs = [];
const lime = (r, g, b) => g > r + 60 && g > b + 60;
// One lean per reed: how far the top of its blob sits to the side of its bottom.
const leans = png => {
  const mask = new Uint8Array(png.width * png.height);
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (lime(png.data[k], png.data[k + 1], png.data[k + 2])) mask[j * png.width + i] = 1;
  }
  const seen = new Uint8Array(mask.length), out = [];
  for (let start = 0; start < mask.length; start++) {
    if (!mask[start] || seen[start]) continue;
    const pixels = [], stack = [start]; seen[start] = 1;
    while (stack.length) {
      const at = stack.pop(); pixels.push(at);
      const x = at % png.width, y = (at - x) / png.width;
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx, ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= png.width || ny >= png.height) continue;
        const next = ny * png.width + nx;
        if (mask[next] && !seen[next]) { seen[next] = 1; stack.push(next); }
      }
    }
    if (pixels.length < 60) continue;
    const ys = pixels.map(p => Math.floor(p / png.width)), top = Math.min(...ys), bottom = Math.max(...ys), span = bottom - top;
    const mean = rows => { const xs = pixels.filter(p => rows(Math.floor(p / png.width))).map(p => p % png.width); return xs.reduce((a, b) => a + b, 0) / xs.length; };
    // A reed is told by where its base stands, which does not move when it leans.
    const base = mean(y => y > bottom - span * 0.15);
    out.push({ x: base, lean: mean(y => y < top + span * 0.15) - base });
  }
  return out.sort((a, b) => a.x - b.x);
};
const open = async (name, program) => {
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto(`http://127.0.0.1:5219/?eval=1&docname=g10-field-${name}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'field'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  return page;
};
const at = async (page, seconds, name) => {
  await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
  await page.waitForTimeout(800);
  return leans(PNG.sync.read(await page.screenshot({ path: `${reports}field-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } })));
};
const spread = list => Math.max(...list.map(l => l.lean)) - Math.min(...list.map(l => l.lean));
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5219')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });

  const together = await open('together', field(''));
  const same = await at(together, 0.5, 'together');
  assert.equal(same.length, 5, `five reeds are visible: ${JSON.stringify(same)}`);
  // At a quarter period every reed leans the same way (perspective makes the amount differ a little across the row).
  assert(same.every(r => r.lean > 12) || same.every(r => r.lean < -12), `without a phase every reed leans the same way: ${same.map(r => r.lean.toFixed(0))}`);
  await together.close();

  const waving = await open('waving', field(', phase_step=0.25'));
  const out = await at(waving, 0.5, 'waving');
  console.log(`G10 field: leans together ${same.map(r => r.lean.toFixed(0))}, with a phase step ${out.map(r => r.lean.toFixed(0))}`);
  assert.equal(out.length, 5, 'five reeds are visible');
  // Phases of 0, 0.25, 0.5, 0.75 and 1 s put the row at +25, +18, 0, -18 and -25 degrees: it leans both ways.
  assert(out[0].lean < -12 && out[4].lean > 12, 'with a phase step the row leans one way at one end and the other way at the far end');
  assert(spread(out) > 2 * spread(same), 'and the reeds differ far more than perspective alone explains');
  const later = await at(waving, 0.9, 'waving-later');
  assert(later.some((r, i) => Math.abs(r.lean - out[i].lean) > 8), 'and they keep moving');

  const bytes = Uint8Array.from(await waving.evaluate(async () => Array.from(await window.__ve.doc.exportGlb())));
  const report = await validator.validateBytes(bytes);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset);
  const gltf = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + view.getUint32(12, true))));
  assert.equal(gltf.nodes.length, 5, 'five copies in the file');
  assert.equal(gltf.animations[0].channels.length, 15, 'each copy has translation, rotation and scale channels');
  await waving.close();
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
