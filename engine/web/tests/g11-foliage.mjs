// Alpha cutout in a real browser: a quad whose texture draws a leaf-shaped ellipse in its alpha. Without `solid.foliage` the whole quad
// shows and it is single-sided; with it only the ellipse shows (in the picture and in the shadow) and it is seen from both sides.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g11/', import.meta.url).pathname;
const port = 5252;
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
  await mkdir(reports, { recursive: true });
  // A fresh page for each scene: staging text adds to the document, so names would collide.
  const openPage = async () => {
    const page = await browser.newPage({ viewport: { width: 640, height: 420 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !driverAdvisory.test(message.text())) errors.push(message.text());
    });
    await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&quality=high&scene=stage&ssao=on`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    return page;
  };

  // The leaf: an ellipse 0.8 wide and 0.57 tall in the quad's centred coordinates (-1 to 1), about 36% of the quad.
  const leaf = 'e = length(q * vec2(1.0, 1.4)); out = vec4(0.1, 0.8, 0.1, 1.0 - step(0.8, e))';
  // poly.grid is one single-sided quad facing +z with UVs over all of it.
  const quad = `card = poly.grid@1(width=1, height=1, columns=1, rows=1)
paint = solid.material@1(card, color=[1, 1, 1], roughness=0.8)
tex = solid.texture@1(paint, program="n = expr(code=\\"${leaf}\\"); albedo = n")
`;
  const green = image => {
    let count = 0;
    for (let y = 0; y < image.height; y++) {
      for (let x = 0; x < image.width; x++) {
        const at = (y * image.width + x) * 4;
        if (image.data[at + 1] > image.data[at] + 40 && image.data[at + 1] > image.data[at + 2] + 40) count++;
      }
    }
    return count;
  };
  // Stages a scene and photographs it from the front (+z) and from behind (-z), panels hidden. Returns green pixel counts.
  const photograph = async (name, tail) => {
    const page = await openPage();
    const settled = () => page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final',
      null, { timeout: 120000 });
    const staged = await page.evaluate(([source, label]) => window.__ve.doc.sendText(source, label), [quad + tail, name]);
    assert(staged.pending, `${name} staged: ${JSON.stringify(staged)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
        || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 120000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${name} evaluates`);
    const counts = {};
    for (const [side, z] of [['front', 3], ['behind', -3]]) {
      await page.evaluate(zeta => window.__runtime.viewer_look_at(0, 0.5, zeta, 0, 0.5, 0), z);
      await settled();
      await page.waitForTimeout(3500);
      await page.evaluate(() => {
        const canvas = document.querySelector('canvas');
        document.querySelectorAll('body *').forEach(element => {
          if (element !== canvas && !element.contains(canvas) && !canvas.contains(element)) element.style.visibility = 'hidden';
        });
      });
      await page.waitForTimeout(400);
      counts[side] = green(PNG.sync.read(await page.screenshot({ path: `${reports}foliage-${name}-${side}.png` })));
    }
    await page.close();
    return counts;
  };

  const plain = await photograph('plain', 'output card = tex');
  const cut = await photograph('cutout', 'leaf = solid.foliage@1(tex, alpha_cutoff=0.5)\noutput card = leaf');
  assert(plain.front > 2000, `the plain quad is on screen (${plain.front} green pixels)`);
  const ratio = cut.front / plain.front;
  assert(ratio > 0.25 && ratio < 0.5, `the cut-out quad shows about 36% of the plain one (${(ratio * 100).toFixed(0)}%: ${cut.front} against ${plain.front})`);
  // Foliage is two-sided: the same leaf from behind. An ordinary surface is culled from behind.
  assert(cut.behind > 0.8 * cut.front, `the leaf is still there from behind (${cut.behind} against ${cut.front} from the front)`);
  assert(plain.behind < 0.05 * plain.front, `the ordinary quad is culled from behind (${plain.behind} green pixels)`);
  assert.deepEqual(errors, [], 'no shader, WebGPU or browser errors');
  console.log(`G11 foliage: plain quad ${plain.front}px green, cut-out ${cut.front}px (${(ratio * 100).toFixed(0)}%); from behind the leaf ${cut.behind}px and the plain quad ${plain.behind}px`);
} finally {
  await browser.close();
  child.kill();
  await child.exited;
}
