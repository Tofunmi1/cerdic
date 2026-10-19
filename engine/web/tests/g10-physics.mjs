import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: physics (ADR 0035). A gold crate and a red ball start in the air above a static floor. In edit mode they hang
// where they were authored. In play mode they fall, the crate lands and rests on the floor, the ball bounces and comes
// to rest; a second play gives exactly the same resting poses; a crate picked up with the mouse hangs in the hand
// instead of falling and drops when let go; leaving play mode puts everything back.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5218'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `floor = solid.box@1(size=[8, 0.2, 8])
slab = xform.move@1(floor, by=[0, -0.1, 0])
ground_body = phys.body@1(slab, kind="static", shape="box")
output ground = ground_body
crate = solid.box@1(size=[0.4, 0.4, 0.4])
gold = solid.material@1(crate, color=[0.9, 0.7, 0.05], roughness=0.5)
high = xform.move@1(gold, by=[0, 1.2, 0])
crate_body = phys.body@1(high, shape="box", friction=0.8)
output crate = crate_body
ball = solid.sphere@1(radius=0.2, segments=16)
red = solid.material@1(ball, color=[0.9, 0.1, 0.1], roughness=0.4)
up = xform.move@1(red, by=[0.7, 2.2, 0])
ball_body = phys.body@1(up, shape="sphere", restitution=0.5)
output ball = ball_body
`;
const logs = [];
const gold = (r, g, b) => r > 150 && g > 100 && b < 90 && r > b + 100;
const where = png => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (gold(png.data[k], png.data[k + 1], png.data[k + 2])) { x += i; y += j; n++; }
  }
  assert(n > 60, `the crate is visible (${n} gold pixels)`);
  return { x: x / n, y: y / n };
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5218')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5218/?eval=1&docname=g10-physics');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'physics'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => w.code));
  assert(!warnings.some(code => code.startsWith('phys.')), `physics warnings: ${warnings}`);
  const bodies = await page.evaluate(() => window.__ve.evaluation.parts.map(p => [p.name, p.body ?? null, p.positions.length]));
  assert(bodies.every(([, body]) => body), `every part reached the page with its body: ${JSON.stringify(bodies)}`);
  const world = () => page.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const shot = async name => PNG.sync.read(await page.screenshot({ path: `${reports}physics-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
  const settle = async () => {
    try {
      await page.waitForFunction(() => {
        const w = JSON.parse(window.__runtime.viewer_physics());
        return w.active && w.steps > 90 && w.bodies.filter(b => b.kind !== 'Static').every(b => Math.hypot(...b.speed) < 0.005);
      }, null, { timeout: 60000, polling: 200 });
    } catch (error) {
      console.log(`not settled: ${JSON.stringify(await world())} logs=${JSON.stringify(logs).slice(0, 800)} behaviors=${await page.evaluate(() => window.__runtime.viewer_behaviors().slice(0, 200))}`);
      throw error;
    }
    await page.waitForTimeout(300);
    return world();
  };
  const pose = (state, node) => state.bodies.find(b => b.node === node);
  const nodeOf = name => page.evaluate(n => window.__ve.evaluation.parts.find(p => p.name === n).node, name);
  const crateNode = await nodeOf('crate'), ballNode = await nodeOf('ball');

  await page.waitForTimeout(800);
  assert.equal((await world()).active, false, 'no physics while editing');
  const start = where(await shot('start'));

  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  const first = await settle();
  assert.equal(first.bodies.length, 3, 'the floor, the crate and the ball are in the world');
  const crate = pose(first, crateNode), ball = pose(first, ballNode);
  assert(Math.abs(crate.position[1] - 0.2) < 0.02, `the crate rests on the floor: y=${crate.position[1]}`);
  assert(Math.abs(ball.position[1] - 0.2) < 0.02, `the ball rests on the floor: y=${ball.position[1]}`);
  const landed = where(await shot('landed'));
  console.log(`G10 physics: the crate fell ${(landed.y - start.y).toFixed(0)} px on screen and rests at y=${crate.position[1].toFixed(3)}`);
  assert(landed.y - start.y > 40, 'the crate is drawn lower once it has landed');

  // Pick the crate up: it hangs in the hand, then falls again when let go.
  await page.mouse.move(landed.x, landed.y); await page.waitForTimeout(500);
  await page.mouse.down(); await page.waitForTimeout(400);
  for (let i = 1; i <= 8; i++) { await page.mouse.move(landed.x, landed.y - i * 14, { steps: 2 }); await page.waitForTimeout(150); }
  await page.waitForTimeout(600);
  const held = pose(await world(), crateNode);
  assert(held.held, 'the crate is held');
  assert(held.position[1] > 0.45, `lifted off the floor and not falling: y=${held.position[1]}`);
  const lifted = where(await shot('held'));
  assert(landed.y - lifted.y > 25, 'it is drawn higher while held');
  await page.mouse.up();
  const second = await settle();
  const fallen = pose(second, crateNode);
  assert(!fallen.held && Math.abs(fallen.position[1] - 0.2) < 0.03, `let go, it falls back to the floor: y=${fallen.position[1]}`);

  // Stop: everything goes back.
  await page.evaluate(() => window.__runtime.viewer_play_mode(false));
  await page.waitForFunction(() => !JSON.parse(window.__runtime.viewer_physics()).active, null, { timeout: 20000 });
  await page.waitForTimeout(700);
  const home = where(await shot('home'));
  assert(Math.hypot(home.x - start.x, home.y - start.y) < 4, 'leaving play mode puts the crate back where it was authored');

  // A second play without touching anything: the same resting poses to the last bit.
  await page.evaluate(() => window.__runtime.viewer_play_mode(true));
  const again = await settle();
  // Where everything was at each simulated second (taken before the crate was picked up in the first play) must be
  // identical, to the last bit, however the frames fell.
  const shared = first.marks.filter(([steps]) => again.marks.some(([other]) => other === steps));
  assert(shared.length >= 1, `there are snapshots to compare (${first.marks.length} and ${again.marks.length})`);
  for (const [steps, positions] of shared) {
    assert.deepEqual(again.marks.find(([other]) => other === steps)[1], positions, `identical poses at step ${steps}`);
  }
  console.log(`G10 physics: ${shared.length} per-second snapshots identical across two plays`);
  await page.evaluate(() => window.__runtime.viewer_play_mode(false));
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
