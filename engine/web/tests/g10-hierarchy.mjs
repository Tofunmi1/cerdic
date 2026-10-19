import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: parent-child assemblies and glow tracks (ADR 0028). A green block follows a red bar that swings about
// one end: with `anim.parent` it travels with the bar, without it it stays put. The exported glTF nests the child
// under the parent and validates. A bulb with a glow track and a light is dark until a switch turns it on.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5213'], {
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
  await page.goto(`http://127.0.0.1:5213/?eval=1&docname=g10-hierarchy-${name}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'hierarchy'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 300)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  return page;
};
const shot = async (page, name) => PNG.sync.read(await page.screenshot({ path: `${reports}hierarchy-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
const centroid = (png, match) => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (match(png.data[k], png.data[k + 1], png.data[k + 2])) { x += i; y += j; n++; }
  }
  return n > 20 ? { x: x / n, y: y / n, n } : null;
};
const green = (r, g, b) => g > r + 60 && g > b + 60;
const at = async (page, seconds, name) => {
  await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
  await page.waitForTimeout(700);
  return shot(page, name);
};
const arm = follow => `bar = solid.box@1(size=[0.6, 0.1, 0.1])
red = solid.material@1(bar, color=[0.85, 0.15, 0.1], roughness=0.6)
swing = anim.track@1(red, property="rotation", keys="0:0,0,0; 1:0,90,0", interpolation="linear", pivot=[-0.3, 0, 0], loop=false)
output arm = swing
block = solid.box@1(size=[0.15, 0.15, 0.15])
lime = solid.material@1(block, color=[0.1, 0.8, 0.2], roughness=0.6)
placed = xform.move@1(lime, by=[0.5, 0, 0])
${follow ? 'tied = anim.parent@1(placed, parent="arm")' : 'tied = xform.move@1(placed, by=[0, 0, 0])'}
output hand = tied
`;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5213')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });

  // Without a parent the block stays where it is while the bar swings.
  const free = await open('free', arm(false));
  const freeStart = centroid(await at(free, 0, 'free-start'), green), freeEnd = centroid(await at(free, 1, 'free-end'), green);
  assert(freeStart && freeEnd, 'the block is visible');
  assert(Math.hypot(freeStart.x - freeEnd.x, freeStart.y - freeEnd.y) < 4, 'a free block does not move');
  await free.close();

  // With a parent it travels with the bar, and the bar's own motion is unchanged.
  const bound = await open('bound', arm(true));
  assert.equal(await bound.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'hand').parent), 'arm');
  const start = centroid(await at(bound, 0, 'bound-start'), green), end = centroid(await at(bound, 1, 'bound-end'), green);
  assert(start && end, 'the block is visible');
  assert(Math.abs(start.x - freeStart.x) < 4 && Math.abs(start.y - freeStart.y) < 4, 'at rest the assembly looks like the free one');
  const moved = Math.hypot(start.x - end.x, start.y - end.y);
  console.log(`G10 hierarchy: the block moved ${moved.toFixed(0)} px with its parent`);
  assert(moved > 25, 'the child follows the parent');
  // The timeline appears for shared-clock animation: scrub, speed, play and pause drive the clock.
  await bound.waitForSelector('#timeline:not([hidden])', { timeout: 10000 });
  assert.equal(await bound.getAttribute('#tl-scrub', 'max'), '1', 'the scrubber spans the clip');
  const clock = () => bound.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()).clock);
  await bound.evaluate(() => { const s = document.getElementById('tl-scrub'); s.value = '0.5'; s.dispatchEvent(new Event('input')); });
  await bound.waitForFunction(() => Math.abs(JSON.parse(window.__runtime.viewer_behaviors()).clock.seconds - 0.5) < 0.01);
  assert.equal((await clock()).playing, false, 'scrubbing pauses');
  await bound.selectOption('#tl-speed', '2');
  await bound.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).clock.speed === 2);
  await bound.click('#tl-play');
  await bound.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).clock.playing);
  assert.equal(await bound.textContent('#tl-play'), 'Pause');
  await bound.click('#tl-play');
  await bound.waitForFunction(() => !JSON.parse(window.__runtime.viewer_behaviors()).clock.playing);
  await bound.selectOption('#tl-speed', '1');
  await bound.evaluate(() => window.__runtime.viewer_animation(false, 1));
  const bytes = Uint8Array.from(await bound.evaluate(async () => Array.from(await window.__ve.doc.exportGlb())));
  const report = await validator.validateBytes(bytes);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  assert.equal(report.issues.numWarnings, 0, `validator warnings: ${messages}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset);
  const json = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + view.getUint32(12, true))));
  const index = name => json.nodes.findIndex(n => n.name === name);
  assert.deepEqual(json.nodes[index('arm')].children, [index('hand')], 'the exported hand hangs under the arm');
  assert.deepEqual(json.scenes[0].nodes, [index('arm')], 'only the arm is a scene root');
  await bound.close();

  // A lamp: dark until a switch turns its glow (and light) on, dark again when switched off.
  const lamp = await open('lamp', `wall = solid.box@1(size=[2.4, 1.6, 0.05])
plaster = solid.material@1(wall, color=[0.9, 0.9, 0.9], roughness=0.9)
backdrop = xform.move@1(plaster, by=[0, 0.8, -0.3])
output wall_out = backdrop
bulb = solid.sphere@1(radius=0.12)
glass = solid.material@1(bulb, color=[0.05, 0.05, 0.05], emissive=[2, 1.7, 1.1], emits_light=true, roughness=0.3)
raised = xform.move@1(glass, by=[0, 0.8, 0.1])
fade = anim.track@1(raised, property="glow", keys="0:0; 0.3:1", loop=false)
listen = behavior.on@1(fade, trigger="signal")
output lamp = listen
cap = solid.box@1(size=[0.15, 0.15, 0.05])
blue = solid.material@1(cap, color=[0.1, 0.3, 0.9], roughness=0.4)
spot = xform.move@1(blue, by=[0.7, 0.4, 0.1])
press = behavior.on@1(spot, trigger="select", target="lamp")
output switch = press
`);
  const ids = await lamp.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.map(p => [p.name, p.node])));
  const lampSettled = want => lamp.waitForFunction(([id, forward]) => {
    const p = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(q => q.node === id);
    return p && !p.running && p.forward === forward;
  }, [ids.lamp, want], { timeout: 20000 });
  const bright = png => { let sum = 0; for (let i = 0; i < png.data.length; i += 4) sum += 0.2126 * png.data[i] + 0.7152 * png.data[i + 1] + 0.0722 * png.data[i + 2]; return sum / (png.data.length / 4); };
  await lamp.waitForTimeout(800);
  const off = await shot(lamp, 'lamp-off');
  await lamp.evaluate(() => window.__runtime.viewer_play_mode(true));
  await lamp.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  await lamp.evaluate(id => window.__runtime.viewer_press(id), ids.switch);
  await lampSettled(true);
  await lamp.waitForTimeout(700);
  const on = await shot(lamp, 'lamp-on');
  console.log(`G10 lamp: mean brightness ${bright(off).toFixed(1)} off, ${bright(on).toFixed(1)} on`);
  assert(bright(on) > bright(off) + 1.5, 'switching the lamp on lights the picture');
  await lamp.evaluate(id => window.__runtime.viewer_press(id), ids.switch);
  await lampSettled(false);
  await lamp.waitForTimeout(700);
  const again = await shot(lamp, 'lamp-off-again');
  assert(Math.abs(bright(again) - bright(off)) < 0.5, 'switching it off darkens it again');
  await lamp.close();

  // A wave bobs without keyframes; a visible track hides the part once its key is reached.
  const motion = await open('motion', `ball = solid.box@1(size=[0.2, 0.2, 0.2])
lime = solid.material@1(ball, color=[0.1, 0.8, 0.2], roughness=0.6)
lifted = xform.move@1(lime, by=[0, 0.5, 0])
bob = anim.wave@1(lifted, property="position", amplitude=[0, 0.25, 0], period=2)
output bobber = bob
cube = solid.box@1(size=[0.2, 0.2, 0.2])
plain = solid.material@1(cube, color=[0.1, 0.8, 0.2], roughness=0.6)
side = xform.move@1(plain, by=[0.6, 0.3, 0])
gone = anim.track@1(side, property="visible", keys="0:1; 1:0", interpolation="step", loop=false)
output vanisher = gone
`);
  const top = centroid(await at(motion, 0.5, 'wave-peak'), green), bottom = centroid(await at(motion, 1.5, 'wave-trough'), green);
  assert(top && bottom, 'the bobbing block is visible');
  console.log(`G10 wave: the block bobs ${Math.abs(top.y - bottom.y).toFixed(0)} px between peak and trough`);
  assert(bottom.y - top.y > 30, 'a quarter period in the block is high, three quarters in it is low');
  const before = await at(motion, 0.5, 'visible-before'), after = await at(motion, 2, 'visible-after');
  const count = png => { let n = 0; for (let i = 0; i < png.data.length; i += 4) if (green(png.data[i], png.data[i + 1], png.data[i + 2])) n++; return n; };
  assert(count(before) > count(after) * 1.5 && count(after) > 20, `the second block disappears (${count(before)} green pixels, then ${count(after)} for the bobber alone)`);
  await motion.close();

  // A colour track fades the surface from red to blue.
  const paint = await open('colour', `cube = solid.box@1(size=[0.3, 0.3, 0.3])
red = solid.material@1(cube, color=[0.9, 0.1, 0.1], roughness=0.6)
lifted = xform.move@1(red, by=[0, 0.4, 0])
fade = anim.track@1(lifted, property="color", keys="0:0.9,0.1,0.1; 1:0.1,0.1,0.9", interpolation="linear", loop=false)
output swatch = fade
`);
  const redOnes = png => centroid(png, (r, g, b) => r > g + 80 && r > b + 80), blueOnes = png => centroid(png, (r, g, b) => b > r + 80 && b > g + 60);
  const first = await at(paint, 0, 'colour-start'), last = await at(paint, 1, 'colour-end');
  assert(redOnes(first) && !blueOnes(first), 'starts red');
  assert(blueOnes(last) && !redOnes(last), 'ends blue');
  await paint.close();
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
