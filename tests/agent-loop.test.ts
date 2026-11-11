// The in-page agent loop: a model that calls tools until it is done, with budgets, cancel, clean errors, bounded results and
// a request that stays small however long the run is. A scripted provider stands in for the model; no network. Written before the code; do not weaken.
import { expect, test } from 'bun:test';
import { formatToolResult, runAgent, type AgentEvent } from '../src/engine/agent/loop';
import type { Block, Message, Provider, ProviderRequest, ProviderResponse, ToolSpec } from '../src/engine/agent/types';

const TOOLS: ToolSpec[] = [
  { name: 'apply_text', description: 'apply', input_schema: { type: 'object', properties: { text: { type: 'string' } } } },
  { name: 'snapshot', description: 'look', input_schema: { type: 'object', properties: {} } },
];
const usage = (input = 10, output = 5, extra = {}) => ({ input_tokens: input, output_tokens: output, ...extra });
const say = (text: string): ProviderResponse => ({ content: [{ type: 'text', text }], stop_reason: 'end_turn', usage: usage() });
const call = (...uses: Array<[string, Record<string, unknown>]>): ProviderResponse => ({
  content: uses.map(([name, input], i) => ({ type: 'tool_use', id: `tu_${name}_${i}_${Math.random().toString(36).slice(2, 6)}`, name, input }) as Block),
  stop_reason: 'tool_use',
  usage: usage(),
});

/** A model that plays back its answers in order and records each request it was sent (deep-copied, so later edits to the history do not rewrite it). */
function scripted(answers: ProviderResponse[]) {
  const requests: ProviderRequest[] = [];
  const provider: Provider = {
    async send(request) {
      requests.push(JSON.parse(JSON.stringify(request)));
      const next = answers[requests.length - 1];
      if (!next) throw new Error('the script ran out of answers');
      return next;
    },
  };
  return { provider, requests };
}
const user = (text: string): Message => ({ role: 'user', content: text });
const results = (message: Message) => (Array.isArray(message.content) ? message.content.filter((b) => b.type === 'tool_result') : []) as Array<Extract<Block, { type: 'tool_result' }>>;
const resultText = (r: { content: unknown }) => (typeof r.content === 'string' ? r.content : (r.content as Array<{ type: string; text?: string }>).filter((b) => b.type === 'text').map((b) => b.text).join('\n'));

test('a model that answers without tools ends the run with its text', async () => {
  const { provider } = scripted([say('Nothing to build.')]);
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({}), messages: [user('hi')] });
  expect(out.stop).toBe('end_turn');
  expect(out.text).toBe('Nothing to build.');
  expect(out.turns).toBe(1);
  expect(out.messages.at(-1)?.role).toBe('assistant');
});

test('tool calls are run and their results go back in the next request, paired by id', async () => {
  const { provider, requests } = scripted([call(['apply_text', { text: 'a = solid.box()' }]), say('Done.')]);
  const calls: Array<[string, unknown]> = [];
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async (name, input) => { calls.push([name, input]); return { ok: true, revision: 3 }; }, messages: [user('make a box')] });
  expect(calls).toEqual([['apply_text', { text: 'a = solid.box()' }]]);
  const last = requests[1].messages.at(-1)!;
  expect(last.role).toBe('user');
  const [r] = results(last);
  const use = (requests[1].messages.at(-2)!.content as Block[]).find((b) => b.type === 'tool_use') as Extract<Block, { type: 'tool_use' }>;
  expect(r.tool_use_id).toBe(use.id);
  expect(resultText(r)).toContain('"revision": 3');
  expect(out.toolCalls).toBe(1);
});

test('several tool calls in one turn all get results, in order, in one user message', async () => {
  const { provider, requests } = scripted([call(['apply_text', { text: 'a' }], ['snapshot', {}]), say('ok')]);
  const order: string[] = [];
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async (name) => { order.push(name); return { ok: true }; }, messages: [user('go')] });
  expect(order).toEqual(['apply_text', 'snapshot']);
  expect(results(requests[1].messages.at(-1)!)).toHaveLength(2);
});

