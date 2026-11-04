import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// D3: `cloth.sim` is live cloth. A red cloth hangs by its top row from a blue rail part that slides sideways. With the animation clock
// paused the cloth settles under the rail; when the rail jumps, the cloth lags behind it (it has mass) and then catches up. Run against
// the Bevy bundle: `VE_DIST=web/dist-bevy-gpu bun web/tests/d3-cloth.mjs` (the server is started here with --dir).
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/d3/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5212', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium',
  args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU', '--ignore-gpu-blocklist'],
});
const program = `sky = scene.sky@1(sun_elevation=50, sun_azimuth=20, sun_intensity=4, clouds=false)
output sky = sky
floor = solid.box@1(size=[12, 0.2, 6])
floor_m = solid.material@1(floor, color=[0.6, 0.58, 0.5], roughness=0.9)
floor_p = xform.move@1(floor_m, by=[0, -0.1, 0])
output floor = floor_p
box = solid.box@1(size=[0.16, 0.16, 0.16])
blue = solid.material@1(box, color=[0.05, 0.1, 0.95], roughness=0.5)
rail_at = xform.move@1(blue, by=[0, 2.4, 0])
rail_bone = rig.bone@1(rail_at, head=[0, 2.4, 0], tail=[0, 2.4, 0.3])
rail = anim.track@1(rail_bone, property="position", keys="0:0,0,0; 0.3:0.6,0,0", interpolation="linear", loop=false)
output rail = rail
grid = poly.grid@1(width=1, height=1, columns=10, rows=10)
hung = xform.move@1(grid, by=[0, 1.9, 0])
top = poly.select@1(hung, element="vertex", by="box", box_min=[-1, 2.39, -1], box_max=[1, 2.41, 1])
cloth = cloth.sim@1(hung, top, pin_bone="rail", bend=0.05, density=0.3)
red = solid.material@1(cloth, color=[0.95, 0.1, 0.05], roughness=0.7)
output cape = red
`;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5212')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text())) return;
    if (message.location().url.endsWith('/favicon.ico')) return; // the dev server has no favicon
    logs.push(`${message.text()} ${message.location().url}`);
  });
  await page.goto('http://127.0.0.1:5212/?viewer=1&eval=1&docname=d3-cloth');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'cloth'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const cloth = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'cape')?.cloth);
  assert(cloth, 'the cape part carries a cloth description');
  console.log('D3 cloth spec:', JSON.stringify({ ...cloth, pins: cloth.pins.length }));
  assert.equal(cloth.pin_bone, 'rail');
  assert.equal(cloth.pins.length, 11, 'the top row of 10 columns has 11 points');

  // Centroid (in pixels) of the strongly red and the strongly blue pixels.
  const find = async name => {
    await page.waitForTimeout(100);
    const png = PNG.sync.read(await page.screenshot({ path: `${reports}cloth-${name}.png` }));
    const sums = { red: [0, 0, 0], blue: [0, 0, 0] };
    let leftEdge = Infinity;
    // The page's side panel covers the right of the picture and its play bar the bottom; the cloth moves in the rest.
    for (let y = 0; y < 395; y++) for (let x = 0; x < 530; x++) {
      const i = (y * png.width + x) * 4, [r, g, b] = [png.data[i], png.data[i + 1], png.data[i + 2]];
      const key = r > 200 && g < 130 && b < 130 ? 'red' : b > 200 && r < 130 && g < 130 ? 'blue' : null;
      if (key) { sums[key][0] += x; sums[key][1] += y; sums[key][2]++; }
      if (key === 'red' && x < leftEdge) leftEdge = x;
    }
    const centre = s => ({ x: s[0] / Math.max(s[2], 1), y: s[1] / Math.max(s[2], 1), n: s[2] });
    // `gap` is how far the cloth's left edge hangs left of the rail: it grows when the cloth lags a rail that moves right.
    return { red: centre(sums.red), blue: centre(sums.blue), gap: centre(sums.blue).x - leftEdge };
  };
  // Look at the middle of the rail's travel from the front, and crop the page's own panels out of every picture.
  const side = Boolean(process.env.VE_SIDE);
  assert(await page.evaluate(side => side ? window.__runtime.viewer_look_at(6, 1.9, 0, 0, 1.9, 0) : window.__runtime.viewer_look_at(0.7, 1.9, 6, 0.7, 1.9, 0), side), 'the camera accepts the pose');
  await page.keyboard.press('h');
  const pause = seconds => page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);

  await pause(0);
  await page.waitForTimeout(4000);
  const live = await page.evaluate(() => JSON.parse(window.__runtime.viewer_cloth()));
  console.log('D3 cloth live:', JSON.stringify(live));
  assert.equal(live.cloths.length, 1, 'the runtime steps one cloth');
  assert(live.cloths[0].steps > 20, 'the solver has been stepped many times');
  const rest = await find('rest');
  // The rail slides 0.6 m in 0.3 s (2 m/s: about 3 cm a frame) and then holds; the cloth swings behind it.
  await page.evaluate(() => window.__runtime.viewer_animation(true, 0));
  // Poll the runtime's own read-out while it moves: how far the cloth's middle trails the rail, in metres.
  let worstLag = 0;
  for (let i = 0; i < 40; i++) {
    await page.waitForTimeout(40);
    const c = (await page.evaluate(() => JSON.parse(window.__runtime.viewer_cloth()))).cloths[0];
    worstLag = Math.max(worstLag, c.pin_matrix[12] - (c.min[0] + c.max[0]) / 2);
  }
  const jumped = await find('just-after-jump');
  await page.waitForTimeout(5000);
  const end = (await page.evaluate(() => JSON.parse(window.__runtime.viewer_cloth()))).cloths[0];
  const endLag = end.pin_matrix[12] - (end.min[0] + end.max[0]) / 2;
  console.log(`D3 cloth: worst lag behind the rail ${worstLag.toFixed(3)} m, after settling ${endLag.toFixed(3)} m, rail moved ${end.pin_matrix[12].toFixed(2)} m, hangs ${end.min[1].toFixed(2)}..${end.max[1].toFixed(2)}`);
  assert(end.pin_matrix[12] > 0.55, 'the rail moved 0.6 m');
  assert(worstLag > 0.03, `the cloth trailed behind the moving rail (${worstLag.toFixed(3)} m)`);
  assert(Math.abs(endLag) < 0.04, 'it caught up and hangs under the rail again');
  assert(end.max[1] <= 2.401 && end.min[1] < 1.6, 'it hangs below its pins');
  const settled = await find('settled');
  console.log(`D3 cloth: rest red ${JSON.stringify(rest.red)} blue ${JSON.stringify(rest.blue)}`);
  console.log(`D3 cloth: left-edge gap at rest ${rest.gap.toFixed(1)} px, just after the jump ${jumped.gap.toFixed(1)}, settled ${settled.gap.toFixed(1)}; rail x ${rest.blue.x.toFixed(0)} -> ${settled.blue.x.toFixed(0)}`);
  assert(rest.red.n > 500 && rest.blue.n > 50, 'both the cloth and the rail are visible');
  assert(rest.red.y > rest.blue.y, 'the cloth hangs below the rail');
  assert(Math.abs(rest.red.x - rest.blue.x) < 25, 'at rest the cloth hangs under the rail');
  assert(settled.blue.x - rest.blue.x > 40, 'the rail moved right across the picture');
  assert(Math.abs(settled.gap - rest.gap) < 12, `the picture agrees: the cloth hangs under the rail again (${(settled.gap - rest.gap).toFixed(1)} px)`);
  console.log('D3 cloth browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
