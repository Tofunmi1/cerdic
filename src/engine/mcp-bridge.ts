// Test bridge: lets the engine's MCP server (engine/tools/mcp/server.mjs) drive this page. Enabled
// with ?mcp=1 (or ?mcp=<port>). Every tool call runs through the same document commands as the
// editor, so the model gets no capability a person lacks. Local testing only: it connects to
// 127.0.0.1 and trusts whatever is listening there.
import type { Command } from './generated';
import type { host as Host } from './host';

type Call = { id: number; method: string; params: Record<string, unknown> };
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

export function connectMcpBridge(host: typeof Host, port: number) {
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
async function settle(host: typeof Host, pendingRevision: number | undefined, failureBefore: unknown) {
  const began = Date.now();
  for (;;) {
    await sleep(40);
    const state = host.evaluation!.state;
    const done = pendingRevision === undefined || (state.revision ?? -1) >= pendingRevision || state.lastFailure !== failureBefore;
    if (state.worker === 'idle' && state.queue === 0 && done) return;
    if (Date.now() - began > 80000) throw new Error('evaluation did not finish in 80 s');
  }
}

async function edit(host: typeof Host, stage: () => unknown) {
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

async function run(host: typeof Host, method: string, params: Record<string, unknown>) {
  if (!host.ready || !host.doc || !host.evaluation || !host.runtime) throw new Error('the engine is still starting');
  const doc = host.doc, runtime = host.runtime;
  switch (method) {
    case 'get_text': return { revision: doc.state().revision, ...doc.text() };
    case 'get_nodes': {
      const { document } = doc.state();
      return { revision: doc.state().revision, nodes: document.nodes.map(n => ({ id: n.id, label: n.label, op: n.op, params: n.params, inputs: n.inputs, flags: n.flags })), outputs: document.outputs };
    }
    case 'list_operators': {
      const all = JSON.parse(runtime.doc_operators()) as Array<{ readiness: string }>;
      return all.filter(o => o.readiness === 'ready');
    }
    case 'get_status': {
      const state = doc.state();
      return { revision: state.revision, nodes: state.nodes, can_undo: state.can_undo, can_redo: state.can_redo, evaluation: host.evaluation.state, projection: JSON.parse(runtime.eval_projection_status()) };
    }
    case 'apply_text': {
      const text = String(params.text ?? '');
      const read = JSON.parse(runtime.doc_parse_text(text)) as { commands?: Command[]; diagnostics?: unknown[] };
      if (!read.commands) return { applied: false, diagnostics: read.diagnostics };
      return edit(host, () => doc.send(read.commands!, String(params.label ?? 'assistant: apply_text')));
    }
    case 'apply_commands': {
      if (!Array.isArray(params.commands)) throw new Error('commands must be an array');
      return edit(host, () => doc.send(params.commands as Command[], String(params.label ?? 'assistant: apply_commands')));
    }
    case 'undo': return edit(host, () => { const moved = doc.undo(); return moved ? { pending: { committed: { revision: (moved as { revision: number }).revision } } } : { nothing: 'there is nothing to undo' }; });
    case 'redo': return edit(host, () => { const moved = doc.redo(); return moved ? { pending: { committed: { revision: (moved as { revision: number }).revision } } } : { nothing: 'there is nothing to redo' }; });
    case 'set_view': {
      const [p, t] = [params.position, params.target] as [number[], number[]];
      return { ok: runtime.viewer_look_at(p[0], p[1], p[2], t[0], t[1], t[2]) };
    }
    default: throw new Error(`unknown method ${method}`);
  }
}
