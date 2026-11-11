import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { Check, Copy, Plug, Terminal, X } from 'lucide-react';
import { host, setMcpEnabled } from '../engine/host';
import { useHost } from './hooks';
import { relay } from './relay';
import type { SidebarAgent } from './sidebarAgent';

type Provider = 'anthropic' | 'openai-compatible';
const REPO_KEY = 'cerdic-mcp-repo';
// Both CLIs sign in with the person's own subscription (Claude: `claude`, then /login; Codex: `codex login`), so no API key is involved.
const CLIENTS = {
  'Claude Code': { login: 'Uses your Claude Pro or Max sign-in. If you have not signed in yet, run claude and then /login.', add: (url: string) => `claude mcp add --transport http --scope user cerdic ${url}`, start: 'claude' },
  'Codex CLI': { login: 'Uses your ChatGPT sign-in. If you have not signed in yet, run codex login. Needs a recent Codex that supports HTTP MCP servers.', add: (url: string) => `codex mcp add cerdic --url ${url}`, start: 'codex' },
};
const quote = (path: string) => /^[\w@%+=:,./-]+$/.test(path) ? path : `"${path.replace(/(["\\$`])/g, '\\$1')}"`;
const storedRepo = () => { try { return localStorage.getItem(REPO_KEY) ?? ''; } catch { return ''; } };
const LINK_CHIP = { off: 'Off', connecting: 'Connecting…', waiting: 'Waiting for agent', connected: 'Agent connected' };

