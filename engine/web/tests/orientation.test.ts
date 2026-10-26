import { afterEach, expect, test } from 'bun:test';
import { spawn, type ChildProcess } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const children: ChildProcess[] = [];
afterEach(() => { for (const child of children.splice(0)) child.kill(); });

// Run the actual stdio server with an in-memory page transport. No listening socket or browser is needed.
function server(connected = false) {
  const bootstrap = `
    Bun.serve = options => {
      const page = {
        close() {},
        send(raw) {
          const call = JSON.parse(raw);
          const result = call.method === 'list_operators' ? [
            {id:'solid.box@1', doc:'A box.', inputs:[], params:[], output:{kind:'fixed',port:'solid'}},
            {id:'curve.circle@1', doc:'A circle.', inputs:[], params:[], output:{kind:'fixed',port:'curve'}}
          ] : {ok:true, forwarded:call.params};
          options.websocket.message(page, JSON.stringify({id:call.id, result}));
        }
      };
      if (${connected}) queueMicrotask(() => options.websocket.open(page));
      return null;
    };
    await import(${JSON.stringify(fileURLToPath(new URL('../../tools/mcp/server.mjs', import.meta.url)))});
  `;
  const child = spawn('bun', ['-e', bootstrap], { stdio: ['pipe', 'pipe', 'pipe'] });
  children.push(child);
  let buffer = '', stderr = '', id = 0;
  const pending = new Map<number, { resolve: (reply: any) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  child.stderr.on('data', chunk => { stderr += String(chunk); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    buffer += chunk;
    let at;
    while ((at = buffer.indexOf('\n')) >= 0) {
      const reply = JSON.parse(buffer.slice(0, at));
      buffer = buffer.slice(at + 1);
      const request = pending.get(reply.id);
      if (request) { clearTimeout(request.timer); pending.delete(reply.id); request.resolve(reply.result); }
    }
  });
  child.on('exit', () => {
    for (const request of pending.values()) { clearTimeout(request.timer); request.reject(new Error(`server exited: ${stderr}`)); }
    pending.clear();
  });
  const rpc = (method: string, params = {}): Promise<any> => new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error(`no answer for ${method}: ${stderr}`)); }, 3000);
    pending.set(requestId, { resolve, reject, timer });
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: requestId, method, params }) + '\n');
  });
  const call = async (name: string, args = {}) => {
    const result = await rpc('tools/call', { name, arguments: args });
    return { text: result.content.map((block: { text?: string }) => block.text ?? '').join('\n'), isError: result.isError };
  };
  return { rpc, call };
}

test('server instructions describe the five-step workflow within the size limits', async () => {
  const { rpc } = server();
  const { instructions } = await rpc('initialize');
  expect(instructions.split('\n')).toHaveLength(5);
  expect(instructions.length).toBeGreaterThan(1000);
  expect(instructions.length).toBeLessThan(2500);
  for (const word of ['get_guide', 'topics', 'operators', 'start_from', 'compact', 'batch', 'run', 'snapshot', 'cameras', 'edit', 'check']) expect(instructions).toContain(word);
  const { tools } = await rpc('tools/list');
  expect(tools.reduce((sum: number, tool: unknown) => sum + JSON.stringify(tool).length, 0)).toBeLessThanOrEqual(13000);
  for (const tool of tools) expect(JSON.stringify(tool).length).toBeLessThanOrEqual(1300);
});

test('several guide topics include core, headings, legacy text and unknown-topic errors', async () => {
  const { call } = server();
  const core = await call('get_guide');
  const curves = await call('get_guide', { topic: 'curves' });
  const topics = await call('get_guide', { topics: ['core', 'curves', 'terrain'] });
  expect(topics.isError).toBe(false);
  expect(topics.text).toStartWith(`## core\n\n${core.text}`);
  expect(topics.text).toContain(`## curves\n\n${curves.text}`);
  expect(topics.text).toContain('\n## terrain\n');
  expect((await call('get_guide', { topic: 'core' })).text).toBe(core.text);
  const bad = await call('get_guide', { topics: ['curves', 'unknown_topic'] });
  expect(bad.isError).toBe(true);
  for (const word of ['unknown_topic', 'core', 'curves', 'terrain']) expect(bad.text).toContain(word);
  expect((await call('get_guide', { topics: 'curves' })).isError).toBe(true);
});

test('a missing editor leaves the requested guide intact with an operators explanation', async () => {
  const { call } = server();
  const guide = await call('get_guide', { topic: 'curves' });
  const combined = await call('get_guide', { topics: ['curves'], operators: { family: 'solid' } });
  expect(combined.isError).toBe(false);
  expect(combined.text).toStartWith(`## curves\n\n${guide.text}`);
  expect(combined.text).toContain('## operators\n\nNo editor page');
});

test('combined guides append exactly the list_operators answer for index, filters, ids and full specs', async () => {
  const { call } = server(true);
  for (const operators of [{}, { family: 'solid' }, { query: 'circle' }, { ids: ['solid.box@1', 'kit.door'] }, { detail: 'full', family: 'solid' }, { ids: ['missing'] }]) {
    const direct = await call('list_operators', operators);
    const combined = await call('get_guide', { topics: ['curves'], operators });
    expect(combined.isError).toBe(false);
    expect(combined.text.split('\n\n## operators\n\n')[1]).toBe(direct.text);
  }
});

test('server validates cameras before forwarding and forwards valid single and multiple cameras intact', async () => {
  const { call } = server(true);
  const camera = { position: [0, 5, 5], target: [0, 0, 0] };
  for (const args of [{ camera }, { cameras: [{ ...camera, label: 'air' }] }]) {
    const result = await call('snapshot', args);
    expect(result.isError).toBe(false);
    expect(JSON.parse(result.text)).toEqual({ ok: true, forwarded: args });
  }
  for (const args of [{ cameras: Array(7).fill(camera) }, { camera: { ...camera, position: [1, 2] } }, { camera, times: [0] }, { cameras: [camera], states: [{}] }]) {
    expect((await call('snapshot', args)).isError).toBe(true);
  }
});

test('server exposes survey guidance and validates survey and tour options before the page', async () => {
  const { rpc, call } = server(true);
  const { tools } = await rpc('tools/list');
  expect(tools.some((tool: { name: string }) => tool.name === 'survey')).toBe(true);
  expect((await call('get_guide', { topic: 'survey' })).text).toContain('snapshot_args');
  for (const [name, args] of [
    ['survey', { spacing: 3, max_views: 8 }], ['snapshot', { tour: true }],
    ['snapshot', { tour: { max_views: 12, eye_height: 1.8 } }],
  ] as const) {
    const result = await call(name, args);
    expect(result.isError).toBe(false);
    expect(JSON.parse(result.text).forwarded).toEqual(args);
  }
  for (const [name, args, range] of [
    ['survey', { spacing: 0 }, '0.25 to 100'], ['survey', { yaws: 1.5 }, '1 to 32'],
    ['snapshot', { tour: { max_views: 13 } }, '1 to 12'],
    ['snapshot', { tour: { target_coverage: 2 } }, '0 to 1'],
  ] as const) {
    const result = await call(name, args);
    expect(result.isError).toBe(true);
    expect(result.text).toContain(range);
  }
});
