// The limits page must say what the code says: every row's number is found in its source file by its pattern, and the page text mentions every row.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { LIMITS, LIMITS_TOPIC, limitsText } from './mcp/guide-limits.mjs';

const root = new URL('../', import.meta.url);
let checked = 0;
for (const row of LIMITS) {
  const text = readFileSync(new URL(row.file, root), 'utf8');
  const found = new RegExp(row.pattern).exec(text);
  assert.ok(found, `${row.what}: "${row.pattern}" not found in ${row.file}`);
  if (found[1] !== undefined) {
    assert.equal(Number(found[1].replace(/_/g, '')), row.number, `${row.what}: the page says ${row.value} but ${row.file} has ${found[1]}`);
  }
  checked += 1;
}
const page = limitsText();
for (const row of LIMITS) assert.ok(page.includes(row.what) && page.includes(row.value), `the page lists: ${row.what}`);
assert.equal(LIMITS_TOPIC.text, page);
assert.ok(page.length < 9000, `the page stays short enough to read in one go (${page.length} characters)`);
// the error codes the page names are raised somewhere in the Rust source
const budget = readFileSync(new URL('crates/ve-modeling/src/budget.rs', root), 'utf8');
for (const row of LIMITS.filter(r => r.code && r.code.startsWith('budget.') && r.code !== 'budget.drawn_heavy')) {
  const everywhere = [budget, readFileSync(new URL('crates/ve-modeling/src/ao.rs', root), 'utf8')].join('\n');
  assert.ok(everywhere.includes(`"${row.code}"`), `${row.code} is a code the engine raises`);
}
console.log(`limits ok: ${checked} rows match the code, ${page.length} characters`);
