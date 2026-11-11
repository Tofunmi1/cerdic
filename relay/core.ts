// The agent link: lets Claude Code or Codex (an MCP client over HTTP) drive the world open in a browser tab, with nothing installed on the person's computer.
//
//   agent  --POST /api/mcp/<token>-->  relay  --(queue in Redis)-->  tab polls /api/relay/poll, runs the tool in the page, POSTs /api/relay/result
//
// The relay holds no scene data of its own: a call and its answer sit in Redis for a few minutes and are deleted when read. The only secret is the token, a random
// 128-bit value the tab makes; whoever has it can drive that tab while its link is on.
// `core` takes a standard Request and returns a Response, so it runs on any host and is tested with a fake store.
import catalog from '../src/studio/assistant-catalog.json';
import { formatToolResult } from '../src/engine/agent/loop';

export interface Store {
  /** Appends to a list that expires `ttl` seconds after its last write. */
  push(key: string, value: string, ttl: number): Promise<void>;
  pop(key: string): Promise<string | null>;
  get(key: string): Promise<string | null>;
  set(key: string, value: string, ttl: number): Promise<void>;
  /** Writes several keys at once. */
  setMany(entries: Array<[string, string]>, ttl: number): Promise<void>;
  getMany(keys: string[]): Promise<Array<string | null>>;
  del(keys: string[]): Promise<void>;
}

const TOKEN = /^[a-f0-9]{32}$/;
const CHUNK = 400_000; // characters per stored piece: stays under a free Redis plan's 1 MB request limit
const MAX_RESULT = 4_000_000; // the platform caps a request body at 4.5 MB
const TEXT_LIMIT = 60_000; // characters of one answer handed to the model
const CALL_WAIT_MS = 240_000;
const POLL_MS = 20_000;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const CORS = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET, POST, DELETE, OPTIONS',
  'access-control-allow-headers': 'content-type, mcp-protocol-version, mcp-session-id, authorization, accept',
  'access-control-max-age': '86400',
};
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { ...CORS, 'content-type': 'application/json', 'cache-control': 'no-store' } });
const key = (token: string) => ({ queue: `cerdic:${token}:q`, alive: `cerdic:${token}:alive`, agent: `cerdic:${token}:agent`, result: (id: string) => `cerdic:${token}:r:${id}` });

type Rpc = { jsonrpc?: string; id?: number | string | null; method?: string; params?: Record<string, unknown> };
const reply = (id: Rpc['id'], result: unknown) => ({ jsonrpc: '2.0', id: id ?? null, result });
const failure = (id: Rpc['id'], code: number, message: string) => ({ jsonrpc: '2.0', id: id ?? null, error: { code, message } });
const toolFailure = (text: string) => ({ content: [{ type: 'text', text }], isError: true });

const TOOLS = (catalog.tools as Array<{ name: string; description: string; input_schema: unknown }>).map((tool) => ({ name: tool.name, description: tool.description, inputSchema: tool.input_schema }));
const NAMES = new Set(TOOLS.map((tool) => tool.name));
const INSTRUCTIONS = `${catalog.instructions}\n\nThe tools act on the world open in the person's browser tab. If a call says the tab is not linked, ask them to open Cerdic, choose Connect assistant and turn the link on.`;

