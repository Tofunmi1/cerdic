// The engine's test MCP server end to end: a scripted MCP client (standing in for Claude Code) builds
// the vase through the page, is refused when it breaks the wall, edits by node ID, undoes, and is
// told plainly when no editor page is connected.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
import { PNG } from 'pngjs';
import { driverAdvisory, serve } from './support.mjs';

const server = new URL('../../tools/mcp/server.mjs', import.meta.url).pathname;
const vase = await readFile(new URL('../../crates/ve-modeling/tests/gallery/vase.txt', import.meta.url), 'utf8');

function client(port) {
  const child = Bun.spawn(['bun', server], { stdin: 'pipe', stdout: 'pipe', stderr: 'ignore', env: { ...process.env, VE_MCP_PORT: String(port) } });
  const waiting = new Map();
  let next = 0;
  (async () => {
    let buffer = '';
    for await (const chunk of child.stdout) {
      buffer += new TextDecoder().decode(chunk);
      let at;
      while ((at = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, at); buffer = buffer.slice(at + 1);
        if (!line.trim()) continue;
        const message = JSON.parse(line);
        waiting.get(message.id)?.(message); waiting.delete(message.id);
      }
    }
  })();
  const request = (method, params) => new Promise((resolve) => {
    const id = ++next; waiting.set(id, resolve);
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  });
  return {
    request,
    async tool(name, args = {}) {
      const reply = await request('tools/call', { name, arguments: args });
      const text = reply.result.content[0].text;
      let data = text; try { data = JSON.parse(text); } catch {}
      return { isError: reply.result.isError, data };
    },
    close: () => child.kill(),
  };
}

