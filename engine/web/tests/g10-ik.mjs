import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory } from './support.mjs';

// G10: inverse kinematics (ADR 0032). An orange two-bone arm (0.5 m bones, shoulder at the blue marker) follows a red
// ball that moves along a path. While the ball is in reach the end of the arm sits on it; when it moves out of reach the
// arm stretches straight toward it. Pixels are measured, not the solver's own numbers.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5220'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `upper = solid.box@1(size=[0.5, 0.08, 0.08])
upper_at = xform.move@1(upper, by=[0.25, 0, 0])
upper_paint = solid.material@1(upper_at, color=[0.9, 0.4, 0.1], roughness=0.6)
upper_bone = rig.bone@1(upper_paint, head=[0, 0, 0], tail=[0.5, 0, 0])
output upper_arm = upper_bone
fore = solid.box@1(size=[0.5, 0.08, 0.08])
fore_at = xform.move@1(fore, by=[0.75, 0, 0])
fore_paint = solid.material@1(fore_at, color=[0.9, 0.4, 0.1], roughness=0.6)
fore_bone = rig.bone@1(fore_paint, head=[0.5, 0, 0], tail=[1, 0, 0])
fore_child = anim.parent@1(fore_bone, parent="upper_arm")
output forearm = fore_child
shoulder = solid.sphere@1(radius=0.05)
shoulder_paint = solid.material@1(shoulder, color=[0.1, 0.2, 0.9], roughness=0.5)
output shoulder_marker = shoulder_paint
ball = solid.sphere@1(radius=0.06)
ball_paint = solid.material@1(ball, color=[0.9, 0.05, 0.05], roughness=0.5)
ball_at = xform.move@1(ball_paint, by=[0.8, 0, 0])
path = anim.track@1(ball_at, property="position", keys="0:0,0,0; 1:-0.5,0.6,0; 2:0.2,0.9,0", interpolation="linear", loop=false)
reach = rig.ik@1(path, bones="upper_arm; forearm", iterations=48)
output target = reach
`;
const logs = [];
const orange = (r, g, b) => r > 150 && g > 70 && g < 160 && b < 80 && r > g + 60;
const red = (r, g, b) => r > 170 && g < 60 && b < 60;
const blue = (r, g, b) => b > r + 100 && b > g + 50;
const centroid = (png, match, what) => {
  let x = 0, y = 0, n = 0;
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (match(png.data[k], png.data[k + 1], png.data[k + 2])) { x += i; y += j; n++; }
  }
  assert(n > 25, `the ${what} is visible (${n} pixels)`);
  return { x: x / n, y: y / n };
};
const farthest = (png, match, from) => {
  let best = { d: -1, x: 0, y: 0 };
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (!match(png.data[k], png.data[k + 1], png.data[k + 2])) continue;
    const d = Math.hypot(i - from.x, j - from.y);
    if (d > best.d) best = { d, x: i, y: j };
  }
  return best;
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5220')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5220/?eval=1&docname=g10-ik');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'ik'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const chain = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'target').ik);
  assert.deepEqual(chain?.bones, ['upper_arm', 'forearm'], 'the page has the chain');
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => w.code));
  assert(!warnings.some(code => code.startsWith('rig.')), `rig warnings: ${warnings}`);

  const at = async (seconds, name) => {
    await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
    await page.waitForTimeout(900);
    const png = PNG.sync.read(await page.screenshot({ path: `${reports}ik-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } }));
    const shoulder = centroid(png, blue, 'shoulder marker'), ball = centroid(png, red, 'target ball');
    const tip = farthest(png, orange, shoulder);
    return { shoulder, ball, tip, png };
  };
  const angle = (from, to) => Math.atan2(to.y - from.y, to.x - from.x) * 180 / Math.PI;
  const apart = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

  // In reach, at the start (target 0.8 m out, arm folds a little) and at 1 s (target up and back): the arm tip is on the ball.
  for (const [seconds, name] of [[0, 'start'], [1, 'raised']]) {
    const r = await at(seconds, name);
    console.log(`G10 ik at ${seconds}s: tip ${apart(r.tip, r.ball).toFixed(0)} px from the ball`);
    assert(apart(r.tip, r.ball) < 40, `at ${seconds}s the end of the arm reaches the ball (${apart(r.tip, r.ball).toFixed(0)} px)`);
  }
  const rest = await at(0, 'start-again');
  const raised = await at(1, 'raised-again');
  assert(Math.abs(angle(rest.shoulder, rest.tip) - angle(raised.shoulder, raised.tip)) > 20, 'the arm swings as the target moves');

  // Out of reach (2 s: the target is 1.35 m away, the arm is 1 m): straight, pointing at the ball.
  const far = await at(2, 'far');
  const toBall = angle(far.shoulder, far.ball), toTip = angle(far.shoulder, far.tip);
  console.log(`G10 ik out of reach: arm points ${toTip.toFixed(1)} deg, ball is at ${toBall.toFixed(1)} deg`);
  assert(Math.abs(toBall - toTip) < 12, 'out of reach the arm points at the target');
  assert(apart(far.shoulder, far.tip) > 0.8 * 1.0 * (apart(rest.shoulder, rest.tip) / 0.8), 'and is stretched out straight');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
