// Entry point bundled for the engine's own viewer page (engine/web/index.html): adds the evaluation
// pipeline (worker, staged edits, scene upload) and the test MCP bridge to a page that is plain
// JavaScript. It is the same code the editor uses, so documents evaluate and appear identically.
import { documentBridge } from './bridge';
import type { Runtime } from './bridge';
import { EvaluationHost } from './evaluation';
import { connectMcpBridge } from './mcp-bridge';

type Options = { workerUrl: string; mcpPort?: number; onChange: () => void };

export function attachEvaluation(runtime: Runtime, { workerUrl, mcpPort, onChange }: Options) {
  const resources = { workers: 0 };
  let framed = false;
  const evaluation = new EvaluationHost(runtime, onChange, resources, {
    workerUrl,
    // The first model that appears is framed so it is in view; later edits keep the camera where the
    // person put it (the viewer's F key frames again).
    onProjected: bounds => { if (!framed) { framed = runtime.viewer_frame(...bounds.min as [number, number, number], ...bounds.max as [number, number, number]); } },
  });
  const staged = documentBridge(runtime, onChange, () => evaluation.request());
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
  };
  const host = { get ready() { return (window as unknown as { __ve?: { ready: boolean } }).__ve?.ready ?? false; }, doc: staged, evaluation, runtime };
  const stop = mcpPort ? connectMcpBridge(host, mcpPort) : () => {};
  window.addEventListener('pagehide', () => { stop(); evaluation.dispose(); }, { once: true });
  evaluation.request();
  return { doc, evaluation, staged };
}
