import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import { PNG } from 'pngjs';
import validator from 'gltf-validator';
import { driverAdvisory } from './support.mjs';

// G10: skinning (ADR 0030). A tall lime tube is skinned to two bone parts at its middle. The lower bone swings 90
// degrees about the joint; the tube bends there, stays in one piece, and the top stays put. The exported glb carries
// a valid skin whose joints are the bone parts.
const root = new URL('../../../', import.meta.url).pathname;
const reports = new URL('../../target/reports/g10/', import.meta.url).pathname;
const child = Bun.spawn(['node', 'engine/tools/serve-lan.mjs', '--http', '--host', '127.0.0.1', '--port', '5216'], {
  cwd: root, stdout: 'ignore', stderr: 'inherit',
});
const browser = await chromium.launch({
  channel: process.env.PLAYWRIGHT_CHANNEL || undefined,
  args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
});
const program = `lower = solid.box@1(size=[0.05, 0.05, 0.05])
lower_head = xform.move@1(lower, by=[0, -0.25, 0])
lower_bone_part = rig.bone@1(lower_head, head=[0, -0.5, 0], tail=[0, 0, 0])
bend = anim.track@1(lower_bone_part, property="rotation", keys="0:0,0,0; 1:0,0,90", interpolation="linear", pivot=[0, 0, 0], loop=false)
output lower_bone = bend
upper = solid.box@1(size=[0.05, 0.05, 0.05])
upper_head = xform.move@1(upper, by=[0, 0.25, 0])
upper_bone_part = rig.bone@1(upper_head, head=[0, 0, 0], tail=[0, 0.5, 0])
output upper_bone = upper_bone_part
tall = solid.box@1(size=[0.2, 1, 0.2])
cage = poly.from_solid@1(tall)
flat = poly.merge_coplanar@1(cage)
uprights = poly.select@1(flat, element="edge", by="box", box_min=[-1, -0.1, -1], box_max=[1, 0.1, 1])
cut = poly.subdivide_edges@1(flat, uprights, cuts=7)
skinned = rig.skin@1(cut, bones="lower_bone; upper_bone")
lime = solid.material@1(skinned, color=[0.1, 0.8, 0.2], roughness=0.6)
output sleeve = lime
`;
const logs = [];
const lime = (r, g, b) => g > r + 60 && g > b + 60;
const measure = png => {
  let minX = Infinity, maxX = -1, minY = Infinity, maxY = -1, n = 0;
  const mask = new Uint8Array(png.width * png.height);
  for (let j = 0; j < png.height; j++) for (let i = 0; i < png.width; i++) {
    const k = (j * png.width + i) * 4;
    if (lime(png.data[k], png.data[k + 1], png.data[k + 2])) {
      mask[j * png.width + i] = 1; n++;
      minX = Math.min(minX, i); maxX = Math.max(maxX, i); minY = Math.min(minY, j); maxY = Math.max(maxY, j);
    }
  }
  // Connected blobs (4-neighbourhood, ignoring specks under 30 pixels).
  const seen = new Uint8Array(mask.length);
  let blobs = 0;
  for (let start = 0; start < mask.length; start++) {
    if (!mask[start] || seen[start]) continue;
    let size = 0; const stack = [start]; seen[start] = 1;
    while (stack.length) {
      const at = stack.pop(); size++;
      const x = at % png.width, y = (at - x) / png.width;
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx, ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= png.width || ny >= png.height) continue;
        const next = ny * png.width + nx;
        if (mask[next] && !seen[next]) { seen[next] = 1; stack.push(next); }
      }
    }
    if (size >= 30) blobs++;
  }
  assert(n > 300, `the tube is visible (${n} lime pixels)`);
  return { width: maxX - minX + 1, height: maxY - minY + 1, top: minY, blobs, n };
};
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch('http://127.0.0.1:5216')).ok) break; } catch {}
    await Bun.sleep(100);
  }
  await mkdir(reports, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  page.on('pageerror', error => logs.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && !driverAdvisory.test(message.text())) logs.push(message.text()); });
  await page.goto('http://127.0.0.1:5216/?eval=1&docname=g10-skin');
  await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
  const staged = await page.evaluate(text => window.__ve.doc.sendText(text, 'skin'), program);
  assert(staged.pending, `staged: ${JSON.stringify(staged).slice(0, 400)}`);
  await page.waitForFunction(
    () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
    null, { timeout: 180000 });
  assert.equal(await page.evaluate(() => window.__ve.evaluation.state.lastFailure), null, 'evaluates');
  const skin = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'sleeve').skin);
  assert.deepEqual(skin?.bones, ['lower_bone', 'upper_bone'], 'the page has the skin');
  const warnings = await page.evaluate(() => window.__ve.evaluation.state.warnings.map(w => w.code));
  assert(!warnings.some(code => code.startsWith('rig.')), `rig warnings: ${warnings}`);

  const at = async (seconds, name) => {
    await page.evaluate(t => window.__runtime.viewer_animation(false, t), seconds);
    await page.waitForTimeout(800);
    return measure(PNG.sync.read(await page.screenshot({ path: `${reports}skin-${name}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } })));
  };
  const straight = await at(0, 'straight'), bent = await at(1, 'bent');
  console.log(`G10 skin: straight ${straight.width}x${straight.height}, bent ${bent.width}x${bent.height}, ${bent.blobs} piece(s)`);
  assert(straight.height > straight.width * 2.2, 'at rest the tube is tall and thin');
  assert(bent.width > straight.width * 2, 'bent, the lower half swings out to the side');
  assert(bent.width / bent.height > 0.5, 'the bend is at the joint, about halfway');
  assert.equal(bent.blobs, 1, 'the bent tube is still one piece');
  assert(Math.abs(bent.top - straight.top) < 6, 'the top does not move');
  const again = await at(0, 'straight-again');
  assert(Math.abs(again.height - straight.height) < 3 && Math.abs(again.width - straight.width) < 3, 'back at the start it is straight again');

  const bytes = Uint8Array.from(await page.evaluate(async () => Array.from(await window.__ve.doc.exportGlb())));
  const report = await validator.validateBytes(bytes);
  const messages = report.issues.messages.filter(m => m.severity < 2).map(m => `${m.code} ${m.pointer ?? ''}`).join('; ');
  assert.equal(report.issues.numErrors, 0, `validator errors: ${messages}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset);
  const gltf = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + view.getUint32(12, true))));
  const index = name => gltf.nodes.findIndex(n => n.name === name);
  assert.deepEqual(gltf.skins?.[0]?.joints, [index('lower_bone'), index('upper_bone')], 'the exported skin joints are the bone parts');
  assert.equal(gltf.nodes[index('sleeve')].skin, 0, 'the tube node uses the skin');

  // Twist (ADR 0033): the lower bone turns a half circle about the tube's own axis. A linear blend pinches the tube to
  // a thread at the joint (every vertex there is the average of itself and its opposite); a dual-quaternion blend turns
  // it a quarter at the joint and keeps its width.
  const twist = blend => `lower = solid.box@1(size=[0.05, 0.05, 0.05])
lower_head = xform.move@1(lower, by=[0, -0.25, 0])
lower_bone_part = rig.bone@1(lower_head, head=[0, -0.5, 0], tail=[0, 0, 0])
turn = anim.track@1(lower_bone_part, property="rotation", keys="0:0,0,0; 1:0,180,0", interpolation="linear", pivot=[0, 0, 0], loop=false)
output lower_bone = turn
upper = solid.box@1(size=[0.05, 0.05, 0.05])
upper_head = xform.move@1(upper, by=[0, 0.25, 0])
upper_bone_part = rig.bone@1(upper_head, head=[0, 0, 0], tail=[0, 0.5, 0])
output upper_bone = upper_bone_part
tall = solid.box@1(size=[0.2, 1, 0.2])
cage = poly.from_solid@1(tall)
flat = poly.merge_coplanar@1(cage)
uprights = poly.select@1(flat, element="edge", by="box", box_min=[-1, -0.1, -1], box_max=[1, 0.1, 1])
cut = poly.subdivide_edges@1(flat, uprights, cuts=7)
skinned = rig.skin@1(cut, bones="lower_bone; upper_bone", blend="${blend}")
lime = solid.material@1(skinned, color=[0.1, 0.8, 0.2], roughness=0.6)
output sleeve = lime
`;
  const pinch = png => {
    const rows = [];
    for (let j = 0; j < png.height; j++) {
      let first = -1, last = -1;
      for (let i = 0; i < png.width; i++) {
        const k = (j * png.width + i) * 4;
        if (lime(png.data[k], png.data[k + 1], png.data[k + 2])) { if (first < 0) first = i; last = i; }
      }
      if (first >= 0) rows.push(last - first + 1);
    }
    // The joint sits at floor level, so only the tube's upper half shows; its lowest rows are the joint.
    const joint = rows.slice(Math.floor(rows.length * 0.1), Math.ceil(rows.length * 0.9));
    return Math.min(...joint) / [...rows].sort((a, b) => a - b)[Math.floor(rows.length / 2)];
  };
  const ratios = {};
  for (const blend of ['linear', 'dual_quaternion']) {
    // sendText adds to the open document, so each blend gets a document of its own.
    await page.goto(`http://127.0.0.1:5216/?eval=1&docname=g10-twist-${blend}`);
    await page.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
    await page.evaluate(async () => { window.__runtime = await import('./ve_runtime.js'); });
    const sent = await page.evaluate(text => window.__ve.doc.sendText(text, 'twist'), twist(blend));
    assert(sent.pending, `twist ${blend} staged: ${JSON.stringify(sent).slice(0, 600)}`);
    await page.waitForFunction(
      () => JSON.parse(window.__runtime.eval_projection_status()).committed?.quality === 'final' || window.__ve.evaluation.state.failureCurrent,
      null, { timeout: 180000 });
    const mode = await page.evaluate(() => window.__ve.evaluation.parts.find(p => p.name === 'sleeve').skin?.blend);
    assert.equal(mode, blend, 'the page has the blend mode');
    await page.evaluate(() => window.__runtime.viewer_animation(false, 1));
    await page.waitForTimeout(800);
    ratios[blend] = pinch(PNG.sync.read(await page.screenshot({ path: `${reports}skin-twist-${blend}.png`, clip: { x: 0, y: 0, width: 900, height: 490 } })));
  }
  console.log(`G10 skin twist: narrowest middle row / typical row: linear ${ratios.linear.toFixed(2)}, dual quaternion ${ratios.dual_quaternion.toFixed(2)}`);
  assert(ratios.linear < 0.45, 'the linear blend pinches at the twist');
  assert(ratios.dual_quaternion > 0.8, 'the dual-quaternion blend keeps its width');
  assert.deepEqual(logs, [], 'no browser errors');
} finally {
  await browser.close(); child.kill(); await child.exited;
}
