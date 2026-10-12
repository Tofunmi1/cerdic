// Child process: evaluate one program and run its task checks. Isolated so a runaway program can be
// killed by the parent without taking the harness down. stdin: {dsl, taskId, phase, before}.
import { evaluate } from './eval.mjs';
import { TASKS } from './tasks.mjs';

const chunks = [];
for await (const c of process.stdin) chunks.push(c);
const { dsl, taskId, phase, before } = JSON.parse(Buffer.concat(chunks).toString());
const task = TASKS.find((t) => t.id === taskId);
const res = evaluate(dsl);
if (!res.ok) {
  console.log(JSON.stringify({ ok: false, stage: res.stage, errors: res.errors.map((e) => ({ ln: e.ln, node: e.node, msg: e.msg })) }));
  process.exit(0);
}
let checks;
try {
  checks = phase === 'edit' ? task.edit.checks(res, before ?? {}) : task.checks(res);
} catch (e) {
  checks = [[`checks threw: ${String(e.message ?? e).slice(0, 120)}`, false]];
}
let snap = null;
try { snap = task.snap ? task.snap(res) : null; } catch { snap = null; }
console.log(JSON.stringify({
  ok: true, ms: res.ms, tris: res.tris, checks, snap,
  parts: res.outputs.map((o) => ({ part: o.part, size: o.size, volume: o.volume, components: o.components, genus: o.genus, material: o.material })),
}));
