import fs from 'node:fs';
import { evaluate } from '../eval.mjs';
import { sheet } from '../render.mjs';
const file = process.argv[2]; const out = process.argv[3];
const blocks = fs.readFileSync(file, 'utf8').split(/^=== /m).filter(Boolean);
const results = [], rows = [];
blocks.forEach((b, i) => {
  const nl = b.indexOf('\n'); const name = b.slice(0, nl).trim(); const r = evaluate(b.slice(nl + 1)); results.push(r);
  rows.push(r.ok ? `${String(i + 1).padStart(2)} ${name.padEnd(30)} ok   tris=${String(r.tris).padStart(6)} parts=${r.components} size=${r.size.join('x')} ${r.ms.toFixed(0)}ms vol=${r.volume.toFixed(4)}` : `${String(i + 1).padStart(2)} ${name.padEnd(30)} FAIL(${r.stage}) ${r.errors.map(e => e.msg).join(' | ')}`);
});
console.log(rows.join('\n')); sheet(out, results);
console.log(`\n${results.filter(r => r.ok).length}/${results.length} evaluate; sheet -> ${out}`);
