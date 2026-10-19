import { expect, test } from 'bun:test';
// @ts-expect-error: a plain JavaScript module of the MCP server, without type declarations.
import { familyOf, fullSpecs, indexLine, indexText, paramText, portsText, select, shortDoc } from '../engine/tools/mcp/operator-index.mjs';

const box = {
  id: 'solid.box@1', doc: 'Axis-aligned box centred on the origin.', inputs: [], output: { kind: 'fixed', port: 'solid' },
  params: [{ name: 'size', ty: { type: 'vec3', unit: 'length', min: 0.000001, max: 1000000 }, default: null, doc: 'Width, height and depth in metres.' }], readiness: 'ready',
};
const loopCut = {
  id: 'poly.loop_cut@1', doc: 'Cuts a closed ring of quad faces with evenly spaced parallel edges. A second sentence that should not appear.',
  inputs: [{ name: 'in', accepts: ['poly'], arity: { kind: 'one' } }], output: { kind: 'fixed', port: 'poly' },
  params: [
    { name: 'at', ty: { type: 'vec3', unit: 'length', min: -1e6, max: 1e6 }, default: [0, 0, 0], doc: 'Point.' },
    { name: 'cuts', ty: { type: 'int', min: 1, max: 32 }, default: 1, doc: 'Number of cuts.' },
    { name: 'smooth', ty: { type: 'num', unit: 'none', min: 0, max: 1 }, default: 0, doc: 'Smoothness.' },
    { name: 'mode', ty: { type: 'choice', options: ['a', 'b', 'c'] }, default: 'a', doc: 'Mode.' },
    { name: 'flip', ty: { type: 'bool' }, default: false, doc: 'Flip.' },
  ], readiness: 'ready',
};
const union = {
  id: 'solid.union@1', doc: 'Joins solids.', inputs: [{ name: 'in', accepts: ['solid'], arity: { kind: 'variadic', min: 2 } }, { name: 'extra', accepts: ['solid'], arity: { kind: 'optional' } }],
  output: { kind: 'same_as_input', port: 'in' }, params: [], readiness: 'ready',
};
const door = { id: 'kit.door', category: 'kit', doc: 'Door 1.0 x 2.1 x 0.14 m with a frame, a leaf and a handle.', source: 'group door\n  a = solid.box@1(size=[1, 1, 1])\nend', usage: 'Paste source once, then call door()' };
const all = [box, loopCut, union, door];

test('the short documentation is the first sentence, cut with an ellipsis when long', () => {
  expect(shortDoc(loopCut.doc)).toBe('Cuts a closed ring of quad faces with evenly spaced parallel edges.');
  expect(shortDoc('A '.repeat(200) + 'end.', 20)).toHaveLength(20);
  expect(shortDoc('A '.repeat(200) + 'end.', 20).endsWith('…')).toBe(true);
  expect(shortDoc(undefined)).toBe('');
  expect(shortDoc('No full stop here')).toBe('No full stop here');
});

test('parameters read as name:type range unit=default', () => {
  expect(paramText(box.params[0])).toBe('size:vec3 m');
  expect(loopCut.params.map(paramText)).toEqual(['at:vec3 m=[0,0,0]', 'cuts:int 1..32=1', 'smooth:num 0..1=0', 'mode:a|b|c=a', 'flip:bool=false']);
  expect(paramText({ name: 'radius', ty: { type: 'num', unit: 'length', min: 1e-6, max: 1e6 }, default: null })).toBe('radius:num 0.000001..1e6 m');
});

test('angles are shown in degrees, as the text language reads them', () => {
  const tilt = { name: 'tilt', ty: { type: 'num', unit: 'angle', min: 0, max: Math.PI / 4 }, default: 0 };
  expect(paramText(tilt)).toBe('tilt:num 0..45 deg=0');
  expect(paramText({ ...tilt, name: 'lean', default: 0.5 })).toBe('lean:num 0..45 deg=28.65');
  expect(paramText({ name: 'turn', ty: { type: 'num', unit: 'angle', min: 0.001, max: 2 * Math.PI }, default: null })).toBe('turn:num 0.06..360 deg');
});

test('ports mark optional and several-link inputs', () => {
  expect(portsText(box)).toBe('');
  expect(portsText(loopCut)).toBe('in');
  expect(portsText(union)).toBe('in..., extra?');
});

test('an index line carries the id, inputs, output, parameters and the first sentence', () => {
  expect(indexLine(box)).toBe('solid.box@1() -> solid | size:vec3 m | Axis-aligned box centred on the origin.');
  expect(indexLine(union)).toBe('solid.union@1(in..., extra?) -> same as in | Joins solids.');
  expect(indexLine(loopCut)).toContain('poly.loop_cut@1(in) -> poly | at:vec3 m=[0,0,0], cuts:int 1..32=1');
  expect(indexLine(loopCut)).not.toContain('second sentence');
});

test('the index groups by family, lists kits last and is far smaller than the full list', () => {
  const text = indexText(all);
  expect(text.split('\n')[0]).toContain('3 operators in 2 families and 1 kits');
  expect(text.indexOf('# solid')).toBeLessThan(text.indexOf('# poly') === -1 ? Infinity : text.indexOf('# kits'));
  expect(text).toContain('# kits');
  expect(text).toContain('kit.door | Door 1.0 x 2.1 x 0.14 m');
  expect(text).not.toContain('group door');
  expect(text.length).toBeLessThan(JSON.stringify(all).length);
});

test('a query and a family narrow the index', () => {
  expect(select(all, { family: 'poly' }).map((o: { id: string }) => o.id)).toEqual(['poly.loop_cut@1']);
  expect(select(all, { query: 'ring quad' }).map((o: { id: string }) => o.id)).toEqual(['poly.loop_cut@1']);
  // The query is case-insensitive and reads ids and documentation, not kit sources.
  expect(select(all, { query: 'BOX' }).map((o: { id: string }) => o.id)).toEqual(['solid.box@1']);
  const narrowed = indexText(all, { family: 'solid' });
  expect(narrowed).toContain('solid.box@1');
  expect(narrowed).not.toContain('poly.loop_cut@1');
  expect(indexText(all, { query: 'nothing matches this' }).split('\n')[0]).toContain('0 operators in 0 families');
});

test('full specs come back for the named ids, with the unknown ones listed', () => {
  const { found, missing } = fullSpecs(all, ['solid.box@1', 'kit.door', 'solid.cube@1']);
  expect(found.map((o: { id: string }) => o.id)).toEqual(['solid.box@1', 'kit.door']);
  expect(missing).toEqual(['solid.cube@1']);
  expect(familyOf('inst.scatter_on@1')).toBe('inst');
});
