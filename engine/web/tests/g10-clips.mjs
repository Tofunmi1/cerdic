import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: named clips and sequences (ADR 0031). One arm part has two clips, "wave" and "reach", that run separately:
// one button runs `arm_out:wave`, another `arm_out:reach`, and each leaves the other where it was. A second part runs a
// sequence a, then b, then c with one click, and replays it on the next. Each clip exports as its own glTF animation.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5217'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
const open = async (name, program) => {
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto(`http://127.0.0.1:5217/?eval=1&docname=g10-clips-${name}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'clips'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const ids = await page.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.map(p => [p.name, p.node])));
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  return { page, ids };
};
const state = page => page.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()));
const press = (page, id) => page.evaluate(node => window.__runtime.viewer_press(node), id);
const partOf = async (page, id) => (await state(page)).parts.find(p => p.node === id);
const shot = async (page, name) => PNG.sync.read(await page.screenshot({ path: `${reports}clips-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
const differing = (a, b) => {
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if (Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2])) > 24) n++;
  }
  return n / (a.width * a.height);
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5217')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });

  // Two clips on one part, run separately by two buttons.
  const arm = await open('arm', `bar = solid.box@1(size=[0.1, 0.8, 0.1])
paint = solid.material@1(bar, color=[0.85, 0.35, 0.1], roughness=0.6)
lifted = xform.move@1(paint, by=[0, 0.4, 0])
wave = anim.track@1(lifted, property="rotation", keys="0:0,0,0; 1:0,0,50", pivot=[0, 0, 0], interpolation="linear", clip="wave", loop=false)
reach = anim.track@1(wave, property="rotation", keys="0:0,0,0; 2:70,0,0", pivot=[0, 0, 0], interpolation="linear", clip="reach", loop=false)
listen = behavior.on@1(reach, trigger="signal", action="play")
output arm_out = listen
b1 = solid.box@1(size=[0.1, 0.1, 0.1])
w = xform.move@1(b1, by=[-0.6, 0.1, 0])
wave_button = behavior.on@1(w, trigger="select", target="arm_out:wave")
output wave_press = wave_button
b2 = solid.box@1(size=[0.1, 0.1, 0.1])
r = xform.move@1(b2, by=[0.6, 0.1, 0])
reach_button = behavior.on@1(r, trigger="select", target="arm_out:reach")
output reach_press = reach_button
`);
  const clips = async () => (await partOf(arm.page, arm.ids.arm_out)).clips;
  assert.deepEqual(Object.keys(await clips()).sort(), ['reach', 'wave'], 'the part has both clips');
  await arm.page.waitForTimeout(800);
  const rest = await shot(arm.page, 'rest');
  await press(arm.page, arm.ids.wave_press);
  await arm.page.waitForFunction(id => { const c = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(p => p.node === id).clips; return c.wave.time > 0.99 && !c.wave.running; }, arm.ids.arm_out, { timeout: 20000 });
  assert.equal((await clips()).reach.time, 0, 'the wave left the reach clip alone');
  await arm.page.waitForTimeout(500);
  const waved = await shot(arm.page, 'waved');
  assert(differing(rest, waved) > 0.003, 'the wave clip moved the arm');
  await press(arm.page, arm.ids.reach_press);
  await arm.page.waitForFunction(id => { const c = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(p => p.node === id).clips; return c.reach.time > 1.99 && !c.reach.running; }, arm.ids.arm_out, { timeout: 30000 });
  assert((await clips()).wave.time > 0.99, 'the reach left the wave clip where it was');
  await arm.page.waitForTimeout(500);
  assert(differing(waved, await shot(arm.page, 'both')) > 0.003, 'the two clips combine: reaching moved the arm further');

  const bytes = Uint8Array.from(await arm.page.evaluate(async () => Array.from(await window.__ve.doc.exportGlb())));
  const report = await validator.validateBytes(bytes);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset);
  const gltf = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + view.getUint32(12, true))));
  assert.deepEqual(gltf.animations.map(a => a.name).sort(), ['reach', 'wave'], 'each clip is its own glTF animation');
  await arm.page.close();

  // A sequence: one click runs a, then b, then c, and the next click plays it again.
  const seq = await open('sequence', `cube = solid.box@1(size=[0.2, 0.2, 0.2])
a = anim.track@1(cube, property="position", keys="0:0,0,0; 0.5:1,0,0", interpolation="linear", clip="a", loop=false)
b = anim.track@1(a, property="position", keys="0:0,0,0; 0.5:0,1,0", interpolation="linear", clip="b", loop=false)
c = anim.track@1(b, property="position", keys="0:0,0,0; 0.5:0,0,1", interpolation="linear", clip="c", loop=false)
go = behavior.on@1(c, trigger="select", clip="a", then="b; c", action="play")
output runner = go
`);
  const sample = async () => seq.page.evaluate(async id => {
    const out = [];
    for (let i = 0; i < 130; i++) {
      const clips = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(p => p.node === id).clips;
      out.push({ a: clips.a.time, b: clips.b.time, c: clips.c.time });
      await new Promise(r => setTimeout(r, 25));
    }
    return out;
  }, seq.ids.runner);
  const first = sample();
  await press(seq.page, seq.ids.runner);
  const seen = await first;
  const startedB = seen.findIndex(s => s.b > 0), startedC = seen.findIndex(s => s.c > 0);
  assert(startedB >= 0 && startedC >= 0, `all three clips ran: ${JSON.stringify(seen.slice(-1))}`);
  // The first time b (or c) is seen moving, the clip before it has already reached its end.
  assert(seen[startedB].a > 0.49, `b starts only once a has finished (a was ${seen[startedB].a})`);
  assert(seen[startedC].b > 0.49, `c starts only once b has finished (b was ${seen[startedC].b})`);
  const last = seen[seen.length - 1];
  assert(last.a > 0.49 && last.b > 0.49 && last.c > 0.49, 'the whole sequence finished');
  // Replay: the second click rewinds the later clips and plays the sequence again.
  const again = sample();
  await press(seq.page, seq.ids.runner);
  const replay = await again;
  assert(replay.some(s => s.b < 0.1 && s.a > 0.1), 'the replay starts over with a');
  assert(replay[replay.length - 1].c > 0.49, 'and runs through to the end again');
  await seq.page.close();

  console.log('G10 clips: two clips run separately and combine, a target names a clip, a sequence chains and replays, each clip exports');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
