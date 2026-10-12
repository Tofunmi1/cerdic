// Aggregates results/*.jsonl into per-model, per-surface tables.
import fs from 'node:fs';

const dir = process.argv[2] ?? 'results';
const models = fs.readdirSync(dir).filter((f) => f.endsWith('.jsonl')).map((f) => f.replace('.jsonl', ''));
const pct = (a, b) => (b ? `${Math.round((100 * a) / b)}%` : '-');
const mean = (xs) => (xs.length ? xs.reduce((s, x) => s + x, 0) / xs.length : 0);

for (const model of models) {
  const rows = fs.readFileSync(`${dir}/${model}.jsonl`, 'utf8').trim().split('\n').map(JSON.parse);
  console.log(`\n## ${model}`);
  for (const phase of ['create', 'edit']) {
    console.log(`\n${phase}: surface | n | valid first try | valid after one repair | all checks pass | mean checks passed | $ per task | out tokens | seconds`);
    for (const surface of ['dsl', 'tools', 'code']) {
      const r = rows.filter((x) => x.surface === surface && x.phase === phase);
      if (!r.length) continue;
      const valid = r.filter((x) => x.ok), first = r.filter((x) => x.firstTry);
      const full = valid.filter((x) => x.checksPassed === x.checksTotal);
      console.log(`${surface.padEnd(5)} | ${r.length} | ${pct(first.length, r.length)} | ${pct(valid.length, r.length)} | ${pct(full.length, r.length)} (${full.length}/${r.length}) | ${pct(mean(r.map((x) => (x.checksTotal ? x.checksPassed / x.checksTotal : 0))) * 100, 100)} | $${mean(r.map((x) => x.cost)).toFixed(3)} | ${Math.round(mean(r.map((x) => x.outTokens)))} | ${Math.round(mean(r.map((x) => x.ms)) / 1000)}`);
    }
  }
  const edits = rows.filter((x) => x.phase === 'edit' && x.retention);
  if (edits.length) {
    console.log('\nedit drift: surface | mean node content kept | mean ids kept');
    for (const surface of ['dsl', 'tools', 'code']) {
      const r = edits.filter((x) => x.surface === surface);
      if (r.length) console.log(`${surface.padEnd(5)} | ${pct(mean(r.map((x) => x.retention.kept)) * 100, 100)} | ${pct(mean(r.map((x) => x.retention.idStable)) * 100, 100)}`);
    }
  }
  console.log('\nfailing checks:');
  const tally = new Map();
  for (const x of rows) for (const f of x.failed ?? []) { const k = `${x.surface}/${x.task}/${x.phase}: ${f}`; tally.set(k, (tally.get(k) ?? 0) + 1); }
  for (const [k] of tally) console.log(' ', k);
  console.log('\nnot valid even after repair:');
  for (const x of rows.filter((x) => !x.ok)) console.log(` ${x.surface}/${x.task}/${x.phase} [${x.stage}] ${String(x.errors?.[0] ?? '').slice(0, 140)}`);
}
