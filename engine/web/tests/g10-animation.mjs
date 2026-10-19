import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: an `anim.track` clip plays in the viewport. A door is hinged at its left edge and swings 90 degrees
// over 1.5 seconds. The shared clock is paused at chosen times and the picture is read: at rest, halfway, at
// the end, long after the end (a one-shot clip holds its last pose) and back at the start again.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5211'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `door = solid.box@1(size=[0.9, 2, 0.05])
paint = solid.material@1(door, color=[0.75, 0.35, 0.15], roughness=0.6)
swing = anim.track@1(paint, property="rotation", keys="0:0,0,0; 1.5:0,90,0", interpolation="linear", pivot=[-0.45, 0, 0], loop=false)
output hinged_door = swing
`;
const logs = [];
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5211')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5211/?viewer=1&eval=1&docname=g10-animation');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'animation'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 300)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const animated = await page.evaluate(() => JSON.parse(window.__runtime.eval_projection_status()).animated_parts);
  assert.equal(animated, 1, 'the viewer knows one part is animated');
  const tracks = await page.evaluate(() => window.__ve.evaluation.parts[0].animation);
  assert.equal(tracks.length, 1);
  assert.equal(tracks[0].property, 'rotation');

  const at = async (seconds, name) => {
    await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
    await page.waitForTimeout(700);
    const shot = await page.screenshot({ path: `${reports}animation-${name}.png` });
    return PNG.sync.read(shot);
  };
  const differing = (a, b) => {
    let n = 0;
    for (let i = 0; i < a.data.length; i += 4) {
      if (Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2])) > 24) n++;
    }
    return n / (a.width * a.height);
  };
  const start = await at(0, 'start');
  const middle = await at(0.75, 'middle');
  const end = await at(1.5, 'end');
  const long = await at(100, 'long-after');
  const again = await at(0, 'start-again');
  const swing = differing(start, end), half = differing(start, middle), rest = differing(middle, end);
  console.log(`G10 animation: start to end ${(swing * 100).toFixed(1)}% of the picture, start to middle ${(half * 100).toFixed(1)}%, middle to end ${(rest * 100).toFixed(1)}%`);
  assert(swing > 0.02, 'the door visibly swings');
  assert(half > 0.005 && rest > 0.005, 'the middle is a pose of its own');
  assert(differing(end, long) < 0.0005, 'a one-shot clip holds its last pose');
  assert(differing(start, again) < 0.0005, 'back at the start the picture is the same as at the start');

  // The same clip leaves in the exported file: one valid glTF animation whose last rotation key is the
  // quarter turn about Y, on the door's node.
  const bytes = Uint8Array.from(await page.evaluate(async () => Array.from(await window.__ve.doc.exportGlb())));
  const report = await validator.validateBytes(bytes);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  assert.equal(report.issues.numWarnings, 0, `validator warnings: ${messages}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset);
  const json = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + view.getUint32(12, true))));
  assert.equal(json.animations?.length, 1, 'one animation');
  const rotation = json.animations[0].channels.find(c => c.target.path === 'rotation');
  assert(rotation, 'a rotation channel');
  const sampler = json.animations[0].samplers[rotation.sampler];
  const binary = 20 + view.getUint32(12, true) + 8;
  const output = json.accessors[sampler.output], outputView = json.bufferViews[output.bufferView];
  const last = new Float32Array(bytes.buffer.slice(bytes.byteOffset + binary + outputView.byteOffset + (output.count - 1) * 16, bytes.byteOffset + binary + outputView.byteOffset + output.count * 16));
  console.log(`G10 animation export: ${output.count} samples, last rotation ${[...last].map(v => v.toFixed(3)).join(', ')}`);
  assert(Math.abs(last[1] - Math.SQRT1_2) < 1e-4 && Math.abs(last[3] - Math.SQRT1_2) < 1e-4, 'the end of the clip is a quarter turn about Y');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
