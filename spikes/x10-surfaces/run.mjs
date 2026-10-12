// Runs every task through every surface with one repair attempt per phase.
//   node run.mjs <model> [concurrency] [taskFilter]   results append to results/<model>.jsonl
import fs from 'node:fs';
import { spawn } from 'node:child_process';
import { ask } from './llm.mjs';
import { TASKS } from './tasks.mjs';
import { SURFACES, repairPrompt, editPrompt } from './surfaces.mjs';
import { retention } from './ir.mjs';

const model = process.argv[2] ?? 'sonnet';
const concurrency = Number(process.argv[3] ?? 4);
const filter = process.argv[4];
fs.mkdirSync('results', { recursive: true });
const file = `results/${model}.jsonl`;
const done = new Set(fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => { const r = JSON.parse(l); return `${r.surface}/${r.task}/${r.phase}`; }) : []);

function evaluateIsolated(payload) {
  return new Promise((resolve) => {
    const child = spawn('node', ['evalcli.mjs'], { stdio: ['pipe', 'pipe', 'pipe'] });
    let out = '';
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolve({ ok: false, stage: 'eval', errors: [{ ln: 0, msg: 'evaluation took longer than 60 seconds' }] }); }, 60000);
    child.stdout.on('data', (d) => { out += d; });
    child.on('close', () => { clearTimeout(timer); try { resolve(JSON.parse(out.trim().split('\n').pop())); } catch { resolve({ ok: false, stage: 'eval', errors: [{ ln: 0, msg: 'the evaluator crashed' }] }); } });
    child.stdin.end(JSON.stringify(payload));
  });
}

// One phase: ask, parse, evaluate; on failure one repair; returns the record and the final state.
async function phase(surfaceName, task, phaseName, prevState, before) {
  const surface = SURFACES[surfaceName];
  const totals = { cost: 0, inTokens: 0, outTokens: 0, ms: 0, calls: 0 };
  const tally = (r) => { totals.cost += r.cost; totals.inTokens += r.inTokens; totals.outTokens += r.outTokens; totals.ms += r.ms; totals.calls++; };
  const request = phaseName === 'create' ? surface.create(task) : { prompt: editPrompt(surfaceName, task, prevState) };
  let attemptReply, outcome = { firstTry: null, repaired: false, stage: null, errors: [] };
  let state = prevState, evaluated = null, parsed = null;
  for (let attempt = 0; attempt < 2; attempt++) {
    const prompt = attempt === 0 ? request.prompt : repairPrompt(surfaceName, task, phaseName, prevState, attemptReply, outcome.errors);
    const r = await ask({ model, system: surface.system, prompt, schema: surface.schema });
    tally(r);
    if (!r.ok) { outcome = { ...outcome, stage: 'model', errors: [r.error ?? 'the model call failed'] }; continue; }
    attemptReply = surfaceName === 'tools' ? r.structured?.commands ?? [] : r.text;
    parsed = surface.parse(surfaceName === 'tools' ? r.structured : r.text, phaseName === 'edit' ? prevState : undefined);
    if (!parsed.ok) { outcome = { ...outcome, stage: 'parse', errors: parsed.errors }; if (attempt === 0) outcome.firstTry = false; continue; }
    evaluated = await evaluateIsolated({ dsl: parsed.dsl, taskId: task.id, phase: phaseName, before });
    if (!evaluated.ok) { outcome = { ...outcome, stage: evaluated.stage, errors: evaluated.errors.map((e) => e.msg) }; if (attempt === 0) outcome.firstTry = false; continue; }
    outcome = { firstTry: attempt === 0, repaired: attempt === 1, stage: 'ok', errors: [] };
    state = parsed.state;
    break;
  }
  if (outcome.stage !== 'ok') outcome.firstTry = false;
  const checks = evaluated?.ok ? evaluated.checks : [];
  return {
    record: {
      surface: surfaceName, task: task.id, phase: phaseName, model, ...outcome, ok: outcome.stage === 'ok',
      checksPassed: checks.filter((c) => c[1]).length, checksTotal: checks.length, failed: checks.filter((c) => !c[1]).map((c) => c[0]),
      ...totals, tris: evaluated?.tris, dsl: parsed?.dsl, ir: parsed?.ir, parts: evaluated?.parts,
    },
    state: outcome.stage === 'ok' ? state : null, snap: evaluated?.snap ?? null, ir: parsed?.ok ? parsed.ir : null,
  };
}

async function runTask(surfaceName, task) {
  const key = (p) => `${surfaceName}/${task.id}/${p}`;
  const created = await phase(surfaceName, task, 'create', undefined, undefined);
  if (!done.has(key('create'))) { fs.appendFileSync(file, JSON.stringify(created.record) + '\n'); done.add(key('create')); }
  if (task.edit && created.state && !done.has(key('edit'))) {
    const edited = await phase(surfaceName, task, 'edit', created.state, created.snap);
    if (created.ir && edited.ir) edited.record.retention = retention(created.ir, edited.ir);
    fs.appendFileSync(file, JSON.stringify(edited.record) + '\n'); done.add(key('edit'));
  }
  console.log(`${model} ${surfaceName.padEnd(5)} ${task.id.padEnd(15)} create ${created.record.ok ? (created.record.firstTry ? 'ok' : 'repaired') : 'FAIL'} ${created.record.checksPassed}/${created.record.checksTotal}`);
}

const queue = [];
for (const task of TASKS) {
  if (filter && !task.id.includes(filter)) continue;
  for (const surfaceName of Object.keys(SURFACES)) if (!done.has(`${surfaceName}/${task.id}/create`)) queue.push([surfaceName, task]);
}
console.log(`${queue.length} jobs for ${model}, concurrency ${concurrency}`);
await Promise.all(Array.from({ length: concurrency }, async () => { while (queue.length) { const [s, t] = queue.shift(); await runTask(s, t); } }));
console.log('done');
