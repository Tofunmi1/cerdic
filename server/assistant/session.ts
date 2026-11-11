import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { CodexRpc } from './rpc';
import { catalog, guide } from './catalog';
import { formatToolResult } from '../../src/engine/agent/loop';
import { providerConfig, requestProviderTurn, providerToolResult, type ProviderConfig } from './provider';

type Scope = { projectId: string; worldId: string; conversationId: string };
type Send = (event: unknown) => void;
const mappingFile = resolve('target/assistant-threads.json');
let mappings: Record<string, string> | undefined;
let mappingLoad: Promise<Record<string, string>> | undefined;
const owners = new Map<string, LocalAssistant>();
let mappingWrite = Promise.resolve();
function scopeKey(scope: Scope) {
  if (!scope || ![scope.projectId, scope.worldId, scope.conversationId].every(id => typeof id === 'string' && /^[a-z0-9_.:-]{1,160}$/i.test(id))) throw new Error('Invalid conversation scope');
  return JSON.stringify([scope.projectId, scope.worldId, scope.conversationId]);
}
async function threadMap() { return mappings ??= await (mappingLoad ??= Bun.file(mappingFile).json().catch(() => ({}))); }
async function remember(key: string, id: string) {
  const map = await threadMap(); map[key] = id;
  mappingWrite = mappingWrite.catch(() => {}).then(async () => { await mkdir(resolve('target'), { recursive: true }); await Bun.write(mappingFile, JSON.stringify(map)); });
  await mappingWrite;
}
const instruction = `You are Cerdic's in-app 3D world builder. The browser scene is your workspace, not repository files. Use ONLY the supplied dynamic engine tools; never use shell, filesystem, external apps, or other MCP connections. All tools target the active project/world. Start by inspecting the current document and reading get_guide for core and the relevant operator families. Use named parts and small targeted edits for follow-ups. Build silhouette, then detail. Check and measure each meaningful change, fix actual diagnostics, use snapshot for visual review and frame_view at the end. Do not replace existing objects unless the user requested it. Never claim something was built if an engine call failed. Give a concise report of what changed. User-requested script.js scene behavior is allowed through engine tools; do not run host operating-system commands. Stop promptly when cancelled.`;

