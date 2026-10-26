// tools/mcp/run-metrics.mjs: what one agent run cost (calls by tool, tokens, time, errors) read from the Codex event log, and how several runs compare.
// bun tools/run-metrics-test.mjs
import { summarizeCodexEvents, aggregate, formatTable } from "./mcp/run-metrics.mjs";

let failed = 0;
const check = (name, ok, detail) => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : `: ${String(detail).slice(0, 500)}`}`); if (!ok) failed++; };

const call = (id, tool, args, text, error = null) => [
  { type: "item.started", item: { id, type: "mcp_tool_call", server: "spatial-engine", tool, arguments: args, result: null, error: null, status: "in_progress" } },
  { type: "item.completed", item: { id, type: "mcp_tool_call", server: "spatial-engine", tool, arguments: args, result: error ? null : { content: [{ type: "text", text }] }, error, status: error ? "failed" : "completed" } },
];
const events = [
  { type: "thread.started", thread_id: "t" },
  { type: "turn.started" },
  { type: "item.completed", item: { id: "item_0", type: "agent_message", text: "I will build it." } },
  ...call("item_1", "get_guide", { topics: ["core"] }, "g".repeat(1000)),
  ...call("item_2", "apply_text", { text: "a = solid.box(size=1)" }, "ok"),
  ...call("item_3", "apply_text", { text: "b = bad(" }, "", { message: "parse error" }),
  ...call("item_4", "batch", { steps: [] }, "{}"),
  ...call("item_5", "snapshot", { cameras: [] }, "s".repeat(200)),
  { type: "item.completed", item: { id: "item_6", type: "agent_message", text: "Built a thing." } },
  { type: "turn.completed", usage: { input_tokens: 5000, cached_input_tokens: 4000, cache_write_input_tokens: 0, output_tokens: 300, reasoning_output_tokens: 120 } },
];
const text = events.map((e) => JSON.stringify(e)).join("\n") + "\nnot json at all\n";

const s = summarizeCodexEvents(text);
check("counts completed tool calls once each (started events are not counted)", s.toolCalls === 5, s.toolCalls);
check("counts calls by tool", s.byTool.apply_text === 2 && s.byTool.get_guide === 1 && s.byTool.batch === 1 && s.byTool.snapshot === 1, JSON.stringify(s.byTool));
check("counts failed calls", s.errors === 1, s.errors);
check("reads the token usage", s.inputTokens === 5000 && s.cachedInputTokens === 4000 && s.outputTokens === 300, JSON.stringify(s));
check("uncached input is input minus cached", s.uncachedInputTokens === 1000, s.uncachedInputTokens);
check("sums the characters the tools sent back", s.resultChars === 1000 + 2 + 2 + 200, s.resultChars);
check("keeps the last agent message as the answer", s.answer === "Built a thing.", s.answer);
check("counts lookups (snapshot and set_view) and orientation (get_guide, list_operators, get_text, get_nodes, find)", s.looks === 1 && s.orientation === 1, `${s.looks} ${s.orientation}`);
check("counts build calls (apply_text, apply_commands, edit, batch, run)", s.builds === 3, s.builds);
check("an empty or broken log gives zeros, not a crash", summarizeCodexEvents("").toolCalls === 0 && summarizeCodexEvents("{").inputTokens === 0, "");

const runs = [
  { name: "a", toolCalls: 10, inputTokens: 100, cachedInputTokens: 50, uncachedInputTokens: 50, outputTokens: 10, seconds: 60 },
  { name: "b", toolCalls: 30, inputTokens: 300, cachedInputTokens: 250, uncachedInputTokens: 50, outputTokens: 30, seconds: 180 },
  { name: "c", toolCalls: 20, inputTokens: 200, cachedInputTokens: 100, uncachedInputTokens: 100, outputTokens: 20, seconds: 120 },
];
const a = aggregate(runs);
check("aggregate gives runs, min, median, mean and max of calls", a.runs === 3 && a.toolCalls.min === 10 && a.toolCalls.median === 20 && a.toolCalls.max === 30 && a.toolCalls.mean === 20, JSON.stringify(a.toolCalls));
check("aggregate does the same for uncached input tokens, output tokens and seconds", a.uncachedInputTokens.median === 50 && a.outputTokens.max === 30 && a.seconds.mean === 120, JSON.stringify(a));
check("aggregate of nothing is zero runs", aggregate([]).runs === 0, "");
const table = formatTable(runs);
check("formatTable has a row per run and a header with calls, tokens and seconds", table.split("\n").length >= 4 && /calls/i.test(table) && /seconds|time/i.test(table) && /a\b/.test(table) && /b\b/.test(table), table);

console.log(failed ? `\n${failed} check(s) failed` : "\nall checks passed");
process.exit(failed ? 1 : 0);
