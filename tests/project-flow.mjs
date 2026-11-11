// Focused integration check against the running Vite app and its real WASM engine.
// Run: bun tests/project-flow.mjs (one headless browser, no additional dev servers).
import { chromium } from 'playwright';
import { expect as baseExpect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
const expect = baseExpect.configure({ timeout: 30000 });
const browser = await chromium.launch({ headless: true, args: [process.platform === 'darwin' ? '--use-angle=metal' : '--use-angle=swiftshader', '--enable-unsafe-webgpu'] });
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.setDefaultTimeout(30000);
const errors = [];
page.on('pageerror', error => errors.push(error.message));
const root = process.env.PROJECT_TEST_URL ?? 'http://127.0.0.1:5173';
const report = 'target/reports/project-flow';
await mkdir(report, { recursive: true });
async function create(name) {
  await page.getByLabel('Project name', { exact: true }).fill(name);
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeEnabled();
  return new URL(page.url()).searchParams.get('project');
}
async function message(text) {
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill(text);
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.locator('.msg.user')).toContainText(text);
}
try {
  await page.goto(`${root}/studio?scene=open`);
  await expect(page.getByRole('dialog', { name: 'A home for every world.' })).toBeVisible();
  await page.screenshot({ path: `${report}/create-project.png` });
  const alpha = await create('Alpha');
  expect(alpha).toBeTruthy();
  await message('Alpha first conversation');
  const firstChat = await page.getByLabel('Conversation', { exact: true }).inputValue();
  await page.getByRole('button', { name: 'New conversation', exact: true }).click();
  await expect(page.locator('.msg.user')).toHaveCount(0);
  await message('Alpha second conversation');
  await page.getByLabel('Conversation', { exact: true }).selectOption(firstChat);
  await expect(page.locator('.msg.user')).toHaveCount(1);
  await expect(page.locator('.msg.user')).toContainText('Alpha first conversation');

  await page.evaluate(async () => { await window.__spatial.doc.sendText('test_box = solid.box@1(size=[1,2,3])\noutput test_box = test_box', 'project-test'); });
  await expect(page.locator('.project-save-state')).toHaveText('Saved locally');
  await page.waitForFunction(() => !window.__spatial.doc.state().pending_document && window.__spatial.doc.state().nodes > 0);
  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  const alphaDocumentId = await page.evaluate(() => window.__spatial.doc.state().id);
  const beta = await create('Beta');
  expect(await page.evaluate(() => window.__spatial.doc.state().id)).not.toBe(alphaDocumentId);
  expect(beta).not.toBe(alpha);
  await expect.poll(async () => page.evaluate(() => JSON.parse(window.__spatial.runtime.eval_projection_status()).live_meshes)).toBe(0);
  await expect(page.locator('.msg.user')).toHaveCount(0);
  await message('Beta private conversation');
  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  await page.locator('.project-open').filter({ hasText: 'Alpha' }).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  await expect(page.locator('.msg.user')).toContainText('Alpha first conversation');
  await expect(page.locator('.msg.user')).not.toContainText('Beta private');
  expect(await page.evaluate(() => window.__spatial.doc.text().text), "world checkpoint 1").toContain('test_box');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Open projects', exact: true })).toContainText('Alpha');
  await expect(page.locator('.msg.user')).toContainText('Alpha first conversation');
  await page.waitForFunction(() => window.__spatial.ready && window.__spatial.doc.state().nodes > 0);
  expect(await page.evaluate(() => window.__spatial.doc.text().text), "world checkpoint 2").toContain('test_box');

  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  await page.getByRole('button', { name: 'Rename Alpha', exact: true }).click();
  await page.getByLabel('New project name', { exact: true }).fill('Alpha renamed');
  await page.getByRole('button', { name: 'Save name', exact: true }).click();
  await expect(page.locator('.project-open').filter({ hasText: 'Alpha renamed' })).toBeVisible();
  await page.getByRole('button', { name: 'Close projects', exact: true }).click();
  expect(new URL(page.url()).searchParams.get('project')).toBe(alpha);
  await expect(page.locator('.msg.user')).toContainText('Alpha first conversation');
  await page.screenshot({ path: `${report}/world-chat.png` });

  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  await page.getByLabel('Starting point', { exact: true }).selectOption('file');
  await page.locator('.project-create input[type=file]').setInputFiles({ name: 'broken.txt', mimeType: 'text/plain', buffer: Buffer.from('this is not a scene program ???') });
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.locator('.project-error')).toBeVisible();
  await expect(page.locator('.project-row')).toHaveCount(2);
  await page.getByRole('button', { name: 'Close projects', exact: true }).click();
  await page.waitForFunction(() => !window.__spatial.doc.state().pending_document && window.__spatial.doc.state().nodes > 0);
  expect(await page.evaluate(() => window.__spatial.doc.text().text), "world checkpoint 3").toContain('test_box');

  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  await page.getByRole('button', { name: 'Close projects', exact: true }).click();
  await page.routeWebSocket('ws://127.0.0.1:8765', () => {});
  await page.getByRole('button', { name: 'Connect assistant', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Connect an assistant' })).toBeVisible();
  await page.screenshot({ path: `${report}/assistant-connection.png` });
  await page.getByRole('button', { name: 'Enable MCP connection', exact: true }).click();
  await expect(page.locator('.assistant-connection-status')).toHaveText('Connected to the local engine bridge.');
  expect(new URL(page.url()).searchParams.get('project')).toBe(alpha);
  expect(new URL(page.url()).searchParams.get('mcp')).toBe('1');
  await page.getByRole('button', { name: 'Disconnect workspace', exact: true }).click();
  await expect(page.locator('.assistant-connection-status')).toHaveText('Workspace connection is off.');
  expect(new URL(page.url()).searchParams.has('mcp')).toBe(false);
  await page.getByRole('button', { name: 'Close assistant connection', exact: true }).click();
  await page.getByRole('button', { name: 'Open projects', exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: `${report}/projects-mobile.png` });
  expect(await page.locator('.project-hub').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  expect(errors).toEqual([]);
  console.log('PASS: create, world autosave, project switching, separate conversations, reload, rename, invalid-import recovery, and mobile layout.');
} catch (error) { await page.screenshot({path: `${report}/failure.png`}); console.error(await page.locator('[role=alert]').allTextContents()); throw error; } finally { await browser.close(); }