const lonely = client(8798);
const mcp = client(8799);
const { server: pageServer, port: pagePort } = await serve();
const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const logs = [];
try {
  // protocol basics, and the honest answer when no page is connected
  const init = await lonely.request('initialize', { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'test', version: '0' } });
  assert.equal(init.result.serverInfo.name, 'spatial-engine');
  const listed = await lonely.request('tools/list', {});
  const names = listed.result.tools.map(t => t.name);
  for (const want of ['get_guide', 'list_operators', 'get_text', 'get_nodes', 'get_status', 'apply_text', 'apply_commands', 'undo', 'redo', 'set_view']) {
    assert(names.includes(want), `tool ${want} is listed`);
  }
  const none = await lonely.tool('get_text');
  assert(none.isError && /No editor page is connected/.test(none.data), 'a clear message when no page is connected');
  assert.match((await lonely.tool('get_guide')).data, /output/, 'the guide explains the language');
  console.log('MCP protocol basics ok');

  const page = await browser.newPage({ viewport: { width: 1100, height: 750 } });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text())) logs.push(m.text()); });
  // The engine's own viewer page (panel, field of view, lenses), with evaluation and the MCP bridge on.
  await page.goto(`http://127.0.0.1:${pagePort}/?viewer=1&docname=mcp-${Date.now()}&mcp=8799`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 60000 });
  assert(await page.evaluate(() => !!document.getElementById('viewer-hud') && !!document.getElementById('doc-panel')), 'the viewer panel and the document panel are shown');
  let status;
  for (let i = 0; i < 100 && !(status = await mcp.tool('get_status')).data?.revision && status.isError; i++) await Bun.sleep(100);
  assert(!status.isError, `the page connected: ${JSON.stringify(status.data)}`);

  const empty = await mcp.tool('get_text');
  assert.equal(empty.data.revision, 0);
  const ops = await mcp.tool('list_operators');
  assert(ops.data.length >= 15 && ops.data.some(o => o.id === 'check.min_wall@1') && ops.data.every(o => o.readiness === 'ready'), 'ready operators listed');

  const built = await mcp.tool('apply_text', { text: vase, label: 'make a hollow vase' });
  assert.equal(built.data.applied, true, JSON.stringify(built.data).slice(0, 300));
  assert.equal(built.data.revision, 1);
  assert.match(built.data.text, /check\.min_wall@1\(outer, inner, min=0\.003\)/);
  assert.equal(built.data.failure, null);
  console.log('MCP built the vase');

  const bad = await mcp.tool('apply_text', { text: 'a = solid.revolv@1(x)\n' });
  assert.equal(bad.data.applied, false);
  assert.equal(bad.data.diagnostics[0].code, 'parse.unknown_op');
  assert.match(bad.data.diagnostics[0].suggestion, /solid\.revolve@1/);
  assert.deepEqual(Object.keys(bad.data.diagnostics[0].span).sort(), ['col', 'len', 'line']);
  console.log('MCP got a diagnostic with a span');

  const nodes = (await mcp.tool('get_nodes')).data;
  const byLabel = Object.fromEntries(nodes.nodes.map(n => [n.label, n]));
  assert.equal(nodes.nodes.length, 6);
  const before = (await mcp.tool('get_text')).data;
  const widened = (node, by) => { const pts = structuredClone(node.params.points); pts[pts.length - 2][0] += by; return { set_param: { node: node.id, param: 'points', value: pts } }; };

  const refused = await mcp.tool('apply_commands', { commands: [widened(byLabel.inner, 0.024)], label: 'widen only the inner profile' });
  assert.equal(refused.data.applied, false);
  assert.equal(refused.data.failure.code, 'constraint.min_wall');
  assert.match(refused.data.note, /unchanged/);
  const after = (await mcp.tool('get_text')).data;
  assert.equal(after.text, before.text, 'the refused edit changed nothing');
  assert.equal(after.revision, before.revision);
  console.log('MCP edit refused by the wall constraint');

  const ok = await mcp.tool('apply_commands', { commands: [widened(byLabel.outer, 0.02), widened(byLabel.inner, 0.024)], label: 'widen its opening' });
  assert.equal(ok.data.applied, true, JSON.stringify(ok.data).slice(0, 300));
  assert.deepEqual((await mcp.tool('get_nodes')).data.nodes.map(n => n.id), nodes.nodes.map(n => n.id), 'widening keeps every ID');

  const undone = await mcp.tool('undo');
  assert.equal(undone.data.applied, true);
  assert.equal(undone.data.text, before.text, 'undo restores the earlier text');
  assert.equal((await mcp.tool('set_view', { position: [0.5, 0.4, 0.6], target: [0, 0.15, 0] })).data.ok, true);
  console.log('MCP widen, undo and view');

  // Looking at the model in the viewer: frame it, zoom with the wheel, orbit with Alt + drag.
  const frames = async (n = 4) => {
    const start = await page.evaluate(() => window.__ve.read().frames);
    await page.waitForFunction((count) => window.__ve.read().frames > count, start + n);
  };
  const camera = () => page.evaluate(() => window.__ve.read().viewer.position);
  const centre = await page.evaluate(() => { const b = window.__ve.evaluation.bounds; return b.min.map((v, i) => (v + b.max[i]) / 2); });
  const away = (p) => Math.hypot(p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]);
  assert.equal((await mcp.tool('frame_view')).data.ok, true);
  await frames();
  const framed = away(await camera());
  assert(framed > 0.3 && framed < 3, `the model is framed from a sensible distance (${framed.toFixed(2)} m)`);
  await page.mouse.move(640, 400);
  await page.mouse.wheel(0, -500);
  await frames();
  const zoomedIn = away(await camera());
  assert(zoomedIn < framed * 0.75, `the wheel zooms in (${framed.toFixed(2)} to ${zoomedIn.toFixed(2)} m)`);
  await page.mouse.wheel(0, 1500);
  await frames();
  assert(away(await camera()) > zoomedIn * 1.5, 'the wheel zooms back out');
  const orbitFrom = await camera();
  await page.keyboard.down('Alt');
  await page.mouse.move(600, 400);
  await page.mouse.down();
  await page.mouse.move(760, 430, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up('Alt');
  await frames();
  const orbitTo = await camera();
  assert(Math.hypot(orbitTo[0] - orbitFrom[0], orbitTo[2] - orbitFrom[2]) > 0.05, 'Alt + drag orbits sideways');
  assert(Math.abs(away(orbitTo) - away(orbitFrom)) < 0.02 * away(orbitFrom), 'orbiting keeps the distance');
  console.log('viewer zoom, orbit and frame');

  // Colour: a material node on the vase, made the output, turns the rendered part red.
  const share = async (test) => {
    const png = PNG.sync.read(await page.locator('canvas#ve').screenshot());
    let hits = 0;
    for (let p = 0; p < png.data.length; p += 4) if (test(png.data[p], png.data[p + 1], png.data[p + 2])) hits++;
    return hits / (png.width * png.height);
  };
  const reddish = (r, g, b) => r > 90 && r > g * 1.7 && r > b * 1.7;
  assert.equal((await mcp.tool('frame_view')).data.ok, true);
  await frames();
  assert((await share(reddish)) < 0.002, 'the vase is not red before it is given a material');
  const painted = await mcp.tool('apply_text', { text: 'red = solid.material@1(vase, color=[0.85, 0.08, 0.08], roughness=0.3)\noutput vase = red\n', label: 'make it red' });
  assert.equal(painted.data.applied, true, JSON.stringify(painted.data).slice(0, 300));
  assert.match(painted.data.text, /solid\.material@1\(vase, color=\[0\.85, 0\.08, 0\.08\], roughness=0\.3\)/);
  await frames(8);
  assert((await share(reddish)) > 0.02, 'the vase renders red after the material');
  const tooBright = await mcp.tool('apply_text', { text: 'x = solid.material@1(vase, color=[2, 0, 0])\n' });
  assert.equal(tooBright.data.applied, false);
  assert.equal(tooBright.data.diagnostics[0].code, 'param.range');
  console.log('MCP colour: the vase renders red');
  assert.deepEqual(logs, [], 'no page errors');
  console.log('MCP acceptance passed');
} finally {
  await browser.close(); pageServer.close(); mcp.close(); lonely.close();
}
