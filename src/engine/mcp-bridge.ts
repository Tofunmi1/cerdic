// Test bridge: lets the engine's MCP server (engine/tools/mcp/server.mjs) drive this page. Enabled
// with ?mcp=1 (or ?mcp=<port>). Every tool call runs through the same document commands as the
// editor, so the model gets no capability a person lacks. Local testing only: it connects to
// 127.0.0.1 and trusts whatever is listening there.
import type { Command } from './generated';
import type { documentBridge, Runtime } from './bridge';
import type { EvaluationHost } from './evaluation';
import type { MeshPart } from './worker-types';
import { buildGlbWithTextures } from './gltf';
import { snapshotView } from './snapshot';
import { rgbaPng } from './png';
import { loadBrowserTextureLibrary } from './texture/browser-library';
import { describe as describeTextureNode, nodeInfo, search as searchTextureNodes, summary as textureNodeSummary } from './texture/catalog';

/** What the bridge needs from the page: the engine, its document and its evaluation host. */
export type McpHost = {
  readonly ready: boolean;
  doc: ReturnType<typeof documentBridge> | null;
  evaluation: EvaluationHost | null;
  runtime: Runtime | null;
};
type Host = McpHost;

type Call = { id: number; method: string; params: Record<string, unknown> };
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

export function connectMcpBridge(host: Host, port: number) {
  let socket: WebSocket | null = null;
  let stopped = false;
  const open = () => {
    if (stopped) return;
    socket = new WebSocket(`ws://127.0.0.1:${port}`);
    socket.onmessage = async event => {
      const call = JSON.parse(String(event.data)) as Call;
      let reply: { id: number; result?: unknown; error?: string };
      try { reply = { id: call.id, result: await run(host, call.method, call.params) }; }
      catch (error) { reply = { id: call.id, error: String(error) }; }
      socket?.send(JSON.stringify(reply));
    };
    socket.onclose = () => { socket = null; if (!stopped) setTimeout(open, 2000); };
    socket.onerror = () => socket?.close();
  };
  open();
  return () => { stopped = true; socket?.close(); };
}

// Waits until the staged edit has been evaluated: the worker is idle and either the new revision was
// projected or a failure was reported.
async function settle(host: Host, pendingRevision: number | undefined, failureBefore: unknown) {
  const began = Date.now();
  for (;;) {
    await sleep(40);
    const state = host.evaluation!.state;
    const done = pendingRevision === undefined || (state.revision ?? -1) >= pendingRevision || state.lastFailure !== failureBefore;
    if (state.worker === 'idle' && state.queue === 0 && done) return;
    if (Date.now() - began > 80000) throw new Error('evaluation did not finish in 80 s');
  }
}

async function edit(host: Host, stage: () => unknown) {
  const evaluation = host.evaluation!;
  const before = evaluation.state.lastFailure;
  const revisionBefore = host.doc!.state().revision;
  const outcome = stage() as Record<string, any>;
  const staged = outcome?.pending as { committed?: { revision: number } } | undefined;
  if (!staged?.committed) return { applied: false, outcome, text: host.doc!.text().text };
  await settle(host, staged.committed.revision, before);
  const failure = evaluation.state.lastFailure !== before ? evaluation.state.lastFailure : null;
  const revision = host.doc!.state().revision;
  return {
    applied: revision > revisionBefore, revision, failure,
    note: failure ? 'The edit was refused by the evaluator and the document is unchanged.' : undefined,
    text: host.doc!.text().text, stats: evaluation.state.stats,
  };
}

// The engine expects one uv per vertex, but a part carries per-triangle-corner uvs so a seam can
// repeat a position with different UVs. When the corners of a vertex disagree the part has a seam
// and no per-vertex uvs to give, so UVs are left out rather than passed wrong.
function perVertexUvs(part: MeshPart): Float32Array {
  const uvs = part.uvs;
  if (!uvs || uvs.length < part.indices.length * 2) return new Float32Array(0);
  const count = part.positions.length / 3;
  const perVertex = new Float32Array(count * 2);
  const assigned = new Uint8Array(count);
  for (let corner = 0; corner < part.indices.length; corner++) {
    const vertex = part.indices[corner];
    if (vertex >= count) return new Float32Array(0);
    const u = uvs[corner * 2], v = uvs[corner * 2 + 1];
    if (!assigned[vertex]) { perVertex[vertex * 2] = u; perVertex[vertex * 2 + 1] = v; assigned[vertex] = 1; }
    else if (perVertex[vertex * 2] !== u || perVertex[vertex * 2 + 1] !== v) return new Float32Array(0);
  }
  return perVertex;
}

// The full report's optional checks and the JSON section each one fills. `collision` is the
// bounding-shape fit a physics body would use.
const MEASURE_SECTIONS: Record<string, string> = {
  thickness: 'thickness', symmetry: 'symmetry', uv: 'uv', collision: 'bounding_shapes',
};

/**
 * Measures the evaluated parts on screen through the same bridge the other tools use. The default is
 * the engine's short text summary; `detail: 'full'` gives the complete JSON report, and `checks`
 * limits which optional sections (`thickness`, `symmetry`, `uv`, `collision`) it keeps.
 */
