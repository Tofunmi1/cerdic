import { expect, test } from 'bun:test';
import { createServer } from 'node:http';

// A tiny stand-in for Upstash's REST API: POST / runs one command, POST /pipeline runs several. Only the commands the relay uses.
function fakeRedis() {
  const values = new Map<string, string>(), lists = new Map<string, string[]>();
  const run = (command: Array<string | number>) => {
    const [name, key, ...rest] = command.map(String);
    switch (name) {
      case 'RPUSH': lists.set(key, [...(lists.get(key) ?? []), rest[0]]); return lists.get(key)!.length;
      case 'EXPIRE': return 1;
      case 'LPOP': return lists.get(key)?.shift() ?? null;
      case 'GET': return values.get(key) ?? null;
      case 'SET': values.set(key, rest[0]); return 'OK';
      case 'DEL': { let n = 0; for (const k of [key, ...rest]) n += values.delete(k) ? 1 : 0; return n; }
      default: throw new Error(`unsupported ${name}`);
    }
  };
  return Bun.serve({
    port: 0,
    async fetch(request) {
      if (request.headers.get('authorization') !== 'Bearer secret') return Response.json({ error: 'unauthorized' }, { status: 401 });
      const body = await request.json() as Array<string | number> | Array<Array<string | number>>;
      if (new URL(request.url).pathname === '/pipeline') return Response.json((body as Array<Array<string | number>>).map((command) => ({ result: run(command) })));
      return Response.json({ result: run(body as Array<string | number>) });
    },
  });
}

test('the Node adapter and the REST store carry a tool call end to end', async () => {
  const redis = fakeRedis();
  process.env.KV_REST_API_URL = `http://127.0.0.1:${redis.port}`;
  process.env.KV_REST_API_TOKEN = 'secret';
  const { default: handler } = await import('./index');
  const server = createServer((req, res) => { void handler(req, res); });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
  const token = 'b'.repeat(32);

  const tab = (async () => {
    for (;;) {
      const { call } = await (await fetch(`${base}/api/relay/poll?token=${token}`)).json() as { call: { id: string; name: string } | null };
      if (!call) continue;
      await fetch(`${base}/api/relay/result`, { method: 'POST', body: JSON.stringify({ token, id: call.id, ok: true, result: { echoed: call.name } }) });
      return;
    }
  })();
  await Bun.sleep(100); // let the tab mark itself linked
  const response = await fetch(`${base}/api/mcp/${token}`, { method: 'POST', body: JSON.stringify({ jsonrpc: '2.0', id: 7, method: 'tools/call', params: { name: 'get_status', arguments: {} } }) });
  const answer = await response.json() as { id: number; result: { content: Array<{ text: string }> } };
  await tab;
  expect(response.headers.get('access-control-allow-origin')).toBe('*');
  expect(answer.id).toBe(7);
  expect(answer.result.content[0].text).toContain('"echoed": "get_status"');
  server.close(); redis.stop(true);
});
