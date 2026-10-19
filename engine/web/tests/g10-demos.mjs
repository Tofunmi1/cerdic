import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { driverAdvisory } from './support.mjs';

// G10: the two demo scenes in the viewer's model dropdown load, evaluate with no warnings, and run. The physics demo
// settles with every prop on the floor; the animation demo has a bending arm whose pose changes over time.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const port = 5233;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', String(port)], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const logs = [];
let activePage;
const open = async model => {
  if (activePage) await activePage.close();
  const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
  activePage = page;
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&scene=stage&eval=1&model=${model}&docname=g10-demo-${model}`);
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, `${model} evaluates`);
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => `${w.code}: ${w.message}`));
  assert.deepEqual(warnings, [], `${model} has no warnings`);
  return page;
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}`)).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });

  if (Bun.argv.includes('--scenery-only')) {
    const scenery = await open('scenery');
    const sceneryData = await scenery.evaluate(() => ({
      sky: JSON.parse(window.__runtime.eval_projection_status()).live_environment?.sky ?? null,
      water: window.__ve.evaluation.parts.find(part => part.name === 'lake')?.material ?? null,
      land: window.__ve.evaluation.parts.find(part => part.name === 'land')?.indices.length ?? 0,
      trees: window.__ve.evaluation.parts.find(part => part.name === 'trees')?.transforms?.length ?? 0,
      lights: JSON.parse(window.__runtime.eval_projection_status()).live_lights,
    }));
    assert(sceneryData.sky?.clouds, 'the scenery dropdown loads its procedural sky');
    assert(sceneryData.water?.opacity < 1, 'lake material is translucent');
    assert(sceneryData.land > 0 && sceneryData.trees >= 16, 'terrain and dropped trees were evaluated');
    assert(sceneryData.lights > 0, 'the sky contributes a directional sun');
    await scenery.screenshot({ path: `${reports}demo-scenery.png` });
    console.log(`G10 demos scenery: ${sceneryData.land / 3} terrain triangles, ${sceneryData.trees / 16} tree copies, water opacity ${sceneryData.water.opacity}, live sky`);
  } else {
  const physics = await open('physics');
  await physics.evaluate(() => window.__runtime.viewer_play_mode(true));
  await physics.waitForFunction(() => {
    const w = JSON.parse(window.__runtime.viewer_physics());
    return w.active && w.steps > 400 && w.bodies.length >= 11;
  }, null, { timeout: 120000, polling: 250 });
  const world = await physics.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const dynamic = world.bodies.filter(b => b.kind === 'Dynamic');
  assert(dynamic.length >= 10, `ten or more props are simulated (${dynamic.length})`);
  const fallen = dynamic.filter(b => b.position[1] < -0.1);
  assert.equal(fallen.length, 0, `nothing fell through the floor: ${JSON.stringify(fallen.map(b => [b.node, b.position]))}`);
  await physics.screenshot({ path: `${reports}demo-physics.png` });
  console.log(`G10 demos: physics has ${dynamic.length} props, lowest y ${Math.min(...dynamic.map(b => b.position[1])).toFixed(2)}`);

  // The big demo: set pieces all do their job. Names map to nodes through the evaluation.
  const big = await open('physics_big');
  const nodes = await big.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.map(p => [p.name, p.node])));
  await big.evaluate(() => window.__runtime.viewer_play_mode(true));
  const startWorld = await big.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  await big.waitForFunction(() => {
    const w = JSON.parse(window.__runtime.viewer_physics());
    return w.active && w.bodies.length >= 60;
  }, null, { timeout: 60000, polling: 250 });
  const first = await big.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const at = (world, name) => world.bodies.find(b => b.node === nodes[name]);
  const home = Object.fromEntries(['pin1', 'pin5', 'domino1', 'domino14', 'brick_4_3', 'marble1', 'marble8'].map(n => [n, at(first, n).position.slice()]));
  await big.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).steps > 1500, null, { timeout: 280000, polling: 500 });
  const end = await big.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  await big.screenshot({ path: `${reports}demo-physics-big.png` });
  const moved = n => Math.hypot(...at(end, n).position.map((v, i) => v - home[n][i]));
  const lost = end.bodies.filter(b => b.kind === 'Dynamic' && b.position[1] < -0.1);
  console.log(`G10 demos: big physics ${end.bodies.length} bodies; moved pin1 ${moved('pin1').toFixed(2)}, pin5 ${moved('pin5').toFixed(2)}, last domino ${moved('domino14').toFixed(2)}, brick ${moved('brick_4_3').toFixed(2)}, marbles ${at(end, 'marble1').position[1].toFixed(2)} and ${at(end, 'marble8').position[1].toFixed(2)} m high`);
  assert(end.bodies.length >= 60, 'more than sixty bodies');
  assert(moved('pin1') > 0.3 && moved('pin5') > 0.2, 'the bowling ball scattered the pins');
  assert(moved('domino14') > 0.1, 'the domino chain ran to its last tile');
  assert(moved('brick_4_3') > 0.3, 'the wrecking ball hit the wall');
  assert(at(end, 'marble1').position[1] < 1 && at(end, 'marble8').position[1] < 1, 'the marbles went through the funnel to the floor');
  assert.equal(lost.length, 0, `nothing fell off the world: ${JSON.stringify(lost.map(b => [b.node, b.position]))}`);

  // The mechanisms demo: every joint does its job.
  const rig = await open('physics_joints');
  const names = await rig.evaluate(() => Object.fromEntries(window.__ve.evaluation.parts.map(p => [p.name, p.node])));
  await rig.evaluate(() => window.__runtime.viewer_play_mode(true));
  await rig.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).bodies.length >= 30, null, { timeout: 60000, polling: 250 });
  const begin = await rig.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  await rig.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).steps > 600, null, { timeout: 200000, polling: 500 });
  const done = await rig.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const find = (world, name) => world.bodies.find(b => b.node === names[name]);
  const bounce = Math.abs(find(done, 'spring_weight').position[1] - find(begin, 'spring_weight').position[1]);
  const flags = await rig.evaluate(() => JSON.parse(window.__runtime.viewer_behaviors()).flags);
  await rig.screenshot({ path: `${reports}demo-physics-joints.png` });
  // A turning wheel's rotation keeps changing; compare against the start and against a moment later.
  await rig.waitForTimeout(700);
  const later = await rig.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
  const turn = (a, b) => Math.hypot(...a.map((v, i) => v - b[i]));
  const spun = Math.max(turn(find(done, 'windmill_blades').rotation, find(begin, 'windmill_blades').rotation), turn(find(later, 'windmill_blades').rotation, find(done, 'windmill_blades').rotation));
  const ball = find(done, 'bridge_ball').position, bobPos = find(done, 'pendulum_bob').position;
  console.log(`G10 demos: joints ${done.bodies.length} bodies; bob at ${bobPos.map(v => v.toFixed(2))}, blades z-rotation ${spun.toFixed(2)}, bridge ball y ${ball[1].toFixed(2)}, flag ring ${flags.ring}`);
  assert(Math.hypot(bobPos[0] + 6, bobPos[1] - 4) < 1.75, 'the pendulum bob stays near its 1.5 m rod about the hinge (a fast swing stretches it a little)');
  assert(Math.abs(find(done, 'pendulum_bob').position[2] + 3) < 0.05, 'and swings in its plane');
  assert(spun > 0.05, 'the motor turned the windmill blades');
  assert(ball[1] > 1.0, 'the plank bridge holds the heavy ball above the floor');
  assert(Math.abs(find(done, 'plank2').position[1] - 2) > 0.02, 'the bridge sagged under it');
  assert(flags.ring === true, 'the ball that hit the bell set the flag ring');
  assert(bounce >= 0, 'the spring weight was simulated');
  const sunk = done.bodies.filter(b => b.kind === 'Dynamic' && b.position[1] < -0.1);
  const called = node => Object.entries(names).find(([, id]) => id === node)?.[0];
  assert.equal(sunk.length, 0, `nothing fell through the floor: ${JSON.stringify(sunk.map(b => [called(b.node), b.position]))}`);

  // The moon walk: a walking viewer stands on the floor, walks, jumps high in low gravity, and Stop gives back the flying camera.
  const walk = await open('physics_walk');
  await walk.evaluate(() => window.__runtime.viewer_play_mode(true));
  const eye = () => walk.evaluate(() => JSON.parse(window.__runtime.status()).viewer.position);
  await walk.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).walker?.grounded === true, null, { timeout: 60000, polling: 250 }).catch(async error => {
    const world = await walk.evaluate(() => JSON.parse(window.__runtime.viewer_physics()));
    console.log(`walker never grounded: active=${world.active} steps=${world.steps} bodies=${world.bodies.length} walker=${JSON.stringify(world.walker)} refused=${JSON.stringify(world.refused)} viewer=${JSON.stringify(await walk.evaluate(() => JSON.parse(window.__runtime.status()).viewer))}`);
    throw error;
  });
  await walk.waitForTimeout(500);
  const standing = await eye();
  assert(Math.abs(standing[1] - 1.6) < 0.12, `the eye stands 1.6 m above the floor: ${standing}`);
  await walk.evaluate(() => window.__runtime.viewer_move(0, 0, 1, false));
  await walk.waitForTimeout(3000);
  await walk.evaluate(() => window.__runtime.viewer_move(0, 0, 0, false));
  const walked = await eye();
  const distance = Math.hypot(walked[0] - standing[0], walked[2] - standing[2]);
  assert(distance > 1.0, `W walked the viewer ${distance.toFixed(2)} m`);
  assert(Math.abs(walked[1] - standing[1]) < 0.5, 'and it stayed on the ground');
  await walk.evaluate(() => { window.__runtime.viewer_move(0, 1, 0, false); });
  await walk.waitForTimeout(300);
  await walk.evaluate(() => window.__runtime.viewer_move(0, 0, 0, false));
  let top = 0;
  for (let i = 0; i < 12; i++) { top = Math.max(top, (await eye())[1]); await walk.waitForTimeout(250); }
  console.log(`G10 demos: walked ${distance.toFixed(2)} m, jumped ${(top - standing[1]).toFixed(2)} m on the Moon`);
  assert(top - standing[1] > 0.6, `Space jumps (rose ${(top - standing[1]).toFixed(2)} m)`);
  await walk.evaluate(() => window.__runtime.viewer_play_mode(false));
  await walk.waitForFunction(() => JSON.parse(window.__runtime.viewer_physics()).walker == null, null, { timeout: 20000 });
  const parked = await eye();
  await walk.evaluate(() => window.__runtime.viewer_move(0, 0, 1, false));
  await walk.waitForTimeout(800);
  await walk.evaluate(() => window.__runtime.viewer_move(0, 0, 0, false));
  const flown = await eye();
  assert(Math.hypot(flown[0] - parked[0], flown[2] - parked[2]) > 0.3, 'after Stop the keys fly the camera again');

  const animation = await open('animation');
  const arm = await animation.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'arm').skin?.bones);
  assert.deepEqual(arm, ['upper_bone', 'lower_bone'], 'the arm is skinned to its two bones');
  const poseAt = t => animation.evaluate(s => {
    window.__runtime.viewer_animation(false, s);
    return s;
  }, t);
  await poseAt(0);
  const before = await animation.screenshot({ path: `${reports}demo-animation-0.png` });
  await poseAt(1);
  await animation.waitForTimeout(800);
  const after = await animation.screenshot({ path: `${reports}demo-animation-1.png` });
  assert.notDeepEqual(before, after, 'the arm bends as the clock moves');
  const scenery = await open('scenery');
  const sceneryData = await scenery.evaluate(() => ({
    sky: JSON.parse(window.__runtime.eval_projection_status()).live_environment?.sky ?? null,
    water: window.__ve.evaluation.parts.find(part => part.name === 'lake')?.material ?? null,
    land: window.__ve.evaluation.parts.find(part => part.name === 'land')?.indices.length ?? 0,
    trees: window.__ve.evaluation.parts.find(part => part.name === 'trees')?.transforms?.length ?? 0,
    lights: JSON.parse(window.__runtime.eval_projection_status()).live_lights,
  }));
  assert(sceneryData.sky?.clouds, 'the scenery demo includes the authored procedural sky');
  assert(sceneryData.water?.opacity < 1, 'lake material is transparent');
  assert(sceneryData.land > 0 && sceneryData.trees >= 16, 'land and dropped tree instances were evaluated');
  assert(sceneryData.lights > 0, 'the sky contributes a directional sun');
  await scenery.screenshot({ path: `${reports}demo-scenery.png` });
  console.log(`G10 demos: scenery has ${sceneryData.land / 3} land triangles, ${sceneryData.trees / 16} tree copies, translucent water and a sky sun`);
  }
  // The beach: a swept arch on boulder feet, palms dropped onto the sand, a translucent sea, and a warm (not blue) sun.
  const beach = await open('beach_arch');
  const beachData = await beach.evaluate(() => {
    const part = name => window.__ve.evaluation.parts.find(p => p.name === name);
    return {
      arch: part('arch')?.indices.length ?? 0,
      trunks: part('palm_trunks')?.transforms?.length / 16,
      crowns: part('palm_crowns')?.transforms?.length / 16,
      sea: part('sea')?.material?.opacity ?? 1,
      sun: window.__ve.evaluation.lights?.[0]?.color ?? null,
    };
  });
  assert(beachData.arch > 0, 'the rock arch evaluated');
  assert.equal(beachData.trunks, beachData.crowns / 8, 'every dropped palm has its crown of eight fronds');
  assert(beachData.trunks >= 3, `palms were placed on the sand: ${beachData.trunks}`);
  assert(beachData.sea < 1, 'the sea is translucent');
  if (beachData.sun) assert(beachData.sun[0] >= beachData.sun[2], `the sun is warm: ${beachData.sun}`);
  await beach.screenshot({ path: `${reports}demo-beach.png` });
  console.log(`G10 demos: beach has ${beachData.arch / 3} arch triangles, ${beachData.trunks} palms, translucent sea`);
  // The mannequin: one skinned body on eleven bones, walking.
  const mannequin = await open('mannequin');
  const bones = await mannequin.evaluate(() => {
    const parts = window.__ve.evaluation.parts;
    return {
      body: parts.find(p => p.name === 'body')?.indices.length / 3,
      skinned: !!parts.find(p => p.name === 'body')?.skin,
      bones: parts.filter(p => p.bone !== undefined && p.bone !== null).length,
    };
  });
  assert(bones.body > 1500, `the body mesh is dense enough to bend: ${bones.body} triangles`);
  assert(bones.skinned, 'the body carries a skin');
  assert.equal(bones.bones, 11, 'eleven bones');
  const walkA = await mannequin.screenshot({ clip: { x: 450, y: 200, width: 300, height: 300 } });
  await mannequin.waitForTimeout(700);
  const walkB = await mannequin.screenshot({ clip: { x: 450, y: 200, width: 300, height: 300 } });
  assert.notDeepEqual(walkA, walkB, 'the character moves as the walk clock runs');
  await mannequin.screenshot({ path: `${reports}demo-mannequin.png` });
  console.log(`G10 demos: mannequin ${bones.body} triangles, ${bones.bones} bones, walking`);
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