export class LocalAssistant {
  private rpc?: CodexRpc;
  private connected?: Promise<void>;
  private models: string[] = [];
  private active?: { key: string; scope: Scope; threadId: string; turnId?: string; stopping: boolean };
  private pending = new Map<string, { rpcId: number | string; timer: ReturnType<typeof setTimeout> }>();
  private provider?: ProviderConfig;
  private providerHistory = new Map<string, unknown[]>();
  private providerPending = new Map<string, { resolve: (value: { result?: unknown; error?: string }) => void; timer: ReturnType<typeof setTimeout> }>();
  private providerAbort?: AbortController;
  private closed = false;
  constructor(private send: Send) {}
  async receive(message: any) {
    try {
      if (message.type === 'connect') await (this.connected ??= this.connect());
      else if (message.type === 'connect_provider') await this.connectProvider(message.config);
      else if (message.type === 'prompt') { await this.connected; await this.prompt(message); }
      else if (message.type === 'tool_result') { if (this.provider) this.providerToolResult(message); else this.toolResult(message); }
      else if (message.type === 'stop') await this.stop();
      else throw new Error('Unknown assistant message');
    } catch (error) { this.send({ type: 'error', message: error instanceof Error ? error.message : String(error) }); }
  }
  private async connectProvider(value: unknown) {
    if (this.active) throw new Error('Stop the current turn before changing assistant connections.');
    const previous = this.rpc; this.rpc = undefined; previous?.close(); this.connected = undefined; this.provider = undefined; this.models = [];
    const config = providerConfig(value);
    this.provider = config;
    this.providerHistory.clear();
    this.send({ type: 'ready', connector: 'provider', models: [{ id: config.model, name: config.model, default: true }] });
  }
  private async connect() {
    this.provider = undefined; this.providerHistory.clear();
    const rpc = this.rpc = new CodexRpc();
    rpc.onMessage = message => { void this.event(message).catch(error => this.send({ type: 'error', message: String(error) })); };
    rpc.onExit = () => { if (!this.closed && this.rpc === rpc) { this.send({ type: 'disconnected', message: 'Local Codex stopped. Reconnect to continue.' }); this.release(); } };
    await rpc.request('initialize', { clientInfo: { name: 'cerdic_sidebar', title: 'Cerdic Sidebar', version: '0.1.0' }, capabilities: { experimentalApi: true } });
    rpc.send({ method: 'initialized', params: {} });
    const [account, models] = await Promise.all([rpc.request('account/read', {}), rpc.request('model/list', { limit: 100, includeHidden: false })]);
    if (!account.account) throw new Error('Codex is not signed in. Run codex login on this computer, then reconnect.');
    this.models = models.data.map((model: any) => model.model ?? model.id);
    this.send({ type: 'ready', models: models.data.map((model: any) => ({ id: model.model ?? model.id, name: model.displayName ?? model.id, default: model.isDefault ?? false })) });
  }
  private async prompt(message: { scope: Scope; text: string; model: string; context?: string }) {
    if (this.provider) return this.promptProvider(message);
    if (!this.rpc || !this.models.length) throw new Error('Connect local Codex first.');
    if (this.active) throw new Error('A turn is already running. Stop it or wait for the reply.');
    if (typeof message.text !== 'string' || !message.text.trim() || message.text.length > 100000) throw new Error('A prompt must contain 1–100,000 characters.');
    if (!this.models.includes(message.model)) throw new Error('Choose a model from the connected account.');
    const key = scopeKey(message.scope);
    if (owners.has(key)) throw new Error('This conversation is already running in another tab.');
    owners.set(key, this);
    this.active = { key, scope: message.scope, threadId: '', stopping: false };
    this.send({ type: 'running', scope: message.scope });
    try {
      const data = await catalog();
      const cwd = resolve('target/assistant-workspace'); await mkdir(cwd, { recursive: true });
      const saved = (await threadMap())[key];
      const common = { model: message.model, cwd, sandbox: 'read-only', approvalPolicy: 'never', baseInstructions: instruction, developerInstructions: data.instructions, environments: [] };
      const result = saved
        ? await this.rpc.request('thread/resume', { ...common, threadId: saved, excludeTurns: true })
        : await this.rpc.request('thread/start', { ...common, dynamicTools: data.tools.map(tool => ({ type: 'function', ...tool })) });
      if (!this.active || this.closed) return;
      this.active.threadId = result.thread.id;
      if (!saved) await remember(key, result.thread.id);
      if (this.active.stopping) { this.send({ type: 'done', scope: this.active.scope, status: 'interrupted' }); this.release(); return; }
      const context = !saved && typeof message.context === 'string' ? message.context.slice(-20000) : '';
      const turn = await this.rpc.request('turn/start', { threadId: result.thread.id, model: message.model, input: [{ type: 'text', text: context ? `Earlier messages in this world conversation:\n${context}\n\nCurrent request:\n${message.text}` : message.text }] });
      if (this.active) { this.active.turnId = turn.turn.id; if (this.active.stopping) await this.stop(); }
    } catch (error) { this.release(); throw error; }
  }
  private async promptProvider(message: { scope: Scope; text: string; model: string; context?: string }) {
    const config = this.provider;
    if (!config) throw new Error('Connect an API provider first.');
    if (this.active) throw new Error('A turn is already running. Stop it or wait for the reply.');
    if (typeof message.text !== 'string' || !message.text.trim() || message.text.length > 100000) throw new Error('A prompt must contain 1–100,000 characters.');
    if (message.model !== config.model) throw new Error('Reconnect to change the provider model.');
    const key = scopeKey(message.scope);
    if (owners.has(key)) throw new Error('This conversation is already running in another tab.');
    owners.set(key, this);
    const active = this.active = { key, scope: message.scope, threadId: '', stopping: false };
    const controller = this.providerAbort = new AbortController();
    this.send({ type: 'running', scope: message.scope });
    try {
      const data = await catalog();
      const history = this.providerHistory.get(key) ?? [];
      if (history.length > 40) history.splice(0, history.length - 40);
      const text = history.length || !message.context ? message.text : `Earlier messages in this world conversation:\n${message.context.slice(-20000)}\n\nCurrent request:\n${message.text}`;
      history.push({ role: 'user', content: text });
      this.providerHistory.set(key, history);
      for (let turn = 0; turn < 24; turn++) {
        if (active.stopping || controller.signal.aborted) break;
        const answer = await requestProviderTurn(config, `${instruction}\n\n${data.instructions}`, data.tools, history, controller.signal);
        history.push(answer.assistantMessage);
        if (answer.text) this.send({ type: 'text', scope: message.scope, id: `provider-${turn}`, delta: answer.text });
        if (!answer.calls.length) {
          this.send({ type: 'done', scope: message.scope, status: 'completed' }); this.release(); return;
        }
        const toolResults: unknown[] = [];
        for (const call of answer.calls) {
          if (!data.tools.some(tool => tool.name === call.name)) { toolResults.push(providerToolResult(config, call.id, undefined, 'This tool is not allowed in the sidebar.')); continue; }
          const result = await this.dispatchProviderTool(active.scope, call);
          toolResults.push(providerToolResult(config, call.id, result.result, result.error));
          if (active.stopping || controller.signal.aborted) break;
        }
        if (config.provider === 'anthropic') history.push({ role: 'user', content: toolResults });
        else history.push(...toolResults);
      }
      this.send({ type: 'done', scope: message.scope, status: active.stopping ? 'interrupted' : 'tool_limit' }); this.release();
    } catch (error) {
      if (controller.signal.aborted) this.send({ type: 'done', scope: message.scope, status: 'interrupted' });
      else { this.send({ type: 'error', message: error instanceof Error ? error.message : String(error) }); }
      this.release();
    }
  }
  private dispatchProviderTool(scope: Scope, call: { id: string; name: string; args: Record<string, unknown> }): Promise<{ result?: unknown; error?: string }> {
    return new Promise(resolve => {
      const timer = setTimeout(() => { this.providerPending.delete(call.id); resolve({ error: 'Browser tool timed out.' }); }, 120000);
      this.providerPending.set(call.id, { resolve, timer });
      void (async () => {
        try {
          const guideText = call.name === 'get_guide' ? await guide(call.args) : undefined;
          this.send({ type: 'tool', id: call.id, scope, name: call.name, args: call.args, guide: guideText });
        } catch (error) {
          clearTimeout(timer); this.providerPending.delete(call.id); resolve({ error: error instanceof Error ? error.message : String(error) });
        }
      })();
    });
  }
  private providerToolResult(message: any) {
    const pending = this.providerPending.get(message.id);
    if (!pending) return;
    clearTimeout(pending.timer); this.providerPending.delete(message.id);
    pending.resolve(message.error ? { error: String(message.error) } : { result: message.result });
  }
  private async event(message: any) {
    const params = message.params ?? {}, active = this.active;
    if (message.method === 'item/tool/call' && message.id !== undefined) {
      if (!active || active.stopping || params.threadId !== active.threadId) { this.respondError(message.id, 'The world is no longer active.'); return; }
      const tool = (await catalog()).tools.find(tool => tool.name === params.tool);
      if (!tool) { this.respondError(message.id, 'This tool is not allowed in the sidebar.'); return; }
      const id = crypto.randomUUID();
      const timer = setTimeout(() => { this.pending.delete(id); this.respondError(message.id, 'Browser tool timed out'); }, 120000);
      this.pending.set(id, { rpcId: message.id, timer });
      try {
        const guideText = params.tool === 'get_guide' ? await guide(params.arguments ?? {}) : undefined;
        this.send({ type: 'tool', id, scope: active.scope, name: params.tool, args: params.arguments ?? {}, guide: guideText });
      } catch (error) { clearTimeout(timer); this.pending.delete(id); this.respondError(message.id, String(error)); }
      return;
    }
    if (message.id !== undefined && message.method) { this.rpc?.send({ id: message.id, error: { code: -32601, message: 'Interactive host requests are unavailable in this scene-only assistant.' } }); return; }
    if (!active || params.threadId && params.threadId !== active.threadId) return;
    if (message.method === 'turn/started') active.turnId = params.turn.id;
    if (message.method === 'item/agentMessage/delta') this.send({ type: 'text', scope: active.scope, id: params.itemId, delta: params.delta });
    if (message.method === 'error') this.send({ type: 'warning', scope: active.scope, message: params.error?.message ?? 'Codex reported an error' });
    if (message.method === 'turn/completed') {
      this.send({ type: 'done', scope: active.scope, status: params.turn.status, error: params.turn.error?.message });
      this.release();
    }
  }
  private respondError(id: number | string, text: string) { this.rpc?.send({ id, result: { success: false, contentItems: [{ type: 'inputText', text }] } }); }
  private toolResult(message: any) {
    const waiting = this.pending.get(message.id);
    if (!waiting) return;
    clearTimeout(waiting.timer); this.pending.delete(message.id);
    const result = formatToolResult('', message.error ? { error: String(message.error) } : message.result);
    this.rpc?.send({ id: waiting.rpcId, result: { success: !result.isError, contentItems: [{ type: 'inputText', text: result.text.slice(0, 30000) }, ...result.images.slice(0, 3).map(image => ({ type: 'inputImage', imageUrl: `data:${image.media_type};base64,${image.data}` }))] } });
  }
  private async stop() {
    if (this.providerAbort) {
      this.active && (this.active.stopping = true);
      this.providerAbort.abort(); this.providerAbort = undefined;
      for (const waiting of this.providerPending.values()) { clearTimeout(waiting.timer); waiting.resolve({ error: 'Cancelled by the user.' }); }
      this.providerPending.clear();
      if (this.active) this.send({ type: 'done', scope: this.active.scope, status: 'interrupted' });
      this.release(); return;
    }
    if (!this.active) return;
    this.active.stopping = true;
    for (const waiting of this.pending.values()) { clearTimeout(waiting.timer); this.respondError(waiting.rpcId, 'Cancelled by the user'); } this.pending.clear();
    if (this.active.threadId && this.active.turnId) await this.rpc?.request('turn/interrupt', { threadId: this.active.threadId, turnId: this.active.turnId });
    else if (!this.active.threadId) this.send({ type: 'stopping' });
  }
  private release() {
    if (this.active) owners.delete(this.active.key);
    this.active = undefined;
    this.providerAbort = undefined;
    for (const waiting of this.pending.values()) { clearTimeout(waiting.timer); this.respondError(waiting.rpcId, 'Turn ended'); } this.pending.clear();
  }
  close() { this.closed = true; void this.stop().catch(() => {}); this.provider = undefined; this.providerHistory.clear(); this.release(); this.rpc?.close(); }
}
