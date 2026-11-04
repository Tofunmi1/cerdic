import { expect, test } from 'bun:test';
import type { NodeTrace } from '../src/engine/evaluator-text';
import { inspectNodes } from '../src/engine/mcp-tools';

const node = (label: string, operator: string, summary: string, extra: Partial<NodeTrace> = {}): NodeTrace => ({
  id: `n_${label}`, label, operator, params: {}, inputs: [], kind: 'solid', summary, facts: {}, effect: [], warnings: [], ...extra,
});
const nodes: NodeTrace[] = [
  node('base', 'solid.box@1', 'a solid of 12 triangles, 1 x 1 x 1 m', { params: { size: [1, 1, 1] } }),
  node('wide', 'xform.scale@1', 'a solid of 12 triangles, 2 x 1 x 1 m', { inputs: [{ port: 'in', from: 'base' }], params: { by: [2, 1, 1] }, effect: ['size x 1 to 2 m'] }),
  node('lifted', 'xform.move@1', 'a solid of 12 triangles, 2 x 1 x 1 m', { inputs: [{ port: 'in', from: 'wide' }], params: { by: [0, 3, 0] }, effect: ['centre y 0 to 3 m'] }),
  node('n', 'value.number@1', 'the number 4', { kind: 'number' }),
];

test('without a node it lists every node in run order with what it made, and can filter by kind', () => {
  const all = inspectNodes(nodes, {}) as { total: number; nodes: { node: string }[] };
  expect(all.total).toBe(4);
  expect(all.nodes.map((n) => n.node)).toEqual(['base', 'wide', 'lifted', 'n']);
  const numbers = inspectNodes(nodes, { kind: 'number' }) as { nodes: { node: string }[] };
  expect(numbers.nodes.map((n) => n.node)).toEqual(['n']);
  const limited = inspectNodes(nodes, { limit: 2 }) as { shown: number; omitted: number };
  expect([limited.shown, limited.omitted]).toEqual([2, 2]);
});

test('for one node it says why: operator, parameters, what it made, what it changed, and the chain upstream nearest first', () => {
  const r = inspectNodes(nodes, { node: 'lifted' }) as { why: string; upstream: { node: string; changed?: string[] }[]; changed: string[]; operator: string };
  expect(r.operator).toBe('xform.move@1');
  expect(r.why).toContain('lifted = xform.move@1(wide, by=[0,3,0])');
  expect(r.why).toContain('centre y 0 to 3 m');
  expect(r.upstream.map((n) => n.node)).toEqual(['wide', 'base']);
  expect(r.upstream[0].changed).toEqual(['size x 1 to 2 m']);
});

test('depth limits the chain, a node is found by id too, and a wrong name suggests the near ones', () => {
  expect((inspectNodes(nodes, { node: 'lifted', depth: 1 }) as { upstream: unknown[] }).upstream.length).toBe(1);
  expect((inspectNodes(nodes, { node: 'n_wide' }) as { node: string }).node).toBe('wide');
  const wrong = inspectNodes(nodes, { node: 'wid' }) as { error: { code: string; suggestion: string } };
  expect(wrong.error.code).toBe('inspect.unknown_node');
  expect(wrong.error.suggestion).toContain('wide');
  expect((inspectNodes(nodes, { node: 'zzz' }) as { error: { suggestion: string } }).error.suggestion).toContain('list them');
});

test('warnings of a node are reported with it', () => {
  const warned = [node('w', 'poly.merge@1', 'a polygon mesh', { warnings: ['poly.degenerate: 2 faces had no area'] })];
  const r = inspectNodes(warned, { node: 'w' }) as { why: string; warnings: string[] };
  expect(r.warnings).toEqual(['poly.degenerate: 2 faces had no area']);
  expect(r.why).toContain('Warnings: poly.degenerate');
});
