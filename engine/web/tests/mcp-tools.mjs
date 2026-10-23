// The model tool surface, slice T1 (ADR 0061): dry runs, state deltas on every edit, labels in commands, the scene overview and the
// contact-sheet snapshot. A scripted MCP client builds the vase and checks what a model would see. Written before the bodies.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile } from 'node:fs/promises';
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
    async raw(name, args = {}) { return (await request('tools/call', { name, arguments: args })).result; },
    async tool(name, args = {}) {
      const reply = await request('tools/call', { name, arguments: args });
      const text = reply.result.content.find(c => c.type === 'text').text;
      let data = text; try { data = JSON.parse(text); } catch {}
      return { isError: reply.result.isError, data };
    },
    close: () => child.kill(),
  };
}

const mcp = client(8797);
const { server: pageServer, port: pagePort } = await serve();
const browser = await chromium.launch({ channel: process.env.VE_CHANNEL ?? undefined, args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-unsafe-webgpu', '--enable-features=Metal,WebGPU'] });
const logs = [];
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 750 } });
  const missing = [];
  page.on('response', r => { if (r.status() === 404) missing.push(r.url()); });
  page.on('pageerror', e => logs.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !driverAdvisory.test(m.text()) && !m.location().url.endsWith('/favicon.ico')) logs.push(`${m.text()} [${m.location().url}]`); });
  await page.goto(`http://127.0.0.1:${pagePort}/?viewer=1&docname=tools-${Date.now()}&mcp=8797`);
  await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === 'idle', null, { timeout: 90000 });
  for (let i = 0; i < 100 && (await mcp.tool('get_status')).isError; i++) await Bun.sleep(100);

  const names = (await mcp.request('tools/list', {})).result.tools.map(t => t.name);
  for (const want of ['scene_overview', 'apply_text', 'apply_commands', 'snapshot']) assert(names.includes(want), `tool ${want} is listed`);
  const applyText = (await mcp.request('tools/list', {})).result.tools.find(t => t.name === 'apply_text');
  assert('dry_run' in applyText.inputSchema.properties, 'apply_text advertises dry_run');

  // A dry run prices and checks a program without touching the document.
  const dry = await mcp.tool('apply_text', { text: vase, dry_run: true });
  assert.equal(dry.data.applied, false, 'a dry run applies nothing');
  assert.equal(dry.data.dry_run, true);
  assert.equal(dry.data.would_apply, true, JSON.stringify(dry.data).slice(0, 300));
  assert(dry.data.delta.triangles.after > 0 && dry.data.delta.triangles.before === 0, `the dry run reports what the program would add: ${JSON.stringify(dry.data.delta)}`);
  const untouched = (await mcp.tool('get_text')).data;
  assert.equal(untouched.text.trim(), '', 'the document text is unchanged (revisions only count up: the edit and its undo are both recorded)');
  assert.equal((await mcp.tool('get_nodes')).data.nodes.length, 0);
  const badDry = await mcp.tool('apply_text', { text: 'a = solid.revolv@1(x)\n', dry_run: true });
  assert.equal(badDry.data.would_apply, false);
  assert.equal(badDry.data.diagnostics[0].code, 'parse.unknown_op', 'parse diagnostics come back from a dry run too');
  console.log('dry run ok');

  // A real edit answers with a state delta, so the model never needs a second call.
  const built = await mcp.tool('apply_text', { text: vase, label: 'make a hollow vase' });
  assert.equal(built.data.applied, true, JSON.stringify(built.data).slice(0, 300));
  const delta = built.data.delta;
  assert(delta.revision >= 1, `the new revision is reported: ${delta.revision}`);
  assert.equal(delta.triangles.before, 0);
  assert(delta.triangles.after > 1000, `triangles after: ${delta.triangles.after}`);
  assert.equal(delta.parts.after, 1);
  assert.equal(delta.diagnostics.count, 0);
  assert(typeof delta.next === 'string' && delta.next.length > 0, 'one suggested next step');
  assert(typeof delta.summary === 'string' && /triangles/.test(delta.summary), `a one-line summary: ${delta.summary}`);
  console.log('state delta ok');

  // Labels stand in for ids in commands.
  const nodes = (await mcp.tool('get_nodes')).data.nodes;
  const outer = nodes.find(n => n.label === 'outer');
  const widen = (node, by) => { const pts = structuredClone(outer.params.points); pts[pts.length - 2][0] += by; return { set_param: { node, param: 'points', value: pts } }; };
  const byLabel = await mcp.tool('apply_commands', { commands: [widen('outer', 0.01)], label: 'widen by label' });
  assert.equal(byLabel.data.applied, true, JSON.stringify(byLabel.data).slice(0, 400));
  const moved = (await mcp.tool('get_nodes')).data.nodes.find(n => n.id === outer.id);
  assert.equal(moved.params.points[moved.params.points.length - 2][0], outer.params.points[outer.params.points.length - 2][0] + 0.01, 'the label reached the right node');
  await mcp.tool('apply_commands', { commands: [{ set_label: { node: 'body', label: 'shape' } }, { set_label: { node: 'cavity', label: 'shape' } }] });
  const ambiguous = await mcp.tool('apply_commands', { commands: [widen('shape', 0.01)] });
  assert.equal(ambiguous.data.applied, false);
  assert.match(JSON.stringify(ambiguous.data), /shape/, 'the refusal names the label');
  assert.match(JSON.stringify(ambiguous.data), /n_/, 'and lists the ids it could mean');
  console.log('labels ok');

  // The overview answers "what is in this scene and what does it cost" in one short call.
  const overview = await mcp.tool('scene_overview');
  assert.equal(overview.isError, false, JSON.stringify(overview.data).slice(0, 300));
  assert.equal(overview.data.parts.length, 1);
  assert(overview.data.totals.triangles > 1000 && overview.data.totals.parts === 1);
  assert(Array.isArray(overview.data.bounds.min) && Array.isArray(overview.data.bounds.max) && overview.data.bounds.size.length === 3);
  assert(typeof overview.data.budget === 'object' && typeof overview.data.summary === 'string', 'a budget section and a one-line summary');
  assert(JSON.stringify(overview.data).length < 3000, 'compact by default');
  console.log('overview ok');

  // An instance set counts once per copy, in triangles and in extent, or the budget would lie about every dense scene.
  const sets = await mcp.tool('apply_text', { text: 'ball = solid.sphere@1(radius=0.1, segments=8)\nfield = inst.array@1(ball, count_x=4, count_y=1, count_z=5, spacing=[0.5, 0.5, 0.5])\noutput field = field\n' });
  assert.equal(sets.data.applied, true, JSON.stringify(sets.data).slice(0, 300));
  const withSet = (await mcp.tool('scene_overview')).data;
  const field = withSet.parts.find(p => p.name === 'field');
  assert.equal(field.copies, 20, JSON.stringify(field));
  assert.equal(field.triangles, field.copies * field.mesh_triangles, 'drawn triangles are the mesh times the copies');
  assert(field.size[0] > 1.4 && field.size[2] > 1.9, `the set's size covers every copy: ${field.size}`);
  assert.equal(withSet.totals.triangles, withSet.parts.reduce((sum, p) => sum + p.triangles, 0), 'the totals add the drawn triangles');
  assert.equal(sets.data.delta.triangles.after - sets.data.delta.triangles.before, field.triangles, 'and so does the edit delta');

  // A contact sheet: several named views in one picture.
  const sheet = await mcp.raw('snapshot', { views: ['front', 'side', 'top', 'iso'] });
  const images = sheet.content.filter(c => c.type === 'image');
  assert.equal(images.length, 1, 'four views come back as one picture');
  const meta = JSON.parse(sheet.content.find(c => c.type === 'text').text);
  assert.deepEqual(meta.views, ['front', 'side', 'top', 'iso']);
  assert(meta.width > 0 && meta.height > 0);
  const single = await mcp.raw('snapshot', { image: false });
  assert.equal(single.content.filter(c => c.type === 'image').length, 0, 'the plain snapshot still works');
  console.log('contact sheet ok');

  // find: nodes by words, operator family or label, answered as short lines with what feeds and uses each.
  const byWord = await mcp.tool('find', { query: 'ball' });
  assert(!byWord.isError, JSON.stringify(byWord.data));
  assert(byWord.data.matches.some(m => m.label === 'ball' && m.op.startsWith('solid.sphere')), JSON.stringify(byWord.data));
  assert(byWord.data.matches.every(m => typeof m.line === 'string' && m.line.includes(m.label)), 'every match has a one-line form naming it');
  const byFamily = await mcp.tool('find', { family: 'inst' });
  assert(byFamily.data.matches.length >= 1 && byFamily.data.matches.every(m => m.op.startsWith('inst.')), JSON.stringify(byFamily.data));
  const one = await mcp.tool('find', { node: 'field' });
  assert.equal(one.data.matches.length, 1);
  assert.deepEqual(one.data.matches[0].inputs.map(i => i.label), ['ball'], 'what feeds it, by label');
  assert.equal(one.data.matches[0].output, 'field', 'the scene output it is');
  const ball = await mcp.tool('find', { node: 'ball' });
  assert.deepEqual(ball.data.matches[0].used_by.map(u => u.label), ['field'], 'what uses it, by label');
  const none = await mcp.tool('find', { query: 'zebra' });
  assert.equal(none.data.matches.length, 0);
  assert(typeof none.data.hint === 'string' && none.data.hint.length > 0, 'an empty answer says what to try');
  console.log('find ok');

  // The game as nodes: describe it, play it with bots, and learn what never happened.
  const noGame = await mcp.tool('game_describe');
  assert.equal(noGame.data.present, false);
  assert(/game\.collect/.test(noGame.data.hint), 'with no game it says how to build one');
  const coins = [
    'score = game.var@1(name="score", value=0)',
    'time = game.var@1(name="time", value=0)',
    'hero = game.kind@1(name="player", size=0.5, speed=4, tags=["hero"])',
    'coin = game.kind@1(name="coin", size=0.4, tags=["pickup"])',
    'p0 = game.spawn@1(kind="player", at=[0, 0, 0])',
    'c0 = game.spawn@1(kind="coin", at=[3, 0, 2], count=8, spread=6)',
    'move = game.input@1(name="move", axis="x,z", target="tag:hero")',
    'pick = game.rule@1(on="touch", a="tag:hero", b="tag:pickup", do="add:score:1;destroy:b")',
    'tick = game.rule@1(on="tick", every=1.0, do="add:time:1")',
    'win = game.goal@1(outcome="win", when="score>=8")',
    'late = game.goal@1(outcome="lose", when="time>=60")',
    'all = game.collect@1(score, time, hero, coin, p0, c0, move, pick, tick, win, late, name="coins")',
    'output rules = all',
  ].join('\n');
  const made = await mcp.tool('apply_text', { text: coins, label: 'a coin game' });
  assert(!made.isError, JSON.stringify(made.data));
  const described = await mcp.tool('game_describe');
  assert.equal(described.data.present, true, JSON.stringify(described.data));
  assert(/coins/.test(described.data.summary) && /rule on=touch/.test(described.data.text), JSON.stringify(described.data));
  const won = await mcp.tool('playtest', { policy: 'seek:hero:pickup', seed: 3 });
  assert.equal(won.data.status, 'won', JSON.stringify(won.data));
  assert.equal(won.data.vars.score, 8);
  const lost = await mcp.tool('playtest', { policy: 'idle', seed: 3 });
  assert.equal(lost.data.status, 'lost', JSON.stringify(lost.data));
  const again = await mcp.tool('playtest', { policy: 'idle', seed: 3 });
  assert.deepEqual(again.data, lost.data, 'the same game, seed and policy give the same report');
  const badRule = await mcp.tool('apply_text', { text: coins.replace('do="add:time:1"', 'do="explode:time"').replace('output rules = all', 'output rules2 = all'), dry_run: true });
  assert(JSON.stringify(badRule.data).includes('tick'), `a bad rule is reported against the node that wrote it: ${JSON.stringify(badRule.data).slice(0, 400)}`);
  console.log('game tools ok');

  // check: findings with a stable id, where, and a fix, so a model repairs what it was told instead of guessing.
  const clean = await mcp.tool('check');
  assert(!clean.isError, JSON.stringify(clean.data));
  assert(Array.isArray(clean.data.findings) && typeof clean.data.ok === 'boolean' && typeof clean.data.summary === 'string', JSON.stringify(clean.data));
  assert(!clean.data.findings.some(f => f.severity === 'error'), `the coin game has no errors: ${JSON.stringify(clean.data.findings)}`);
  const dangling = await mcp.tool('apply_commands', { commands: [{ set_param: { node: 'hero', param: 'part', value: 'ghost' } }] });
  assert(!dangling.isError, JSON.stringify(dangling.data));
  const broken = await mcp.tool('check');
  const missingPart = broken.data.findings.find(f => f.id === 'game.part_missing');
  assert(missingPart, JSON.stringify(broken.data));
  assert.equal(missingPart.node, 'hero', 'names the node to change');
  assert.equal(broken.data.ok, false);
  assert(/ghost/.test(missingPart.message) && typeof missingPart.fix === 'string' && missingPart.fix.length > 10, JSON.stringify(missingPart));
  assert(missingPart.fix.includes('field') || missingPart.fix.includes('output'), `the fix names real parts or says how to make one: ${missingPart.fix}`);
  await mcp.tool('apply_commands', { commands: [{ set_param: { node: 'hero', param: 'part', value: '' } }] });
  // A sound that does not exist is found here, not discovered as silence in the headset.
  await mcp.tool('apply_commands', { commands: [{ set_param: { node: 'pick', param: 'do', value: 'add:score:1;destroy:b;effect:sfx:zork@b' } }] });
  const silent = await mcp.tool('check');
  const unknownSound = silent.data.findings.find(f => f.id === 'game.sfx_unknown');
  assert(unknownSound, JSON.stringify(silent.data));
  assert.equal(unknownSound.node, 'pick');
  assert(/zork/.test(unknownSound.message) && /coin/.test(unknownSound.fix), `the fix lists real sounds: ${unknownSound.fix}`);
  await mcp.tool('apply_commands', { commands: [{ set_param: { node: 'pick', param: 'do', value: 'add:score:1;destroy:b;effect:sfx:coin@b' } }] });
  assert(!(await mcp.tool('check')).data.findings.some(f => f.id === 'game.sfx_unknown'), 'a real sound is not flagged');
  console.log('check ok');

  // A texture program that cannot be baked is reported by name, with a fix, instead of only in the browser console; the other textures still bake.
  const badTexture = await mcp.tool('apply_text', { text: 'slab = solid.box@1(size=[1, 1, 1])\nslab_m = solid.material@1(slab, color=[0.5, 0.5, 0.5])\nslab_t = solid.texture@1(slab_m, program="albedo = no_such_node()", tile=1)\noutput slab = slab_t\n', label: 'a slab with a bad texture' });
  assert(!badTexture.isError, JSON.stringify(badTexture.data));
  let bakeFinding;
  for (let i = 0; i < 80 && !bakeFinding; i++) {
    bakeFinding = (await mcp.tool('check')).data.findings.find(f => f.id === 'texture.bake_failed');
    if (!bakeFinding) await Bun.sleep(250);
  }
  assert(bakeFinding, 'a texture program that fails to bake shows up in check');
  assert.equal(bakeFinding.node, 'slab_t', 'by the label of the node that wrote it');
  assert(/no_such_node/.test(bakeFinding.message), `the message says what failed: ${bakeFinding.message}`);
  assert(/texture_nodes/.test(bakeFinding.fix), `and the fix points at the catalog: ${bakeFinding.fix}`);
  console.log('texture errors ok');

  assert.deepEqual(logs, [], `no page errors: ${logs.join('; ')} (404s: ${missing.join(', ')})`);
  console.log('mcp tools slice T1 ok');
} finally {
  await browser.close();
  mcp.close();
  pageServer.close();
}
