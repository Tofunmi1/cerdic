import { expect, test } from 'bun:test';
import { editDocument, diffDocument } from '../tool-edits.js';
import { validateToolArguments } from '../tool-arguments.js';
import type { Command, Document, Node } from '../../../src/engine/generated';
import type { TextReply } from '../../../src/engine/evaluator-text';

const node = (id: string, label: string, inputs: Node['inputs'] = {}): Node => ({ id, label, op: 'solid.box@1', params: { size: [1, 1, 1] }, inputs, flags: { muted: false, bypass: false } });
function fixture(nodes = [node('n_a', 'base'), node('n_b', 'lid')]) {
  const document: Document = { id: 'd_test', revision: 7, schema: 1, units: 'm', seed: 1, ids: { next: 3, salt: 0 }, nodes, outputs: [], materials: [], groups: [], editor: { positions: {}, collapsed: [] } };
  const calls: Array<{ commands: Command[]; label: string; dry: boolean }> = [];
  const delta = { revision: 8, triangles: { before: 12, after: 24 } };
  const services = {
    doc: { state: () => ({ document }) },
    async parse(_base: Document, _text: string): Promise<TextReply> { return { commands: [{ add_node: { op: 'solid.sphere@1', params: { radius: 2 }, label: 'base', alias: '$base', group: null } }] }; },
    async mutate(commands: Command[], label: string, dry: boolean) { calls.push({ commands, label, dry }); return { applied: !dry, delta }; },
  };
  return { document, calls, delta, services };
}

test('edits compile in order into one transaction and return its delta', async () => {
  const f = fixture();
  const result = await editDocument({ ops: [{ op: 'set', node: 'base', params: { size: [2, 3, 4] } }, { op: 'rename', node: 'base', to: 'body' }, { op: 'set', node: 'body', params: { size: [3, 4, 5] } }, { op: 'delete', node: 'lid' }], label: 'Reshape' }, f.services);
  expect(result).toEqual({ applied: true, delta: f.delta });
  expect(f.calls).toEqual([{ commands: [{ set_param: { node: 'n_a', param: 'size', value: [2, 3, 4] } }, { set_label: { node: 'n_a', label: 'body' } }, { set_param: { node: 'n_a', param: 'size', value: [3, 4, 5] } }, { remove_node: { node: 'n_b', cascade: false } }], label: 'Reshape', dry: false }]);
  expect(f.document.nodes[0].label).toBe('base');
});

test('dry_run uses the shared mutation path and delete clears visible outputs', async () => {
  const f = fixture();
  f.document.outputs = [{ name: 'body', from: { node: 'n_a', port: 'out' } }];
  await editDocument({ ops: [{ op: 'delete', node: 'base' }], dry_run: true }, f.services);
  expect(f.calls[0].dry).toBe(true);
  expect(f.calls[0].commands[0]).toEqual({ set_output: { name: 'body', from: null } });
});

test('bad labels and later bad operations cannot partially apply a batch', async () => {
  for (const label of ['missing', 'base']) {
    const f = fixture([node('n_a', 'base'), node('n_b', 'base')]);
    await expect(editDocument({ ops: [{ op: 'set', node: label, params: { size: [2, 2, 2] } }] }, f.services)).rejects.toThrow(/candidates:.*n_a.*n_b/);
    expect(f.calls).toHaveLength(0);
  }
  const f = fixture();
  await expect(editDocument({ ops: [{ op: 'set', node: 'base', params: { size: [2, 2, 2] } }, { op: 'rename', node: 'lid', to: 'base' }] }, f.services)).rejects.toThrow(/already taken.*n_a/);
  expect(f.calls).toHaveLength(0);
});

test('replacement preserves output, flags and each variadic link without inserting extras', async () => {
  const f = fixture([node('n_a', 'base'), node('n_b', 'lid', { solids: [{ node: 'n_a', port: 'out' }, { node: 'n_a', port: 'out' }] })]);
  f.document.nodes[0].flags.muted = true;
  f.document.outputs = [{ name: 'body', from: { node: 'n_a', port: 'out' } }];
  let parsedBase: Document | undefined;
  f.services.parse = async (base, text) => {
    parsedBase = base;
    expect(text).toBe('base = solid.sphere(radius=2)');
    return { commands: [{ add_node: { op: 'solid.sphere@1', params: { radius: 2 }, label: 'base', alias: '$base', group: null } }] };
  };
  await editDocument({ ops: [{ op: 'replace', node: 'base', text: 'base = solid.sphere(radius=2)' }, { op: 'set', node: 'base', params: { radius: 3 } }] }, f.services);
  expect(parsedBase?.nodes.some(n => n.id === 'n_a')).toBe(false);
  expect(f.calls).toHaveLength(1);
  const commands = f.calls[0].commands;
  expect(commands.filter(c => 'disconnect' in c)).toHaveLength(2);
  expect(commands.filter(c => 'connect' in c)).toHaveLength(2);
  expect(commands).toContainEqual({ set_flags: { node: '$edit_replace_0', muted: true, bypass: false } });
  expect(commands).toContainEqual({ set_output: { name: 'body', from: { node: '$edit_replace_0', port: 'out' } } });
  expect(commands.at(-1)).toEqual({ set_param: { node: '$edit_replace_0', param: 'radius', value: 3 } });
});

