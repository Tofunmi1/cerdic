// Runs a set of prose requests through a real model (the Claude CLI over the engine's MCP server, no API
// key) and checks each result by measurement: did the document evaluate, were the expected operators
// used, are the size and volume what the request implies. Needs a built web/dist.
//
//   bun engine/tools/mcp/eval-prompts.mjs [--set blender-tools] [--model sonnet] [--parallel 3] [--only name,name]
//
// Writes target/reports/mcp-eval/<set>.json and prints a table. Uses your Claude Code login; each request
// costs a few cents. The prompt files live in tools/mcp/prompts/.
import { chromium } from 'playwright';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { serve } from '../../web/tests/support.mjs';
import { compileTexture } from '../../../src/engine/texture/compile.ts';
import { loadHelpers, loadLibrary } from '../../../src/engine/texture/library.ts';
import { parseTextureProgram } from '../../../src/engine/texture/program.ts';

const materialMaker = path.resolve(import.meta.dir, '../../lib/material-maker');
const textureLibrary = loadLibrary(materialMaker);
const textureHelpers = loadHelpers(materialMaker);

const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(name); if (i < 0) return fallback; const [, value] = args.splice(i, 2); return value; };
const setName = flag('--set', 'blender-tools');
const model = flag('--model', 'sonnet');
const parallel = Number(flag('--parallel', '3'));
const only = flag('--only', '')?.split(',').filter(Boolean) ?? [];
const server = path.resolve(import.meta.dir, 'server.mjs');
const prompts = JSON.parse(readFileSync(path.resolve(import.meta.dir, 'prompts', `${setName}.json`), 'utf8'))
  .filter(p => only.length === 0 || only.includes(p.name));
const reports = path.resolve(import.meta.dir, '../../target/reports/mcp-eval');
mkdirSync(reports, { recursive: true });

const SYSTEM = 'You build 3D objects by calling the spatial-engine tools. Call get_guide and list_operators first. Keep going until the request is done, check the result with measure, call frame_view so the person can see it, then say in one or two sentences what you built.';

/**
 * What a run cost and how the tools behaved, from the CLI's event stream: the final answer, turns, dollars and tokens,
 * how often each tool was called, how many calls failed, and how many bytes each tool put into the conversation.
 * The tools' own bytes are the part we control: they are what the model has to read.
 */
function measureRun(stream) {
  const calls = {}, bytes = {}, names = new Map();
  let errors = 0, answer = '', result = {};
  for (const line of stream.split('\n')) {
    let event;
    try { event = JSON.parse(line); } catch { continue; }
    if (event.type === 'assistant') {
      for (const block of event.message?.content ?? []) if (block.type === 'tool_use') {
        const name = String(block.name).replace('mcp__spatial-engine__', '');
        calls[name] = (calls[name] ?? 0) + 1;
        names.set(block.id, name);
      }
    } else if (event.type === 'user') {
      for (const block of event.message?.content ?? []) if (block.type === 'tool_result') {
        const text = typeof block.content === 'string' ? block.content : JSON.stringify(block.content ?? '');
        const name = names.get(block.tool_use_id) ?? 'unknown';
        bytes[name] = (bytes[name] ?? 0) + text.length;
        if (block.is_error || /"isError":\s*true|^error|diagnostics/i.test(text.slice(0, 200))) errors++;
      }
    } else if (event.type === 'result') {
      result = event;
      answer = String(event.result ?? '');
    }
  }
  const usage = result.usage ?? {};
  return {
    answer: answer.trim(),
    metrics: {
      turns: result.num_turns ?? null,
      costUsd: result.total_cost_usd ?? null,
      outputTokens: usage.output_tokens ?? null,
      inputTokens: (usage.input_tokens ?? 0) + (usage.cache_read_input_tokens ?? 0) + (usage.cache_creation_input_tokens ?? 0),
      toolCalls: calls,
      toolCallCount: Object.values(calls).reduce((a, b) => a + b, 0),
      toolErrors: errors,
      toolResultBytes: bytes,
    },
  };
}

