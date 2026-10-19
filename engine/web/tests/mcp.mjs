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
  for (const want of ['get_guide', 'list_operators', 'texture_nodes', 'get_text', 'get_nodes', 'get_status', 'apply_text', 'apply_commands', 'undo', 'redo', 'set_view']) {
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
  const textureMatches = await mcp.tool('texture_nodes', { query: 'noise', limit: 5 });
  assert(!textureMatches.isError && textureMatches.data.length > 0 && textureMatches.data.every(node => typeof node === 'string'), 'MCP searches the browser texture catalog');
  const textureName = textureMatches.data[0].match(/^[^(]+/)[0];
  const textureDetails = await mcp.tool('texture_nodes', { node: textureName });
  assert(!textureDetails.isError && textureDetails.data.includes(textureName), 'MCP describes a selected texture node');
  const guide = await mcp.tool('get_guide');
  for (const kit of ['desk', 'shelf', 'window']) {
    assert.match(guide.data, new RegExp(`group ${kit}\\b`), `guide includes the canonical ${kit} group source`);
    assert.match(guide.data, new RegExp(`item = ${kit}\\(\\)`), `guide explains how to instantiate ${kit}`);
  }
  const ops = await mcp.tool('list_operators');
  assert(ops.data.length >= 15 && ops.data.some(o => o.id === 'check.min_wall@1') && ops.data.some(o => o.id === 'solid.sweep@1') && ops.data.some(o => o.id === 'solid.loft@1') && ops.data.every(o => o.readiness === 'ready'), 'ready operators listed');
  for (const kit of ['desk', 'shelf', 'window']) {
    assert(ops.data.some(o => o.id === `kit.${kit}` && o.category === 'kit' && o.source.includes(`group ${kit}`)),
      `list_operators includes the source and usage for the ${kit} kit`);
  }

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

  // Groups: a model defines a wheel once and uses it four times; the document evaluates, prints
  // back as the same text, and a change to the definition is one edit for all four.
  const wheel = ['group wheel',
    '  tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)',
    '  hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)',
    '  spoke = solid.box@1(size=[0.22, 0.03, 0.03])',
    '  spokes = xform.radial@1(spoke, count=4, radius=0.14)',
    '  rim = solid.union@1(tyre, hub, spokes)',
    '  expose radius = tyre.major', '  result rim', 'end',
    ...[0, 1, 2, 3].flatMap(i => [`w${i} = wheel(${i === 2 ? 'radius=0.35' : ''})`, `m${i} = xform.move@1(w${i}, by=[${i}, 0, 0])`, `output m${i}`])].join('\n') + '\n';
  const grouped = await mcp.tool('apply_text', { text: wheel, label: 'a wheel, used four times' });
  assert.equal(grouped.data.applied, true, JSON.stringify(grouped.data).slice(0, 400));
  assert.match(grouped.data.text, /group wheel\n/);
  assert.match(grouped.data.text, /w2 = wheel\(radius=0\.35\)/);
  const shown = (await mcp.tool('get_text')).data.text;
  assert.match(shown, /expose radius = tyre\.major/);
  const members = (await mcp.tool('get_nodes')).data.nodes.filter(n => n.group);
  assert.equal(members.length, 5, 'the wheel has five members');
  const spokeNode = members.find(n => n.label === 'spoke');
  const thicker = await mcp.tool('apply_commands', { commands: [{ set_param: { node: spokeNode.id, param: 'size', value: [0.22, 0.03, 0.05] } }], label: 'thicker spokes' });
  assert.equal(thicker.data.applied, true, JSON.stringify(thicker.data).slice(0, 300));
  const empty2 = await mcp.tool('apply_text', { text: 'x = nowhere()\n' });
  assert.equal(empty2.data.applied, false);
  assert.equal(empty2.data.diagnostics[0].code, 'parse.unknown_group');
  console.log('MCP groups: a wheel used four times');
  const curved = await mcp.tool('apply_text', { text: [
    'path = curve.bezier@1(points=[[0,0,0],[0,0.4,0],[0.4,0.6,0],[0.4,1,0]], steps=24)',
    'section = profile.circle@1(radius=0.025, segments=24)',
    'pipe = solid.sweep@1(path, section)',
    'output pipe',
  ].join('\n') + '\n', label: 'a curved pipe' });
  assert.equal(curved.data.applied, true, JSON.stringify(curved.data).slice(0, 300));
  assert.equal(curved.data.failure, null);
  const curvedNodes = (await mcp.tool('get_nodes')).data.nodes;
  const curveIds = curvedNodes.map((node) => node.id);
  const sectionNode = curvedNodes.find((node) => node.label === 'section');
  const pipeEdit = await mcp.tool('apply_commands', { commands: [
    { set_param: { node: sectionNode.id, param: 'radius', value: 0.03 } },
  ], label: 'widen the pipe by stable node ID' });
  assert.equal(pipeEdit.data.applied, true, JSON.stringify(pipeEdit.data).slice(0, 300));
  assert.deepEqual((await mcp.tool('get_nodes')).data.nodes.map((node) => node.id), curveIds,
    'the follow-up pipe edit preserves every authored ID');
  assert.equal(pipeEdit.data.failure, null);
  console.log('MCP built a swept curve');
  const lofted = await mcp.tool('apply_text', { text: [
    'loft_bottom = profile.circle@1(radius=0.2, segments=24)',
    'loft_waist = profile.circle@1(radius=0.1, segments=24)',
    'loft_body = solid.loft@1(loft_bottom, loft_waist, spacing=0.2)',
    'output loft_body',
  ].join('\n') + '\n', label: 'a lofted body' });
  assert.equal(lofted.data.applied, true, JSON.stringify(lofted.data).slice(0, 300));
  assert.equal(lofted.data.failure, null);
  const loftNodes = (await mcp.tool('get_nodes')).data.nodes;
  const loftIds = loftNodes.map((node) => node.id);
  const loftOperator = loftNodes.find((node) => node.label === 'loft_body');
  const loftEdit = await mcp.tool('apply_commands', { commands: [
    { set_param: { node: loftOperator.id, param: 'spacing', value: 0.25 } },
  ], label: 'raise the loft by stable node ID' });
  assert.equal(loftEdit.data.applied, true, JSON.stringify(loftEdit.data).slice(0, 300));
  assert.deepEqual((await mcp.tool('get_nodes')).data.nodes.map((node) => node.id), loftIds,
    'the follow-up loft edit preserves every authored ID');
  assert.equal(loftEdit.data.failure, null);
  console.log('MCP built a loft');
  const closed = await mcp.tool('apply_text', { text: [
    'ring_path = curve.circle@1(center=[2,0,0], radius=1, segments=24)',
    'ring_solid = solid.sweep@1(ring_path, section)',
    'output ring_solid',
  ].join('\n') + '\n', label: 'a closed ring sweep' });
  assert.equal(closed.data.applied, true, JSON.stringify(closed.data).slice(0, 300));
  assert.equal(closed.data.failure, null);
  console.log('MCP built a closed circular sweep');
  // G6: polygon editing through the same tools. A box gains a raised top panel, then the raise is
  // edited by stable node ID; an impossible inset is refused with the operator's own code and leaves
  // the document unchanged.
  const polygon = await mcp.tool('apply_text', { text: [
    'p_box = solid.box@1(size=[0.3, 0.3, 0.3])',
    'p_triangles = poly.from_solid@1(p_box)',
    'p_flat = poly.merge_coplanar@1(p_triangles)',
    'p_top = poly.select@1(p_flat, by="normal", direction=[0, 1, 0], angle=0.05, expect="one")',
    'p_raised = poly.extrude_faces@1(p_flat, p_top, offset=[0, 0.05, 0])',
    'p_solid = poly.to_solid@1(p_raised)',
    'output p_solid',
  ].join('\n') + '\n', label: 'a box with a raised top' });
  assert.equal(polygon.data.applied, true, JSON.stringify(polygon.data).slice(0, 300));
  assert.equal(polygon.data.failure, null);
  const polygonNodes = (await mcp.tool('get_nodes')).data.nodes;
  const polygonIds = polygonNodes.map((node) => node.id);
  const raised = polygonNodes.find((node) => node.label === 'p_raised');
  const raiseEdit = await mcp.tool('apply_commands', { commands: [
    { set_param: { node: raised.id, param: 'offset', value: [0, 0.08, 0] } },
  ], label: 'raise the top further by stable node ID' });
  assert.equal(raiseEdit.data.applied, true, JSON.stringify(raiseEdit.data).slice(0, 300));
  assert.equal(raiseEdit.data.failure, null);
  assert.deepEqual((await mcp.tool('get_nodes')).data.nodes.map((node) => node.id), polygonIds,
    'the follow-up polygon edit preserves every authored ID');
  const impossible = await mcp.tool('apply_text', { text: [
    'q_grid = poly.grid@1(width=1, height=1, columns=1, rows=1)',
    'q_all = poly.select@1(q_grid)',
    'q_bad = poly.inset_faces@1(q_grid, q_all, distance=0.9)',
    'output q_bad',
  ].join('\n') + '\n', label: 'an inset that is too large' });
  assert.equal(impossible.data.applied, false, JSON.stringify(impossible.data).slice(0, 300));
  assert.equal(impossible.data.failure.code, 'poly.inset_too_large');
  assert.deepEqual((await mcp.tool('get_nodes')).data.nodes.map((node) => node.id), polygonIds,
    'the refused inset left the document unchanged');
  const subdivided = await mcp.tool('apply_text', { text: [
    's_box = solid.box@1(size=[0.2, 0.2, 0.2])',
    's_cage = poly.from_solid@1(s_box)',
    's_smooth = poly.subdivide@1(s_cage, levels=1)',
    'output s_smooth',
  ].join('\n') + '\n', label: 'a subdivided box' });
  assert.equal(subdivided.data.applied, true, JSON.stringify(subdivided.data).slice(0, 300));
  assert.equal(subdivided.data.failure, null);
  console.log('MCP polygon editing and subdivision: built, edited by ID, and refusal preserved');
  assert.deepEqual(logs, [], 'no page errors');
  console.log('MCP acceptance passed');
} finally {
  await browser.close(); pageServer.close(); mcp.close(); lonely.close();
}
