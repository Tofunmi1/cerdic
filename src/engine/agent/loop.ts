// The agent loop: call the model, run the tools it asks for, send the results back, repeat until it answers without a tool or a budget or a cancel stops it.
// It runs in the page; the tools are plain functions (`callTool`). Design notes are in docs/plans/llm-pipeline-and-harness.md.
//  - The history is kept whole (for the log and for resuming); each request is a trimmed view of it: old tool results are cleared and only the newest pictures stay.
//  - The system prompt and tools are passed through untouched every turn, so a provider can cache them as a prefix.
//  - Whatever stops the run, every tool_use in the history has its tool_result, so the conversation can be continued.
import type { Block, ImageBlock, Message, Provider, TextBlock, ToolResultBlock, ToolSpec, ToolUseBlock, Usage } from './types';

export type AgentEvent =
  | { type: 'turn'; turn: number }
  | { type: 'text'; text: string }
  | { type: 'tool_call'; id: string; name: string; input: Record<string, unknown> }
  | { type: 'tool_result'; id: string; name: string; isError: boolean; chars: number; images: number }
  | { type: 'done'; stop: string };

export type Budget = { turns?: number; toolCalls?: number; tokens?: number; millis?: number };

export type AgentOptions = {
  provider: Provider;
  system: string;
  tools: ToolSpec[];
  callTool: (name: string, input: Record<string, unknown>) => Promise<unknown>;
  messages: Message[];
  model?: string;
  maxTokens?: number;
  budget?: Budget;
  /** Characters of one tool result sent to the model; the rest is cut with a note (default 6000). */
  maxResultChars?: number;
  /** How many of the newest tool results are sent whole; older ones are replaced by a one-line marker (default 8). */
  keepResults?: number;
  /** How many of the newest pictures are sent (default 2). */
  keepImages?: number;
  signal?: AbortSignal;
  onEvent?: (event: AgentEvent) => void;
  now?: () => number;
};

export type AgentResult = {
  stop: 'end_turn' | 'max_tokens' | 'cancelled' | 'provider_error' | 'budget:turns' | 'budget:tool_calls' | 'budget:tokens' | 'budget:time';
  messages: Message[];
  /** The text of the last assistant message. */
  text: string;
  turns: number;
  toolCalls: number;
  usage: Usage;
  error?: string;
};

type Picture = { media_type: string; data: string };
export type FormattedResult = { text: string; images: Picture[]; isError: boolean };

/** A tool's answer as the model sees it: pictures split out as image blocks (snapshot, bake) and the rest as indented JSON. */
export function formatToolResult(name: string, value: unknown): FormattedResult {
  if (typeof value === 'string') return { text: value, images: [], isError: false };
  const images: Picture[] = [];
  let rest: unknown = value;
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    const { jpegBase64, sheets, maps, ...others } = value as Record<string, unknown>;
    const body: Record<string, unknown> = { ...others };
    if (typeof jpegBase64 === 'string') images.push({ media_type: 'image/jpeg', data: jpegBase64 });
    if (Array.isArray(sheets)) {
      body.sheets = sheets.map((sheet) => {
        const { jpegBase64: data, ...kept } = (sheet ?? {}) as Record<string, unknown>;
        if (typeof data === 'string') images.push({ media_type: 'image/jpeg', data });
        return kept;
      });
    }
    if (maps && typeof maps === 'object') {
      for (const data of Object.values(maps as Record<string, unknown>)) if (typeof data === 'string') images.push({ media_type: 'image/png', data });
      body.maps = Object.keys(maps as Record<string, unknown>);
    }
    rest = body;
  }
  const record = (rest ?? {}) as Record<string, unknown>;
  const isError = record.ok === false || (typeof record.error === 'string' && record.ok !== true);
  void name;
  return { text: JSON.stringify(rest, null, 2) ?? 'null', images, isError };
}

const textOf = (blocks: Block[]) => blocks.filter((b): b is TextBlock => b.type === 'text').map((b) => b.text).join('\n');
const resultChars = (r: ToolResultBlock) => (typeof r.content === 'string' ? r.content.length : r.content.reduce((n, b) => n + (b.type === 'text' ? b.text.length : 0), 0));

/**
 * The messages for one request: the newest `keepResults` tool results whole, older ones replaced by a marker that names the tool and says how much was dropped, and
 * only the newest `keepImages` pictures. Every tool_use keeps a tool_result, so the request stays valid.
 */
export function trimHistory(messages: Message[], keepResults: number, keepImages: number): Message[] {
  const names = new Map<string, string>();
  for (const m of messages) if (Array.isArray(m.content)) for (const b of m.content) if (b.type === 'tool_use') names.set(b.id, b.name);
  const all: ToolResultBlock[] = [];
  for (const m of messages) if (Array.isArray(m.content)) for (const b of m.content) if (b.type === 'tool_result') all.push(b);
  const clearFrom = Math.max(0, all.length - keepResults);
  let pictures = 0;
  const pictureAllowance = new Map<ToolResultBlock, number>();
  for (let i = all.length - 1; i >= 0; i--) {
    const r = all[i];
    if (!Array.isArray(r.content) || i < clearFrom) continue;
    const have = r.content.filter((b) => b.type === 'image').length;
    const allow = Math.max(0, Math.min(have, keepImages - pictures));
    pictures += allow;
    pictureAllowance.set(r, allow);
  }
  return messages.map((m) => {
    if (!Array.isArray(m.content)) return m;
    return { ...m, content: m.content.map((b): Block => {
      if (b.type !== 'tool_result') return b;
      const index = all.indexOf(b);
      if (index < clearFrom) return { ...b, content: `[cleared: the ${names.get(b.tool_use_id) ?? 'tool'} result, ${resultChars(b)} characters, was dropped to keep the conversation short; call the tool again if you need it]` };
      if (!Array.isArray(b.content)) return b;
      let allow = pictureAllowance.get(b) ?? 0;
      const kept: Array<TextBlock | ImageBlock> = [];
      let dropped = 0;
      for (const part of b.content) {
        if (part.type !== 'image') kept.push(part);
        else if (allow > 0) { kept.push(part); allow--; } else dropped++;
      }
      if (dropped) kept.push({ type: 'text', text: `[${dropped} older picture${dropped === 1 ? '' : 's'} dropped; take a new snapshot to see the scene again]` });
      return { ...b, content: kept };
    }) };
  });
}

