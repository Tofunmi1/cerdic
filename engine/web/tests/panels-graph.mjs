// The graph helpers behind the outliner, the selection highlight and the diagnostics badges.
import assert from 'node:assert/strict';
import { diagnosticsByNode, fromDegrees, nodeName, outlineRows, partsFor, show, sinks, toDegrees } from '../panels/graph.js';

const node = (id, op, inputs = {}, extra = {}) => ({
  id, op, inputs: Object.fromEntries(Object.entries(inputs).map(([port, ids]) => [port, ids.map((n) => ({ node: n, port: 'out' }))])), ...extra,
});
// a, b -> union u -> move m (output "part"); lone box z is read by nothing and is not an output;
// c feeds both u2 and u3 (read twice); u3 is the output "other".
const doc = {
  nodes: [
    node('a', 'solid.box@1', {}, { label: 'base' }), node('b', 'solid.sphere@1'),
    node('u', 'solid.union@1', { in: ['a', 'b'] }), node('m', 'xform.move@1', { in: ['u'] }),
    node('z', 'solid.cylinder@1'),
    node('c', 'solid.box@1'), node('u2', 'solid.union@1', { in: ['c', 'a'] }), node('u3', 'solid.union@1', { in: ['u2', 'c'] }),
  ],
  outputs: [{ name: 'part', from: { node: 'm' } }, { name: 'other', from: { node: 'u3' } }],
};

assert.deepEqual([...sinks(doc)].sort(), ['m', 'u3']);
assert.deepEqual([...sinks({ ...doc, outputs: [] })].sort(), ['m', 'u3', 'z'], 'without named outputs, whatever nothing reads is shown');

assert.deepEqual(partsFor(doc, 'm'), ['m'], 'an output builds itself');
assert.deepEqual(partsFor(doc, 'a').sort(), ['m', 'u3'], 'a feeds both parts, through different paths');
assert.deepEqual(partsFor(doc, 'b'), ['m']);
assert.deepEqual(partsFor(doc, 'z'), [], 'an unused node builds nothing that is shown');

assert.equal(nodeName(doc.nodes[0]), 'base');
assert.equal(nodeName(doc.nodes[1]), 'solid.sphere');

const rows = outlineRows(doc);
const text = rows.map((row) => (row.header ? `# ${row.header}` : `${'  '.repeat(row.depth)}${row.id}${row.output ? `=${row.output}` : ''}${row.reference ? ' (again)' : ''}`));
assert.deepEqual(text, [
  'm=part', '  u', '    a', '    b',
  'u3=other', '  u2', '    c', '    a (again)', '  c (again)',
  '# Not used by a part', 'z',
], text.join('\n'));
assert.equal(rows.find((row) => row.id === 'a' && !row.reference).children, 0);
assert.equal(rows[0].children, 1);

const collapsed = outlineRows(doc, new Set(['u']));
assert.deepEqual(collapsed.filter((row) => !row.header).map((row) => row.id).slice(0, 3), ['m', 'u', 'u3'], 'a collapsed node hides what builds it');
assert.equal(collapsed[1].collapsed, true);

const muted = outlineRows({ nodes: [node('a', 'solid.box@1', {}, { flags: { muted: true, bypass: false } })], outputs: [] });
assert.equal(muted[0].muted, true);

const issues = diagnosticsByNode(
  [{ code: 'graph.cycle', node: 'u', related: ['m', 'u'] }, null],
  [{ code: 'constraint.min_wall', node: 'a' }, { code: 'parse.syntax' }],
);
assert.deepEqual(issues.get('u').map((d) => d.code), ['graph.cycle'], 'a node listed twice in one diagnostic counts once');
assert.deepEqual(issues.get('m').map((d) => d.code), ['graph.cycle']);
assert.deepEqual(issues.get('a').map((d) => d.code), ['constraint.min_wall']);
assert.equal(issues.size, 3, 'a diagnostic that names no node belongs to no node');

assert(Math.abs(fromDegrees(toDegrees(1.234)) - 1.234) < 1e-12);
assert.equal(toDegrees(Math.PI), 180);
assert.equal(show(0.1 + 0.2), '0.3');
assert.equal(show(2), '2');
assert.equal(show(0.00004), '0');
assert.equal(show(1.50001, 2), '1.5');
console.log('panels graph test passed');
