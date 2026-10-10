// Minimal MCP (stdio, JSON-RPC) exposing the geometry language to an LLM client.
import { compile, spec } from './lang.mjs';
import readline from 'node:readline';

const TOOLS = [
  { name: 'get_spec', description: 'Return the geometry language reference (nodes, types, example).', inputSchema: { type: 'object', properties: {} } },
  { name: 'compile', description: 'Validate a geometry program. Returns errors with line numbers; fix and call again until ok.',
    inputSchema: { type: 'object', properties: { program: { type: 'string' } }, required: ['program'] } },
];
const send = (m) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...m }) + '\n');
const text = (t, isError = false) => ({ content: [{ type: 'text', text: t }], isError });

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  let m; try { m = JSON.parse(line); } catch { return; }
  const { id, method, params } = m;
  if (method === 'initialize') return send({ id, result: { protocolVersion: params?.protocolVersion ?? '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'geom-lang', version: '0.0.1' } } });
  if (method === 'tools/list') return send({ id, result: { tools: TOOLS } });
  if (method === 'tools/call') {
    const { name, arguments: a } = params;
    if (name === 'get_spec') return send({ id, result: text(spec()) });
    if (name === 'compile') { const r = compile(a.program ?? ''); return send({ id, result: text(JSON.stringify({ ok: r.ok, nodes: r.count, errors: r.errors, warnings: r.warnings }), !r.ok) }); }
    return send({ id, error: { code: -32601, message: 'unknown tool' } });
  }
  if (id !== undefined) send({ id, result: {} });
});
