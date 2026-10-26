// Cost and activity summaries for Codex JSONL logs and saved build runs.
const looks = new Set(['snapshot', 'set_view', 'frame_view']);
const orientation = new Set(['get_guide', 'list_operators', 'get_text', 'get_nodes', 'find', 'scene_overview']);
const builds = new Set(['apply_text', 'apply_commands', 'edit', 'batch', 'run']);

export function summarizeCodexEvents(text) {
  const summary = { toolCalls: 0, byTool: {}, errors: 0, resultChars: 0, answer: '',
    inputTokens: 0, cachedInputTokens: 0, uncachedInputTokens: 0, outputTokens: 0, looks: 0, orientation: 0, builds: 0 };
  for (const line of text.split('\n')) {
    let event;
    try { event = JSON.parse(line); } catch { continue; }
    if (event?.type === 'turn.completed') {
      summary.inputTokens += event.usage?.input_tokens ?? 0;
      summary.cachedInputTokens += event.usage?.cached_input_tokens ?? 0;
      summary.outputTokens += event.usage?.output_tokens ?? 0;
    }
    if (event?.type !== 'item.completed') continue;
    const item = event.item;
    if (item?.type === 'agent_message') summary.answer = item.text ?? '';
    if (item?.type !== 'mcp_tool_call') continue;
    summary.toolCalls++;
    summary.byTool[item.tool] = (summary.byTool[item.tool] ?? 0) + 1;
    if (item.error != null) summary.errors++;
    for (const content of item.result?.content ?? []) {
      if (typeof content.text === 'string') summary.resultChars += content.text.length;
    }
    if (looks.has(item.tool)) summary.looks++;
    if (orientation.has(item.tool)) summary.orientation++;
    if (builds.has(item.tool)) summary.builds++;
  }
  summary.uncachedInputTokens = summary.inputTokens - summary.cachedInputTokens;
  return summary;
}

export function aggregate(runs) {
  const result = { runs: runs.length };
  for (const field of ['toolCalls', 'uncachedInputTokens', 'outputTokens', 'seconds']) {
    // Unavailable measurements (for example Claude tokens) stay unavailable.
    const values = runs.map((run) => run[field]).filter(Number.isFinite).sort((a, b) => a - b);
    const n = values.length;
    result[field] = n ? { min: values[0], median: (values[Math.floor((n - 1) / 2)] + values[Math.floor(n / 2)]) / 2,
      mean: values.reduce((sum, value) => sum + value, 0) / n, max: values[n - 1] } : { min: null, median: null, mean: null, max: null };
  }
  return result;
}

export function formatTable(runs) {
  const rows = [['run', 'calls', 'uncached input tokens', 'output tokens', 'seconds'],
    ...runs.map((run) => [run.name ?? '', run.toolCalls, run.uncachedInputTokens, run.outputTokens, run.seconds]
      .map((value) => value == null ? 'n/a' : String(value)))];
  const widths = rows[0].map((_, i) => Math.max(...rows.map((row) => row[i].length)));
  return rows.map((row) => row.map((value, i) => value.padEnd(widths[i])).join('  ').trimEnd()).join('\n');
}
