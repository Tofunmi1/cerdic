import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: `behavior.on` (ADR 0027). A door hinged at its left edge opens when clicked in play mode and closes on
// the next click. In edit mode it rests and a click only selects. Leaving play mode puts it back at rest. A second
// door with a `near` trigger opens by itself when the viewer is within its radius.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5212'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const door = trigger => `door = solid.box@1(size=[0.9, 2, 0.05])
paint = solid.material@1(door, color=[0.75, 0.35, 0.15], roughness=0.6)
swing = anim.track@1(paint, property="rotation", keys="0:0,0,0; 1:0,90,0", interpolation="linear", pivot=[-0.45, 0, 0], loop=true)
react = behavior.on@1(swing, ${trigger})
output hinged_door = react
`;
const logs = [];
const open = async (name, program) => {
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto(`http://127.0.0.1:5212/?viewer=1&eval=1&docname=g10-behavior-${name}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'behavior'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 300)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  return page;
};
const state = page => page.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()));
const part = async page => (await state(page)).parts[0];
const settled = async (page, want, what) => {
  try {
    await page.waitForFunction(
      ([forward]) => { const p = JSON.parse(window.__runtime.viewer_behaviors()).parts[0]; return p && !p.running && p.forward === forward; },
      [want], { timeout: 20000 });
  } catch (error) {
    const events = await page.evaluate(() => window.__ve.read().interact?.events?.slice(-6));
    throw new Error(`${what}: never settled ${want ? 'open' : 'closed'}; state ${JSON.stringify(await state(page))}; events ${JSON.stringify(events)}`, { cause: error });
  }
};
// The interaction machine runs once per frame, so the pointer needs a frame or more between each step.
const click = async (page, x, y) => {
  await page.mouse.move(x, y); await page.waitForTimeout(700);
  await page.mouse.down(); await page.waitForTimeout(400);
  await page.mouse.up(); await page.waitForTimeout(700);
};
// The picture above the Play button, whose label changes with the mode.
const shot = async (page, name) => PNG.sync.read(await page.screenshot({ path: `${reports}behavior-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 560 } }));
const differing = (a, b) => {
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if (Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2])) > 24) n++;
  }
  return n / (a.width * a.height);
};
const doorCentre = png => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4, [r, g, b] = [png.data[k], png.data[k + 1], png.data[k + 2]];
    if (r > g + 40 && r > b + 80) { x += i; y += j; n++; }
  }
  assert(n > 200, `the door is visible (${n} pixels)`);
  return [Math.round(x / n), Math.round(y / n)];
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5212')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });

  const page = await open('click', door('trigger="select", action="toggle"'));
  assert.deepEqual(await page.evaluate(() => window.__ve.evaluation.parts[0].behavior), { trigger: 'select', target: null, radius: 2, action: 'toggle', sets: [], needs: [], watch: null, after: 0, every: 0, accepts: null, clip: '', target_clip: '', then: [] });
  let now = await state(page);
  assert.equal(now.play, false);
  assert.equal(now.parts.length, 1, 'the viewer knows one reactive part');
  // Edit mode: it rests (it does not follow the shared clock) and a click only selects.
  await page.waitForTimeout(1500);
  const rest = await shot(page, 'rest');
  assert.equal((await part(page)).time, 0, 'at rest in edit mode');
  const [cx, cy] = doorCentre(rest);
  await click(page, cx, cy);
  assert.deepEqual(await part(page), { node: now.parts[0].node, time: 0, forward: false, running: false, clips: {} }, 'a click in edit mode does not trigger it');
  const selected = await page.evaluate(() => JSON.parse(window.__runtime.selection_get()));
  assert.equal(selected.node, now.parts[0].node, `the click selected the door: ${JSON.stringify(selected)} at ${cx},${cy}`);

  // Play mode: a pointer click opens it, held open at the end of the clip.
  await page.waitForSelector('#play-mode:not([hidden])', { timeout: 10000 });
  assert.equal(await page.textContent('#play-mode'), 'Play');
  await page.click('#play-mode');
  await page.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  assert.equal(await page.textContent('#play-mode'), 'Stop');
  assert.equal(await page.evaluate(() => JSON.parse(window.__runtime.selection_get()).node), null, 'entering play mode clears the selection');
  await click(page, cx, cy);
  assert.equal(await page.evaluate(() => JSON.parse(window.__runtime.selection_get()).node), null, 'a click in play mode does not select');
  await settled(page, true, 'pointer click');
  assert(Math.abs((await part(page)).time - 1) < 1e-4, 'opened to the end of the clip');
  const opened = await shot(page, 'open');
  assert(differing(rest, opened) > 0.02, 'the door is visibly open');
  // Another click closes it again: the picture is the resting one.
  await page.evaluate(() => window.__runtime.viewer_press(JSON.parse(window.__runtime.viewer_behaviors()).parts[0].node));
  await settled(page, false, 'press to close');
  assert.equal((await part(page)).time, 0, 'closed again');
  assert(differing(rest, await shot(page, 'closed')) < 0.0005, 'closed looks like at rest');
  // Leaving play mode while open resets it.
  await page.evaluate(() => window.__runtime.viewer_press(JSON.parse(window.__runtime.viewer_behaviors()).parts[0].node));
  await settled(page, true, 'press to open');
  await page.evaluate(() => window.__runtime.viewer_play_mode(false));
  await page.waitForTimeout(500);
  assert.deepEqual((await part(page)).time, 0, 'leaving play mode puts it back at rest');
  assert(differing(rest, await shot(page, 'reset')) < 0.0005, 'reset looks like at rest');
  const stepsA = (await state(page)).steps;
  await page.waitForTimeout(500);
  assert.equal((await state(page)).steps, stepsA, 'no steps run outside play mode');
  await page.close();

  // A near trigger: with the viewer inside the radius the door opens by itself in play mode.
  const near = await open('near', door('trigger="near", action="toggle", radius=100'));
  await near.waitForTimeout(500);
  assert.equal((await part(near)).time, 0, 'rests until play mode');
  await near.evaluate(() => window.__runtime.viewer_play_mode(true));
  await settled(near, true, 'near trigger');
  assert(Math.abs((await part(near)).time - 1) < 1e-4, 'opens on its own when the viewer is near');
  await near.close();

  const far = await open('far', door('trigger="near", action="toggle", radius=0.1'));
  await far.evaluate(() => window.__runtime.viewer_play_mode(true));
  await far.waitForTimeout(1500);
  assert.equal((await part(far)).time, 0, 'stays shut when the viewer is outside the radius');
  await far.close();

  // A switch with a target: clicking the blue button runs the door's clip, and the button itself has none.
  const wired = await open('switch', `door = solid.box@1(size=[0.9, 2, 0.05])
paint = solid.material@1(door, color=[0.75, 0.35, 0.15], roughness=0.6)
swing = anim.track@1(paint, property="rotation", keys="0:0,0,0; 1:0,90,0", interpolation="linear", pivot=[-0.45, 0, 0], loop=false)
listen = behavior.on@1(swing, trigger="signal")
output front_door = listen
cap = solid.box@1(size=[0.25, 0.25, 0.05])
blue = solid.material@1(cap, color=[0.1, 0.3, 0.9], roughness=0.4)
placed = xform.move@1(blue, by=[0.2, 0.6, 0.15])
press = behavior.on@1(placed, trigger="select", target="front_door")
output door_button = press
`);
  const ids = await wired.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.map(p => [p.name, p.node])));
  const doorState = async () => (await state(wired)).parts.find(p => p.node === ids.front_door);
  assert.equal((await state(wired)).parts.length, 2, 'both the sender and the listener are reactive');
  const resting = await shot(wired, 'switch-rest');
  const blueSpot = (() => {
    let x = 0, y = 0, n = 0;
    for (let j = 0; j < resting.height; j++) for (let i = 0; i < resting.width; i++) {
      const k = (j * resting.width + i) * 4;
      if (resting.data[k + 2] > resting.data[k] + 100 && resting.data[k + 2] > 150) { x += i; y += j; n++; }
    }
    assert(n > 30, `the button is visible (${n} pixels)`);
    return [Math.round(x / n), Math.round(y / n)];
  })();
  await wired.waitForSelector('#play-mode:not([hidden])');
  await wired.click('#play-mode');
  await wired.waitForFunction(() => JSON.parse(window.__runtime.viewer_behaviors()).play);
  await click(wired, ...blueSpot);
  await wired.waitForFunction(id => { const p = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(q => q.node === id); return p && !p.running && p.forward; }, ids.front_door, { timeout: 20000 });
  assert(Math.abs((await doorState()).time - 1) < 1e-4, 'the button opened the door');
  assert(differing(resting, await shot(wired, 'switch-open')) > 0.02, 'the door is visibly open');
  await click(wired, ...blueSpot);
  await wired.waitForFunction(id => { const p = JSON.parse(window.__runtime.viewer_behaviors()).parts.find(q => q.node === id); return p && !p.running && !p.forward; }, ids.front_door, { timeout: 20000 });
  assert.equal((await doorState()).time, 0, 'the second click closed it');
  await wired.close();
  console.log('G10 behavior: click toggles in play mode, edit mode rests, leaving play resets, near trigger opens and far stays shut, a button opens a door');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
