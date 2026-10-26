// Asks a real model to build something that MOVES, through the engine's MCP tools against the CURRENT viewer build (the page served on
// :8801 from engine/web/dist-bevy-gpu), then saves the world it wrote. The point is to see where a model fails with the language as it is.
//
//   bun engine/tools/mcp/try-animated.mjs [--agent codex|claude] [--model gpt-6.1-sol] [--effort high] [--name creature_try] "a request"
//
// --agent codex (default): the Codex CLI, `-m gpt-6.1-sol` with high reasoning effort unless told otherwise, run in an EMPTY scratch
//   directory with a read-only sandbox so it cannot read this repository's source through its shell; the engine's tools come over MCP.
// --agent claude: the Claude CLI (default model sonnet). Both use their own logins; neither needs an API key here.
// Writes engine/web/dist-bevy-gpu/worlds/<name>.txt (the model's world) and <name>.json (the tool calls it made, its answer, any warnings).
import { chromium } from 'playwright';
import { writeFileSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { summarizeCodexEvents, aggregate, formatTable } from './run-metrics.mjs';

const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(name); if (i < 0) return fallback; const [, value] = args.splice(i, 2); return value; };
const agent = flag('--agent', 'codex');
const model = flag('--model', agent === 'codex' ? 'gpt-6.1-sol' : 'sonnet');
const effort = flag('--effort', 'high');
// --provider agentrouter: the Codex CLI talks to agentrouter.org over the responses API (key in the AGENTROUTER_KEY environment variable).
const provider = flag('--provider', null);
const providerArgs = provider === 'agentrouter'
  ? ['-c', 'model_provider="agentrouter"', '-c', 'model_providers.agentrouter={name="agentrouter", base_url="https://agentrouter.org/v1", env_key="AGENTROUTER_KEY", wire_api="responses"}']
  : [];
const name = flag('--name', 'creature_try');
const port = Number(flag('--port', '8767'));
// --repeat N: independent runs from the seed, on consecutive MCP ports.
const repeating = args.includes('--repeat');
const repeat = Number(flag('--repeat', '1'));
if (!Number.isInteger(repeat) || repeat < 1 || !Number.isInteger(port) || port < 1 || port + repeat - 1 > 65535) {
  throw new Error('--repeat must be a positive integer and all ports must be in 1..65535');
}
// --from worlds/blackout.txt: start from an existing world (a path under web/dist-bevy-gpu) instead of an empty document, so the model extends it.
const from = flag('--from', '');
// Every flag is read before the rest is taken as the request.
const request = args.join(' ') || 'a four-legged creature that walks and turns its head';
const server = path.resolve(import.meta.dir, 'server.mjs');
const root = path.resolve(import.meta.dir, '../..');

const dir = mkdtempSync(path.join(tmpdir(), 've-mcp-'));
// A server that does not even parse gives the model no tools and a confusing "tools are not available": check it first.
const preflight = Bun.spawnSync(['bun', 'build', server, '--target', 'bun', '--outfile', path.join(dir, 'server-check.js')], { stderr: 'pipe', stdout: 'pipe' });
if (preflight.exitCode !== 0) { console.error(`the MCP server does not build:\n${preflight.stderr.toString().slice(0, 800)}`); process.exit(2); }
const system = 'You build interactive 3D things by calling the spatial-engine tools (and only those: do not read files or run shell commands). Call get_guide and list_operators first. Keep going until the request is done: check the result (snapshot, including its motion mode for anything that moves, and the check tool) and fix what is wrong. Then say in two sentences what you built and what you could not do.';