async function runOne(spec, index, browser) {
  const port = 8800 + index;
  const dir = mkdtempSync(path.join(tmpdir(), 've-eval-'));
  const config = path.join(dir, 'mcp.json');
  writeFileSync(config, JSON.stringify({ mcpServers: { 'spatial-engine': { command: 'bun', args: [server], env: { VE_MCP_PORT: String(port) } } } }));
  const { server: pageServer, port: pagePort } = await serve();
  const started = Date.now();
  const outcome = { name: spec.name, prompt: spec.prompt, checks: {}, ok: false };
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    await page.goto(`http://127.0.0.1:${pagePort}/?viewer=1&docname=eval-${spec.name}-${Date.now()}&mcp=${port}`);
    await page.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation, null, { timeout: 90000 });
    const claude = Bun.spawn(['claude', '-p', '--model', model, '--mcp-config', config, '--strict-mcp-config',
      '--allowedTools', 'mcp__spatial-engine__*', '--tools', '', '--setting-sources', '', '--no-session-persistence',
      '--output-format', 'stream-json', '--verbose',
      '--system-prompt', SYSTEM, spec.prompt], { stdout: 'pipe', stderr: 'pipe' });
    const timer = setTimeout(() => claude.kill(), 600000);
    const stream = await new Response(claude.stdout).text();
    await claude.exited;
    clearTimeout(timer);
    Object.assign(outcome, measureRun(stream));
    const seen = await page.evaluate(() => {
      const e = window.__ve.evaluation;
      return {
        text: window.__ve.doc.text().text,
        failureCurrent: e.state.failureCurrent, failure: e.state.lastFailure,
        bounds: e.bounds, measures: e.measures,
        tracks: e.parts.flatMap(part => part.animation ?? []),
        behaviors: e.parts.flatMap(part => part.behavior ? [{ ...part.behavior, part: part.name, tracks: (part.animation ?? []).length }] : []),
        iks: e.parts.filter(part => part.ik).map(part => ({ bones: part.ik.bones.length, tracks: (part.animation ?? []).length, trigger: part.behavior?.trigger ?? null })),
        sets: e.parts.filter(part => part.transforms?.length).map(part => part.transforms.length / 16),
        skins: e.parts.filter(part => part.skin).map(part => ({ name: part.name, bones: part.skin.bones, blend: part.skin.blend ?? 'linear', stamps: (part.skin.paint ?? []).length })),
        bodies: e.parts.filter(part => part.body).map(part => ({ name: part.name, ...part.body })),
        bones: e.parts.filter(part => part.bone).map(part => ({ name: part.name, moves: (part.animation ?? []).length > 0 || Boolean(part.parent) })),
        links: e.parts.map(part => ({ name: part.name, parent: part.parent ?? null, tracks: (part.animation ?? []).length })),
        materials: e.parts.map(part => ({ name: part.name, material: part.material ?? null })),
        projection: window.__ve.projection(),
        warnings: (e.state.warnings ?? []).map(w => w.code ?? String(w)),
      };
    });
    outcome.text = seen.text;
    const { checks } = outcome;
    checks.evaluates = !seen.failureCurrent && seen.measures.length > 0;
    if (spec.materialOpacity) {
      const matching = seen.materials.filter(item => new RegExp(spec.materialOpacity.part, 'i').test(item.name));
      outcome.materialOpacity = matching.map(item => ({ name: item.name, opacity: item.material?.opacity ?? 1 }));
      checks.materialOpacity = matching.length > 0 && matching.some(item => (item.material?.opacity ?? 1) < spec.materialOpacity.max);
    }
    if (spec.water) {
      const matching = seen.materials.filter(item => new RegExp(spec.water.part, 'i').test(item.name));
      outcome.water = matching.map(item => ({ name: item.name, opacity: item.material?.opacity ?? 1 }));
      checks.water = seen.text.includes('water.surface@') && matching.length > 0
        && matching.some(item => (item.material?.opacity ?? 1) < (spec.water.maxOpacity ?? 1));
    }
    if (spec.sky) {
      const sky = seen.projection.live_environment?.sky;
      outcome.sky = sky ? { clouds: sky.clouds, turbidity: sky.turbidity, sun: seen.projection.live_lights > 0 } : null;
      checks.sky = seen.text.includes('scene.sky@') && Boolean(sky)
        && seen.projection.live_lights > 0
        && (spec.sky.clouds === undefined || sky.clouds === spec.sky.clouds);
    }
    // An entry may list alternatives ("a|b"): an older operator that does the job equally well counts.
    checks.uses = (spec.uses ?? []).every(op => op.split('|').some(name => seen.text.includes(`${name}@`)));
    outcome.usedBlender = /poly\.(bevel_network|inset_region|extrude_region|solidify|poke|wireframe|subdivide_edges|dissolve_limit|symmetrize|unwrap_uv|decimate|unsubdivide)@/.test(seen.text);
    if (seen.bounds && spec.size) {
      const size = seen.bounds.max.map((v, k) => v - seen.bounds.min[k]);
      // The model may orient the object differently; compare sorted extents.
      const sorted = a => [...a].sort((x, y) => x - y);
      const want = sorted(spec.size), got = sorted(size);
      outcome.size = size.map(v => Math.round(v * 1000) / 1000);
      checks.size = want.every((v, k) => Math.abs(v - got[k]) <= (spec.sizeTolerance ?? 0.05));
    } else checks.size = !spec.size;
    const total = seen.measures.reduce((sum, m) => sum + m.volume, 0);
    outcome.volume = Math.round(total * 1e4) / 1e4;
    checks.volume = !spec.volume || (total >= spec.volume[0] && total <= spec.volume[1]);
    const triangles = seen.measures.reduce((sum, m) => sum + m.triangles, 0);
    outcome.triangles = triangles;
    checks.triangles = !spec.triangles || (triangles >= spec.triangles[0] && triangles <= spec.triangles[1]);
    if (spec.animation) {
      // The evaluated parts must carry tracks of the properties the request implies, over a clip of about the
      // length asked for, looping (or holding) as asked, and a rotation hinged at an edge when that was the point.
      const { tracks } = seen;
      const wanted = spec.animation;
      const longest = Math.max(0, ...tracks.map(t => t.keys[t.keys.length - 1].time));
      outcome.animation = { tracks: tracks.length, properties: [...new Set(tracks.map(t => t.property))], longest, looped: tracks.map(t => t.looped) };
      checks.animation = tracks.length > 0
        && (wanted.properties ?? []).every(p => tracks.some(t => t.property === p))
        && longest >= (wanted.minDuration ?? 0) && longest <= (wanted.maxDuration ?? Infinity)
        && (wanted.loop === undefined || tracks.every(t => t.looped === wanted.loop))
        && (!wanted.pivotNearEdge || tracks.some(t => t.property === 'rotation' && Math.abs(Math.abs(t.pivot[0]) - 0.45) < 0.06));
    }
    if (spec.logic) {
      // The flag logic must have the shape the request implies, and must be reachable: every flag that is needed
      // (to be on) or watched is set by some behaviour, so nothing waits forever.
      const all = seen.behaviors, want = spec.logic;
      const setters = new Set(all.flatMap(b => b.sets.map(s => s.name)));
      const counts = { sets: all.filter(b => b.sets.length).length, needs: all.filter(b => b.needs.length).length, watch: all.filter(b => b.watch).length };
      const reachable = all.every(b => b.needs.every(n => !n.value || setters.has(n.name)) && (!b.watch || setters.has(b.watch)));
      outcome.logic = { ...counts, reachable, flags: [...setters], triggers: [...new Set(all.map(b => b.trigger))] };
      checks.logic = (want.triggers ?? []).every(t => all.some(b => b.trigger === t)) && counts.sets >= (want.sets ?? 0) && counts.needs >= (want.needs ?? 0) && counts.watch >= (want.watch ?? 0) && reachable
        && (want.properties ?? []).every(p => seen.tracks.some(t => t.property === p));
    }
    if (spec.ik) {
      // Every chain must have survived the evaluator's checks (no rig.ik warnings), be long enough, and its target must
      // move (it has tracks) or be carried (a grab trigger), or the arm would never reach anywhere.
      const want = spec.ik;
      const targets = seen.iks;
      const moves = targets.some(t => t.tracks > 0), carried = targets.some(t => ['grab', 'hold'].includes(t.trigger));
      outcome.ik = { chains: targets.length, longest: Math.max(0, ...targets.map(t => t.bones)), moves, carried, dropped: seen.warnings.filter(c => c.startsWith('rig.ik')) };
      checks.ik = targets.length >= (want.chains ?? 1) && outcome.ik.longest >= (want.minBones ?? 2) && outcome.ik.dropped.length === 0
        && (!want.targetMoves || moves) && (!want.targetGrabbable || carried);
    }
    if (spec.field) {
      // A placed set must carry a track with a phase, so the copies are out of step, and it must have the copies asked for.
      const sets = seen.sets, phased = seen.tracks.some(t => (t.phase_step ?? 0) !== 0 || (t.phase_jitter ?? 0) > 0);
      outcome.field = { copies: sets, phased };
      checks.field = (!spec.field.phased || phased) && sets.some(count => count >= (spec.field.copies ?? 2));
    }
    if (spec.clips) {
      // The parts must have named clips, senders must name a clip on their target, and a sequence must chain.
      const want = spec.clips;
      const tracks = seen.tracks;
      const names = [...new Set(tracks.map(t => t.clip || ''))].filter(Boolean);
      const withClip = seen.behaviors.filter(b => b.target && b.target_clip).length;
      const chain = Math.max(0, ...seen.behaviors.map(b => (b.then?.length ?? 0) + (b.then?.length ? 1 : 0)));
      outcome.clips = { names, targetsWithClip: withClip, longestSequence: chain };
      checks.clips = (want.names ?? []).every(n => n.split('|').length > 1 ? names.length >= 3 : names.includes(n))
        && withClip >= (want.targetsWithClip ?? 0) && chain >= (want.sequence ?? 0);
    }
    if (spec.physics) {
      // The scene needs enough moving bodies and static ground, none dropped by the evaluator, and (for a bounce)
      // a restitution that actually bounces.
      const want = spec.physics;
      const dynamic = seen.bodies.filter(b => b.kind === 'dynamic').length, fixed = seen.bodies.filter(b => b.kind === 'static').length;
      outcome.physics = { dynamic, static: fixed, dropped: seen.warnings.filter(c => c.startsWith('phys.')) };
      const joints = seen.bodies.flatMap(b => b.joints ?? []);
      outcome.physics.joints = joints.map(j => j.kind);
      checks.physics = dynamic >= (want.dynamic ?? 1) && fixed >= (want.static ?? 1)
        && joints.length >= (want.joints ?? 0) && (want.kinds ?? []).every(kind => joints.some(j => j.kind === kind))
        && (!want.collide || seen.behaviors.some(b => b.trigger === 'collide'))
        && (!want.bouncy || seen.bodies.some(b => b.kind === 'dynamic' && b.restitution >= 0.4))
        && !seen.warnings.some(c => c.startsWith('phys.'));
    }
    if (spec.rig) {
      // A skinned mesh must exist with enough usable bones (the evaluator drops bad skins with a warning), some of the
      // bones must actually move, and nothing may have been dropped.
      const want = spec.rig;
      const widest = Math.max(0, ...seen.skins.map(s => s.bones.length));
      const moving = seen.bones.filter(b => b.moves).length;
      outcome.rig = { skinned: seen.skins.length, widest, moving, dropped: seen.warnings.filter(c => c.startsWith('rig.')) };
      checks.rig = seen.skins.length >= (want.skinned ?? 1) && widest >= (want.minBones ?? 2) && moving >= (want.movingBones ?? 1)
        && (!want.blend || seen.skins.some(s => s.blend === want.blend))
        && (!want.painted || seen.skins.some(s => s.stamps > 0))
        && !seen.warnings.some(c => c.startsWith('rig.'));
    }
    if (spec.assembly) {
      // Child parts must name parents that exist (the evaluator drops bad ones with a warning), and enough parts
      // must follow another; `moving` counts parts with a track of their own.
      const followers = seen.links.filter(l => l.parent);
      outcome.assembly = { followers: followers.map(l => `${l.name}->${l.parent}`), dropped: seen.warnings.filter(c => c.startsWith('anim.parent')) };
      checks.assembly = followers.length >= (spec.assembly.followers ?? 1)
        && followers.every(l => seen.links.some(p => p.name === l.parent))
        && !seen.warnings.some(c => c.startsWith('anim.parent'))
        && seen.links.filter(l => l.tracks > 0).length >= (spec.assembly.moving ?? 1);
    }
    if (spec.wired) {
      // A sender (a trigger with a target) must name a listener (a signal part with a clip) that exists.
      const senders = seen.behaviors.filter(b => b.target);
      const listeners = seen.behaviors.filter(b => b.trigger === 'signal' && b.tracks > 0);
      outcome.wired = { senders: senders.map(b => `${b.part}->${b.target}`), listeners: listeners.map(b => b.part) };
      checks.wired = senders.length >= (spec.wired.senders ?? 1)
        && senders.every(s => listeners.some(l => l.part === s.target))
        && (!spec.wired.trigger || senders.every(s => s.trigger === spec.wired.trigger))
        && (!spec.wired.properties || spec.wired.properties.every(p => seen.tracks.some(t => t.property === p)));
    }
    if (spec.behavior) {
      // The part must react the way the request implies: trigger, action and (for near) a radius in range.
      const [found] = seen.behaviors, wanted = spec.behavior;
      outcome.behavior = found ?? null;
      const radius = Array.isArray(wanted.radius) ? wanted.radius : null;
      checks.behavior = Boolean(found) && found.trigger === wanted.trigger && found.action === wanted.action
        && (!radius || (found.radius >= radius[0] && found.radius <= radius[1]));
    }
    if (spec.texture) {
      // Every texture program in the document must parse and compile; the channels, node families and
      // space the request implies must be there. (GPU linking of the node library is covered by g10-texture.)
      const programs = [...seen.text.matchAll(/program="((?:[^"\\]|\\.)*)"/g)].map(m => m[1].replace(/\\"/g, '"').replace(/\\\\/g, '\\'));
      const problems = [];
      const channels = new Set(), spaces = new Set();
      let source = '';
      for (const text of programs) {
        try {
          const program = parseTextureProgram(text, textureLibrary);
          source += ` ${text}`;
          for (const [channel, output] of Object.entries(program.channels)) {
            channels.add(channel);
            spaces.add(compileTexture(textureLibrary, { nodes: program.nodes, output }, textureHelpers).space);
          }
        } catch (error) { problems.push(String(error.message).slice(0, 160)); }
      }
      outcome.texture = { programs: programs.length, channels: [...channels], spaces: [...spaces], problems };
      checks.texture = programs.length > 0 && problems.length === 0
        && (spec.texture.channels ?? []).every(c => channels.has(c))
        && (spec.texture.uses ?? []).every(alt => alt.split('|').some(name => new RegExp(`=\\s*${name}\\(`).test(source)))
        && (!spec.texture.space || spaces.has(spec.texture.space));
    }
    outcome.ok = Object.values(checks).every(Boolean);
    await page.close();
  } catch (error) {
    outcome.error = String(error);
  } finally {
    pageServer.close();
  }
  outcome.seconds = Math.round((Date.now() - started) / 1000);
  const m = outcome.metrics;
  console.log(`${outcome.ok ? 'PASS' : 'FAIL'} ${spec.name} (${outcome.seconds}s${m ? `, ${m.turns} turns, ${m.toolCallCount} calls, ${m.toolErrors} failed, $${m.costUsd}` : ''}) ${JSON.stringify(outcome.checks)} size=${JSON.stringify(outcome.size)} volume=${outcome.volume}`);
  return outcome;
}

