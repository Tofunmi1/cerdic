// The page console: a JavaScript function body with tools.<name>(args) and log(), run in a worker with limits. Every tool call goes to the
// callTool it is given (in the page, the typed-command dispatcher). Written before the code; do not weaken.
import { expect, test } from 'bun:test';
import { runConsole } from '../src/engine/console';

const worker = () => new Worker(new URL('../src/engine/console-sandbox.ts', import.meta.url));
const run = (code: string, callTool = async (_name: string, _args: Record<string, unknown>): Promise<unknown> => ({ ok: true }), options = {}) =>
  runConsole(code, callTool, { worker, ...options });

test('returns the value and the log lines, and counts the tool calls', async () => {
  const seen: Array<[string, unknown]> = [];
  const r = await run(`log('start'); const a = await tools.get_text({ format: 'compact' }); log(a); return 6 * 7;`, async (name, args) => { seen.push([name, args]); return 'text-of-document'; });
  expect(r).toEqual({ ok: true, result: 42, logs: ['start', 'text-of-document'], calls: 1 });
  expect(seen).toEqual([['get_text', { format: 'compact' }]]);
});

test('a tool answers with text: strings as they are, anything else as JSON', async () => {
  const r = await run(`const x = await tools.scene_overview({}); return [typeof x, JSON.parse(x).parts];`, async () => ({ parts: 3 }));
  expect(r.ok && r.result).toEqual(['string', 3]);
});

test('a tool that fails throws inside the code, which can catch it and read the reason', async () => {
  const r = await run(`try { await tools.apply_text({ text: 'x' }); } catch (e) { return 'caught: ' + e.message; }`, async () => { throw new Error('line 1: unknown operator'); });
  expect(r.ok && r.result).toBe('caught: line 1: unknown operator');
});

test('batch, run and console cannot be called from inside the console', async () => {
  for (const name of ['batch', 'run', 'console']) {
    const r = await run(`try { await tools.${name}({}); return 'allowed'; } catch (e) { return e.message; }`);
    expect(r.ok && r.result).toBe(`${name} may not be called inside the console`);
  }
});

test('more tool calls than the limit stops the run and says so', async () => {
  const r = await run(`for (let i = 0; i < 10; i++) await tools.get_status({}); return 'finished';`, undefined, { maxCalls: 3 });
  expect(r.ok).toBe(false);
  expect(!r.ok && r.error).toContain('more than 3 tool calls');
});

test('code that never ends is stopped at the time limit', async () => {
  const started = Date.now();
  const r = await run(`while (true) {}`, undefined, { timeoutMs: 400 });
  expect(r.ok).toBe(false);
  expect(!r.ok && r.error).toContain('longer than its limit');
  expect(Date.now() - started).toBeLessThan(5000);
});

test('an error in the code comes back as its message with the logs so far', async () => {
  const r = await run(`log('before'); throw new Error('no good');`);
  expect(r).toEqual({ ok: false, error: 'no good', logs: ['before'], calls: 0 });
});

test('import is refused and the network is not reachable', async () => {
  const imported = await run(`const m = await import('./x.js'); return 1;`);
  expect(!imported.ok && imported.error).toContain('import is not allowed');
  const reach = await run(`return ['fetch', 'XMLHttpRequest', 'WebSocket', 'Worker', 'importScripts'].map(n => typeof globalThis[n]).join();`);
  expect(reach.ok && reach.result).toBe('undefined,undefined,undefined,undefined,undefined');
});

test('a name that merely starts with import is not mistaken for an import', async () => {
  const r = await run(`const important = 5; const importCount = important + 1; return importCount;`);
  expect(r.ok && r.result).toBe(6);
});

test('a long result is cut at the limit and marked', async () => {
  const r = await run(`return 'x'.repeat(20000);`, undefined, { maxResultChars: 100 });
  expect(r.ok && (r.result as string).length).toBe(100);
  expect(r.ok && r.truncated).toBe(true);
});

test('a result that cannot be turned into JSON is an error, not a hang', async () => {
  const r = await run(`const a = {}; a.self = a; return a;`);
  expect(!r.ok && r.error).toContain('not JSON serializable');
});

test('a program can loop over tool calls, using what each answered', async () => {
  const sizes: number[] = [];
  const r = await run(`let total = 0; for (const n of [1, 2, 3]) { total += JSON.parse(await tools.measure({ part: 'p' + n })).size; } return total;`, async (_n, args) => { const size = Number(String(args.part).slice(1)) * 10; sizes.push(size); return { size }; });
  expect(r.ok && r.result).toBe(60);
  expect(sizes).toEqual([10, 20, 30]);
});