export function measureParts(host: Host, params: Record<string, unknown>) {
  const evaluation = host.evaluation!, runtime = host.runtime!;
  const wanted = typeof params.part === 'string' ? params.part.trim() : '';
  const parts = wanted
    ? evaluation.parts.filter(part => part.name === wanted || part.node === wanted)
    : evaluation.parts;
  if (parts.length === 0) return wanted ? `no part named ${wanted} is on screen` : 'there is nothing to measure yet';
  const detail = params.detail === 'full' ? 'full' : 'summary';
  const checks = Array.isArray(params.checks) ? params.checks.filter((check): check is string => typeof check === 'string') : [];
  const blocks: string[] = [];
  const reports: unknown[] = [];
  for (const part of parts) {
    const text = runtime.measure_mesh(part.positions, part.indices, perVertexUvs(part), detail);
    if (detail !== 'full') { blocks.push(`${part.name || part.node}\n${text}`); continue; }
    let report: Record<string, unknown>;
    try { report = JSON.parse(text) as Record<string, unknown>; } catch { report = { error: text }; }
    if (checks.length > 0) for (const key of Object.values(MEASURE_SECTIONS)) if (key in report && !checks.some(check => MEASURE_SECTIONS[check] === key)) delete report[key];
    reports.push({ name: part.name || part.node, node: part.node, ...report });
  }
  return detail === 'full' ? { revision: evaluation.state.revision, parts: reports } : blocks.join('\n\n');
}

/** Bake selected maps from an evaluated high part onto the UVs of a low part. */
export function bakeParts(host: Host, params: Record<string, unknown>) {
  const evaluation = host.evaluation!, runtime = host.runtime!;
  const pick = (value: unknown) => typeof value === 'string' ? value.trim() : '';
  const lowName = pick(params.low), highName = pick(params.high);
  if (!lowName || !highName) return { ok: false, error: 'low and high part names are required' };
  const part = (name: string) => evaluation.parts.find(p => p.name === name || p.node === name);
  const names = evaluation.parts.map(p => p.name || p.node);
  const low = part(lowName), high = part(highName);
  if (!low || !high) return { ok: false, error: `part not found; on-screen parts: ${names.join(', ') || '(none)'}` };
  if (!low.uvs || low.uvs.length !== low.indices.length * 2) return { ok: false, error: 'low part has no per-corner UVs; unwrap it with poly.unwrap_uv first' };
  const size = params.size === undefined ? 512 : Number(params.size);
  const requestedMaps = params.maps === undefined ? ['normal', 'ao'] : params.maps;
  if (!Number.isInteger(size) || size < 64 || size > 2048) return { ok: false, error: 'size must be 64 to 2048' };
  if (!Array.isArray(requestedMaps) || requestedMaps.length === 0 || requestedMaps.some(m => m !== 'normal' && m !== 'ao')) return { ok: false, error: 'maps must contain normal and/or ao' };
  const wanted = [...new Set(requestedMaps)];
  const rays = params.rays === undefined ? 64 : Number(params.rays);
  if (!Number.isInteger(rays) || rays < 1 || rays > 256) return { ok: false, error: 'rays must be 1 to 256' };
  const bounds = (positions: Float32Array) => {
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < positions.length; i += 3) for (let axis = 0; axis < 3; axis++) { min[axis] = Math.min(min[axis], positions[i + axis]); max[axis] = Math.max(max[axis], positions[i + axis]); }
    return Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]);
  };
  const diagonal = bounds(low.positions), setting = (key: string, fallback: number) => params[key] === undefined ? fallback : Number(params[key]);
  const cage = setting('cage', diagonal * 0.05), maxDistance = setting('max_distance', diagonal * 0.1), bias = setting('bias', diagonal * 0.001);
  if (![cage, maxDistance, bias].every(Number.isFinite) || cage < 0 || maxDistance <= 0 || bias < 0) return { ok: false, error: 'cage and bias must be non-negative; max_distance must be positive' };
  const mapMode = wanted.length === 2 ? 'both' : wanted[0] as string;
  const result = JSON.parse(runtime.bake_maps(low.positions, low.indices, low.uvs, high.positions, high.indices, size, mapMode, cage, maxDistance, rays, bias)) as { ok: boolean; error?: string; suggestion?: string; size: number; normal: null | { covered: number; rgbaBase64: string }; ao: null | { covered: number; rgbaBase64: string } };
  if (!result.ok) return { ok: false, error: `${result.error ?? 'bake failed'}${result.suggestion ? `; ${result.suggestion}` : ''}` };
  const maps: Record<string, string> = {};
  for (const [name, item] of [['normal', result.normal], ['ao', result.ao]] as const) if (item) {
    const binary = atob(item.rgbaBase64), rgba = Uint8Array.from(binary, c => c.charCodeAt(0));
    if (rgba.length !== size * size * 4) return { ok: false, error: `${name} map has ${rgba.length} RGBA bytes; expected ${size * size * 4}` };
    const png = rgbaPng(rgba, size); let encoded = ''; for (let i = 0; i < png.length; i += 0x8000) encoded += String.fromCharCode(...png.subarray(i, i + 0x8000));
    maps[name] = btoa(encoded);
  }
  return { ok: true, size, covered: Math.max(result.normal?.covered ?? 0, result.ao?.covered ?? 0), maps };
}

