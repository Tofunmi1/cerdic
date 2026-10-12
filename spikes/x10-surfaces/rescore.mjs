// Re-runs the checks over the stored programs (no model calls), after a check was corrected.
// Writes results-rescored/<model>.jsonl with the same fields, new check results.
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';

function evaluate(payload) {
  const r = spawnSync('node', ['evalcli.mjs'], { input: JSON.stringify(payload), encoding: 'utf8', timeout: 90000 });
  try { return JSON.parse(r.stdout.trim().split('\n').pop()); } catch { return { ok: false, stage: 'eval', errors: [{ msg: 'evaluator crashed' }] }; }
}

fs.mkdirSync('results-rescored', { recursive: true });
for (const file of fs.readdirSync('results').filter((f) => f.endsWith('.jsonl'))) {
  const rows = fs.readFileSync(`results/${file}`, 'utf8').trim().split('\n').map(JSON.parse);
  const snaps = new Map();
  const out = [];
  for (const phase of ['create', 'edit']) {
    for (const r of rows.filter((x) => x.phase === phase)) {
      if (r.ok && r.dsl) {
        const key = `${r.surface}/${r.task}`;
        const before = phase === 'edit' ? snaps.get(key) : undefined;
        const e = evaluate({ dsl: r.dsl, taskId: r.task, phase, before });
        if (phase === 'create') snaps.set(key, e.snap);
        if (e.ok) { r.checksPassed = e.checks.filter((c) => c[1]).length; r.checksTotal = e.checks.length; r.failed = e.checks.filter((c) => !c[1]).map((c) => c[0]); }
      } else if (phase === 'create') snaps.set(`${r.surface}/${r.task}`, undefined);
      out.push(r);
    }
  }
  fs.writeFileSync(`results-rescored/${file}`, out.map((r) => JSON.stringify(r)).join('\n') + '\n');
  console.log(file, out.length, 'rows rescored');
}
