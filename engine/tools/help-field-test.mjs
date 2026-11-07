// help_field: exact answers for a parameter, an operator, a family, a bare name and a wrong path.
// Runs on a small fixture; set OPERATORS_JSON to a dump of `doc_operators()` to check every operator and parameter of the real list too.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { distance, helpField, nearest } from './mcp/help-field.mjs';

const port = (name, accepts) => ({ name, accepts: [accepts], arity: { kind: 'one' } });
const ops = [
  {
    id: 'solid.sweep@1', doc: 'Sweeps a closed profile along a curve (a pipe, rope, horn). scale_end tapers the tube.', readiness: 'ready',
    inputs: [port('curve', 'curve'), port('profile', 'profile')], output: { kind: 'fixed', port: 'solid' },
    params: [
      { name: 'scale_end', ty: { type: 'num', unit: 'ratio', min: 0.05, max: 4 }, default: 1, doc: 'Size of the profile at the end of the curve relative to the start.' },
      { name: 'twist', ty: { type: 'num', unit: 'angle', min: -50.26548245743669, max: 50.26548245743669 }, default: 0, doc: 'Total turn in radians.' },
    ],
  },
  {
    id: 'solid.box@1', doc: 'Axis-aligned box centred on the origin.', readiness: 'ready', inputs: [], output: { kind: 'fixed', port: 'solid' },
    params: [{ name: 'size', ty: { type: 'vec3', unit: 'length' }, default: null, doc: 'Width, height and depth in metres.' }],
  },
  {
    id: 'game.rule@1', doc: 'Declare an event rule.', readiness: 'ready', inputs: [], output: { kind: 'fixed', port: 'game' },
    params: [
      { name: 'on', ty: { type: 'str' }, default: null, doc: 'Event name.' },
      { name: 'mode', ty: { type: 'choice', options: ['a', 'b'] }, default: 'a', doc: 'Which way.' },
    ],
  },
];

const parameter = helpField(ops, 'solid.sweep.scale_end');
assert.ok(parameter.ok);
assert.match(parameter.text, /solid\.sweep\.scale_end/);
assert.match(parameter.text, /scale_end:num 0\.05\.\.4/);
assert.match(parameter.text, /range 0\.05 to 4 \(ratio\)/);
assert.match(parameter.text, /Size of the profile/);
assert.equal(helpField(ops, 'solid.sweep@1.scale_end').text, parameter.text, 'the version is optional');

assert.match(helpField(ops, 'solid.sweep.twist').text, /range -2880 to 2880 degrees/, 'angles are shown in degrees');
assert.match(helpField(ops, 'solid.box.size').text, /required: no default/);
assert.match(helpField(ops, 'game.rule.mode').text, /one of: a, b/);

const operator = helpField(ops, 'solid.sweep');
assert.ok(operator.ok);
assert.match(operator.text, /solid\.sweep@1\(curve, profile\) -> solid/);
assert.match(operator.text, /scale_end:num/);
assert.match(operator.text, /twist:num/);

const family = helpField(ops, 'solid');
assert.ok(family.ok);
assert.match(family.text, /solid: 2 operators/);
assert.match(family.text, /solid\.box@1/);

const bare = helpField(ops, 'size');
assert.ok(bare.ok);
assert.match(bare.text, /solid\.box\.size/);

// wrong paths lead to the right one
const typo = helpField(ops, 'solid.sweep.scale_edn');
assert.ok(!typo.ok);
assert.match(typo.text, /no parameter "scale_edn"/);
assert.match(typo.text, /Nearest: scale_end/);
const wrongFamily = helpField(ops, 'sold.sweep.scale_end');
assert.ok(!wrongFamily.ok);
assert.match(wrongFamily.text, /Nearest: solid\.sweep\.scale_end/);
const guessed = helpField(ops, 'solid.sweep.taper');
assert.ok(!guessed.ok);
assert.match(guessed.text, /Its parameters: scale_end, twist/);
assert.ok(!helpField(ops, '').ok);
assert.ok(!helpField(ops, 'zzzz').ok);
assert.equal(nearest(ops, 'solid.boxx')[0], 'solid.box');
assert.equal(distance('kitten', 'sitting'), 3);

if (process.env.OPERATORS_JSON) {
  const all = JSON.parse(readFileSync(process.env.OPERATORS_JSON, 'utf8')).filter(op => op.readiness === 'ready');
  let checked = 0;
  for (const op of all) {
    const named = helpField(all, op.id);
    assert.ok(named.ok, `${op.id} has an answer`);
    for (const param of op.params ?? []) {
      const answer = helpField(all, `${op.id.split('@')[0]}.${param.name}`);
      assert.ok(answer.ok, `${op.id}.${param.name} has an answer`);
      assert.ok(String(param.doc ?? '').trim().length > 0, `${op.id}.${param.name} has documentation`);
      checked += 1;
    }
  }
  console.log(`checked ${all.length} operators and ${checked} parameters`);
}
console.log('help-field ok');
