// Writes src/studio/assistant-catalog.json: the tool list, instructions and guide topics the sidebar assistant gives a model.
// The MCP server builds these in Node (it reads files); the hosted page has no server, so the text is captured here and shipped as data.
//   bun scripts/gen-assistant-catalog.ts
const script = new URL('../engine/tools/mcp/server.mjs', import.meta.url).pathname;
const run = async (...args: string[]) => {
  const child = Bun.spawn([process.execPath, script, ...args], { stdout: 'pipe', stderr: 'pipe' });
  const [text, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  if (exit) throw new Error(`server.mjs ${args.join(' ')} failed: ${error.slice(0, 300)}`);
  return text;
};
// The sidebar may use these tools; shell, files and other apps are not offered.
const allowed = new Set(['get_guide', 'get_text', 'get_nodes', 'list_operators', 'get_status', 'measure', 'survey', 'bake', 'apply_text', 'scene_overview', 'find', 'inspect', 'check', 'game_describe', 'playtest', 'make_character', 'apply_commands', 'edit', 'diff', 'undo', 'redo', 'snapshot', 'script', 'world', 'frame_view', 'set_view', 'texture_nodes']);
const described = JSON.parse(await run('--describe')) as { instructions: string; guide: string; tools: Array<{ name: string; description: string; inputSchema: Record<string, unknown> }> };
const ids = [...described.guide.matchAll(/^- ([a-z0-9_]+): /gm)].map((match) => match[1]).filter((id) => id !== 'core');
const topics: Record<string, string> = {};
for (const id of ids) topics[id] = (await run('--guide', id)).trim();
const catalog = {
  instructions: described.instructions,
  core: described.guide,
  topics,
  tools: described.tools.filter((tool) => allowed.has(tool.name)).map((tool) => ({ name: tool.name, description: tool.description, input_schema: tool.inputSchema })),
};
await Bun.write(new URL('../src/studio/assistant-catalog.json', import.meta.url).pathname, JSON.stringify(catalog));
console.log(`assistant catalog: ${catalog.tools.length} tools, ${ids.length} topics`);
