import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// E1: the reported bug was that the CPU (dual quaternion) skin broke the left toes. Each foot and its first toe joint are bent the same way; the body is
// skinned with linear blending (GPU) and with dual_quaternion (CPU), and a close-up of each foot is compared between the two blends and between the
// two sides. A broken toe would show as a large difference between the blends on the left foot but not on the right.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/e1/', import.meta.url).pathname;
const bones = ['root', 'pelvis_l', 'upperleg01_l', 'upperleg02_l', 'lowerleg01_l', 'lowerleg02_l', 'foot_l', 'toe1_1_l', 'pelvis_r', 'upperleg01_r', 'upperleg02_r', 'lowerleg01_r', 'lowerleg02_r', 'foot_r', 'toe1_1_r'];
const parents = { pelvis_l: 'root', upperleg01_l: 'pelvis_l', upperleg02_l: 'upperleg01_l', lowerleg01_l: 'upperleg02_l', lowerleg02_l: 'lowerleg01_l', foot_l: 'lowerleg02_l', toe1_1_l: 'foot_l', pelvis_r: 'root', upperleg01_r: 'pelvis_r', upperleg02_r: 'upperleg01_r', lowerleg01_r: 'upperleg02_r', lowerleg02_r: 'lowerleg01_r', foot_r: 'lowerleg02_r', toe1_1_r: 'foot_r' };
const scene = blend => {
  let text = `sky = scene.sky@1(sun_elevation=40, sun_azimuth=30, sun_intensity=4, clouds=false)
output sky = sky
floor_g = solid.box@1(size=[8, 0.2, 8])
floor_a = xform.move@1(floor_g, by=[0, -0.1, 0])
floor_m = solid.material@1(floor_a, color=[0.45, 0.45, 0.42], roughness=0.9)
output floor = floor_m
body = body.human@1(height=0.5)
`;
  for (const bone of bones) {
    text += `b_${bone} = body.bone@1(body, bone="${bone}")\n`;
    const parent = parents[bone];
    let last = parent ? `p_${bone}` : `b_${bone}`;
    if (parent) text += `p_${bone} = anim.parent@1(b_${bone}, parent="${parent}")\n`;
    if (bone.startsWith('foot_') || bone.startsWith('toe1_1_')) {
      text += `t_${bone} = anim.track@1(${last}, property="rotation", keys="0:0,0,0; 1:-35,0,0", interpolation="linear", loop=false, pivot_at="head")\n`;
      last = `t_${bone}`;
    }
    text += `output ${bone} = ${last}\n`;
  }
  text += `skinned = body.skin@1(body, bones="${bones.join('; ')}", blend="${blend}")
skin_m = solid.material@1(skinned, color=[0.86, 0.66, 0.52], roughness=0.6)
output person = skin_m
`;
  return text;
};
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5224', '--dir', process.env.VE_DIST_DIR || 'web/dist-bevy-gpu'], { cwd: root, stdout: 'ignore', stderr: 'inherit' });
const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  for (let i = 0; i < 100; i++) { try { if ((await fetch('http://127.0.0.1:5224')).ok) break; } catch {} await Bun.sleep(100); }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error' || driverAdvisory.test(message.text()) || message.location().url.endsWith('/favicon.ico')) return;
    logs.push(`${message.text()} ${message.location().url}`);
  });
  const shoot = async (blend, side) => {
    await page.goto(`http://127.0.0.1:5224/?viewer=1&eval=1&docname=e1-toes-${blend}-${side}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'toes'), scene(blend));
    assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 500)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 300000 });
    assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
    await page.evaluate(() => window.__runtime.viewer_animation(false, 1));
    // The left foot is on the +x side. Look at either foot from the front and a little above.
    const x = side === 'l' ? 0.09 : -0.09;
    await page.evaluate(([fx]) => window.__runtime.viewer_look_at(fx, 0.35, 0.7, fx, 0.04, 0.1), [x]);
    await page.keyboard.press('h');
    await page.waitForTimeout(2500);
    return PNG.sync.read(await page.screenshot({ path: `${reports}toes-${blend}-${side}.png` }));
  };
  const diff = (a, b, mirror = false) => {
    let n = 0;
    for (let y = 100; y < 560; y++) for (let x = 0; x < 520; x++) {
      const i = (y * a.width + x) * 4, j = (y * a.width + (mirror ? 899 - x : x)) * 4;
      if (Math.max(Math.abs(a.data[i] - b.data[j]), Math.abs(a.data[i + 1] - b.data[j + 1]), Math.abs(a.data[i + 2] - b.data[j + 2])) > 30) n++;
    }
    return n;
  };
  const linearLeft = await shoot('linear', 'l');
  const dualLeft = await shoot('dual', 'l');
  const linearRight = await shoot('linear', 'r');
  const dualRight = await shoot('dual', 'r');
  const [left, right] = [diff(linearLeft, dualLeft), diff(linearRight, dualRight)];
  console.log(`E1 toes: linear against dual quaternion, pixels differing: left foot ${left}, right foot ${right}`);
  // Two blends of the same bent foot differ a little at the joints; a broken toe is a large, one-sided difference.
  assert(left < 6000 && right < 6000, 'the blends agree about the shape of each foot');
  assert(Math.abs(left - right) < Math.max(1500, 0.5 * Math.max(left, right)), 'the left foot is no worse than the right');
  console.log('E1 browser errors:', JSON.stringify(logs));
  assert.deepEqual(logs, [], 'no browser errors');
} finally { await browser.close(); child.kill(); await child.exited; }
