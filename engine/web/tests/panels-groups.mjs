// Groups in the viewer page's panels: the outliner shows a group with its members and its uses, a member
// lights up every part its group is used in, an instance's inspector overrides and resets exposed
// parameters, an interface edit upgrades the instances in the same transaction, the group has its own
// editor, and an instance that is behind its group says so and can be upgraded.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { serve } from './support.mjs';

const WHEEL = `group wheel
  tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)
  hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)
  spoke = solid.box@1(size=[0.22, 0.03, 0.03])
  spokes = xform.radial@1(spoke, count=4, radius=0.14)
  rim = solid.union@1(tyre, hub, spokes)
  expose radius = tyre.major
  expose width = hub.height
  result rim
end
front = wheel()
back = wheel(radius=0.4)
shifted = xform.move@1(back, by=[1, 0, 0])
output front
output shifted
`;
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const problems = [];
const step = (name) => console.log(`-- ${name}`);

const open = async (docname, { evaluate = true, text = null } = {}) => {
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1280, height: 900 });
  page.on('pageerror', (error) => problems.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1${evaluate ? '&eval=1' : ''}&docname=${docname}`);
  await page.waitForFunction((e) => window.__ve?.ready && (!e || window.__ve.evaluation?.state.worker === 'idle'), evaluate, { timeout: 90000 });
  if (text) {
    await page.evaluate((t) => window.__ve.doc.sendText(t, 'Build'), text);
    await settled(page, 1);
  }
  return page;
};
const settled = (page, revision) => page.waitForFunction((r) => {
  const state = window.__ve.doc.state();
  return state.revision >= r && !state.pending_document && (!window.__ve.evaluation || window.__ve.evaluation.state.worker === 'idle');
}, revision, { timeout: 60000 });
const doc = (page) => page.evaluate(() => window.__ve.doc.state().document);
const idOf = async (page, label) => (await doc(page)).nodes.find((n) => n.label === label).id;
const rows = (page) => page.$$eval('.dock .orow', (items) => items.map((item) => ({
  id: item.dataset.id, text: item.querySelector('.oname').textContent, group: item.classList.contains('ogroup'),
  badges: [...item.querySelectorAll('.obadge')].map((b) => b.textContent), op: item.querySelector('.oop')?.textContent,
})));
const pick = async (page, id) => { await page.click(`.dock .orow[data-id="${id}"]`); };
const captions = (page) => page.$$eval('#dock-inspector .irow .cap', (c) => c.map((x) => x.textContent));
const selection = (page) => page.evaluate(async () => JSON.parse((await import('./ve_runtime.js')).selection_get()));

try {
  const page = await open(`groups-a-${Date.now()}`, { text: WHEEL });

  step('the outliner shows the parts, then the group with its members');
  const tree = await rows(page);
  assert.deepEqual(tree.map((r) => r.text), ['front', 'shifted', 'back', 'wheel', 'rim', 'tyre', 'hub', 'spokes', 'spoke'], tree.map((r) => r.text).join(','));
  const header = tree.find((r) => r.group);
  assert.deepEqual(header.badges, ['v3'], 'the group shows its version');
  assert.equal(header.op, '2 uses');
  assert.equal(tree.find((r) => r.text === 'front').op, 'wheel', 'an instance names its group');
  const groupId = (await doc(page)).groups[0].id;
  assert.equal(header.id, groupId);

  step('a node inside the group lights up every part the group is used in');
  const tyre = await idOf(page, 'tyre');
  await pick(page, tyre);
  const lit = await selection(page);
  assert.equal(lit.node, tyre);
  assert.deepEqual(lit.parts.sort(), [await idOf(page, 'front'), await idOf(page, 'shifted')].sort(), 'front, and the move that reads back');
  assert(await page.isVisible('#dock-inspector .banner'), 'the inspector says editing it edits the definition');
  assert.match(await page.textContent('#dock-inspector .banner'), /Part of the group wheel/);
  assert.equal(await page.textContent('#dock-inspector .irow button.link:has-text("exposed as")'), 'exposed as radius ×');

  step('an instance shows the exposed parameters and overrides them one by one');
  const back = await idOf(page, 'back');
  await pick(page, back);
  assert.deepEqual((await captions(page)).filter((c) => ['radius', 'width'].includes(c)), ['radius', 'width']);
  assert.equal(await page.$eval('#dock-inspector .irow.overridden .cap', (c) => c.textContent), 'radius', 'back overrides only the radius');
  const radius = await page.locator('#dock-inspector .irow:has(.cap:text-is("radius")) input.num').inputValue();
  assert.equal(Number(radius), 0.4);
  const width = page.locator('#dock-inspector .irow:has(.cap:text-is("width")) input.num');
  assert.equal(Number(await width.inputValue()), 0.2, 'an unoverridden parameter shows the definition');
  await width.fill('0.3');
  await width.press('Enter');
  await settled(page, 2);
  assert.equal((await doc(page)).nodes.find((n) => n.id === back).instance.overrides.width, 0.3);
  assert.equal(await page.$$eval('#dock-inspector .irow.overridden', (r) => r.length), 2);
  await page.click('#dock-inspector .irow:has(.cap:text-is("width")) button:has-text("reset")');
  await settled(page, 3);
  assert.deepEqual(Object.keys((await doc(page)).nodes.find((n) => n.id === back).instance.overrides), ['radius']);
  const refused = page.locator('#dock-inspector .irow:has(.cap:text-is("radius")) input.num');
  await refused.fill('-1');
  await refused.press('Enter');
  await page.waitForSelector('#dock-inspector .issue');
  assert.match(await page.textContent('#dock-inspector .issue'), /param\.range/, 'the member parameter\'s own range applies');

  step('an interface edit upgrades the instances in the same transaction');
  const version = (await doc(page)).groups[0].version;
  await pick(page, tyre);
  await page.click('#dock-inspector button:has-text("exposed as radius")');
  await settled(page, 4);
  const after = await doc(page);
  assert.equal(after.groups[0].version, version + 1);
  assert(after.nodes.filter((n) => n.instance).every((n) => n.instance.version === version + 1), 'every instance moved with it');
  assert.deepEqual(after.nodes.find((n) => n.id === back).instance.overrides, {}, 'the override of the parameter that went is dropped');
  const history = await page.$$eval('.dock .hrow', (h) => h.map((x) => x.textContent));
  assert.equal(history.at(-1), 'Unexpose radius (upgrading 2)', 'one undo step');
  assert.equal((await rows(page)).find((r) => r.group).badges[0], `v${version + 1}`);

  step('the group has its own editor');
  await pick(page, groupId);
  assert.equal(await page.inputValue('#dock-inspector input.title'), 'wheel');
  assert.match(await page.textContent('#dock-inspector .ophelp'), /version 4 · 2 uses/);
  assert.deepEqual(await page.$$eval('#dock-inspector .irow .cap', (c) => c.map((x) => x.textContent)), ['result', 'width']);
  await page.fill('#dock-inspector input.title', 'bad name');
  await page.press('#dock-inspector input.title', 'Enter');
  await page.waitForSelector('#dock-inspector .issue');
  assert.match(await page.textContent('#dock-inspector .issue'), /group\.name/);
  assert.equal((await doc(page)).groups[0].name, 'wheel', 'a bad name changed nothing');
  await page.fill('#dock-inspector input.title', 'wheel_set');
  await page.press('#dock-inspector input.title', 'Enter');
  await settled(page, 5);
  assert.equal((await rows(page)).find((r) => r.group).text, 'wheel_set');
  await page.click('#dock-inspector button:has-text("Add instance")');
  await settled(page, 6);
  assert.equal((await doc(page)).nodes.filter((n) => n.instance).length, 3);
  assert.equal((await rows(page)).find((r) => r.group).op, '3 uses');
  await page.close();

  step('an instance behind its group says so, and can be upgraded');
  const plain = await open(`groups-b-${Date.now()}`, { evaluate: false });
  const fixture = JSON.parse(await readFile(new URL('../../crates/ve-graph/tests/fixtures/wheel_group.json', import.meta.url), 'utf8'));
  await plain.evaluate((commands) => window.__ve.doc.send(commands, 'wheel'), fixture);
  const group = (await doc(plain)).groups[0];
  await plain.evaluate((g) => {
    window.__ve.doc.send([{ add_instance: { group: g, label: 'old' } }], 'use');
    window.__ve.doc.send([{ set_instance_param: { node: window.__ve.doc.state().document.nodes.find((n) => n.label === 'old').id, param: 'width', value: 0.3 } }], 'override');
    window.__ve.doc.send([{ unexpose_param: { group: g, name: 'width' } }], 'interface change');
    window.__ve.docPanel.render();
  }, group.id);
  const old = await idOf(plain, 'old');
  await pick(plain, old);
  assert.match(await plain.textContent('#dock-inspector .banner'), /Made against version 3; the group is now version 4\. Upgrading drops: override width\./);
  assert.equal(await plain.$$eval('.dock .obadge.warn', (b) => b.map((x) => x.textContent)).then((b) => b[0]), 'outdated');
  await plain.click('#dock-inspector .banner button:has-text("Upgrade")');
  const upgraded = (await doc(plain)).nodes.find((n) => n.id === old).instance;
  assert.equal(upgraded.version, 4);
  assert.deepEqual(upgraded.overrides, {}, 'the override the group no longer exposes is gone');
  assert.equal(await plain.$$('#dock-inspector .banner').then((b) => b.length), 0, 'and the warning goes');
  await plain.close();

  assert.deepEqual(problems, [], 'no page errors');
  console.log('panels groups test passed');
} finally {
  await browser.close();
  server.close();
}