export function AssistantConnection({ onClose, agent }: { onClose: () => void; agent: SidebarAgent }) {
  useHost();
  const link = useSyncExternalStore(relay.subscribe, () => relay.state);
  const dialog = useRef<HTMLDialogElement>(null);
  const [copied, setCopied] = useState('');
  const [copyError, setCopyError] = useState('');
  const [provider, setProvider] = useState<Provider>('anthropic');
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [remember, setRemember] = useState(false);
  const [client, setClient] = useState<keyof typeof CLIENTS>('Claude Code');
  const [repo, setRepo] = useState(storedRepo);
  useEffect(() => { const element = dialog.current; element?.showModal(); return () => element?.close(); }, []);
  useEffect(() => { if (agent.status === 'ready' && agent.connector === 'provider') setApiKey(''); }, [agent.status, agent.connector]);
  const on = link.status !== 'off';
  const add = CLIENTS[client].add(relay.url());
  const localEnabled = host.mcpStatus !== 'off';
  const server = quote((repo.trim().replace(/\/+$/, '') || '/path/to/cerdic') + '/engine/tools/mcp/server.mjs');
  const localAdd = client === 'Claude Code' ? `claude mcp add --scope user cerdic -- bun ${server}` : `codex mcp add cerdic -- bun ${server}`;
  const readyProvider = agent.status === 'ready' && agent.connector === 'provider';
  const setRepoPath = (value: string) => { setRepo(value); try { localStorage.setItem(REPO_KEY, value); } catch { /* Optional. */ } };
  const copy = (id: string, value: string) => { void navigator.clipboard.writeText(value).then(() => { setCopied(id); setCopyError(''); }).catch(() => setCopyError('Copy is unavailable. Select and copy the text above.')); };
  const submitProvider = (event: React.FormEvent) => {
    event.preventDefault();
    agent.connectProvider({ provider, apiKey, model, ...(baseUrl.trim() ? { baseUrl: baseUrl.trim() } : {}) }, remember);
  };
  const command = (id: string, text: string) => <div className="assistant-connect-command"><code>{text}</code><button className="btn icon" type="button" aria-label={`Copy ${id}`} onClick={() => copy(id, text)}>{copied === id ? <Check size={15} /> : <Copy size={15} />}</button></div>;
  return <dialog ref={dialog} className="assistant-connection glass strong" aria-labelledby="assistant-connect-title" onCancel={event => { event.preventDefault(); onClose(); }}>
    <header><div className="assistant-connect-icon"><Plug size={23} /></div><button className="btn icon" type="button" onClick={onClose} aria-label="Close assistant connection"><X size={18} /></button></header>
    <h2 id="assistant-connect-title">Connect an assistant</h2><p className="assistant-connect-intro">Drive this world from Claude Code or Codex with your own subscription, or chat here with an API key. There is nothing to install or clone, and no Cerdic account.</p>
    <section className="assistant-connect-method"><div className="assistant-connect-method-head"><Terminal size={18} /><h3>Claude Code or Codex · agent link</h3><span className={`chip ${link.status === 'connected' ? 'ok' : on ? 'live' : ''}`}>{LINK_CHIP[link.status]}</span></div>
      <p>The agent runs in a terminal on your computer and edits the world open in this tab. {CLIENTS[client].login} It does not use the chat box on the left; talk to it in its own terminal.</p>
      <label className="assistant-mcp-client">Agent<select value={client} onChange={event => setClient(event.target.value as keyof typeof CLIENTS)}>{Object.keys(CLIENTS).map(name => <option key={name}>{name}</option>)}</select></label>
      <p><b>1.</b> Add Cerdic to {client}. Run this once in a terminal.</p>{command('MCP setup command', add)}
      <p><b>2.</b> Turn the link on, keep this tab open, then start <code>{CLIENTS[client].start}</code>.</p>
      <div className="assistant-provider-actions"><button className={`btn ${on ? '' : 'primary'}`} type="button" onClick={() => on ? relay.stop() : relay.start()}>{on ? 'Turn the link off' : 'Turn the link on'}<Plug size={14} /></button><button className="btn" type="button" onClick={() => relay.replace()}>New address</button></div>
      <span className="assistant-connection-status" role="status">{link.status === 'connected' ? `Agent connected${link.calls ? ` · ${link.calls} tool call${link.calls === 1 ? '' : 's'} so far` : ''}.` : link.status === 'waiting' ? 'Link is on. Waiting for the agent to make its first call.' : link.status === 'connecting' ? (link.error || 'Reaching the link service…') : 'The link is off.'}</span>
      <p className="assistant-connect-note">While the link is on, each tool call and its result pass through Cerdic’s relay (a Vercel function with a short-lived Redis queue) and are deleted within minutes. Anyone who has the address above can edit this world, so keep it private; <b>New address</b> cancels the old one at once.</p>
      <details className="assistant-connect-note"><summary>Use a local copy of the repository instead</summary>
        <label className="assistant-mcp-path">Cerdic folder on this computer<input value={repo} placeholder="/path/to/cerdic" spellCheck={false} onChange={event => setRepoPath(event.target.value)} /></label>
        {command('local MCP setup command', localAdd)}
        <p>Needs <a href="https://bun.sh" target="_blank" rel="noreferrer">Bun</a>. Then:</p><button className={`btn ${localEnabled ? '' : 'primary'}`} type="button" onClick={() => setMcpEnabled(!localEnabled)}>{localEnabled ? 'Turn off local link' : 'Turn on local link'}<Plug size={14} /></button><span className="assistant-connection-status" role="status">{host.mcpStatus === 'connected' ? 'Linked to 127.0.0.1:8765.' : localEnabled ? 'Waiting for the agent on 127.0.0.1:8765…' : 'Off.'}</span>
      </details>
    </section>
    <section className="assistant-connect-api assistant-provider"><div><h3>Sidebar chat · API key</h3><span className={`chip ${readyProvider ? 'ok' : ''}`}>{readyProvider ? 'Configured · checked on first prompt' : 'Key stays in this browser'}</span></div><p>Connect Anthropic or any OpenAI-compatible provider. The key stays in this browser and goes only to the provider you pick. It is kept in memory for this session, or on this device if you choose to remember it.</p>
      <form onSubmit={submitProvider}>
        <label>Provider<select value={provider} disabled={agent.running} onChange={event => { setProvider(event.target.value as Provider); setModel(''); setBaseUrl(''); }}><option value="anthropic">Anthropic · Claude API</option><option value="openai-compatible">OpenAI-compatible · OpenAI, OpenRouter, etc.</option></select></label>
        <label>Model ID<input required maxLength={160} value={model} placeholder={provider === 'anthropic' ? 'Enter your Anthropic model ID' : 'Enter your provider model ID'} onChange={event => setModel(event.target.value)} /></label>
        {provider === 'openai-compatible' && <label>API base URL <span className="assistant-optional">optional</span><input type="url" value={baseUrl} placeholder="https://api.openai.com/v1" onChange={event => setBaseUrl(event.target.value)} /></label>}
        <label>API key<input required type="password" autoComplete="off" value={apiKey} placeholder={provider === 'anthropic' ? 'Anthropic API key' : 'Provider API key'} onChange={event => setApiKey(event.target.value)} /></label>
        <label className="assistant-remember"><input type="checkbox" checked={remember} onChange={event => setRemember(event.target.checked)} /> Remember on this device (stored unencrypted in this browser)</label>
        <div className="assistant-provider-actions"><button className="btn primary" type="submit" disabled={agent.running || agent.status === 'connecting' || !model.trim() || !apiKey.trim()}>{readyProvider ? 'Reconnect' : 'Connect'}</button>{readyProvider && <button className="btn" type="button" onClick={agent.disconnect}>Disconnect{agent.remembered ? ' and forget key' : ''}</button>}</div>
      </form>
      {readyProvider && <button className="btn primary" type="button" onClick={onClose}>Start chatting</button>}
      {provider === 'openai-compatible' && <p className="assistant-connect-note">The provider must allow requests from a web page (CORS). If it does not, use Anthropic, or put a proxy you run in the base URL.</p>}
      {agent.error && <p role="alert">{agent.error}</p>}
    </section>
    {copyError && <p role="alert">{copyError}</p>}
  </dialog>;
}
