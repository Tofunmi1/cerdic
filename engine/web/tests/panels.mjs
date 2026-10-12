// V1 acceptance through the real UI of the viewer page: the outliner, the inspector, the history list
// and the problems list. Selecting in a panel lights up the part in the viewport and clicking the part
// selects the node; an edit made in the inspector is the same command a script would send; bad values
// are refused with the reason; undo and reload work from the history.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { serve } from './support.mjs';

const PROGRAM = `base = solid.box@1(size=[0.4, 0.2, 0.3])
ball = solid.sphere@1(radius=0.15, segments=24)
ball_up = xform.move@1(ball, by=[0, 0.15, 0])
body = solid.union@1(base, ball_up)
red = solid.material@1(body, color=[0.8, 0.1, 0.1], roughness=0.3)
output part = red
`;
const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const problems = [];
const step = (name) => console.log(`-- ${name}`);

// Pages in one `context` share local storage, which is how a reload finds a saved document.
const open = async (docname, program = PROGRAM, context = null) => {
  const page = await (context ? context.newPage() : browser.newPage());
  await page.setViewportSize({ width: 1280, height: 800 });
  page.on('pageerror', (error) => problems.push(error.message));
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=${docname}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  if (program) {
    await page.evaluate((text) => window.__ve.doc.sendText(text, 'Build the model'), program);
    await settled(page, 1);
  }
  return page;
};
// The staged edit has been evaluated and committed, and the worker is idle.
const settled = (page, revision) => page.waitForFunction((r) => {
  const state = window.__ve.doc.state();
  return state.revision >= r && !state.pending_document && window.__ve.evaluation.state.worker === 'idle';
}, revision, { timeout: 60000 });
const doc = (page) => page.evaluate(() => window.__ve.doc.state().document);
const idOf = async (page, label) => (await doc(page)).nodes.find((node) => node.label === label).id;
const rows = (page) => page.$$eval('.dock .orow', (items) => items.map((item) => ({
  text: item.querySelector('.oname').textContent, selected: item.classList.contains('selected'), id: item.dataset.id,
})));
const frames = async (page, n = 6) => {
  const start = await page.evaluate(() => window.__ve.read().frames);
  await page.waitForFunction((target) => window.__ve.read().frames >= target, start + n, { timeout: 30000 });
};
const modelPixels = async (page) => PNG.sync.read(await page.locator('canvas#ve').screenshot({ clip: { x: 340, y: 200, width: 560, height: 420 } }));
const changed = (a, b) => {
  let count = 0;
  for (let p = 0; p < a.data.length; p += 4) {
    if (Math.abs(a.data[p] - b.data[p]) + Math.abs(a.data[p + 1] - b.data[p + 1]) + Math.abs(a.data[p + 2] - b.data[p + 2]) > 24) count++;
  }
  return count;
};
const engineSelection = (page) => page.evaluate(() => JSON.parse(window.__ve.docPanel.selection && window.__spatialRuntime.selection_get()));

