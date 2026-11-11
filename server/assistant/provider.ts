import type { EngineTool } from './catalog';
import { formatToolResult } from '../../src/engine/agent/loop';

export type ProviderConfig = { provider: 'anthropic' | 'openai-compatible'; apiKey: string; model: string; baseUrl?: string };
export function providerConfig(value: unknown): ProviderConfig {
  if (!value || typeof value !== 'object') throw new Error('Choose an API provider and enter its connection details.');
  const input = value as Record<string, unknown>;
  const provider = input.provider;
  const apiKey = input.apiKey;
  const model = input.model;
  const baseUrl = input.baseUrl;
  if (provider !== 'anthropic' && provider !== 'openai-compatible') throw new Error('Unsupported API provider.');
  if (typeof apiKey !== 'string' || apiKey.length < 8 || apiKey.length > 4096 || /[\r\n]/.test(apiKey)) throw new Error('Enter a valid API key.');
  if (typeof model !== 'string' || !model.trim() || model.length > 160 || /[\r\n]/.test(model)) throw new Error('Enter a model ID.');
  let normalized: string | undefined;
  if (baseUrl !== undefined && baseUrl !== '') {
    if (typeof baseUrl !== 'string' || baseUrl.length > 500) throw new Error('Invalid API base URL.');
    let url: URL;
    try { url = new URL(baseUrl); } catch { throw new Error('Enter a complete API base URL.'); }
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) throw new Error('API base URL must be an HTTP(S) endpoint without credentials, query or fragment.');
    if (url.protocol === 'http:' && !['localhost', '127.0.0.1', '::1'].includes(url.hostname)) throw new Error('Use HTTPS for remote API providers.');
    normalized = url.toString().replace(/\/$/, '');
  }
  return { provider, apiKey, model: model.trim(), baseUrl: normalized };
}

type ChatMessage = { role: 'user' | 'assistant' | 'tool'; content?: string; tool_calls?: Array<{ id: string; type: 'function'; function: { name: string; arguments: string } }>; tool_call_id?: string };
export type ProviderTurn = { text: string; calls: Array<{ id: string; name: string; args: Record<string, unknown> }>; assistantMessage: unknown };
const endpoint = (config: ProviderConfig) => {
  const base = config.baseUrl ?? (config.provider === 'anthropic' ? 'https://api.anthropic.com' : 'https://api.openai.com/v1');
  return config.provider === 'anthropic' ? `${base}/v1/messages` : `${base}/chat/completions`;
};

export async function requestProviderTurn(config: ProviderConfig, system: string, tools: EngineTool[], messages: unknown[], signal: AbortSignal): Promise<ProviderTurn> {
  const schemas = tools.map(tool => ({ name: tool.name, description: tool.description, input_schema: tool.inputSchema }));
  const response = await fetch(endpoint(config), config.provider === 'anthropic' ? {
    method: 'POST', signal, headers: { 'content-type': 'application/json', 'x-api-key': config.apiKey, 'anthropic-version': '2023-06-01' },
    body: JSON.stringify({ model: config.model, max_tokens: 8192, system, tools: schemas, messages }),
  } : {
    method: 'POST', signal, headers: { 'content-type': 'application/json', authorization: `Bearer ${config.apiKey}` },
    body: JSON.stringify({ model: config.model, max_tokens: 8192, messages: [{ role: 'system', content: system }, ...messages], tools: schemas.map(tool => ({ type: 'function', function: { ...tool, parameters: tool.input_schema } })), tool_choice: 'auto' }),
  });
  const payload = await response.json().catch(() => ({})) as any;
  if (!response.ok) throw new Error(`Provider request failed (${response.status}): ${String(payload?.error?.message ?? payload?.message ?? response.statusText).slice(0, 700)}`);
  if (config.provider === 'anthropic') {
    const content = Array.isArray(payload.content) ? payload.content : [];
    const text = content.filter((block: any) => block.type === 'text').map((block: any) => block.text).join('\n');
    const calls = content.filter((block: any) => block.type === 'tool_use').map((block: any) => ({ id: String(block.id), name: String(block.name), args: block.input && typeof block.input === 'object' ? block.input : {} }));
    return { text, calls, assistantMessage: { role: 'assistant', content } };
  }
  const message = payload.choices?.[0]?.message;
  if (!message || typeof message !== 'object') throw new Error('Provider returned no assistant message.');
  const calls = (Array.isArray(message.tool_calls) ? message.tool_calls : []).map((call: any) => {
    let args: unknown = {};
    try { args = JSON.parse(call.function?.arguments ?? '{}'); } catch { throw new Error(`Provider returned invalid JSON for tool ${call.function?.name ?? 'call'}.`); }
    if (!args || typeof args !== 'object' || Array.isArray(args)) throw new Error('Provider tool arguments must be an object.');
    return { id: String(call.id), name: String(call.function?.name), args: args as Record<string, unknown> };
  });
  return { text: typeof message.content === 'string' ? message.content : '', calls, assistantMessage: message as ChatMessage };
}

export function providerToolResult(config: ProviderConfig, id: string, result: unknown, error?: string) {
  const formatted = formatToolResult('', error ? { error } : result);
  const clean = formatted.text.slice(0, 30000);
  return config.provider === 'anthropic'
    ? { type: 'tool_result', tool_use_id: id, content: [...(clean ? [{ type: 'text', text: clean }] : []), ...formatted.images.slice(0, 3).map(image => ({ type: 'image', source: { type: 'base64', media_type: image.media_type, data: image.data } }))], is_error: Boolean(error) }
    : { role: 'tool', tool_call_id: id, content: clean || '(No textual result)' };
}