test('a tool that throws becomes an error result the model can read, and the run goes on', async () => {
  const { provider, requests } = scripted([call(['apply_text', {}]), say('I will fix it.')]);
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => { throw new Error('text is required'); }, messages: [user('go')] });
  const [r] = results(requests[1].messages.at(-1)!);
  expect(r.is_error).toBe(true);
  expect(resultText(r)).toContain('text is required');
  expect(out.stop).toBe('end_turn');
});

test('an unknown tool name is refused with the names that exist', async () => {
  const { provider, requests } = scripted([call(['make_game', {}]), say('ok')]);
  let ran = false;
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => { ran = true; return {}; }, messages: [user('go')] });
  expect(ran).toBe(false);
  const [r] = results(requests[1].messages.at(-1)!);
  expect(r.is_error).toBe(true);
  expect(resultText(r)).toContain('apply_text');
  expect(resultText(r)).toContain('snapshot');
});

test('a huge result is cut with a note that says how much, so one tool cannot flood the window', async () => {
  const { provider, requests } = scripted([call(['apply_text', {}]), say('ok')]);
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({ blob: 'x'.repeat(50_000) }), messages: [user('go')], maxResultChars: 2000 });
  const text = resultText(results(requests[1].messages.at(-1)!)[0]);
  expect(text.length).toBeLessThan(2400);
  expect(text).toMatch(/cut|truncated/i);
  expect(text).toMatch(/\d+ more characters/);
});

test('old tool results are cleared but the newest few stay, and every tool_use keeps its result', async () => {
  const turns: ProviderResponse[] = Array.from({ length: 8 }, (_, i) => call(['apply_text', { text: `n${i}` }]));
  const { provider, requests } = scripted([...turns, say('done')]);
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async (_n, input) => ({ echoed: input, pad: 'p'.repeat(300) }), messages: [user('go')], keepResults: 3 });
  const finalRequest = requests.at(-1)!;
  const all = finalRequest.messages.flatMap(results);
  expect(all).toHaveLength(8);
  const cleared = all.filter((r) => /cleared/i.test(resultText(r)));
  expect(cleared).toHaveLength(5);
  expect(resultText(cleared[0])).toContain('apply_text');
  expect(all.slice(-3).every((r) => !/cleared/i.test(resultText(r)))).toBe(true);
});

test('the full history is kept for the log even when requests are trimmed', async () => {
  const turns: ProviderResponse[] = Array.from({ length: 5 }, (_, i) => call(['apply_text', { text: `n${i}` }]));
  const { provider } = scripted([...turns, say('done')]);
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({ pad: 'p'.repeat(300) }), messages: [user('go')], keepResults: 1 });
  const all = out.messages.flatMap(results);
  expect(all.every((r) => !/cleared/i.test(resultText(r)))).toBe(true);
});

test('a snapshot result becomes image blocks plus text, and only the newest pictures stay', async () => {
  const picture = { jpegBase64: 'QUJD', ok: true, view: 'front' };
  const shots = Array.from({ length: 3 }, () => call(['snapshot', {}]));
  const { provider, requests } = scripted([...shots, say('done')]);
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => picture, messages: [user('go')], keepImages: 1 });
  const last = requests.at(-1)!;
  const rs = last.messages.flatMap(results);
  const imageCount = (r: { content: unknown }) => (Array.isArray(r.content) ? (r.content as Array<{ type: string }>).filter((b) => b.type === 'image').length : 0);
  expect(rs.map(imageCount)).toEqual([0, 0, 1]);
  expect(resultText(rs[0])).toMatch(/picture.*dropped|image.*cleared|cleared.*image/i);
});

test('formatToolResult splits pictures from the rest', () => {
  const formatted = formatToolResult('snapshot', { jpegBase64: 'QUJD', sheets: [{ jpegBase64: 'REVG', label: 'iso' }], ok: true });
  expect(formatted.images).toEqual([{ media_type: 'image/jpeg', data: 'QUJD' }, { media_type: 'image/jpeg', data: 'REVG' }]);
  expect(formatted.text).not.toContain('QUJD');
  expect(formatted.text).toContain('iso');
  expect(formatted.isError).toBe(false);
  expect(formatToolResult('apply_text', { ok: false, error: 'bad' }).isError).toBe(true);
});

