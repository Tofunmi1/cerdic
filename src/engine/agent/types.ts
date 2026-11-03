// The shapes the in-page agent loop and its model providers share. They follow the Messages API (text, image, tool_use and tool_result blocks), so a provider is a thin
// translation and the history can be logged, saved and resumed as it is.
export type TextBlock = { type: 'text'; text: string };
export type ImageBlock = { type: 'image'; source: { type: 'base64'; media_type: string; data: string } };
export type ToolUseBlock = { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> };
export type ToolResultBlock = { type: 'tool_result'; tool_use_id: string; content: string | Array<TextBlock | ImageBlock>; is_error?: boolean };
export type Block = TextBlock | ImageBlock | ToolUseBlock | ToolResultBlock;
export type Message = { role: 'user' | 'assistant'; content: string | Block[] };

/** One tool the model may call: the same name, description and JSON schema the MCP server lists. */
export type ToolSpec = { name: string; description: string; input_schema: Record<string, unknown> };

export type Usage = { input_tokens: number; output_tokens: number; cache_read_input_tokens?: number; cache_creation_input_tokens?: number };

export type ProviderRequest = {
  system: string;
  tools: ToolSpec[];
  messages: Message[];
  model?: string;
  max_tokens?: number;
  signal?: AbortSignal;
};

export type ProviderResponse = { content: Block[]; stop_reason: string; usage: Usage };

/** A model behind one call. The Anthropic provider is in `anthropic.ts`; tests use a scripted one. */
export interface Provider {
  send(request: ProviderRequest): Promise<ProviderResponse>;
}
