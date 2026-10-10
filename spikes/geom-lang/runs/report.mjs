// prints the feedback an LLM would receive from compile+analyze for each block
import fs from 'node:fs';
import { evaluate } from '../eval.mjs';
const only = process.argv[3];
const blocks = fs.readFileSync(process.argv[2], 'utf8').split(/^=== /m).filter(Boolean);
for (const b of blocks) {
  const nl = b.indexOf('\n'); const name = b.slice(0, nl).trim(); if (only && !name.includes(only)) continue;
  const r = evaluate(b.slice(nl + 1));
  if (!r.ok) { console.log(`${name}: ERROR ${r.errors.map(e => e.msg)}`); continue; }
  if (r.components > 1) {
    const big = r.gaps.length > 12 ? r.gaps.slice(0, 4) : r.gaps; const touch = r.gaps.filter(g => g.gap === 0).length, float = r.gaps.length - touch;
    console.log(`${name}: ${r.components} separate pieces (${touch} touching-without-overlap, ${float} floating). Overlap touching parts by >=1mm; close floating gaps. ` + big.map(g => `#${g.component} size${JSON.stringify(g.size)} at${JSON.stringify(g.center)} ${g.gap === 0 ? 'touches' : 'gap ' + g.gap + 'm to'} #${g.nearest}`).join('; ') + (r.gaps.length > 12 ? ` ...and ${r.gaps.length - 4} more` : ''));
  }
  else console.log(`${name}: one piece, size ${r.size.join('x')}${r.voids ? ` (${r.voids} sealed internal void${r.voids > 1 ? 's' : ''})` : ''}`);
}