test('budgets stop the run with a named reason and leave a valid conversation', async () => {
  const forever = Array.from({ length: 20 }, () => call(['apply_text', {}]));
  for (const [budget, reason] of [[{ toolCalls: 2 }, 'budget:tool_calls'], [{ turns: 2 }, 'budget:turns'], [{ tokens: 40 }, 'budget:tokens']] as const) {
    const { provider } = scripted(forever);
    const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({ ok: true }), messages: [user('go')], budget });
    expect(out.stop).toBe(reason);
    // every tool_use has its tool_result, so the conversation can be continued
    const uses = out.messages.flatMap((m) => (Array.isArray(m.content) ? m.content.filter((b) => b.type === 'tool_use') : [])).length;
    expect(out.messages.flatMap(results)).toHaveLength(uses);
  }
});

test('the wall-clock budget stops a slow run', async () => {
  let t = 0;
  const { provider } = scripted(Array.from({ length: 10 }, () => call(['apply_text', {}])));
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => { t += 4000; return { ok: true }; }, messages: [user('go')], budget: { millis: 10_000 }, now: () => t });
  expect(out.stop).toBe('budget:time');
});

test('cancel stops before the next model call and leaves every tool_use answered', async () => {
  const controller = new AbortController();
  const { provider, requests } = scripted([call(['apply_text', {}]), call(['apply_text', {}]), say('never')]);
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => { controller.abort(); return { ok: true }; }, messages: [user('go')], signal: controller.signal });
  expect(out.stop).toBe('cancelled');
  expect(requests).toHaveLength(1);
  const uses = out.messages.flatMap((m) => (Array.isArray(m.content) ? m.content.filter((b) => b.type === 'tool_use') : [])).length;
  expect(out.messages.flatMap(results)).toHaveLength(uses);
});

test('a provider failure ends the run with the error, keeping the conversation so far', async () => {
  const provider: Provider = { async send() { throw new Error('401 invalid x-api-key'); } };
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({}), messages: [user('go')] });
  expect(out.stop).toBe('provider_error');
  expect(out.error).toContain('401');
  expect(out.messages).toHaveLength(1);
});

test('the system prompt and tools reach the model unchanged every turn, so a cached prefix stays valid', async () => {
  const { provider, requests } = scripted([call(['apply_text', {}]), call(['snapshot', {}]), say('done')]);
  await runAgent({ provider, system: 'SYSTEM TEXT', tools: TOOLS, callTool: async () => ({ ok: true }), messages: [user('go')] });
  expect(new Set(requests.map((r) => JSON.stringify([r.system, r.tools]))).size).toBe(1);
  expect(requests[0].system).toBe('SYSTEM TEXT');
});

test('usage adds up across turns, including cache reads', async () => {
  const a = call(['apply_text', {}]); a.usage = usage(100, 20, { cache_read_input_tokens: 80, cache_creation_input_tokens: 5 });
  const b = say('done'); b.usage = usage(150, 30, { cache_read_input_tokens: 120 });
  const { provider } = scripted([a, b]);
  const out = await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({}), messages: [user('go')] });
  expect(out.usage).toMatchObject({ input_tokens: 250, output_tokens: 50, cache_read_input_tokens: 200, cache_creation_input_tokens: 5 });
});

test('events let a chat panel show the run as it happens', async () => {
  const { provider } = scripted([{ content: [{ type: 'text', text: 'Building.' }, { type: 'tool_use', id: 'tu_1', name: 'apply_text', input: { text: 'a' } }], stop_reason: 'tool_use', usage: usage() }, say('Done.')]);
  const seen: string[] = [];
  await runAgent({ provider, system: 's', tools: TOOLS, callTool: async () => ({ ok: true }), messages: [user('go')], onEvent: (e: AgentEvent) => seen.push(e.type) });
  expect(seen).toEqual(['turn', 'text', 'tool_call', 'tool_result', 'turn', 'text', 'done']);
});
