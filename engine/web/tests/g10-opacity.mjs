import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';

const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const server = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5230'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const program = opacity => `box = solid.box@1(size=[0.85,0.85,0.25])
red = solid.material@1(box, color=[0.95,0.04,0.02], roughness=0.8)
back = xform.move@1(red, by=[0,0,-0.22])
output back
pane = solid.box@1(size=[1.3,1.3,0.04])
blue = solid.material@1(pane, color=[0.03,0.12,0.95], opacity=${opacity}, roughness=0.7)
front = xform.move@1(blue, by=[0,0,0.22])
output pane = front
`;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5230')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  const evaluate = async opacity => {
    await page.goto(`http://127.0.0.1:5230/?eval=1&docname=g10-opacity-${opacity}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'opacity'), program(opacity));
    assert(staged.pending, `staged opacity ${opacity}`);
    await page.waitForFunction(() => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final'
      || window.__ve.evaluation.state.failureCurrent, null, { timeout: 180000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `opacity ${opacity} evaluates`);
    await page.waitForTimeout(250);
    const rect = await page.locator('canvas').first().boundingBox();
    assert(rect, 'viewport canvas exists');
    const path = `${reports}opacity-${opacity}.png`;
    await page.screenshot({ path, clip: rect });
    return PNG.sync.read(await readFile(path));
  };
  const mixedPixels = png => {
    let mixed = 0;
    const x0 = Math.floor(png.width * 0.35), x1 = Math.ceil(png.width * 0.65);
    const y0 = Math.floor(png.height * 0.2), y1 = Math.ceil(png.height * 0.8);
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
      const i = (y * png.width + x) * 4;
      const [r, g, b] = png.data.subarray(i, i + 3);
      if (r > 65 && b > 65 && r > g * 1.25 && b > g * 1.25) mixed++;
    }
    return mixed;
  };
  const transparent = await evaluate(0.5);
  const opaque = await evaluate(1);
  const throughGlass = mixedPixels(transparent), blocked = mixedPixels(opaque);
  console.log(`G10 opacity: mixed red/blue pixels through half-opacity pane=${throughGlass}, opaque pane=${blocked}`);
  assert(throughGlass > 30, 'red behind the pane contributes to blended pixels');
  assert(throughGlass > blocked * 1.5, 'opacity 1 hides the red box behind the pane');
} finally {
  await browser.close();
  server.kill();
  await server.exited;
}
