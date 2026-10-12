// Tries the engine MCP server with a real model: opens the engine's viewer page in a browser, then
// runs the Claude CLI with the engine's tools and a request. The model is the provider; no API key.
// Needs a build first: cargo xtask web dev
//
//   bun engine/tools/mcp/try-with-claude.mjs "make a hollow vase, then widen its opening"
//   bun engine/tools/mcp/try-with-claude.mjs --headed --model sonnet "make a twisted tower"
//
// --headed shows the browser so you can watch it build (and zoom or orbit the result with the wheel
// and Alt + drag). Uses your Claude Code login; costs a few cents per request.
import { chromium } from 'playwright';
import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { serve } from '../../web/tests/support.mjs';

const args = process.argv.slice(2);
const flag = (name) => { const i = args.indexOf(name); if (i < 0) return null; const [, value] = args.splice(i, 2); return value; };
const headed = args.includes('--headed') && args.splice(args.indexOf('--headed'), 1).length > 0;
const model = flag('--model') ?? 'sonnet';
const request = args.join(' ') || 'make a hollow vase, then widen its opening';
const server = path.resolve(import.meta.dir, 'server.mjs');
const port = 8766;

const dir = mkdtempSync(path.join(tmpdir(), 've-mcp-'));
const config = path.join(dir, 'mcp.json');
writeFileSync(config, JSON.stringify({ mcpServers: { 'spatial-engine': { command: 'bun', args: [server], env: { VE_MCP_PORT: String(port) } } } }));

const { server: pageServer, port: pagePort } = await serve();
const browser = await chromium.launch({ headless: !headed, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto(`http://127.0.0.1:${pagePort}/?viewer=1&docname=try-${Date.now()}&mcp=${port}`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation, null, { timeout: 60000 });
  console.log(`viewer ready; asking ${model}: ${request}`);
  const claude = Bun.spawn(['claude', '-p', '--model', model, '--mcp-config', config, '--strict-mcp-config',
    '--allowedTools', 'mcp__spatial-engine__*', '--tools', '', '--setting-sources', '', '--no-session-persistence',
    '--system-prompt', 'You build 3D objects by calling the spatial-engine tools. Call get_guide and list_operators first. Keep going until the request is done, call frame_view so the person can see it, then say in one or two sentences what you built.',
    request], { stdout: 'pipe', stderr: 'inherit' });
  const answer = await new Response(claude.stdout).text();
  await claude.exited;
  console.log('\nmodel says:', answer.trim());
  const result = await page.evaluate(() => ({ text: window.__ve.doc.text().text, revision: window.__ve.doc.state().revision,
    failure: window.__ve.evaluation.state.lastFailure }));
  console.log(`\ndocument at revision ${result.revision}:\n${result.text}`);
  if (result.failure) console.log('last evaluation failure:', result.failure);
  if (headed) { console.log('\nleaving the browser open for 60 s: scroll to zoom, Alt + drag to orbit'); await Bun.sleep(60000); }
} finally {
  await browser.close(); pageServer.close();
}
