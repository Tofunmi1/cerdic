// Browser contract for the evaluator's floating-parts diagnostics. Claude runs this in a browser.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { serve } from './support.mjs';

const { server, port } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const open = async (name, program) => {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/?viewer=1&eval=1&docname=${name}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  await page.evaluate(text => window.__ve.doc.sendText(text, 'Build attachment fixture'), program);
  await page.waitForFunction(() => window.__ve.evaluation?.state.worker === 'idle'
    && window.__ve.evaluation?.attachments !== null, null, { timeout: 90000 });
  await page.locator('#gizmo-diagnostics').check();
  return page;
};

try {
  const floating = await open(`attach-floating-${Date.now()}`, `
left = solid.box@1(size=[1, 1, 1])
right0 = solid.box@1(size=[1, 1, 1])
right = xform.move@1(right0, by=[2, 0, 0])
output left_part = left
output right_part = right
`);
  await floating.waitForFunction(() => document.querySelector('#gizmo-floating-parts')?.textContent.includes('left_part'));
  assert.match(await floating.locator('#gizmo-floating-parts').innerText(), /left_part\s+1\.00 m from\s+right_part/);
  await floating.locator('#gizmo-floating-parts button').first().click();
  assert.equal(await floating.evaluate(() => window.__ve.docPanel.selection.node),
    await floating.evaluate(() => window.__ve.evaluation.parts.find(part => part.name === 'left_part').node));
  await floating.close();

  const touching = await open(`attach-touching-${Date.now()}`, `
left = solid.box@1(size=[1, 1, 1])
right0 = solid.box@1(size=[1, 1, 1])
right = xform.move@1(right0, by=[1, 0, 0])
output left_part = left
output right_part = right
`);
  await touching.waitForFunction(() => document.querySelector('#gizmo-floating-parts')?.textContent.includes('all parts attached'));
  assert.match(await touching.locator('#gizmo-floating-parts').innerText(), /all parts attached/);
  await touching.close();
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