test('replacement parse errors and concurrent revision changes never send commands', async () => {
  const f = fixture();
  f.services.parse = async () => ({ diagnostics: [{ code: 'parse.syntax', severity: 'error', message: 'bad text' }] } as TextReply);
  expect(await editDocument({ ops: [{ op: 'replace', node: 'base', text: 'bad' }] }, f.services)).toMatchObject({ applied: false });
  expect(f.calls).toHaveLength(0);
  f.services.parse = async () => { f.document.revision++; return { commands: [{ add_node: { op: 'solid.box@1', params: {}, label: 'base', alias: '$base', group: null } }] }; };
  await expect(editDocument({ ops: [{ op: 'replace', node: 'base', text: 'base = solid.box()' }] }, f.services)).rejects.toThrow(/document changed/);
  expect(f.calls).toHaveLength(0);
});

test('replacement accepts an instance and explicit flags, and later set uses instance parameters', async () => {
  const f = fixture();
  f.document.groups = [{ id: 'g_block', name: 'block', version: 1, output: 'n_b', params: [] }];
  f.services.parse = async () => ({ commands: [
    { add_instance: { group: 'g_block', label: 'base', alias: '$base', into: null } },
    { set_instance_param: { node: '$base', param: 'width', value: 2 } },
    { set_flags: { node: '$base', muted: true, bypass: null } },
  ] });
  await editDocument({ ops: [{ op: 'replace', node: 'base', text: 'base = block(width=2, muted=true)' }, { op: 'set', node: 'base', params: { width: 3 } }] }, f.services);
  expect(f.calls[0].commands).toContainEqual({ add_instance: { group: 'g_block', label: 'base', alias: '$edit_replace_0', into: null } });
  expect(f.calls[0].commands).toContainEqual({ set_flags: { node: '$edit_replace_0', muted: true, bypass: false } });
  expect(f.calls[0].commands.at(-1)).toEqual({ set_instance_param: { node: '$edit_replace_0', param: 'width', value: 3 } });
});

test('replacement resolves member inputs within their group and preserves its interface', async () => {
  const f = fixture([node('n_a', 'base'), node('n_b', 'lid')]);
  for (const member of f.document.nodes) member.group = 'g_block';
  f.document.groups = [{ id: 'g_block', name: 'block', version: 1, output: 'n_a', params: [{ name: 'width', node: 'n_a', param: 'radius' }] }];
  f.services.parse = async base => {
    expect(base.nodes.map(n => [n.label, n.group])).toEqual([['lid', null]]);
    return { commands: [{ add_node: { op: 'solid.sphere@1', params: { radius: 2 }, label: 'base', alias: '$base', group: null } }] };
  };
  await editDocument({ ops: [{ op: 'replace', node: 'base', text: 'base = solid.sphere(radius=2)' }] }, f.services);
  expect(f.calls[0].commands).toContainEqual({ set_group_output: { group: 'g_block', node: '$edit_replace_0' } });
  expect(f.calls[0].commands).toContainEqual({ unexpose_param: { group: 'g_block', name: 'width' } });
  expect(f.calls[0].commands).toContainEqual({ expose_param: { group: 'g_block', name: 'width', node: '$edit_replace_0', param: 'radius' } });
});

test('edit and diff validate without a document', () => {
  for (const ops of [[], [{ op: 'explode', node: 'base' }]]) expect(validateToolArguments('edit', { ops })).toMatch(/set, rename, delete, replace/);
  for (const since of ['7', null, -1, 1.5, Infinity, Number.MAX_SAFE_INTEGER + 1]) expect(validateToolArguments('diff', { since })).toMatch(/revision number/);
  expect(validateToolArguments('diff', {})).toBeNull();
});

test('diff defaults to the previous revision and reports compact added/changed/removed lines', async () => {
  const f = fixture();
  const requested: number[] = [];
  const services = {
    doc: { ...f.services.doc, atRevision(revision: number) {
      requested.push(revision);
      const lines: Record<string, string> = revision === 6 ? { n_a: 'base = solid.box()', n_c: 'old = solid.box()' } : { n_a: 'base = solid.sphere()', n_b: 'lid = solid.box()' };
      return { document: { ...f.document, revision }, lines };
    } },
  };
  const result = await diffDocument({}, services);
  expect(requested).toEqual([6, 7]);
  expect(result).toEqual({ since: 6, revision: 7, text: '~ base = solid.sphere()\n- old = solid.box()\n+ lid = solid.box()', remaining: 0 });
  expect(f.calls).toHaveLength(0);
});

test('diff caps node lines at 80 and explains unavailable revisions', async () => {
  const f = fixture();
  const services = {
    doc: { ...f.services.doc, atRevision: (revision: number): { document: Document; lines: Record<string, string> } | { error: string } => ({ document: { ...f.document, revision }, lines: revision === 0 ? {} : Object.fromEntries(Array.from({ length: 91 }, (_, i) => [`n_${i}`, `part_${i} = solid.box()`])) }) },
  };
  expect(await diffDocument({ since: 0 }, services)).toMatchObject({ remaining: 11, text: expect.stringContaining('... 11 more node lines') });
  services.doc.atRevision = () => ({ error: 'revision 0 is unavailable; retained revisions: 3..=7' });
  await expect(diffDocument({ since: 0 }, services)).rejects.toThrow(/retained revisions: 3/);
});
