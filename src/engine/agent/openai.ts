// An OpenAI-compatible Chat Completions provider that runs in the page, next to the Anthropic one. It translates the shared Messages-style history
// (text, tool_use, tool_result blocks) to chat messages and back, so the same agent loop drives either. The key lives only in the request header.
// Whether the page may call a provider directly depends on that provider allowing browser requests (CORS); `baseUrl` can point at a proxy that does.
import type { Block, Message, Provider, ProviderRequest, ProviderResponse, ToolUseBlock } from './types';

export type OpenAIOptions = { apiKey: string; model?: string; maxTokens?: number; baseUrl?: string; fetch?: typeof fetch };

type Chat = Record<string, unknown>;

function toChat(system: string, messages: Message[]): Chat[] {
  const out: Chat[] = [{ role: 'system', content: system }];
  for (const message of messages) {
    if (typeof message.content === 'string') { out.push({ role: message.role, content: message.content }); continue; }
    if (message.role === 'assistant') {
      const text = message.content.filter((b): b is Extract<Block, { type: 'text' }> => b.type === 'text').map((b) => b.text).join('\n');
      const calls = message.content.filter((b): b is ToolUseBlock => b.type === 'tool_use')
        .map((b) => ({ id: b.id, type: 'function', function: { name: b.name, arguments: JSON.stringify(b.input ?? {}) } }));
      out.push({ role: 'assistant', content: text || null, ...(calls.length ? { tool_calls: calls } : {}) });
      continue;
    }
    const pictures: Chat[] = [];
    for (const block of message.content) {
      if (block.type === 'tool_result') {
        const parts = typeof block.content === 'string' ? [{ type: 'text' as const, text: block.content }] : block.content;
        out.push({ role: 'tool', tool_call_id: block.tool_use_id, content: parts.filter((p) => p.type === 'text').map((p) => (p as { text: string }).text).join('\n') || '(No textual result)' });
        for (const part of parts) if (part.type === 'image') pictures.push({ type: 'image_url', image_url: { url: `data:${part.source.media_type};base64,${part.source.data}` } });
      } else if (block.type === 'text') out.push({ role: 'user', content: block.text });
    }
    // Tool messages cannot carry pictures in this API, so they follow as one user message.
    if (pictures.length) out.push({ role: 'user', content: [{ type: 'text', text: 'Images from the tool results above.' }, ...pictures] });
  }
  return out;
}

export function openaiProvider(options: OpenAIOptions): Provider {
  const base = (options.baseUrl ?? 'https://api.openai.com/v1').replace(/\/$/, '');
  const doFetch = options.fetch ?? ((...args: Parameters<typeof fetch>) => fetch(...args));
  return {
    async send(request: ProviderRequest): Promise<ProviderResponse> {
      if (!options.apiKey) throw new Error('there is no API key: add yours in the assistant connection');
      const body = {
        model: request.model ?? options.model,
        max_tokens: request.max_tokens ?? options.maxTokens ?? 8192,
        messages: toChat(request.system, request.messages),
        tools: request.tools.map((tool) => ({ type: 'function', function: { name: tool.name, description: tool.description, parameters: tool.input_schema } })),
        tool_choice: 'auto',
      };
      const response = await doFetch(`${base}/chat/completions`, {
        method: 'POST', signal: request.signal, body: JSON.stringify(body),
        headers: { 'content-type': 'application/json', authorization: `Bearer ${options.apiKey}` },
      });
      const payload = await response.json().catch(() => ({})) as { error?: { message?: string }; message?: string; choices?: Array<{ finish_reason?: string; message?: { content?: string | null; tool_calls?: Array<{ id: string; function?: { name?: string; arguments?: string } }> } }>; usage?: { prompt_tokens?: number; completion_tokens?: number } };
      if (!response.ok) throw new Error(`${response.status} ${String(payload.error?.message ?? payload.message ?? response.statusText).slice(0, 500)}`.trim());
      const choice = payload.choices?.[0];
      if (!choice?.message) throw new Error('the provider returned no assistant message');
      const content: Block[] = [];
      if (choice.message.content) content.push({ type: 'text', text: choice.message.content });
      for (const call of choice.message.tool_calls ?? []) {
        let input: Record<string, unknown> = {};
        try { const parsed = JSON.parse(call.function?.arguments || '{}'); if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) input = parsed; } catch { throw new Error(`the provider sent invalid JSON for tool ${call.function?.name ?? 'call'}`); }
        content.push({ type: 'tool_use', id: String(call.id), name: String(call.function?.name), input });
      }
      return {
        content,
        stop_reason: choice.finish_reason === 'length' ? 'max_tokens' : content.some((b) => b.type === 'tool_use') ? 'tool_use' : 'end_turn',
        usage: { input_tokens: payload.usage?.prompt_tokens ?? 0, output_tokens: payload.usage?.completion_tokens ?? 0 },
      };
    },
  };
}
