import fs from 'node:fs';
import { compile } from '../lang.mjs';
const blocks = fs.readFileSync(process.argv[2], 'utf8').split(/^=== /m).filter(Boolean);
let ok = 0;
for (const b of blocks) {
  const nl = b.indexOf('\n'); const name = b.slice(0, nl).trim(); const r = compile(b.slice(nl + 1));
  if (r.ok) ok++;
  console.log(`${r.ok ? 'PASS' : 'FAIL'}  ${name}${r.errors.map(e => `\n      L${e.ln}: ${e.msg}`).join('')}`);
}
console.log(`\n${ok}/${blocks.length} compile on first try`);
