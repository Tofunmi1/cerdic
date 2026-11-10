// Dedicated evaluator entry; kernel geometry stays off the window. Render packing is bounded there.
import init, * as api from './ve_evaluator.js';
import { bindMemory } from './wasi_snapshot_preview1.js';

let serial = 0;
let wasm;
const started = performance.now();
const ready = init().then(instance => {
  wasm = instance; bindMemory(instance.memory);
  postMessage({ kind: 'ready', startup_ms: performance.now() - started });
});
const yieldTask = () => new Promise(resolve => setTimeout(resolve, 0));
const testMode = new URL(import.meta.url).searchParams.get('test') === '1';
// The human body pack (about 9 MB) is fetched the first time a document uses body.human, never for other scenes.
let humanPack;
const loadHumanPack = () => humanPack ??= fetch(new URL('./human.pack', import.meta.url))
  .then(response => response.ok ? response.arrayBuffer() : Promise.reject(new Error(`human.pack: HTTP ${response.status}`)))
  .then(bytes => { const problem = api.human_pack_set(new Uint8Array(bytes)); if (problem) throw new Error(problem); });

self.onmessage = async ({ data }) => {
  if (data.kind === 'document_text') {
    await ready;
    const document = JSON.stringify(data.document);
    // A trace evaluates the whole document on its own evaluator, so a body needs its pack loaded first.
    if (data.operation === 'trace' && document.includes('body.human')) await loadHumanPack();
    const reply = data.operation === 'parse'
      ? api.doc_parse_text(document, data.text)
      : data.operation === 'trace'
        ? api.eval_trace(document, 'final')
        : api.doc_text_compact(document, data.operation === 'nodes');
    postMessage({ kind: 'document_text_result', id: data.id, reply: JSON.parse(reply) });
    return;
  }
  if (data.kind === 'cancel') { serial++; api.eval_cancel(); postMessage({ kind: 'cancelled', token: data.token }); return; }
  // A playtest is a pure function of the game text; it neither cancels nor waits for an evaluation.
  if (data.kind === 'playtest') {
    await ready;
    postMessage({ kind: 'playtest_result', id: data.id, report: api.game_playtest(data.text, data.policy, data.seed, data.steps) });
    return;
  }
  const own = ++serial;
  await ready;
  if (own !== serial) return;
  if (testMode && data.test === 'spin') {
    const until = performance.now() + 15000;
    while (performance.now() < until) {} // the supervisor must terminate this worker
  }
  const began = performance.now();
  try {
    if (testMode && data.test === 'crash') throw new Error('forced worker crash');
    const documentText = JSON.stringify(data.document);
    if (documentText.includes('body.human')) await loadHumanPack();
    let state = JSON.parse(api.eval_begin(documentText, data.token.quality));
    if (state.error) { postMessage({ kind: 'failure', token: data.token, error: state.error }); return; }
    let quantum = performance.now(), lastProgress = 0, lastDraft = performance.now();
    while (own === serial) {
      state = JSON.parse(api.eval_step());
      if (state.error) { postMessage({ kind: 'failure', token: data.token, error: state.error }); return; }
      if (state.done) break;
      // The outputs that have grown since the last look, as they stand now (a box, then the box bevelled, then the finished part): sent at most
      // three times a second so the page can show the model growing while the rest evaluates.
      const draft = performance.now() - lastDraft > 300 ? JSON.parse(api.eval_drafts()).draft : null;
      if (draft) {
        lastDraft = performance.now();
        const draftParts = readParts(draft.parts);
        const transfer = draftParts.flatMap(part => [part.positions.buffer, part.indices.buffer, part.uvs.buffer, part.occlusion.buffer, part.normals.buffer, part.colors.buffer, part.transforms.buffer, part.instance_indices.buffer, part.skin_joints.buffer, part.skin_weights.buffer, ...(part.morph_targets ?? []).map(target => target.positions.buffer), part.baked?.normal?.buffer, part.baked?.ao?.buffer].filter(Boolean));
        postMessage({ kind: 'draft', token: data.token, parts: draftParts, evaluated: state.progress, total: data.document.nodes?.length ?? 0 }, transfer);
      }
      if (performance.now() - quantum > 8) {
        // Tell the page how far along the build is (nodes finished out of the document's nodes) at most ten times a second.
        if (state.progress !== undefined && performance.now() - lastProgress > 100) {
          lastProgress = performance.now();
          postMessage({ kind: 'progress', token: data.token, evaluated: state.progress, total: data.document.nodes?.length ?? 0 });
        }
        await yieldTask(); quantum = performance.now();
      }
    }
    if (own !== serial) return;
    const parts = readParts(state.done.parts);
    function readParts(metas) { return metas.map((part, index) => {
      const positions = api.eval_positions(index), indices = api.eval_indices(index), uvs = api.eval_uvs(index);
      const occlusion = api.eval_occlusion(index);
      const normals = api.eval_normals(index);
      const colors = api.eval_colors(index);
      const bakedSize = api.eval_baked_size(index);
      const baked = bakedSize ? { size: bakedSize, normal: api.eval_baked_normal(index), ao: api.eval_baked_ao(index) } : undefined;
      const transforms = api.eval_transforms(index), instance_indices = api.eval_instance_indices(index);
      const visible_range = api.eval_visible_range(index);
      const visible_fade = api.eval_visible_fade(index);
      // Per-vertex bone indices and weights only exist for a skinned part (ADR 0030).
      const skinned = Boolean(part.skin);
      const skin_joints = skinned ? api.eval_skin_joints(index) : new Uint32Array(0);
      const skin_weights = skinned ? api.eval_skin_weights(index) : new Float32Array(0);
      const morphCount = api.eval_morph_count(index);
      const morph_targets = Array.from({ length: morphCount }, (_, target) => ({
        name: api.eval_morph_name(index, target), weight: api.eval_morph_weight(index, target),
        positions: api.eval_morph_positions(index, target),
      }));
      return { ...part, positions, indices, uvs, occlusion, normals, colors, baked, transforms, instance_indices, visible_range: visible_range.length === 2 ? [visible_range[0], visible_range[1]] : undefined, visible_fade, skin_joints, skin_weights, morph_targets };
    }); }
    const warnings = state.done.warnings ?? [];
    const lights = state.done.lights ?? [];
    const environment = state.done.environment ?? null;
    const bytes = parts.reduce((sum, part) => sum + part.positions.byteLength + part.indices.byteLength + part.uvs.byteLength + part.occlusion.byteLength + part.normals.byteLength + part.colors.byteLength + part.transforms.byteLength + part.instance_indices.byteLength
      + part.skin_joints.byteLength + part.skin_weights.byteLength
      + (part.morph_targets ?? []).reduce((bytes, target) => bytes + target.positions.byteLength, 0)
      + (part.baked?.normal?.byteLength ?? 0) + (part.baked?.ao?.byteLength ?? 0)
      + new TextEncoder().encode(JSON.stringify(part.frames ?? {})).byteLength, 0)
      + new TextEncoder().encode(JSON.stringify(lights)).byteLength
      + new TextEncoder().encode(JSON.stringify(environment)).byteLength
      + new TextEncoder().encode(JSON.stringify(warnings)).byteLength;
    const transfer = parts.flatMap(part => [part.positions.buffer, part.indices.buffer, part.uvs.buffer, part.occlusion.buffer, part.normals.buffer, part.colors.buffer, part.transforms.buffer, part.instance_indices.buffer, part.skin_joints.buffer, part.skin_weights.buffer, ...(part.morph_targets ?? []).map(target => target.positions.buffer), ...(part.baked?.normal ? [part.baked.normal.buffer] : []), ...(part.baked?.ao ? [part.baked.ao.buffer] : [])]);
    const game = JSON.parse(api.eval_game());
    // The `script.js` nodes of the document (behaviour saved with the world); the page keeps its running scripts in step with them.
    const scripts = typeof api.eval_scripts === 'function' ? JSON.parse(api.eval_scripts()) : [];
    const attachments = data.token.quality === 'final' ? JSON.parse(api.eval_attachments(0.02)) : undefined;
    postMessage({ kind: 'result', token: data.token, parts, lights, environment, warnings, game, scripts, attachments, bytes, sent_ms: performance.timeOrigin + performance.now(),
      stats: { ...state.done, parts: undefined, warnings: undefined, elapsed_ms: performance.now() - began,
        memory_bytes: wasm.memory.buffer.byteLength } }, transfer);
    api.eval_cancel();
  } catch (error) {
    postMessage({ kind: 'crash', token: data.token, error: { code: 'eval.worker_crash', message: String(error) } });
  }
};
