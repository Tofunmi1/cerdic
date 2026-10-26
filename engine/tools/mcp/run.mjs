const timeoutMs = () => {
  const configured = Number(process.env.VE_RUN_TIMEOUT_MS ?? 120000);
  return Number.isFinite(configured) && configured > 0 ? configured : 120000;
};

/** Execute async-function code in a terminable worker; its tool calls return through callTool. */
export function callRun({ code } = {}, callTool) {
  if (typeof code !== 'string') return Promise.resolve({ content: [{ type: 'text', text: 'run requires code as a string' }], isError: true });
  return new Promise((resolve) => {
    const duration = timeoutMs();
    const worker = new Worker(new URL('./run-sandbox.mjs', import.meta.url));
    let calls = 0;
    let settled = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      worker.terminate();
      resolve(result);
    };
    const timer = setTimeout(() => {
      finish({ content: [{ type: 'text', text: `run exceeded its time limit of ${duration / 1000} seconds` }], isError: true });
    }, duration);

    worker.addEventListener('message', async ({ data }) => {
      if (data?.type === 'tool') {
        if (data.name === 'batch' || data.name === 'run') {
          worker.postMessage({ type: 'tool_result', id: data.id, isError: true, text: `${data.name} may not be called inside run` });
          return;
        }
        calls++;
        if (calls > 200) {
          finish({ content: [{ type: 'text', text: 'run exceeded the limit of 200 tool calls' }], isError: true });
          return;
        }
        let result;
        try { result = await callTool(data.name, data.args); }
        catch (error) { result = { isError: true, content: [{ type: 'text', text: error?.message ?? String(error) }] }; }
        const text = (result.content ?? []).filter((block) => block.type === 'text').map((block) => block.text).join('\n');
        worker.postMessage({ type: 'tool_result', id: data.id, isError: result.isError === true, text });
      } else if (data?.type === 'done') {
        const body = { result: data.result, logs: data.logs, ...(data.truncated ? { truncated: true } : {}) };
        finish({ content: [{ type: 'text', text: JSON.stringify(body) }], isError: false });
      } else if (data?.type === 'failure') {
        finish({ content: [{ type: 'text', text: data.error }], isError: true });
      }
    });
    worker.addEventListener('error', (event) => {
      finish({ content: [{ type: 'text', text: event.message || 'run worker failed' }], isError: true });
    });
    worker.postMessage({ type: 'execute', code });
  });
}
