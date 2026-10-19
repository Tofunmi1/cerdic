import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { driverAdvisory } from './support.mjs';

// G10: scene logic (ADR 0029). Behaviours set named flags, need flags before they act, and parts follow flags.
// A locked door ignores clicks until a key is picked up; a lamp follows its switch; a gate part (no clip at all)
// sets a flag only when one flag changes while another is on. State is read from the viewer, not from pixels.
const root = new URL('../../../', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5214'], {
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
  await page.goto(`http://127.0.0.1:5214/?eval=1&docname=g10-logic-${name}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'logic'), program);
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
const flag = (page, name, on) => page.evaluate(([n, v]) => window.__runtime.viewer_flag(n, v), [name, on]);
const partOf = async (page, id) => (await state(page)).parts.find(p => p.node === id);
const settled = (page, id, forward) => page.waitForFunction(([node, want]) => {
  const p = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(q => q.node === node);
  return p && !p.running && p.forward === want;
}, [id, forward], { timeout: 20000 });
const steps = async (page, n) => { const from = (await state(page)).steps; await page.waitForFunction(([s, k]) => JSON.parse(window.__runtime.viewer_behaviors()).steps >= s + k, [from, n], { timeout: 20000 }); };
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5214')).ok) break; } catch {}
    await Bun.sleep(100);
  }

  // A locked door and its key.
  const lock = await open('lock', `door = solid.box@1(size=[0.9, 2, 0.05])
swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 1:0,90,0", interpolation="linear", pivot=[-0.45, 0, 0], loop=false)
latch = behavior.on@1(swing, trigger="select", action="toggle", needs="has_key")
output front_door = latch
key = solid.box@1(size=[0.1, 0.05, 0.02])
placed = xform.move@1(key, by=[0.6, 0.5, 0.1])
vanish = anim.track@1(placed, property="visible", keys="0:1; 0.2:0", interpolation="step", loop=false)
take = behavior.on@1(vanish, trigger="select", action="play", sets="has_key")
output brass_key = take
`);
  assert.deepEqual(await lock.page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'front_door').behavior.needs), [{ name: 'has_key', value: true }]);
  await press(lock.page, lock.ids.front_door);
  await steps(lock.page, 30);
  assert.equal((await partOf(lock.page, lock.ids.front_door)).time, 0, 'the locked door does not open without the key');
  assert.deepEqual((await state(lock.page)).flags, {}, 'no flags yet');
  await press(lock.page, lock.ids.brass_key);
  await settled(lock.page, lock.ids.brass_key, true);
  assert.equal((await state(lock.page)).flags.has_key, true, 'picking up the key sets the flag');
  await press(lock.page, lock.ids.front_door);
  await settled(lock.page, lock.ids.front_door, true);
  assert((await partOf(lock.page, lock.ids.front_door)).time > 0.99, 'with the key the door opens');
  await lock.page.evaluate(() => window.__runtime.viewer_play_mode(false));
  await lock.page.waitForFunction(() => !JSON.parse(window.__runtime.viewer_behaviors()).play);
  assert.deepEqual((await state(lock.page)).flags, {}, 'leaving play mode clears the flags');
  await lock.page.close();

  // A lamp that always matches its switch, however often it is flipped.
  const lamp = await open('lamp', `bulb = solid.sphere@1(radius=0.1)
glass = solid.material@1(bulb, color=[0.05, 0.05, 0.05], emissive=[2, 1.7, 1.1], emits_light=true)
fade = anim.track@1(glass, property="glow", keys="0:0; 0.3:1", loop=false)
mirror = behavior.on@1(fade, trigger="flag", watch="powered", action="follow")
output lamp = mirror
box = solid.box@1(size=[0.1, 0.1, 0.05])
flip = behavior.on@1(box, trigger="select", sets="powered=toggle")
output switch = flip
`);
  await press(lamp.page, lamp.ids.switch);
  await settled(lamp.page, lamp.ids.lamp, true);
  assert.equal((await state(lamp.page)).flags.powered, true);
  assert((await partOf(lamp.page, lamp.ids.lamp)).time > 0.29, 'the lamp is fully on');
  await press(lamp.page, lamp.ids.switch);
  await settled(lamp.page, lamp.ids.lamp, false);
  assert.equal((await partOf(lamp.page, lamp.ids.lamp)).time, 0, 'and off again');
  // Flipping twice quickly never leaves it out of step with the flag.
  await press(lamp.page, lamp.ids.switch);
  await steps(lamp.page, 3);
  await press(lamp.page, lamp.ids.switch);
  await steps(lamp.page, 3);
  await press(lamp.page, lamp.ids.switch);
  await steps(lamp.page, 40);
  assert.equal((await state(lamp.page)).flags.powered, true);
  assert((await partOf(lamp.page, lamp.ids.lamp)).forward, 'the lamp heads for on, as the flag says');
  await lamp.page.close();

  // A gate with no clip: when `a` changes while `b` is on it sets `c`.
  const gate = await open('gate', `a1 = solid.box@1(size=[0.1, 0.1, 0.1])
and_gate = behavior.on@1(a1, trigger="flag", watch="a", needs="b", sets="c=toggle")
output gate_out = and_gate
`);
  await flag(gate.page, 'a', true);
  await steps(gate.page, 5);
  assert.equal((await state(gate.page)).flags.c, undefined, 'b is off, so the gate does nothing');
  await flag(gate.page, 'b', true);
  await flag(gate.page, 'a', false);
  await steps(gate.page, 5);
  assert.equal((await state(gate.page)).flags.c, true, 'a changed while b was on');
  await flag(gate.page, 'a', true);
  await steps(gate.page, 5);
  assert.equal((await state(gate.page)).flags.c, false, 'and it toggles on the next change');
  await gate.page.close();

  // Timers: a door that opens by itself two seconds into play, and a blinker that flips a flag every half second.
  const timers = await open('timers', `door = solid.box@1(size=[0.9, 2, 0.05])
swing = anim.track@1(door, property="rotation", keys="0:0,0,0; 0.5:0,90,0", interpolation="linear", pivot=[-0.45, 0, 0], loop=false)
auto = behavior.on@1(swing, trigger="timer", action="play", after=2)
output auto_door = auto
tick = solid.box@1(size=[0.1, 0.1, 0.1])
blink = behavior.on@1(tick, trigger="timer", every=0.5, sets="lit=toggle")
output blinker = blink
`);
  assert.deepEqual(await timers.page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'auto_door').behavior.after), 2);
  await timers.page.evaluate(() => { window.__seen = new Set(); });
  const watchFlag = timers.page.evaluate(async () => {
    for (let i = 0; i < 90; i++) { window.__seen.add(JSON.stringify(JSON.parse(window.__runtime.viewer_behaviors()).flags.lit ?? null)); await new Promise(r => setTimeout(r, 40)); }
  });
  const doorBefore = (await partOf(timers.page, timers.ids.auto_door)).time;
  await settled(timers.page, timers.ids.auto_door, true, 'timer door');
  await watchFlag;
  assert.equal(doorBefore, 0, 'the door waits for its timer');
  assert((await partOf(timers.page, timers.ids.auto_door)).time > 0.49, 'the timer opened the door with nobody touching it');
  const seenStates = await timers.page.evaluate(() => [...window.__seen]);
  assert(seenStates.includes('true') && seenStates.includes('false'), `the repeating timer flipped its flag both ways: ${seenStates}`);
  await timers.page.close();

  console.log('G10 logic: a locked door needs its key, a lamp follows its switch, a clipless gate reacts to flag changes, timers fire on their own');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