try {
  const page = await open(`panels-a-${Date.now()}`);
  await page.exposeFunction('noop', () => {});
  await page.evaluate(async () => { window.__spatialRuntime = await import('./ve_runtime.js'); });

  step('the outliner shows the part and what builds it, indented');
  const tree = await rows(page);
  assert.deepEqual(tree.map((row) => row.text), ['red', 'body', 'base', 'ball_up', 'ball'], 'the part, then the nodes that build it');
  assert((await page.$$eval('.dock .obadge.part', (b) => b.map((x) => x.textContent))).includes('part'), 'the output name is badged');
  assert(tree.every((row) => !row.selected), 'nothing is selected at first');
  assert.equal((await engineSelection(page)).node, null);

  step('selecting in the outliner lights up the part in the viewport');
  await frames(page);
  const before = await modelPixels(page);
  const baseId = await idOf(page, 'base');
  await page.click(`.dock .orow[data-id="${baseId}"]`);
  const selected = await engineSelection(page);
  assert.equal(selected.node, baseId, 'the engine knows the selection');
  assert.deepEqual(selected.parts, [await idOf(page, 'red')], 'base builds the red part, so that part lights up');
  await frames(page, 30); // the glow eases in
  const after = await modelPixels(page);
  const lit = changed(before, after);
  assert(lit > 3000, `the selected part is visibly highlighted (${lit} pixels changed)`);
  assert((await rows(page)).find((row) => row.id === baseId).selected, 'the row is marked selected');
  await page.screenshot({ path: new URL('../../target/reports/panels-selected.png', import.meta.url).pathname });

  step("the inspector shows the node's parameters from its spec");
  const caption = (await page.$$eval('.dock #dock-inspector .cap', (c) => c.map((x) => x.textContent)));
  assert(caption.includes('size') && caption.includes('in') === false || caption.includes('size'), `the box's size shows (${caption})`);
  const size = await page.$$eval('#dock-inspector input.num', (inputs) => inputs.map((input) => Number(input.value)));
  assert.deepEqual(size, [0.4, 0.2, 0.3], 'lengths show in metres, as stored');

  step('editing a parameter is the command a script would send');
  const ballId = await idOf(page, 'ball');
  await page.click(`.dock .orow[data-id="${ballId}"]`);
  const radius = page.locator('#dock-inspector input.num').first();
  await radius.fill('0.2');
  await radius.press('Enter');
  await settled(page, 2);
  const viaUi = await doc(page);
  assert.equal(viaUi.nodes.find((n) => n.id === ballId).params.radius, 0.2, 'the document holds the new radius');

  const mock = await open(`panels-mock-${Date.now()}`);
  await mock.evaluate((id) => window.__ve.doc.send([{ set_param: { node: id, param: 'radius', value: 0.2 } }], 'Set radius'), ballId);
  await settled(mock, 2);
  assert.deepEqual((await doc(mock)).nodes, viaUi.nodes, 'the inspector edit equals the same command sent directly');
  await mock.close();

  step('a bad value is refused with the reason, and the document does not change');
  await radius.fill('-1');
  await radius.press('Enter');
  await page.waitForSelector('#dock-inspector .issue');
  const reason = await page.textContent('#dock-inspector .issue');
  assert(/range|radius|param/i.test(reason), `the refusal says why: ${reason}`);
  assert.equal((await doc(page)).nodes.find((n) => n.id === ballId).params.radius, 0.2, 'a refused edit changed nothing');
  assert.equal(await page.$eval('#doc-message', (m) => m.classList.contains('bad')), true, 'the document panel reports it too');

  step('deleting a node something reads is refused too');
  await page.click(`.dock .orow[data-id="${baseId}"]`);
  await page.click('#dock-inspector .danger');
  await page.waitForSelector('#dock-inspector .issue');
  assert(/in_use|read/i.test(await page.textContent('#dock-inspector .issue')), 'deleting a node that is read says so');
  assert((await doc(page)).nodes.some((n) => n.id === baseId), 'it is still there');

  step('the label');
  await page.fill('#dock-inspector input.title', 'plinth');
  await page.press('#dock-inspector input.title', 'Enter');
  await settled(page, 3);
  assert.equal((await rows(page)).find((row) => row.id === baseId).text, 'plinth', 'the outliner shows the new name');

  step('history: every edit is listed; clicking one steps back to it; redo steps forward');
  await page.click('summary:has-text("History")');
  const labels = await page.$$eval('.dock .hrow', (items) => items.map((item) => item.textContent));
  assert.deepEqual(labels, ['Build the model', 'Set radius', 'Rename'], 'every transaction is listed, oldest first');
  await page.click('.dock .hrow.done >> nth=0');
  await settled(page, 5); // two undos: revisions only move forward
  assert.equal((await doc(page)).nodes.find((n) => n.id === ballId).params.radius, 0.15, 'going back to the first edit restores the radius');
  assert.deepEqual(await page.$$eval('.dock .hrow.undone', (items) => items.map((item) => item.textContent)), ['Set radius', 'Rename']);
  await page.click('.dock .hrow.undone >> nth=0');
  await settled(page, 6);
  assert.equal((await doc(page)).nodes.find((n) => n.id === ballId).params.radius, 0.2, 'redo brings it back');

  step('clicking the part in the viewport selects its node (the reverse direction)');
  await page.click('.dock .orow.selected');            // deselect
  assert.equal((await engineSelection(page)).node, null);
  let hit = null;
  for (let y = 240; y < 560 && !hit; y += 14) {
    for (let x = 380; x < 900 && !hit; x += 14) {
      await page.mouse.move(x, y);
      await frames(page, 2);
      if ((await page.evaluate(() => window.__ve.read().interact.hover.length)) > 0) hit = { x, y };
    }
  }
  assert(hit, 'some screen position hovers the model');
  await page.mouse.down(); await page.mouse.up();
  await page.waitForFunction(() => document.querySelector('.dock .orow.selected'), null, { timeout: 5000 });
  const clicked = await engineSelection(page);
  assert.equal(clicked.node, await idOf(page, 'red'), 'the click selected the node that builds the part');
  assert.equal((await rows(page)).find((row) => row.selected).text, 'red', 'and the outliner follows');
  await page.close();

  step('the problems list: the engine refuses an edit that breaks the wall, and says where');
  const vase = await open(`panels-vase-${Date.now()}`, await readFile(new URL('../../crates/ve-modeling/tests/gallery/ceramic_vase.txt', import.meta.url), 'utf8'));
  const wallId = await idOf(vase, 'wall');
  await vase.click(`.dock .orow[data-id="${wallId}"]`);
  const minimum = vase.locator('#dock-inspector input.num').first();
  await minimum.fill('0.05');
  await minimum.press('Enter');
  await vase.waitForSelector('#dock-problems:not([hidden])', { timeout: 60000 });
  const listed = await vase.$$eval('#dock-problems .prow', (items) => items.map((item) => item.textContent));
  assert(listed.some((text) => text.includes('constraint.min_wall')), `the wall problem is listed: ${listed}`);
  assert.equal((await doc(vase)).nodes.find((n) => n.id === wallId).params.min, 0.003, 'the refused edit did not stay');
  await vase.click('.dock .orow.selected');            // deselect, then jump to the node from the problem
  await vase.click('#dock-problems .prow');
  assert.equal((await rows(vase)).find((row) => row.selected)?.id, wallId, 'clicking a problem selects the node it names');

  step('save, then a fresh page (a reload) has the same document');
  const name = `panels-save-${Date.now()}`;
  const storage = await browser.newContext();
  const saver = await open(name, PROGRAM, storage);
  await saver.evaluate(() => window.__ve.doc.save());
  const saved = await doc(saver);
  await saver.close();
  const reopened = await open(name, null, storage);
  assert.deepEqual((await doc(reopened)).nodes, saved.nodes, 'the reloaded document equals the saved one');
  assert.deepEqual((await rows(reopened)).map((row) => row.text), ['red', 'body', 'base', 'ball_up', 'ball'], 'and the outliner shows it');

  assert.deepEqual(problems, [], 'no page errors');
  console.log('panels test passed');
} finally {
  await browser.close();
  server.close();
}
