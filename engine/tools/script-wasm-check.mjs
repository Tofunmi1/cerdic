// node tools/script-wasm-check.mjs [dist-dir]   (default web/dist-bevy-gpu)
// Runs the script engine inside the real evaluator wasm, with the page's own wasi shim, and checks results, repeatability, error
// positions and every limit. Exits non-zero on the first wrong answer.
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const dir = path.resolve(process.argv[2] ?? new URL("../web/dist-bevy-gpu", import.meta.url).pathname);
const api = await import(pathToFileURL(path.join(dir, "ve_evaluator.js")).href);
const { bindMemory } = await import(pathToFileURL(path.join(dir, "wasi_snapshot_preview1.js")).href);
const exports = await api.default({ module_or_path: fs.readFileSync(path.join(dir, "ve_evaluator_bg.wasm")) });
bindMemory(exports.memory);

let failed = 0;
function run(source, seed = 7, polls = 0) {
  const started = performance.now();
  const answer = JSON.parse(api.script_run(source, seed, polls));
  return { ...answer, ms: performance.now() - started };
}
function check(name, ok, detail) {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failed += 1;
}

check("adds", run("1 + 2").ok === "3", run("1 + 2"));
check("json", run("[1,{a:2}]").ok === '[1,{"a":2}]', run("[1,{a:2}]"));
const a = run("[Math.random(), Math.random()].join()", 5).ok;
check("seeded random repeats", a === run("[Math.random(), Math.random()].join()", 5).ok && a !== run("[Math.random(), Math.random()].join()", 6).ok, a);
check("no clock", run("typeof Date").ok === "undefined", run("typeof Date"));
check("no outside world", run("['fetch','require','process','window','console'].map(n => typeof globalThis[n]).join()").ok === "undefined,undefined,undefined,undefined,undefined", 0);
const thrown = run("let a = 1;\nlet b = 2;\nthrow new Error('no good');");
check("error text and line", thrown.error?.message === "no good" && thrown.error.line === 3, thrown);
const unknown = run("let a = 1;\nundefinedFunction(a);");
check("reference error names kind and line", unknown.error?.message.startsWith("ReferenceError") && unknown.error.line === 2, unknown);
const endless = run("while (true) {}", 1, 50);
check("endless loop is stopped", endless.error?.stopped === true && endless.ms < 2000, endless);
const bomb = run("const k = []; while (true) k.push(new Array(1000000).fill(1.5));", 1, 1e9);
check("memory bomb is refused quickly", /memory/i.test(bomb.error?.message ?? "") && bomb.ms < 3000, bomb);
const deep = run("function f(n) { return f(n + 1) + 1; } f(0);");
check("runaway recursion is an error", /stack/i.test(deep.error?.message ?? ""), deep);
check("engine still usable after the failures", run("6 * 7").ok === "42", run("6 * 7"));

process.exit(failed === 0 ? 0 : 1);
