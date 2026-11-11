import { host } from '../engine/host';
import { runPageTool } from '../engine/mcp-bridge';
import type { ToolSpec } from '../engine/agent/types';
import { validateToolArguments } from '../../engine/web/tool-arguments.js';
import { fullSpecs, indexText, select } from '../../engine/tools/mcp/operator-index.mjs';

// Runs one engine tool against the open world. The sidebar assistant and the agent link both come here, so a tool behaves the same whoever calls it.
export type Catalog = { instructions: string; core: string; topics: Record<string, string>; tools: ToolSpec[] };

let catalogLoad: Promise<Catalog> | undefined;
export const loadCatalog = () => catalogLoad ??= import('./assistant-catalog.json').then(module => module.default as unknown as Catalog);

async function operatorDocs(selection: Record<string, unknown>) {
  const operators = await runPageTool(host, 'list_operators', {}) as Array<{ id: string }>;
  if (Array.isArray(selection.ids)) return fullSpecs(operators, selection.ids.map(String));
  const filters = { query: selection.query, family: selection.family };
  return selection.detail === 'full' ? select(operators, filters) : indexText(operators, filters);
}
function guideText(catalog: Catalog, args: Record<string, unknown>) {
  const topics = Array.isArray(args.topics) ? args.topics : args.topic ? [args.topic] : [];
  if (!topics.length) return catalog.core;
  if (topics.length > 12 || topics.some(topic => typeof topic !== 'string' || !/^[a-z0-9_-]+$/i.test(topic))) throw new Error('Invalid guide topics');
  return topics.map(topic => {
    const id = String(topic).toLowerCase();
    if (id === 'core') return catalog.core;
    if (id === 'all') return [catalog.core, ...Object.values(catalog.topics)].join('\n\n');
    return catalog.topics[id] ?? `Unknown guide topic "${id}". Topics: ${Object.keys(catalog.topics).join(', ')}.`;
  }).join('\n\n');
}

export async function runEngineTool(name: string, args: Record<string, unknown>) {
  const invalid = validateToolArguments(name, args);
  if (invalid) throw new Error(invalid);
  if (name === 'get_guide') return { guide: guideText(await loadCatalog(), args), ...(args.operators ? { operators: await operatorDocs(args.operators as Record<string, unknown>) } : {}) };
  if (name === 'list_operators') return operatorDocs(args);
  return runPageTool(host, name, args);
}
