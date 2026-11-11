// The Messages API called straight from the page with the person's own key (the `anthropic-dangerous-direct-browser-access` header is Anthropic's switch for this).
// The key lives only in the header: it is never put in a body, a log or an error message. A hosted demo with no key of its own needs a proxy,
// so `baseUrl` can point at one.
import type { Block, Message, Provider, ProviderRequest, ProviderResponse, ToolSpec } from './types';

export type AnthropicOptions = {
  apiKey: string;
  model?: string;
  maxTokens?: number;
  baseUrl?: string;
  fetch?: typeof fetch;
  /** Retries for 429, 529 and 5xx answers (default 3). */
  retries?: number;
  /** Base wait between retries, doubled each time; a `retry-after` header wins (default 1000). */
  retryDelayMs?: number;
};

const CACHE = { type: 'ephemeral' } as const;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** The request's messages with one cache marker on the last block of the last message, so the growing conversation is cached up to the newest turn. */
function withCacheMarker(messages: Message[]): unknown[] {
  return messages.map((message, index) => {
    if (index !== messages.length - 1) return message;
    const blocks: Block[] = typeof message.content === 'string' ? [{ type: 'text', text: message.content }] : message.content;
    return { ...message, content: blocks.map((block, i) => (i === blocks.length - 1 ? { ...block, cache_control: CACHE } : block)) };
  });
}

export function anthropicProvider(options: AnthropicOptions): Provider {
  const base = options.baseUrl ?? 'https://api.anthropic.com';
  const doFetch = options.fetch ?? ((...args: Parameters<typeof fetch>) => fetch(...args));
  const retries = options.retries ?? 3;
  const delay = options.retryDelayMs ?? 1000;
  return {
    async send(request: ProviderRequest): Promise<ProviderResponse> {
      if (!options.apiKey) throw new Error('there is no API key: add yours in the chat settings');
      const body = {
        model: request.model ?? options.model ?? 'claude-sonnet-5-5',
        max_tokens: request.max_tokens ?? options.maxTokens ?? 8192,
        system: [{ type: 'text', text: request.system, cache_control: CACHE }],
        tools: request.tools as ToolSpec[],
        messages: withCacheMarker(request.messages),
      };
      const headers = {
        'content-type': 'application/json',
        'x-api-key': options.apiKey,
        'anthropic-version': '2023-06-01',
        'anthropic-dangerous-direct-browser-access': 'true',
      };
      for (let attempt = 0; ; attempt++) {
        const response = await doFetch(`${base}/v1/messages`, { method: 'POST', headers, body: JSON.stringify(body), signal: request.signal });
        if (response.ok) {
          const answer = (await response.json()) as ProviderResponse;
          return { content: answer.content, stop_reason: answer.stop_reason, usage: answer.usage };
        }
        const detail = await response.json().then((j: { error?: { message?: string } }) => j.error?.message ?? '', () => '');
        const busy = response.status === 429 || response.status === 529 || response.status >= 500;
        if (!busy || attempt >= retries) throw new Error(`${response.status} ${detail || response.statusText}`.trim());
        const wait = Number(response.headers.get('retry-after'));
        await sleep(Number.isFinite(wait) && response.headers.has('retry-after') ? wait * 1000 : delay * 2 ** attempt);
      }
    },
  };
}
