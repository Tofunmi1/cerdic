import { expect, test } from 'bun:test';
import { documentBridge } from '../src/engine/bridge';
import type { Runtime } from '../src/engine/bridge';
import { EvaluatorText } from '../src/engine/evaluator-text';
import { findNodes } from '../src/engine/mcp-tools';
import type { Document } from '../src/engine/generated';

const document = { nodes: [], outputs: [] } as unknown as Document;

test('worker text requests correlate replies and fail outstanding work on restart', async () => {
  const sent: any[] = [];
  const worker = { postMessage: (message: unknown) => sent.push(message) } as unknown as Worker;
  const text = new EvaluatorText(() => worker);
  const first = text.request(document, 'compact');
  const second = text.request(document, 'nodes');
  text.receive(sent[1].id, { lines: { n_1: 'a = solid.box()' } });
  text.receive(sent[0].id, { text: 'compact' });
  expect(await first).toEqual({ text: 'compact' });
  expect(await second).toEqual({ lines: { n_1: 'a = solid.box()' } });
  const pending = text.request(document, 'parse', 'a = solid.box()');
  const rejection = pending.catch((error: Error) => error.message);
  text.fail('worker restarted');
  expect(await rejection).toBe('worker restarted');
});

test('reading defaults to evaluator compact text while the panel stays canonical', async () => {
  const sent: any[] = [];
  const text = new EvaluatorText(() => ({ postMessage: (message: unknown) => sent.push(message) }) as unknown as Worker);
  const runtime = {
    doc_state: () => JSON.stringify({ document, revision: 3 }),
    doc_text: () => JSON.stringify({ text: 'canonical@1' }),
  } as unknown as Runtime;
  const doc = documentBridge(runtime, () => {}, undefined, text);
  expect(doc.text()).toEqual({ text: 'canonical@1' });
  expect(await doc.readText('canonical')).toEqual({ text: 'canonical@1' });
  expect(sent).toHaveLength(0);
  const read = doc.readText();
  expect(sent[0].operation).toBe('compact');
  text.receive(sent[0].id, { text: 'compact' });
  expect(await read).toEqual({ text: 'compact' });
});

test('text edits parse the candidate in the evaluator before sending commands', async () => {
  const sent: any[] = [], batches: any[] = [];
  const candidate = { ...document, revision: 4 };
  const text = new EvaluatorText(() => ({ postMessage: (message: unknown) => sent.push(message) }) as unknown as Worker);
  const runtime = {
    doc_state: () => JSON.stringify({ id: 'd_test', document, revision: 3, pending_document: candidate }),
    doc_apply: (batch: string) => { batches.push(JSON.parse(batch)); return '{"committed":{}}'; },
  } as unknown as Runtime;
  const doc = documentBridge(runtime, () => {}, undefined, text);
  const edit = doc.sendText('for i in 0..2\nend', 'loop');
  expect(sent[0].operation).toBe('parse');
  expect(sent[0].document).toEqual(candidate);
  expect(batches).toHaveLength(0);
  text.receive(sent[0].id, { commands: [] });
  await edit;
  expect(batches[0].base_revision).toBe(4);
});

test('find uses exact compact lines and retains node details and query matching', () => {
  const node = { id: 'n_1', label: 'turn', op: 'xform.rotate@1', params: { angles: [0, 0, Math.PI] }, inputs: {} };
  const doc = { nodes: [node], outputs: [] } as unknown as Document;
  const line = 'turn = xform.rotate(angles=[0deg, 0deg, 180deg])';
  const result = findNodes(doc, { node: 'turn' }, { n_1: line });
  expect(result.matches[0]).toMatchObject({ line, params: node.params, op: node.op });
  expect(findNodes(doc, { query: 'angles', family: 'xform' }, { n_1: line }).matches).toHaveLength(1);
});