export function createRelay(store: Store, now: () => number = Date.now) {
  async function callTool(token: string, name: string, args: Record<string, unknown>) {
    const k = key(token);
    if (!NAMES.has(name)) return toolFailure(`Unknown tool "${name}".`);
    if (!(await store.get(k.alive))) return toolFailure('The Cerdic tab is not linked. Open Cerdic in a browser, choose Connect assistant, turn the link on, then try again.');
    const id = crypto.randomUUID();
    await store.set(k.agent, '1', 600);
    await store.push(k.queue, JSON.stringify({ id, name, args }), 600);
    const done = `${k.result(id)}:n`;
    const deadline = now() + CALL_WAIT_MS;
    for (let turn = 0; now() < deadline; turn++) {
      const count = await store.get(done);
      if (count !== null) {
        const keys = Array.from({ length: Number(count) }, (_, i) => `${k.result(id)}:${i}`);
        const parts = await store.getMany(keys);
        await store.del([...keys, done]);
        let answer: { ok?: boolean; result?: unknown; error?: string };
        try { answer = JSON.parse(parts.map((part) => part ?? '').join('')); } catch { return toolFailure('The tab sent an unreadable answer.'); }
        const formatted = formatToolResult(name, answer.ok === false ? { error: answer.error ?? 'The tool failed.' } : answer.result);
        const text = formatted.text.length > TEXT_LIMIT ? `${formatted.text.slice(0, TEXT_LIMIT)}\n…[cut: ${formatted.text.length - TEXT_LIMIT} more characters. Ask for a narrower result.]` : formatted.text;
        return {
          content: [{ type: 'text', text },...formatted.images.slice(0, 4).map((image) => ({ type: 'image', data: image.data, mimeType: image.media_type }))],
          ...(answer.ok === false || formatted.isError ? { isError: true } : {}),
        };
      }
      // Every few checks, make sure the tab is still there so a closed tab fails fast instead of after the full wait.
      if (turn % 12 === 11 && !(await store.get(k.alive))) return toolFailure('The Cerdic tab stopped answering. Check that it is open with the link on, then try again.');
      await sleep(400);
    }
    return toolFailure('The tab did not answer in time. The tool may still be running; check the world, then try again.');
  }

  async function rpc(token: string, message: Rpc) {
    const { id, method, params = {} } = message;
    if (id === undefined || id === null) return null; // a notification: nothing to answer
    switch (method) {
      case 'initialize': return reply(id, { protocolVersion: typeof params.protocolVersion === 'string' ? params.protocolVersion : '2025-03-26', capabilities: { tools: {} }, serverInfo: { name: 'cerdic', version: '0.1.0' }, instructions: INSTRUCTIONS });
      case 'ping': return reply(id, {});
      case 'tools/list': return reply(id, { tools: TOOLS });
      case 'tools/call': {
        const args = params.arguments && typeof params.arguments === 'object' && !Array.isArray(params.arguments) ? params.arguments as Record<string, unknown> : {};
        return reply(id, await callTool(token, String(params.name), args));
      }
      default: return failure(id, -32601, `Method not found: ${method}`);
    }
  }

  async function mcp(request: Request, token: string) {
    if (request.method === 'DELETE') return new Response(null, { status: 200, headers: CORS });
    if (request.method !== 'POST') return new Response('This endpoint takes MCP over POST.', { status: 405, headers: { ...CORS, allow: 'POST' } });
    let body: unknown;
    try { body = JSON.parse(await request.text()); } catch { return json(failure(null, -32700, 'Parse error'), 400); }
    if (Array.isArray(body)) {
      const answers = (await Promise.all(body.map((message) => rpc(token, message as Rpc)))).filter((answer) => answer !== null);
      return answers.length ? json(answers) : new Response(null, { status: 202, headers: CORS });
    }
    const answer = await rpc(token, body as Rpc);
    return answer ? json(answer) : new Response(null, { status: 202, headers: CORS });
  }

  /** The tab asks for its next call; the answer comes after up to 20 s so a quiet link costs few requests. */
  async function poll(token: string) {
    const k = key(token);
    await store.set(k.alive, '1', 60);
    const agent = Boolean(await store.get(k.agent));
    const every = agent ? 500 : 2500;
    const deadline = now() + POLL_MS;
    for (;;) {
      const raw = await store.pop(k.queue);
      if (raw) return json({ call: JSON.parse(raw), agent: true });
      if (now() + every >= deadline) return json({ call: null, agent });
      await sleep(every);
    }
  }

  async function result(request: Request) {
    if (request.method !== 'POST') return json({ error: 'POST only' }, 405);
    const text = await request.text();
    if (text.length > MAX_RESULT + 4096) return json({ error: 'Result too large' }, 413);
    let body: { token?: string; id?: string; ok?: boolean; result?: unknown; error?: string };
    try { body = JSON.parse(text); } catch { return json({ error: 'Bad JSON' }, 400); }
    if (!body.token || !TOKEN.test(body.token) || typeof body.id !== 'string' || !/^[a-f0-9-]{36}$/.test(body.id)) return json({ error: 'Bad request' }, 400);
    const k = key(body.token);
    const payload = JSON.stringify({ ok: body.ok !== false, result: body.result, error: body.error });
    const parts: string[] = [];
    for (let at = 0; at < payload.length; at += CHUNK) parts.push(payload.slice(at, at + CHUNK));
    if (!parts.length) parts.push('{}');
    // The count is written after the pieces, so the waiting call never reads a partial answer.
    await store.setMany(parts.map((part, i): [string, string] => [`${k.result(body.id!)}:${i}`, part]), 300);
    await store.set(`${k.result(body.id)}:n`, String(parts.length), 300);
    return json({ ok: true });
  }

  return async function handle(request: Request): Promise<Response> {
    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: CORS });
    const url = new URL(request.url);
    let kind = url.searchParams.get('kind');
    let token = url.searchParams.get('token') ?? '';
    const asMcp = url.pathname.match(/^\/api\/mcp\/([^/]+)\/?$/);
    if (asMcp) { kind = 'mcp'; token = asMcp[1]; }
    const asRelay = url.pathname.match(/^\/api\/relay\/(poll|result)\/?$/);
    if (asRelay) kind = asRelay[1];
    try {
      if (kind === 'result') return await result(request);
      if (!TOKEN.test(token)) return json({ error: 'Invalid link' }, 404);
      if (kind === 'mcp') return await mcp(request, token);
      if (kind === 'poll') return await poll(token);
      return json({ error: 'Not found' }, 404);
    } catch (error) {
      return json({ error: error instanceof Error ? error.message : 'Relay error' }, 500);
    }
  };
}

/** Redis over its REST API (Upstash), so the function needs no client library or open socket. */
export function restStore(url: string, token: string): Store {
  const send = async (path: string, body: unknown) => {
    const response = await fetch(`${url.replace(/\/$/, '')}${path}`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: JSON.stringify(body) });
    const data = await response.json().catch(() => ({})) as unknown;
    if (!response.ok) throw new Error(`Store error ${response.status}`);
    return data;
  };
  const one = async (command: Array<string | number>) => {
    const data = await send('', command) as { result?: unknown; error?: string };
    if (data.error) throw new Error(data.error);
    return data.result;
  };
  const many = async (commands: Array<Array<string | number>>) => {
    const data = await send('/pipeline', commands) as Array<{ result?: unknown; error?: string }>;
    const failed = data.find((item) => item.error);
    if (failed) throw new Error(failed.error);
    return data.map((item) => item.result);
  };
  return {
    async push(listKey, value, ttl) { await many([['RPUSH', listKey, value], ['EXPIRE', listKey, ttl]]); },
    async pop(listKey) { return (await one(['LPOP', listKey])) as string | null; },
    async get(valueKey) { return (await one(['GET', valueKey])) as string | null; },
    async set(valueKey, value, ttl) { await one(['SET', valueKey, value, 'EX', ttl]); },
    async setMany(entries, ttl) { await many(entries.map(([k, v]) => ['SET', k, v, 'EX', ttl])); },
    async getMany(keys) { return keys.length ? (await many(keys.map((k) => ['GET', k]))) as Array<string | null> : []; },
    async del(keys) { if (keys.length) await one(['DEL', ...keys]); },
  };
}