function toBlock(use: ToolUseBlock, formatted: FormattedResult, isError: boolean, maxChars: number): ToolResultBlock {
  let text = formatted.text;
  if (text.length > maxChars) text = `${text.slice(0, maxChars)}\n[cut: ${text.length - maxChars} more characters; ask for less (a detail option, a filter or a smaller range)]`;
  if (!formatted.images.length) return { type: 'tool_result', tool_use_id: use.id, content: text, ...(isError ? { is_error: true } : {}) };
  const images: ImageBlock[] = formatted.images.map((p) => ({ type: 'image', source: { type: 'base64', media_type: p.media_type, data: p.data } }));
  return { type: 'tool_result', tool_use_id: use.id, content: [...images, { type: 'text', text }], ...(isError ? { is_error: true } : {}) };
}

export async function runAgent(o: AgentOptions): Promise<AgentResult> {
  const now = o.now ?? Date.now;
  const started = now();
  const budget = o.budget ?? {};
  const maxChars = o.maxResultChars ?? 6000;
  const messages = [...o.messages];
  const usage: Usage = { input_tokens: 0, output_tokens: 0 };
  let turns = 0;
  let toolCalls = 0;
  const emit = (event: AgentEvent) => o.onEvent?.(event);
  const lastText = () => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const m = messages[i];
      if (m.role === 'assistant') return Array.isArray(m.content) ? textOf(m.content) : m.content;
    }
    return '';
  };
  const finish = (stop: AgentResult['stop'], error?: string): AgentResult => {
    emit({ type: 'done', stop });
    return { stop, messages, text: lastText(), turns, toolCalls, usage, ...(error ? { error } : {}) };
  };
  const known = new Set(o.tools.map((t) => t.name));

  for (;;) {
    if (o.signal?.aborted) return finish('cancelled');
    if (budget.turns !== undefined && turns >= budget.turns) return finish('budget:turns');
    if (budget.millis !== undefined && now() - started > budget.millis) return finish('budget:time');
    emit({ type: 'turn', turn: turns + 1 });
    let response;
    try {
      response = await o.provider.send({
        system: o.system, tools: o.tools, model: o.model, max_tokens: o.maxTokens, signal: o.signal,
        messages: trimHistory(messages, o.keepResults ?? 8, o.keepImages ?? 2),
      });
    } catch (error) {
      if (o.signal?.aborted) return finish('cancelled');
      return finish('provider_error', error instanceof Error ? error.message : String(error));
    }
    turns++;
    for (const key of ['input_tokens', 'output_tokens', 'cache_read_input_tokens', 'cache_creation_input_tokens'] as const) {
      const add = response.usage[key];
      if (add !== undefined) usage[key] = (usage[key] ?? 0) + add;
    }
    messages.push({ role: 'assistant', content: response.content });
    for (const block of response.content) if (block.type === 'text' && block.text) emit({ type: 'text', text: block.text });
    const uses = response.content.filter((b): b is ToolUseBlock => b.type === 'tool_use');
    if (!uses.length) return finish(response.stop_reason === 'max_tokens' ? 'max_tokens' : 'end_turn');

    let halt: AgentResult['stop'] | undefined = budget.tokens !== undefined && usage.input_tokens + usage.output_tokens > budget.tokens ? 'budget:tokens' : undefined;
    const results: ToolResultBlock[] = [];
    for (const use of uses) {
      emit({ type: 'tool_call', id: use.id, name: use.name, input: use.input });
      if (!halt && o.signal?.aborted) halt = 'cancelled';
      if (!halt && budget.toolCalls !== undefined && toolCalls >= budget.toolCalls) halt = 'budget:tool_calls';
      let block: ToolResultBlock;
      let formatted: FormattedResult;
      if (halt) {
        formatted = { text: `not run: the ${halt.replace('budget:', '')} ${halt === 'cancelled' ? 'request stopped the run' : 'budget was reached'}`, images: [], isError: true };
        block = toBlock(use, formatted, true, maxChars);
      } else if (!known.has(use.name)) {
        formatted = { text: `unknown tool "${use.name}"; the tools are: ${[...known].join(', ')}`, images: [], isError: true };
        block = toBlock(use, formatted, true, maxChars);
      } else {
        toolCalls++;
        try {
          formatted = formatToolResult(use.name, await o.callTool(use.name, use.input ?? {}));
        } catch (error) {
          formatted = { text: error instanceof Error ? error.message : String(error), images: [], isError: true };
        }
        block = toBlock(use, formatted, formatted.isError, maxChars);
      }
      results.push(block);
      emit({ type: 'tool_result', id: use.id, name: use.name, isError: formatted.isError, chars: formatted.text.length, images: formatted.images.length });
    }
    messages.push({ role: 'user', content: results });
    if (halt) return finish(halt);
  }
}
