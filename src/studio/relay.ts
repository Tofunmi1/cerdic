import { workspace } from './library/workspace';
import { runEngineTool } from './toolRunner';

// The agent link. While it is on, this tab asks the relay for tool calls that Claude Code or Codex made, runs each one against the open world and sends the answer
// back. The link is a random token kept in this browser; whoever has the address built from it can drive this tab, so it can be turned off or replaced at any time.
export const RELAY_ORIGIN = '';
const TOKEN_KEY = 'cerdic-relay-token';
const MAX_BODY = 4_000_000; // the relay's request limit is 4.5 MB

export type LinkState = { status: 'off' | 'connecting' | 'waiting' | 'connected'; error: string; token: string; calls: number };
type Call = { id: string; name: string; args: Record<string, unknown> };

const newToken = () => Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, '0')).join('');
function loadToken() {
  try { const saved = localStorage.getItem(TOKEN_KEY); if (saved && /^[a-f0-9]{32}$/.test(saved)) return saved; } catch { /* Storage can be blocked. */ }
  const made = newToken();
  try { localStorage.setItem(TOKEN_KEY, made); } catch { /* Kept in memory only. */ }
  return made;
}

let state: LinkState = { status: 'off', error: '', token: loadToken(), calls: 0 };
let stop: AbortController | null = null;
const listeners = new Set<() => void>();
const set = (patch: Partial<LinkState>) => { state = { ...state, ...patch }; listeners.forEach(listener => listener()); };
const sleep = (ms: number, signal: AbortSignal) => new Promise<void>(resolve => { const timer = setTimeout(resolve, ms); signal.addEventListener('abort', () => { clearTimeout(timer); resolve(); }, { once: true }); });

async function answer(call: Call, token: string, signal: AbortSignal) {
  let body: Record<string, unknown>;
  workspace.setAgentBusy(true);
  try { body = { token, id: call.id, ok: true, result: await runEngineTool(call.name, call.args ?? {}) }; }
  catch (problem) { body = { token, id: call.id, ok: false, error: problem instanceof Error ? problem.message : String(problem) }; }
  finally { workspace.setAgentBusy(false); }
  let text = JSON.stringify(body);
  if (text.length > MAX_BODY) text = JSON.stringify({ token, id: call.id, ok: false, error: `The result was ${(text.length / 1e6).toFixed(1)} MB, over the 4 MB link limit. Ask for less (a smaller snapshot or a narrower query).` });
  for (let attempt = 0; attempt < 3 && !signal.aborted; attempt++) {
    try {
      const response = await fetch(`${RELAY_ORIGIN}/api/relay/result`, { method: 'POST', body: text, signal, headers: { 'content-type': 'application/json' } });
      if (response.ok) return;
    } catch { if (signal.aborted) return; }
    await sleep(800 * (attempt + 1), signal);
  }
}

async function run(token: string, signal: AbortSignal) {
  let failures = 0;
  while (!signal.aborted) {
    try {
      const response = await fetch(`${RELAY_ORIGIN}/api/relay/poll?token=${token}`, { signal, cache: 'no-store' });
      if (!response.ok) throw new Error(response.status === 503 ? 'The link service is not set up yet.' : `The link service answered ${response.status}.`);
      const data = await response.json() as { call: Call | null; agent: boolean };
      failures = 0;
      set({ status: data.agent ? 'connected' : 'waiting', error: '' });
      if (data.call) { await answer(data.call, token, signal); set({ calls: state.calls + 1 }); }
    } catch (problem) {
      if (signal.aborted) return;
      failures++;
      set({ status: 'connecting', error: problem instanceof Error ? problem.message : String(problem) });
      await sleep(Math.min(1000 * 2 ** failures, 15000), signal);
    }
  }
}

export const relay = {
  get state() { return state; },
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
  /** The address an agent adds as its MCP server. */
  url(origin = RELAY_ORIGIN || location.origin) { return `${origin}/api/mcp/${state.token}`; },
  start() {
    if (stop) return;
    stop = new AbortController();
    set({ status: 'connecting', error: '' });
    void run(state.token, stop.signal);
  },
  stop() { stop?.abort(); stop = null; workspace.setAgentBusy(false); set({ status: 'off', error: '' }); },
  /** A new token: the old address stops working at once. */
  replace() {
    const running = Boolean(stop);
    relay.stop();
    const token = newToken();
    try { localStorage.setItem(TOKEN_KEY, token); } catch { /* Kept in memory only. */ }
    set({ token, calls: 0 });
    if (running) relay.start();
  },
};
