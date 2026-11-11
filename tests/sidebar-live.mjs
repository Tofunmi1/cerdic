// Opt-in live check: uses the Codex account signed in on this machine and consumes model tokens.
// Run with Vite and `bun server/index.ts` running: bun tests/sidebar-live.mjs
import { chromium } from 'playwright';
import { expect as baseExpect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
const expect = baseExpect.configure({ timeout: 180000 });
const browser = await chromium.launch({ headless: true, args: [process.platform === 'darwin' ? '--use-angle=metal' : '--use-angle=swiftshader', '--enable-unsafe-webgpu'] });
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.setDefaultTimeout(30000);
const errors = [];
page.on('pageerror', error => errors.push(error.message));
await mkdir('target/reports/sidebar-live', { recursive: true });
try {
  await page.goto('http://127.0.0.1:5173/studio?projects=1');
  await page.getByLabel('Project name', { exact: true }).fill('Sidebar live verification');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  await page.getByRole('button', { name: 'Connect assistant', exact: true }).click();
  await page.getByRole('button', { name: 'Connect local Codex', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Start chatting', exact: true })).toBeVisible();
  console.log('CONNECTED', await page.getByLabel('Connected Codex model', { exact: true }).inputValue());
  await page.getByRole('button', { name: 'Start chatting', exact: true }).click();
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Use the engine tools to create one blue box named sidebar_box, size [1,2,1] metres, as a visible output in this empty world. Read the needed guide first. Check it and frame the view. Do not use shell or filesystem tools. Keep this verification tiny, then reply briefly.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop assistant', exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.__spatial.doc.state().nodes), { timeout: 180000 }).toBeGreaterThan(0);
  await expect(page.getByRole('button', { name: 'Stop assistant', exact: true })).toBeHidden();
  await expect(page.locator('.msg.assistant')).not.toHaveCount(0);
  const text = await page.evaluate(() => window.__spatial.doc.text().text);
  expect(text).toContain('sidebar_box');
  console.log('BUILT', (text ?? '').slice(0, 500));
  const originalBoxId = await page.evaluate(() => window.__spatial.doc.state().document.nodes.find(n => n.op === 'solid.box@1')?.id);
  const firstScope = new URL(page.url()).searchParams.get('project');
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Edit the existing sidebar_box to be 3 metres tall, keeping its width and depth at 1 metre and keeping its existing node identity. Check it, then reply briefly.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop assistant', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Stop assistant', exact: true })).toBeHidden();
  const next = await page.evaluate(() => window.__spatial.doc.text().text);
  expect(next).toContain('3');
  expect(await page.evaluate(() => window.__spatial.doc.state().document.nodes.find(n => n.op === 'solid.box@1')?.id)).toBe(originalBoxId);
  await expect.poll(() => page.evaluate(() => window.__spatial.evaluation.measures.some(part => part.size && Math.abs(part.size[1] - 3) < 0.001))).toBe(true);
  expect(new URL(page.url()).searchParams.get('project')).toBe(firstScope);
  await page.screenshot({ path: 'target/reports/sidebar-live/box-and-followup.png' });
  await page.getByRole('button', { name: 'New conversation', exact: true }).click();
  await expect(page.locator('.msg.user')).toHaveCount(0);
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Do not modify the scene. Describe 100 different courtyard design ideas in detail.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await page.getByRole('button', { name: 'Stop assistant', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop assistant', exact: true })).toBeHidden();
  await expect(page.getByRole('button', { name: 'Open projects', exact: true })).toBeEnabled();
  expect(await page.evaluate(() => window.__spatial.doc.state().document.nodes.find(n => n.op === 'solid.box@1')?.id)).toBe(originalBoxId);
  expect(errors).toEqual([]);
  console.log('PASS: actual sidebar → local signed-in Codex → browser engine tools → visible geometry → same-conversation follow-up.');
} catch (error) {
  await page.screenshot({ path: 'target/reports/sidebar-live/failure.png' });
  console.error('COMPOSER', await page.locator('.composer .note').allTextContents());
  console.error('ACTIVITY', (await page.locator('.chat-scroll').innerText()).slice(-6000));
  throw error;
} finally { await browser.close(); }