const browser = await chromium.launch({ channel: 'chromium', args: ['--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU', '--ignore-gpu-blocklist'] });
async function runOnce(name, port) {
  const runDir = mkdtempSync(path.join(dir, 'run-'));
  const config = path.join(runDir, 'mcp.json');
  writeFileSync(config, JSON.stringify({ mcpServers: { 'spatial-engine': { command: 'bun', args: [server], env: { VE_MCP_PORT: String(port) } } } }));
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  try {
    // A seed world loads through the page's own `world=` parameter (the path the viewer uses); sending a big world as text through the page's API timed out.
    await page.goto(`http://127.0.0.1:8801/?viewer=1&docname=try-${path.basename(runDir)}&mcp=${port}&scene=stage&quality=high${from ? `&world=${encodeURIComponent(from)}` : ''}`);
    await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation, null, { timeout: 120000 });
    if (from) {
      const seed = readFileSync(path.join(root, 'web/dist-bevy-gpu', from), 'utf8');
      await page.waitForFunction(() => window.__ve.doc.state().revision > 0, null, { timeout: 300000 });
      await page.waitForTimeout(8000);
      console.log(`seeded from ${from} (${seed.split('\n').length} lines)`);
    }
    console.log(`viewer ready; asking ${agent} (${model}${agent === 'codex' ? `, effort ${effort}` : ''}): ${request}`);
    const calls = [];
    let answer = '';
    let metrics;
    const started = performance.now();
    if (agent === 'claude') {
      const claude = Bun.spawn(['claude', '-p', '--model', model, '--mcp-config', config, '--strict-mcp-config',
        '--allowedTools', 'mcp__spatial-engine__*', '--tools', '', '--setting-sources', '', '--no-session-persistence',
        '--output-format', 'stream-json', '--verbose', '--system-prompt', system, request], { stdout: 'pipe', stderr: 'inherit' });
      const raw = await new Response(claude.stdout).text();
      await claude.exited;
      for (const line of raw.split('\n')) {
        if (!line.trim()) continue;
        try {
          const event = JSON.parse(line);
          for (const part of event.message?.content ?? []) {
            if (part.type === 'tool_use') calls.push({ tool: part.name.replace('mcp__spatial-engine__', ''), input: part.input });
            if (part.type === 'text' && event.type === 'assistant') answer = part.text;
          }
        } catch { /* not a JSON line */ }
      }
      const activity = summarizeCodexEvents(calls.map((call) => JSON.stringify({ type: 'item.completed', item: { type: 'mcp_tool_call', tool: call.tool } })).join('\n'));
      metrics = { toolCalls: activity.toolCalls, byTool: activity.byTool, looks: activity.looks, orientation: activity.orientation, builds: activity.builds };
    } else {
      const scratch = mkdtempSync(path.join(tmpdir(), 've-codex-'));
      const out = path.join(runDir, 'answer.txt');
      const codex = Bun.spawn(['codex', 'exec', '-m', model, '-c', `model_reasoning_effort=${effort}`, '-s', 'read-only', '--skip-git-repo-check', '--json', '-o', out, '-C', scratch, ...providerArgs,
        '-c', 'mcp_servers.spatial-engine.command="bun"',
        '-c', `mcp_servers.spatial-engine.args=["${server}"]`,
        '-c', `mcp_servers.spatial-engine.env={VE_MCP_PORT="${port}"}`,
        '-c', 'mcp_servers.spatial-engine.default_tools_approval_mode="approve"',
        '-c', 'approval_policy="never"',
        `${system}\n\nRequest: ${request}`], { stdout: 'pipe', stderr: 'inherit' });
      const raw = await new Response(codex.stdout).text();
      await codex.exited;
      writeFileSync(path.join(runDir, 'codex-events.jsonl'), raw);
      metrics = summarizeCodexEvents(raw);
      for (const line of raw.split('\n')) {
        if (!line.trim()) continue;
        try {
          const item = JSON.parse(line).item;
          if (!item || item.status === 'in_progress') continue;
          if (item.type === 'mcp_tool_call') calls.push({ tool: item.tool, input: item.arguments });
          else if (item.type === 'command_execution') calls.push({ tool: 'SHELL (not allowed)', input: item.command });
        } catch { /* not a JSON line */ }
      }
      try { answer = readFileSync(out, 'utf8'); } catch { /* no final message */ }
      console.log(`codex events: ${path.join(runDir, 'codex-events.jsonl')}`);
    }
    metrics.seconds = Math.round((performance.now() - started) / 100) / 10;
    console.log(`metrics: calls ${metrics.toolCalls}, uncached input ${metrics.uncachedInputTokens ?? 'n/a'}, output ${metrics.outputTokens ?? 'n/a'}, ${metrics.seconds} s`);
    console.log(`\nmodel says: ${answer.trim()}`);
    console.log(`\n${calls.length} tool calls: ${calls.map((c) => c.tool).join(', ')}`);
    const result = await page.evaluate(() => ({ text: window.__ve.doc.text().text, revision: window.__ve.doc.state().revision,
      // Only a failure that still applies: the last one is kept after a later edit succeeds.
      failure: window.__ve.evaluation.state.failureCurrent ? window.__ve.evaluation.state.lastFailure : null, warnings: window.__ve.evaluation.state.warnings }));
    writeFileSync(path.join(root, 'web/dist-bevy-gpu/worlds', `${name}.txt`), result.text);
    writeFileSync(path.join(root, 'web/dist-bevy-gpu/worlds', `${name}.json`), JSON.stringify({ request, agent, model, calls, answer, metrics, warnings: result.warnings, failure: result.failure }, null, 1));
    console.log(`\nworld saved: worlds/${name}.txt (revision ${result.revision}, ${result.text.split('\n').length} lines)`);
    if (result.failure) console.log('last evaluation failure:', JSON.stringify(result.failure));
    if (result.warnings?.length) console.log('warnings:', JSON.stringify(result.warnings).slice(0, 600));
    return { name, ...metrics };
  } finally { await page.close(); }
}
try {
  const runs = [];
  for (let i = 0; i < repeat; i++) runs.push(await runOnce(repeating ? `${name}_${i + 1}` : name, port + i));
  if (repeating) {
    console.log(formatTable(runs));
    console.log(JSON.stringify(aggregate(runs), null, 2));
  }
} finally {
  await browser.close();
}
