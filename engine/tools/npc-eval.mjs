// Compares models as NPC minds: runs one scripted conversation with a character (default: the built-in baker Anna, who has a locked secret)
// through ve-npc's chat example and prints one row per model: how many replies were valid, fell back, needed a repair, had a secret leak blocked,
// passed the scenario's checks, and how long they took. Every number comes from ve-npc's own report code (`run_script`).
//
//   bun engine/tools/npc-eval.mjs [--models haiku,sonnet,opus] [--scenario engine/tools/npc/scenario-anna.json] [--character file.json]
//                                 [--cmd "claude -p --model {model} ..."] [--timeout 90] [--out dir]
// The default command is the Claude CLI with tools off; `{model}` in --cmd is replaced by each model name. Needs `claude` on the PATH for the default.
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const engine = resolve(here, "..");
const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : fallback; };
const models = flag("--models", "haiku,sonnet,opus").split(",");
const scenario = resolve(flag("--scenario", join(here, "npc", "scenario-anna.json")));
const character = flag("--character", null);
const timeout = flag("--timeout", "90");
const out = resolve(flag("--out", join(engine, "target", "npc-eval")));
const command = flag("--cmd", 'claude -p --model {model} --tools "" --no-session-persistence --system-prompt "You are the dialogue engine of a video game. Follow the reply format in the prompt exactly and output only the JSON object."');

const env = { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(engine, "target") };
const build = spawnSync("cargo", ["build", "-q", "-p", "ve-npc", "--example", "chat"], { cwd: engine, env, stdio: "inherit" });
if (build.status !== 0) process.exit(build.status ?? 1);
const binary = join(env.CARGO_TARGET_DIR, "debug", "examples", "chat");
mkdirSync(out, { recursive: true });

const rows = [];
for (const model of models) {
  const cmd = command.replaceAll("{model}", model);
  const started = Date.now();
  const run = spawnSync(binary, ["--script", scenario, ...(character ? ["--character", character] : []), "--cmd", cmd, "--timeout", timeout], { cwd: engine, env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  let report;
  try { report = JSON.parse(run.stdout); } catch { rows.push({ model, error: (run.stderr || run.stdout || "no output").slice(0, 200) }); continue; }
  writeFileSync(join(out, `${model}.json`), JSON.stringify(report, null, 2));
  const s = report.summary;
  rows.push({ model, turns: s.turns, valid: `${Math.round(s.valid_rate * 100)}%`, fallback: `${Math.round(s.fallback_rate * 100)}%`, retries: s.retries, leaks_blocked: s.leaks_blocked, checks: `${s.checks_passed}/${s.checks_total}`, median_s: (s.median_ms / 1000).toFixed(1), p95_s: (s.p95_ms / 1000).toFixed(1), wall_s: ((Date.now() - started) / 1000).toFixed(0) });
}
console.table(rows);
console.log(`full reports (every reply and check): ${out}/<model>.json`);
