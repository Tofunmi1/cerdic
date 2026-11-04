// The page console: a person (or the in-page model) writes a JavaScript function body that calls `tools.<name>(args)` and `log(...)`,
// the same shape as the MCP `run` tool, so a program that works there works here. Each tool call goes to `callTool`, which in the page
// is the same dispatcher the MCP bridge uses, so the console edits the document only through typed commands and can do nothing a tool
// call cannot. Limits: 200 tool calls, 120 s, 8,000 characters of result; `batch` and `run` may not be called from inside it.
export type ConsoleOptions = {
  /** Makes the worker (the page passes the bundled one; tests pass their own). */
  worker?: () => Worker;
  maxCalls?: number;
  timeoutMs?: number;
  maxResultChars?: number;
};

export type ConsoleResult =
  | { ok: true; result: unknown; logs: string[]; calls: number; truncated?: boolean }
  | { ok: false; error: string; logs: string[]; calls: number };

export const CONSOLE_LIMITS = { calls: 200, timeoutMs: 120_000, resultChars: 8000 } as const;
const BLOCKED = new Set(['batch', 'run', 'console']);

/** What a tool answers, as the text the worker's `tools.x()` resolves to (strings as they are, anything else as JSON). */
const asText = (value: unknown) => (typeof value === 'string' ? value : JSON.stringify(value) ?? 'null');

export function bundledWorker() {
  return new Worker(new URL('./console-sandbox.ts', import.meta.url), { type: 'module' });
}

export function runConsole(
  code: string,
  callTool: (name: string, args: Record<string, unknown>) => Promise<unknown>,
  options: ConsoleOptions = {},
): Promise<ConsoleResult> {
  const maxCalls = options.maxCalls ?? CONSOLE_LIMITS.calls;
  const timeoutMs = options.timeoutMs ?? CONSOLE_LIMITS.timeoutMs;
  const limit = options.maxResultChars ?? CONSOLE_LIMITS.resultChars;
  return new Promise((resolve) => {
    let worker: Worker;
    try { worker = (options.worker ?? bundledWorker)(); }
    catch (error) { resolve({ ok: false, error: `the console worker did not start: ${String(error)}`, logs: [], calls: 0 }); return; }
    let calls = 0;
    let settled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const finish = (result: ConsoleResult) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      worker.terminate();
      resolve(result);
    };
    timer = setTimeout(() => finish({ ok: false, error: `the console code ran longer than its limit of ${timeoutMs / 1000} seconds`, logs: [], calls }), timeoutMs);
    worker.addEventListener('message', async (event: MessageEvent) => {
      const data = event.data as { type: string; id: number; name: string; args: Record<string, unknown>; result: unknown; logs: string[]; truncated?: boolean; error: string };
      if (data.type === 'tool') {
        if (BLOCKED.has(data.name)) { worker.postMessage({ type: 'tool_result', id: data.id, isError: true, text: `${data.name} may not be called inside the console` }); return; }
        calls++;
        if (calls > maxCalls) { finish({ ok: false, error: `the console code made more than ${maxCalls} tool calls`, logs: [], calls }); return; }
        try {
          const answer = await callTool(data.name, data.args);
          if (!settled) worker.postMessage({ type: 'tool_result', id: data.id, isError: false, text: asText(answer) });
        } catch (error) {
          if (!settled) worker.postMessage({ type: 'tool_result', id: data.id, isError: true, text: error instanceof Error ? error.message : String(error) });
        }
      } else if (data.type === 'done') {
        finish({ ok: true, result: data.result, logs: data.logs, calls, ...(data.truncated ? { truncated: true } : {}) });
      } else if (data.type === 'failure') {
        finish({ ok: false, error: data.error, logs: data.logs ?? [], calls });
      }
    });
    worker.addEventListener('error', (event: ErrorEvent) => finish({ ok: false, error: event.message || 'the console worker failed', logs: [], calls }));
    worker.postMessage({ type: 'execute', code, limit });
  });
}
