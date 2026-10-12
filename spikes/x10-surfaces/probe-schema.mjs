// Checks how --json-schema output comes back from the CLI before relying on it.
import { ask } from './llm.mjs';

const schema = {
  type: 'object',
  properties: { commands: { type: 'array', items: { type: 'object', properties: { op: { type: 'string', enum: ['add', 'output'] }, id: { type: 'string' }, kind: { type: 'string' }, params: { type: 'object' } }, required: ['op', 'id'] } } },
  required: ['commands'],
};
const r = await ask({ model: 'sonnet', system: 'You output only the requested structure.', prompt: 'Add a box node named b1 with params size [1,1,1], then output b1.', schema });
console.log(JSON.stringify({ ok: r.ok, ms: r.ms, text: r.text.slice(0, 200), structured: r.structured, cost: r.cost, keys: Object.keys(r.raw ?? {}) }, null, 1));
