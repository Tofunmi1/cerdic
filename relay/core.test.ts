import { expect, test } from 'bun:test';
import { createRelay, type Store } from './core';

function memory(): Store {
  const values = new Map<string, string>(), lists = new Map<string, string[]>();
  return {
    async push(key, value) { lists.set(key, [...(lists.get(key) ?? []), value]); },
    async pop(key) { return lists.get(key)?.shift() ?? null; },
    async get(key) { return values.get(key) ?? null; },
    async set(key, value) { values.set(key, value); },
    async setMany(entries) { for (const [key, value] of entries) values.set(key, value); },
    async getMany(keys) { return keys.map((key) => values.get(key) ?? null); },
    async del(keys) { for (const key of keys) values.delete(key); },
  };
}
const token = 'a'.repeat(32);
const post = (handle: (r: Request) => Promise<Response>, path: string, body: unknown) => handle(new Request(`https://x.test${path}`, { method: 'POST', body: JSON.stringify(body) }));
const rpc = async (handle: (r: Request) => Promise<Response>, method: string, params: unknown = {}, id = 1) => (await post(handle, `/api/mcp/${token}`, { jsonrpc: '2.0', id, method, params })).json() as Promise<any>;

/** A stand-in for the tab: polls, runs the call with `run`, and posts the answer. */
async function tab(handle: (r: Request) => Promise<Response>, run: (name: string, args: any) => unknown, calls = 1) {
  for (let done = 0; done < calls;) {
    const { call } = await (await handle(new Request(`https://x.test/api/relay/poll?token=${token}`))).json() as any;
    if (!call) continue;
    await post(handle, '/api/relay/result', { token, id: call.id, ok: true, result: run(call.name, call.args) });
    done++;
  }
}

test('initialize, list tools and ignore notifications', async () => {
  const handle = createRelay(memory());
  const init = await rpc(handle, 'initialize', { protocolVersion: '2025-06-18' });
  expect(init.result.protocolVersion).toBe('2025-06-18');
  expect(init.result.capabilities.tools).toEqual({});
  const list = await rpc(handle, 'tools/list');
  expect(list.result.tools.length).toBeGreaterThan(20);
  expect(list.result.tools.every((tool: any) => tool.name && tool.inputSchema)).toBe(true);
  const note = await post(handle, `/api/mcp/${token}`, { jsonrpc: '2.0', method: 'notifications/initialized' });
  expect(note.status).toBe(202);
});

test('a tool call fails fast when no tab is linked', async () => {
  const handle = createRelay(memory());
  const answer = await rpc(handle, 'tools/call', { name: 'get_status', arguments: {} });
  expect(answer.result.isError).toBe(true);
  expect(answer.result.content[0].text).toContain('not linked');
});

test('a tool call reaches the tab and its answer comes back, with an image', async () => {
  const handle = createRelay(memory());
  const running = tab(handle, (name, args) => ({ ok: true, name, args, jpegBase64: 'QUJD' }));
  const answer = await rpc(handle, 'tools/call', { name: 'get_status', arguments: { x: 1 } });
  await running;
  expect(answer.result.isError).toBeUndefined();
  expect(answer.result.content[0].text).toContain('"name": "get_status"');
  expect(answer.result.content[1]).toEqual({ type: 'image', data: 'QUJD', mimeType: 'image/jpeg' });
});

test('a large answer is split into pieces and rejoined', async () => {
  const handle = createRelay(memory());
  const big = 'z'.repeat(1_300_000);
  const running = tab(handle, () => ({ ok: true, text: big }));
  const answer = await rpc(handle, 'tools/call', { name: 'get_text', arguments: {} });
  await running;
  expect(answer.result.content[0].text.length).toBeGreaterThan(50_000);
});

test('an error from the tab is an isError result', async () => {
  const handle = createRelay(memory());
  const running = (async () => {
    const { call } = await (await handle(new Request(`https://x.test/api/relay/poll?token=${token}`))).json() as any;
    await post(handle, '/api/relay/result', { token, id: call.id, ok: false, error: 'engine not ready' });
  })();
  const answer = await rpc(handle, 'tools/call', { name: 'get_status', arguments: {} });
  await running;
  expect(answer.result.isError).toBe(true);
  expect(answer.result.content[0].text).toContain('engine not ready');
});

test('bad tokens, unknown tools and wrong methods are refused', async () => {
  const handle = createRelay(memory());
  expect((await handle(new Request('https://x.test/api/mcp/short', { method: 'POST', body: '{}' }))).status).toBe(404);
  expect((await handle(new Request(`https://x.test/api/mcp/${token}`))).status).toBe(405);
  expect((await post(handle, '/api/relay/result', { token: 'nope', id: 'x' })).status).toBe(400);
  const link = createRelay(memory());
  const running = tab(link, () => ({}), 0);
  void running;
  const unknown = await rpc(link, 'tools/call', { name: 'rm_rf', arguments: {} });
  expect(unknown.result.isError).toBe(true);
  expect((await handle(new Request('https://x.test/api/relay/poll', { method: 'OPTIONS' }))).status).toBe(204);
});
