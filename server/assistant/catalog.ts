import { resolve } from 'node:path';
export type EngineTool = { name: string; description: string; inputSchema: Record<string, unknown> };
const script = resolve('engine/tools/mcp/server.mjs');
const allowed = new Set(['get_guide', 'get_text', 'get_nodes', 'list_operators', 'get_status', 'measure', 'survey', 'bake', 'apply_text', 'scene_overview', 'find', 'inspect', 'check', 'game_describe', 'playtest', 'make_character', 'apply_commands', 'edit', 'diff', 'undo', 'redo', 'snapshot', 'script', 'world', 'frame_view', 'set_view', 'texture_nodes']);
let cached: Promise<{ instructions: string; guide: string; tools: EngineTool[] }> | undefined;
async function execute(args: string[]) {
  const child = Bun.spawn([process.execPath, script, ...args], { stdout: 'pipe', stderr: 'pipe' });
  const [text, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (exit) throw new Error(`Could not load engine tool catalogue: ${error.slice(0, 300)}`);
  return text;
}
export function catalog(): Promise<{ instructions: string; guide: string; tools: EngineTool[] }> { return cached ??= execute(['--describe']).then(text => { const data = JSON.parse(text); return { instructions: data.instructions, guide: data.guide, tools: (data.tools as EngineTool[]).filter(tool => allowed.has(tool.name)) }; }); }
export async function guide(args: Record<string, unknown>) {
  const topics = Array.isArray(args.topics) ? args.topics : args.topic ? [args.topic] : [];
  if (!topics.length) return (await catalog()).guide;
  if (topics.length > 12 || topics.some(topic => typeof topic !== 'string' || !/^[a-z0-9_-]+$/i.test(topic))) throw new Error('Invalid guide topics');
  return (await Promise.all(topics.map(topic => execute(['--guide', String(topic)])))).join('\n\n');
}