const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const results = [];
let next = 0;
async function worker() {
  while (next < prompts.length) {
    const index = next++;
    results[index] = await runOne(prompts[index], index, browser);
  }
}
try {
  await Promise.all(Array.from({ length: Math.min(parallel, prompts.length) }, worker));
} finally {
  await browser.close();
}
const passed = results.filter(r => r.ok).length;
// Totals over the set: what a model pays to do these requests, and where the bytes come from.
const sum = (pick) => results.reduce((total, r) => total + (pick(r.metrics ?? {}) ?? 0), 0);
const bytesByTool = {};
for (const r of results) for (const [tool, n] of Object.entries(r.metrics?.toolResultBytes ?? {})) bytesByTool[tool] = (bytesByTool[tool] ?? 0) + n;
const totals = {
  turns: sum(m => m.turns), toolCalls: sum(m => m.toolCallCount), toolErrors: sum(m => m.toolErrors),
  costUsd: Math.round(sum(m => m.costUsd) * 1000) / 1000, inputTokens: sum(m => m.inputTokens), outputTokens: sum(m => m.outputTokens),
  toolResultBytesByTool: Object.fromEntries(Object.entries(bytesByTool).sort((a, b) => b[1] - a[1])),
};
writeFileSync(path.join(reports, `${setName}.json`), JSON.stringify({ model, passed, total: results.length, totals, results }, null, 2));
console.log(`\n${passed}/${results.length} passed with ${model}; details in ${path.join(reports, `${setName}.json`)}`);
console.log(`totals: ${totals.turns} turns, ${totals.toolCalls} tool calls (${totals.toolErrors} failed), $${totals.costUsd}, ${totals.inputTokens} input / ${totals.outputTokens} output tokens`);
console.log(`bytes the tools put in the conversation: ${JSON.stringify(totals.toolResultBytesByTool)}`);
process.exit(0);
