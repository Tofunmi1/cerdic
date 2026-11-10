// The texture node catalog from the command line, for a model or a person without the MCP `texture_nodes` tool.
//   bun engine/tools/texture-nodes.ts                 every node name, grouped by what it makes
//   bun engine/tools/texture-nodes.ts bricks voronoi  those nodes in full: parameters (with choices), inputs, numbered outputs
//   bun engine/tools/texture-nodes.ts --search wood   nodes whose name contains a word
// In a texture program `n.1` is the second output of node `n`; the channels are albedo, roughness, metallic, height, emissive.
import { fileURLToPath } from 'node:url';
import { loadLibrary } from '../../src/engine/texture/library';
import { nodeInfo } from '../../src/engine/texture/catalog';

const root = fileURLToPath(new URL('../lib/material-maker', import.meta.url));
const library = loadLibrary(root);
const names = Object.keys(library).filter(n => !n.startsWith('preview')).sort();
const args = process.argv.slice(2);

function describe(name: string): string {
  if (!library[name]) return `${name}: not a node (run without arguments for the names, or --search word)`;
  let info: any;
  try {
    info = nodeInfo(name, library[name]);
  } catch (error) {
    return `${name}\n  no catalog entry (${error instanceof Error ? error.message : error}); it is a helper node, not one to use in a program`;
  }
  if (!info) return `${name}: not a node`;
  const params = (info.params ?? []).map((p: any) => `${p.name}${p.options ? `{${p.options.join('|')}}` : ''}${p.default !== undefined && p.default !== '' ? `=${typeof p.default === 'object' ? '…' : p.default}` : ''}`);
  const inputs = (info.inputs ?? []).map((i: any) => `${i.name}:${i.type}`);
  const outputs = (info.outputs ?? []).map((o: any, i: number) => `${i}${o.name ? ` ${o.name}` : ''}:${o.type ?? '?'}`);
  return `${name}\n  params  ${params.join(', ') || '-'}\n  inputs  ${inputs.join(', ') || '-'}\n  outputs ${outputs.join(', ') || '-'}`;
}

if (args.length === 0) {
  console.log(names.filter(n => !n.startsWith('tex3d')).join(' '));
  console.log(`\n${names.filter(n => n.startsWith('tex3d')).length} more nodes start with tex3d (they colour 3D solids); run with a name for details.`);
} else if (args[0] === '--search') {
  const word = (args[1] ?? '').toLowerCase();
  const found = names.filter(n => n.toLowerCase().includes(word));
  console.log(found.length ? found.join(' ') : `no node name contains "${word}"`);
} else {
  for (const name of args) console.log(describe(name));
}
