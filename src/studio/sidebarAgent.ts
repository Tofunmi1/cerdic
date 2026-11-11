import { useCallback, useEffect, useRef, useState } from 'react';
import { runAgent } from '../engine/agent/loop';
import { anthropicProvider } from '../engine/agent/anthropic';
import { openaiProvider } from '../engine/agent/openai';
import type { Message, Provider } from '../engine/agent/types';
import { loadCatalog, runEngineTool } from './toolRunner';
import { workspace } from './library/workspace';
import type { Entry } from './transcript';

// The sidebar assistant runs entirely in this page. There is no server: the person's API key goes from the page straight to the provider they chose,
// the model's tool calls run against the open world here, and nothing passes through anything we host.
export type AgentScope = { projectId: string; worldId: string; conversationId: string };
export type ProviderConfig = { provider: 'anthropic' | 'openai-compatible'; apiKey: string; model: string; baseUrl?: string };
export type SidebarAgent = {
  status: 'off' | 'connecting' | 'ready'; connector: 'provider' | null; models: Array<{ id: string; name: string; default: boolean }>; model: string;
  running: boolean; error: string; remembered: boolean; connectProvider: (config: ProviderConfig, remember: boolean) => void; disconnect: () => void; setModel: (model: string) => void;
  submit: (text: string) => void; stop: () => void;
};
type Log = (entry: Omit<Entry, 'ts'> & { ts?: number }) => void;

const STORED = 'cerdic-assistant-provider';
const MODEL = 'cerdic-assistant-model';
const key = (scope: AgentScope | null) => scope ? JSON.stringify([scope.projectId, scope.worldId, scope.conversationId]) : '';
const instruction = `You are Cerdic's in-app 3D world builder. The browser scene is your workspace, not repository files. Use ONLY the supplied engine tools; never use shell, filesystem, external apps, or other MCP connections. All tools target the active project/world. Start by inspecting the current document and reading get_guide for core and the relevant operator families. Use named parts and small targeted edits for follow-ups. Build silhouette, then detail. Check and measure each meaningful change, fix actual diagnostics, use snapshot for visual review and frame_view at the end. Do not replace existing objects unless the user requested it. Never claim something was built if an engine call failed. Give a concise report of what changed. User-requested script.js scene behavior is allowed through engine tools; do not run host operating-system commands. Stop promptly when cancelled.`;

export function validateProvider(value: ProviderConfig): ProviderConfig {
  if (value.provider !== 'anthropic' && value.provider !== 'openai-compatible') throw new Error('Unsupported API provider.');
  const apiKey = value.apiKey.trim(), model = value.model.trim();
  if (apiKey.length < 8 || apiKey.length > 4096 || /[\r\n]/.test(apiKey)) throw new Error('Enter a valid API key.');
  if (!model || model.length > 160 || /[\r\n]/.test(model)) throw new Error('Enter a model ID.');
  let baseUrl: string | undefined;
  if (value.baseUrl?.trim()) {
    let url: URL;
    try { url = new URL(value.baseUrl.trim()); } catch { throw new Error('Enter a complete API base URL.'); }
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) throw new Error('API base URL must be an HTTP(S) endpoint without credentials, query or fragment.');
    if (url.protocol === 'http:' && !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) throw new Error('Use HTTPS for remote API providers.');
    baseUrl = url.toString().replace(/\/$/, '');
  }
  return { provider: value.provider, apiKey, model, ...(baseUrl ? { baseUrl } : {}) };
}
const makeProvider = (config: ProviderConfig): Provider => config.provider === 'anthropic'
  ? anthropicProvider({ apiKey: config.apiKey, model: config.model, baseUrl: config.baseUrl })
  : openaiProvider({ apiKey: config.apiKey, model: config.model, baseUrl: config.baseUrl });
function readStored(): ProviderConfig | null {
  try { const raw = localStorage.getItem(STORED); return raw ? validateProvider(JSON.parse(raw)) : null; } catch { return null; }
}