async function run(host: Host, method: string, params: Record<string, unknown>) {
  if (!host.ready || !host.doc || !host.evaluation || !host.runtime) throw new Error('the engine is still starting');
  const doc = host.doc, runtime = host.runtime;
  switch (method) {
    case 'get_text': return { revision: doc.state().revision, ...doc.text() };
    case 'texture_nodes': {
      const { library } = await loadBrowserTextureLibrary();
      const node = typeof params.node === 'string' ? params.node : '';
      if (node) return describeTextureNode(library, node);
      const query = typeof params.query === 'string' ? params.query.trim() : '';
      const limit = Math.max(1, Math.min(80, Number(params.limit) || 30));
      const names = query ? searchTextureNodes(library, query, limit) : Object.keys(library).sort().slice(0, limit);
      return names.map(name => nodeInfo(name, library[name])).filter(Boolean).map(info => textureNodeSummary(info!));
    }
    case 'get_nodes': {
      const { document } = doc.state();
      // `group` is the group a node is a member of (a definition node), `instance` the group, version
      // and overrides of an instance; `groups` lists each group's version, result and exposed parameters.
      return { revision: doc.state().revision, nodes: document.nodes.map(n => ({ id: n.id, label: n.label, op: n.op, params: n.params, inputs: n.inputs, flags: n.flags, group: n.group ?? null, instance: n.instance ?? null })), outputs: document.outputs, groups: document.groups ?? [] };
    }
    case 'list_operators': {
      const all = JSON.parse(runtime.doc_operators()) as Array<{ readiness: string }>;
      return all.filter(o => o.readiness === 'ready');
    }
    case 'get_status': {
      const state = doc.state();
      return { revision: state.revision, nodes: state.nodes, can_undo: state.can_undo, can_redo: state.can_redo, evaluation: host.evaluation.state, projection: JSON.parse(runtime.eval_projection_status()) };
    }
    case 'measure': return measureParts(host, params);
    case 'bake': return bakeParts(host, params);
    case 'export_glb': {
      // The server writes the file; the bytes cross the socket as base64.
      const parts = host.evaluation.parts;
      if (parts.length === 0) return { exported: false, reason: 'there is no model to export yet' };
      const glb = await buildGlbWithTextures(parts, { name: 'model', mikkTangents: host.runtime!.eval_tangents.bind(host.runtime) });
      let binary = '';
      for (let i = 0; i < glb.length; i += 0x8000) binary += String.fromCharCode(...glb.subarray(i, i + 0x8000));
      return {
        exported: true, bytes: glb.byteLength, parts: parts.length,
        triangles: parts.reduce((sum, p) => sum + p.indices.length / 3, 0), base64: btoa(binary),
      };
    }
    case 'apply_text': {
      const text = String(params.text ?? '');
      const read = JSON.parse(runtime.doc_parse_text(text)) as { commands?: Command[]; diagnostics?: unknown[] };
      if (!read.commands) return { applied: false, diagnostics: read.diagnostics };
      return edit(host, () => doc.send(read.commands!, String(params.label ?? 'assistant: apply_text')));
    }
    case 'make_character': {
      const { label: _label, ...request } = params;
      const made = JSON.parse(runtime.creature_make(JSON.stringify(request))) as { text?: string; error?: string };
      if (made.text === undefined) return { applied: false, error: made.error };
      const read = JSON.parse(runtime.doc_parse_text(made.text)) as { commands?: Command[]; diagnostics?: unknown[] };
      if (!read.commands) return { applied: false, diagnostics: read.diagnostics };
      return edit(host, () => doc.send(read.commands!, String(params.label ?? 'assistant: make_character')));
    }
    case 'apply_commands': {
      if (!Array.isArray(params.commands)) throw new Error('commands must be an array');
      return edit(host, () => doc.send(params.commands as Command[], String(params.label ?? 'assistant: apply_commands')));
    }
    case 'undo': return edit(host, () => { const moved = doc.undo(); return moved ? { pending: { committed: { revision: (moved as { revision: number }).revision } } } : { nothing: 'there is nothing to undo' }; });
    case 'redo': return edit(host, () => { const moved = doc.redo(); return moved ? { pending: { committed: { revision: (moved as { revision: number }).revision } } } : { nothing: 'there is nothing to redo' }; });
    case 'snapshot': return snapshotView(params);
    case 'frame_view': {
      const bounds = host.evaluation.bounds;
      if (!bounds) return { ok: false, reason: 'there is no model to frame yet' };
      return { ok: runtime.viewer_frame(...bounds.min as [number, number, number], ...bounds.max as [number, number, number]), bounds };
    }
    case 'set_view': {
      const [p, t] = [params.position, params.target] as [number[], number[]];
      return { ok: runtime.viewer_look_at(p[0], p[1], p[2], t[0], t[1], t[2]) };
    }
    default: throw new Error(`unknown method ${method}`);
  }
}
