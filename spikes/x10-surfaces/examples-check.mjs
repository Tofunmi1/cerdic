// The worked examples in every surface's prompt must themselves evaluate, in all three forms.
import { SURFACES } from './surfaces.mjs';
import { evaluate } from './eval.mjs';

const sys = (s) => SURFACES[s].system;
for (const surface of ['dsl', 'tools', 'code']) {
  const text = sys(surface);
  let blocks;
  if (surface === 'tools') {
    blocks = [...text.matchAll(/Example \d commands:\n(\{"commands".*)/g)].map((m) => JSON.parse(m[1]));
  } else {
    blocks = [...text.matchAll(/```(?:js)?\n([\s\S]*?)```/g)].map((m) => m[1]);
  }
  blocks.forEach((b, i) => {
    const r = SURFACES[surface].parse(surface === 'tools' ? b : '```\n' + b + '```', undefined);
    if (!r.ok) { console.log(surface, i + 1, 'PARSE FAIL', r.errors); return; }
    const e = evaluate('', r.compiled);
    if (!e.ok) { console.log(surface, i + 1, 'EVAL FAIL', e.errors); return; }
    console.log(surface, i + 1, e.outputs.map((o) => `${o.part}:${o.size.join('x')} v=${o.volume.toFixed(6)} comps=${o.components} genus=${o.genus}`).join(' | '));
  });
}
