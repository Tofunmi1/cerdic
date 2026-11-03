// The browser-side Anthropic provider: the Messages API called straight from the page with the person's own key. A fake fetch stands in for the network.
// Written before the code; do not weaken.
import { expect, test } from 'bun:test';
import { anthropicProvider } from '../src/engine/agent/anthropic';
import type { Message, ToolSpec } from '../src/engine/agent/types';

const TOOLS: ToolSpec[] = [
  { name: 'a_tool', description: 'first', input_schema: { type: 'object', properties: {} } },
  { name: 'b_tool', description: 'second', input_schema: { type: 'object', properties: {} } },
];
const OK = { content: [{ type: 'text', text: 'hi' }], stop_reason: 'end_turn', usage: { input_tokens: 12, output_tokens: 3, cache_read_input_tokens: 7 } };

function fakeFetch(replies: Array<{ status: number; body: unknown; headers?: Record<string, string> }>) {
  const seen: Array<{ url: string; init: RequestInit; body: any }> = [];
  const fn = async (url: string | URL | Request, init?: RequestInit) => {
    seen.push({ url: String(url), init: init ?? {}, body: JSON.parse(String(init?.body ?? '{}')) });
    const reply = replies[Math.min(seen.length - 1, replies.length - 1)];
    return new Response(JSON.stringify(reply.body), { status: reply.status, headers: { 'content-type': 'application/json', ...reply.headers } });
  };
  return { fn: fn as unknown as typeof fetch, seen };
}
const history: Message[] = [
  { role: 'user', content: 'make a box' },
  { role: 'assistant', content: [{ type: 'tool_use', id: 'tu_1', name: 'a_tool', input: {} }] },
  { role: 'user', content: [{ type: 'tool_result', tool_use_id: 'tu_1', content: 'ok' }] },
];

test('the request carries the key, the version and the browser-access header, and nothing else leaks', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }]);
  await anthropicProvider({ apiKey: 'sk-ant-secret-123', fetch: fn }).send({ system: 'SYS', tools: TOOLS, messages: history });
  const headers = seen[0].init.headers as Record<string, string>;
  expect(seen[0].url).toBe('https://api.anthropic.com/v1/messages');
  expect(headers['x-api-key']).toBe('sk-ant-secret-123');
  expect(headers['anthropic-version']).toBe('2023-06-01');
  expect(headers['anthropic-dangerous-direct-browser-access']).toBe('true');
  expect(JSON.stringify(seen[0].body)).not.toContain('sk-ant-secret-123');
});

test('the system prompt and tools are marked as a cacheable prefix, and so is the end of the conversation', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }]);
  await anthropicProvider({ apiKey: 'k', fetch: fn }).send({ system: 'SYS', tools: TOOLS, messages: history });
  const body = seen[0].body;
  expect(body.system).toEqual([{ type: 'text', text: 'SYS', cache_control: { type: 'ephemeral' } }]);
  expect(body.tools.map((t: { name: string }) => t.name)).toEqual(['a_tool', 'b_tool']);
  const lastBlock = body.messages.at(-1).content.at(-1);
  expect(lastBlock.cache_control).toEqual({ type: 'ephemeral' });
  const marked = JSON.stringify(body.messages).split('cache_control').length - 1;
  expect(marked).toBe(1);
});

test('a plain string message still gets a cache marker on its text', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }]);
  await anthropicProvider({ apiKey: 'k', fetch: fn }).send({ system: 'S', tools: [], messages: [{ role: 'user', content: 'hello' }] });
  expect(seen[0].body.messages[0].content).toEqual([{ type: 'text', text: 'hello', cache_control: { type: 'ephemeral' } }]);
});

test('the model and the output limit come from the options, with defaults', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }, { status: 200, body: OK }]);
  const p = anthropicProvider({ apiKey: 'k', fetch: fn, model: 'claude-opus-5-5', maxTokens: 4000 });
  await p.send({ system: 'S', tools: [], messages: history });
  await p.send({ system: 'S', tools: [], messages: history, model: 'claude-haiku-5-5', max_tokens: 500 });
  expect([seen[0].body.model, seen[0].body.max_tokens]).toEqual(['claude-opus-5-5', 4000]);
  expect([seen[1].body.model, seen[1].body.max_tokens]).toEqual(['claude-haiku-5-5', 500]);
});

test('the answer comes back as content, stop reason and usage including cache reads', async () => {
  const { fn } = fakeFetch([{ status: 200, body: OK }]);
  const out = await anthropicProvider({ apiKey: 'k', fetch: fn }).send({ system: 'S', tools: [], messages: history });
  expect(out.stop_reason).toBe('end_turn');
  expect(out.content).toEqual([{ type: 'text', text: 'hi' }]);
  expect(out.usage).toMatchObject({ input_tokens: 12, output_tokens: 3, cache_read_input_tokens: 7 });
});

test('an error answer becomes a message with the status and the API text, never the key', async () => {
  const { fn } = fakeFetch([{ status: 401, body: { type: 'error', error: { type: 'authentication_error', message: 'invalid x-api-key' } } }]);
  const err = await anthropicProvider({ apiKey: 'sk-ant-secret-123', fetch: fn }).send({ system: 'S', tools: [], messages: history }).then(() => null, (e: Error) => e);
  expect(err?.message).toContain('401');
  expect(err?.message).toContain('invalid x-api-key');
  expect(err?.message).not.toContain('sk-ant-secret-123');
});

test('a busy server (429, 529) or a 5xx is retried, a client error is not', async () => {
  const slow = fakeFetch([{ status: 529, body: { error: { message: 'overloaded' } } }, { status: 429, body: { error: { message: 'slow down' } }, headers: { 'retry-after': '0' } }, { status: 200, body: OK }]);
  const out = await anthropicProvider({ apiKey: 'k', fetch: slow.fn, retryDelayMs: 0 }).send({ system: 'S', tools: [], messages: history });
  expect(slow.seen).toHaveLength(3);
  expect(out.stop_reason).toBe('end_turn');
  const bad = fakeFetch([{ status: 400, body: { error: { message: 'bad request' } } }, { status: 200, body: OK }]);
  await expect(anthropicProvider({ apiKey: 'k', fetch: bad.fn, retryDelayMs: 0 }).send({ system: 'S', tools: [], messages: history })).rejects.toThrow('400');
  expect(bad.seen).toHaveLength(1);
});

test('it gives up after the retries and says so', async () => {
  const { fn, seen } = fakeFetch([{ status: 529, body: { error: { message: 'overloaded' } } }]);
  await expect(anthropicProvider({ apiKey: 'k', fetch: fn, retryDelayMs: 0, retries: 2 }).send({ system: 'S', tools: [], messages: history })).rejects.toThrow('529');
  expect(seen).toHaveLength(3);
});

test('the abort signal reaches the request', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }]);
  const controller = new AbortController();
  await anthropicProvider({ apiKey: 'k', fetch: fn }).send({ system: 'S', tools: [], messages: history, signal: controller.signal });
  expect(seen[0].init.signal).toBe(controller.signal);
});

test('a missing key is refused before any request', async () => {
  const { fn, seen } = fakeFetch([{ status: 200, body: OK }]);
  await expect(anthropicProvider({ apiKey: '', fetch: fn }).send({ system: 'S', tools: [], messages: history })).rejects.toThrow(/key/i);
  expect(seen).toHaveLength(0);
});