export function useSidebarAgent(scope: AgentScope | null, log: Log, entries: Entry[]): SidebarAgent {
  const [status, setStatus] = useState<SidebarAgent['status']>('off');
  const [connector, setConnector] = useState<SidebarAgent['connector']>(null);
  const [models, setModels] = useState<SidebarAgent['models']>([]);
  const [model, setModelState] = useState(() => { try { return localStorage.getItem(MODEL) ?? ''; } catch { return ''; } });
  const [running, setRunning] = useState(false);
  const [error, setError] = useState('');
  const [remembered, setRemembered] = useState(false);
  const config = useRef<ProviderConfig | null>(null);
  const histories = useRef(new Map<string, Message[]>());
  const current = useRef({ scope, log }); current.current = { scope, log };
  const entriesRef = useRef(entries); entriesRef.current = entries;
  const active = useRef<{ scope: AgentScope; log: Log; id: string; controller: AbortController } | null>(null);
  const finish = () => { active.current = null; setRunning(false); workspace.setAgentBusy(false); };

  const apply = useCallback((value: ProviderConfig, remember: boolean) => {
    config.current = value; histories.current.clear();
    setModels([{ id: value.model, name: value.model, default: true }]); setModelState(value.model);
    setConnector('provider'); setStatus('ready'); setError(''); setRemembered(remember);
    try { if (remember) localStorage.setItem(STORED, JSON.stringify(value)); else localStorage.removeItem(STORED); } catch { /* Remembering is optional. */ }
  }, []);
  const disconnect = useCallback(() => {
    active.current?.controller.abort(); config.current = null; histories.current.clear();
    setStatus('off'); setConnector(null); setModels([]); setRemembered(false); finish();
    try { localStorage.removeItem(STORED); } catch { /* Nothing stored. */ }
  }, []);
  const connectProvider = useCallback((value: ProviderConfig, remember: boolean) => {
    if (active.current) { setError('Stop the current turn before changing the connection.'); return; }
    try { apply(validateProvider(value), remember); } catch (problem) { setError(problem instanceof Error ? problem.message : String(problem)); }
  }, [apply]);
  // A key the person chose to remember on this device is picked up again after a reload.
  useEffect(() => { const stored = readStored(); if (stored) apply(stored, true); }, [apply]);
  useEffect(() => () => { active.current?.controller.abort(); workspace.setAgentBusy(false); }, []);

  return { status, connector, models, model, running, error, remembered, connectProvider, disconnect,
    setModel(value) { setModelState(value); try { localStorage.setItem(MODEL, value); } catch { /* Optional preference. */ } },
    submit(text) {
      const settings = config.current;
      if (status !== 'ready' || !settings || !scope || active.current) { setError('Connect an assistant first.'); return; }
      if (!text.trim() || text.length > 100000) { setError('A prompt must contain 1–100,000 characters.'); return; }
      setError(''); setRunning(true); workspace.setAgentBusy(true);
      const run = active.current = { scope, log, id: crypto.randomUUID(), controller: new AbortController() };
      const scopeId = key(scope);
      const earlier = entriesRef.current.filter(entry => entry.text && ['user', 'assistant'].includes(entry.role)).slice(-20).map(entry => `${entry.role}: ${entry.text}`).join('\n\n');
      const history = histories.current.get(scopeId) ?? [];
      void (async () => {
        let texts = 0, failure = '';
        const fail = (message: string) => { setError(message); run.log({ id: `agent-${run.id}-error`, role: 'activity', tool: { name: 'assistant', status: 'error', summary: message } }); };
        try {
          const catalog = await loadCatalog();
          const first = !history.length && earlier;
          const content = first ? `Earlier messages in this world conversation:\n${earlier.slice(-20000)}\n\nCurrent request:\n${text}` : text;
          const result = await runAgent({
            provider: makeProvider({ ...settings, model: model || settings.model }), model: model || settings.model,
            system: `${instruction}\n\n${catalog.instructions}`, tools: catalog.tools,
            messages: [...history.slice(-40), { role: 'user', content }], budget: { turns: 24 }, maxResultChars: 30000, signal: run.controller.signal,
            onEvent(event) {
              if (event.type === 'text') run.log({ id: `agent-${run.id}-${texts++}`, role: 'assistant', text: event.text });
              else if (event.type === 'tool_call') { failure = ''; run.log({ id: event.id, role: 'activity', tool: { name: event.name, status: 'running', input: event.input } }); }
              else if (event.type === 'tool_result') run.log({ id: event.id, role: 'activity', tool: { name: event.name, status: event.isError ? 'error' : 'done', summary: event.isError ? failure || 'The tool reported an error' : 'Completed' } });
            },
            async callTool(name, args) {
              try {
                if (run.controller.signal.aborted || key(current.current.scope) !== scopeId) throw new Error('This conversation is no longer active.');
                return await runEngineTool(name, args);
              } catch (problem) { failure = problem instanceof Error ? problem.message : String(problem); throw problem; }
            },
          });
          histories.current.set(scopeId, result.messages.slice(-60));
          if (result.stop === 'provider_error') fail(result.error ?? 'The provider request failed.');
          else if (result.stop === 'cancelled') run.log({ id: `agent-${run.id}-stopped`, role: 'activity', text: 'Stopped. Completed scene edits are kept; use Undo to reverse them.' });
          else if (result.stop === 'max_tokens') fail('The reply was cut off by the provider’s output limit. Ask it to continue.');
          else if (result.stop.startsWith('budget:')) fail('The assistant stopped after its turn limit. Ask it to continue.');
        } catch (problem) { fail(problem instanceof Error ? problem.message : String(problem)); }
        finally { finish(); }
      })();
    },
    stop() { active.current?.controller.abort(); },
  };
}
