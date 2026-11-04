// Entry point bundled for the engine's own viewer page (engine/web/index.html): adds the evaluation
// pipeline (worker, staged edits, scene upload) and the test MCP bridge to a page that is plain
// JavaScript. It is the same code the editor uses, so documents evaluate and appear identically.
import { documentBridge } from './bridge';
import type { Runtime } from './bridge';
import { EvaluationHost } from './evaluation';
import { buildGlbWithTextures } from './gltf';
import { attachGameAudio } from './game-audio';
import { connectMcpBridge, runPageTool } from './mcp-bridge';
import { liveTextureStats } from './mesh-upload';
import { createScriptHost } from './scripts';
import { createScriptUi } from './script-ui';

type Options = { workerUrl: string; mcpPort?: number; onChange: () => void };

export function attachEvaluation(runtime: Runtime, { workerUrl, mcpPort, onChange }: Options) {
  const resources = { workers: 0 };
  let framed = false;
  const scriptUi = createScriptUi(document.getElementById('ve'));
  const scripts = createScriptHost(runtime, {
    ui: (name, message) => scriptUi.apply(name, message),
    clearUi: name => scriptUi.clearScript(name),
    pose: () => { const raw = runtime.viewer_pose(); return raw ? JSON.parse(raw) : null; },
    teleport: (p, t) => { runtime.viewer_look_at(p[0], p[1], p[2], t[0], t[1], t[2]); },
  });
  const evaluation = new EvaluationHost(runtime, onChange, resources, {
    workerUrl,
    onScripts: defs => scripts.sync(defs),
    // The first model that appears is framed so it is in view; later edits keep the camera where the
    // person put it (the viewer's F key frames again).
    onProjected: bounds => { if (!framed) { framed = runtime.viewer_frame(...bounds.min as [number, number, number], ...bounds.max as [number, number, number]); } },
  });
  const staged = documentBridge(runtime, onChange, () => evaluation.request(), evaluation.text);
  const parse = (value: string) => JSON.parse(value) as Record<string, any>;
  // The same shape as the page's plain document API, so the Document panel works unchanged. Edits
  // are staged and evaluated before they commit; the panel re-renders when evaluation finishes.
  const doc = {
    open(name = 'default', seed = 1) { const view = parse(runtime.doc_open(name, seed)); framed = false; evaluation.request(); return view; },
    reset(name = 'default', seed = 1) { framed = false; return parse(runtime.doc_reset(name, seed)); },
    state: () => staged.state(),
    operators: () => JSON.parse(runtime.doc_operators()),
    undo: () => staged.undo(),
    redo: () => staged.redo(),
    save: () => staged.save(),
    apply: (batch: Parameters<typeof staged.apply>[0]) => staged.apply(batch),
    send(commands: Parameters<typeof staged.send>[0], label: string) {
      const outcome = staged.send(commands, label) as Record<string, unknown>;
      return (outcome && 'pending' in outcome ? outcome.pending : outcome) as Record<string, unknown>;
    },
    sendText: (text: string, label: string) => staged.sendText(text, label),
    text: () => staged.text(),
    /** The evaluated model as a binary glTF file (metres, Y up), or null when nothing is evaluated yet. */
    exportGlb: async () => (evaluation.parts.length > 0 ? buildGlbWithTextures(evaluation.parts, { name: 'model', mikkTangents: runtime.eval_tangents.bind(runtime) }) : null),
  };
  const host = { get ready() { return (window as unknown as { __ve?: { ready: boolean } }).__ve?.ready ?? false; }, doc: staged, evaluation, runtime, scripts };
  const stop = mcpPort ? connectMcpBridge(host, mcpPort) : () => {};
  const gameAudio = attachGameAudio(runtime);
  window.addEventListener('pagehide', () => { stop(); scripts.stopAll(); gameAudio.stop(); evaluation.dispose(); }, { once: true });
  evaluation.request();
  return { doc, evaluation, staged, gameAudio, scripts, liveTextures: liveTextureStats, tool: (name: string, params: Record<string, unknown> = {}) => runPageTool(host as never, name, params) };
}
