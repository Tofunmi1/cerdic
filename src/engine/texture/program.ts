// The texture program: a short text that a person or a model writes to describe a procedural texture, one
// node per statement, in the same spirit as the engine's node language.
//
//   n    = fbm(scale_x=6, scale_y=6, iterations=6)            // a node: name = type(inputs, parameters)
//   wood = colorize(n, gradient=[0:#3b2210, 1:#c98a4b])        // positional arguments feed the inputs in order
//   albedo = wood                                              // a channel names the node that fills it
//   roughness = fbm(scale_x=30, scale_y=30)                    // (a channel can be given a node directly)
//   height = n
//
// Statements end at a newline or `;`. Comments start with `//`. Values: a number, `true`/`false`, a colour
// `#rgb`, `#rrggbb` or `#rrggbbaa`, an enum option by name (`noise=perlin`), a gradient `[0:#000, 1:#fff]`, a
// curve `[0:0, 0.5:0.8, 1:1]`. `n.1` is the second output of node `n`. Channels: albedo, roughness, metallic,
// height (becomes a normal map), emissive.
import { EXPR_NODE, EXPR_TYPE } from './expr/node';
import { IMAGE_NODE, IMAGE_TYPE } from './image-node';
import { SIM_NODE, SIM_TYPE } from './simulate-node';
import { expandGeneric } from './generic';
import type { InputDef, Library, NodeDef, ParamDef, TexNode, ParamValue } from './types';

export const CHANNELS = ['albedo', 'roughness', 'metallic', 'height', 'emissive', 'normal'] as const;
export type Channel = (typeof CHANNELS)[number];

export type TextureProgram = {
  nodes: TexNode[];
  channels: Partial<Record<Channel, { node: string; output?: number }>>;
};

/** A mistake in a texture program, with the line it is on and what would fix it. */
export class TextureError extends Error {
  constructor(message: string, readonly line?: number) {
    super(line === undefined ? message : `line ${line}: ${message}`);
  }
}

/** Splits at `separator` outside brackets, parentheses and double-quoted strings. */
function split(text: string, separator: RegExp | string): string[] {
  const out: string[] = [];
  let depth = 0;
  let start = 0;
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '\\') i++;
      else if (c === '"') quoted = false;
    } else if (c === '"') quoted = true;
    else if (c === '(' || c === '[') depth++;
    else if (c === ')' || c === ']') depth--;
    else if (depth === 0 && (typeof separator === 'string' ? c === separator : separator.test(c))) {
      out.push(text.slice(start, i));
      start = i + 1;
    }
  }
  out.push(text.slice(start));
  return out;
}

/** A line without its `//` comment; a `//` inside a double-quoted string is kept. */
function withoutComment(line: string): string {
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    if (quoted) {
      if (line[i] === '\\') i++;
      else if (line[i] === '"') quoted = false;
    } else if (line[i] === '"') quoted = true;
    else if (line[i] === '/' && line[i + 1] === '/') return line.slice(0, i);
  }
  return line;
}

const nameOf = (s: string) => s.trim().toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');

function colour(text: string, where: string): { r: number; g: number; b: number; a: number } {
  const m = /^#([0-9a-f]{3,8})$/i.exec(text.trim());
  if (!m || ![3, 6, 8].includes(m[1].length)) throw new TextureError(`${where}: "${text.trim()}" is not a colour (#rgb, #rrggbb or #rrggbbaa)`);
  const hex = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
  const byte = (i: number) => parseInt(hex.slice(i * 2, i * 2 + 2), 16) / 255;
  return { r: byte(0), g: byte(1), b: byte(2), a: hex.length === 8 ? byte(3) : 1 };
}

function list(text: string, where: string): [number, string][] {
  const t = text.trim();
  if (!t.startsWith('[') || !t.endsWith(']')) throw new TextureError(`${where}: expected a list like [0:..., 1:...]`);
  return split(t.slice(1, -1), ',').filter((s) => s.trim()).map((entry) => {
    const at = entry.indexOf(':');
    const position = Number(entry.slice(0, at));
    if (at < 0 || !Number.isFinite(position)) throw new TextureError(`${where}: "${entry.trim()}" must be position:value`);
    return [position, entry.slice(at + 1).trim()];
  });
}

function value(def: ParamDef, text: string, node: string): ParamValue {
  const where = `${node}.${def.name}`;
  const t = text.trim();
  switch (def.type) {
    case 'float': {
      const n = Number(t);
      if (!Number.isFinite(n)) throw new TextureError(`${where}: "${t}" is not a number`);
      return n;
    }
    case 'size': {
      const n = Number(t);
      if (!Number.isFinite(n) || n <= 0) throw new TextureError(`${where}: "${t}" is not a size`);
      return n >= 4 ? Math.log2(n) : n;
    }
    case 'boolean':
      if (t !== 'true' && t !== 'false') throw new TextureError(`${where}: use true or false`);
      return t === 'true';
    case 'enum': {
      const values = def.values ?? [];
      const index = values.findIndex((v) => nameOf(v.name) === nameOf(t) || nameOf(v.value) === nameOf(t));
      if (index >= 0) return index;
      if (/^\d+$/.test(t) && Number(t) < values.length) return Number(t);
      throw new TextureError(`${where}: "${t}" is not one of ${values.map((v) => nameOf(v.name)).join(', ')}`);
    }
    case 'string': {
      const quoted = /^"([\s\S]*)"$/.exec(t);
      if (!quoted) throw new TextureError(`${where}: write the text in double quotes, for example ${def.name}="out = vec3(p, 0)"`);
      return quoted[1].replace(/\\n/g, '\n').replace(/\\"/g, '"');
    }
    case 'color':
      return colour(t, where);
    case 'gradient': {
      const points = list(t, where).map(([pos, c]) => ({ pos, ...colour(c, where) }));
      if (points.length < 2) throw new TextureError(`${where}: a gradient needs at least two stops`);
      return { type: 'Gradient', interpolation: 1, points };
    }
    case 'curve': {
      const points = list(t, where).map(([x, y]) => ({ x, y: Number(y), ls: 0, rs: 0 }));
      if (points.length < 2 || points.some((p) => !Number.isFinite(p.y))) throw new TextureError(`${where}: a curve needs at least two x:y points`);
      return { type: 'Curve', points };
    }
    default:
      throw new TextureError(`${where}: parameters of type ${def.type} cannot be set from a texture program`);
  }
}

const reference = /^([A-Za-z_][A-Za-z0-9_]*)(?:\.(\d+))?$/;

/** Parses a texture program against a node library. Throws `TextureError`. */
export function parseTextureProgram(text: string, library: Library): TextureProgram {
  const program: TextureProgram = { nodes: [], channels: {} };
  const known = new Set<string>();
  const statements = split(text.split('\n').map(withoutComment).join('\n'), /[;\n]/);
  let lineNumber = 0;
  const channelOf = (name: string): name is Channel => (CHANNELS as readonly string[]).includes(name);
  for (const raw of statements) {
    lineNumber++;
    const statement = raw.trim();
    if (!statement) continue;
    const eq = statement.indexOf('=');
    if (eq < 1) throw new TextureError(`"${statement}" is not name = node(...)`, lineNumber);
    const name = statement.slice(0, eq).trim();
    const right = statement.slice(eq + 1).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) throw new TextureError(`"${name}" is not a valid name`, lineNumber);
    const alias = reference.exec(right);
    if (alias && !right.includes('(')) {
      if (!channelOf(name)) throw new TextureError(`only a channel (${CHANNELS.join(', ')}) can be set to another node, not "${name}"`, lineNumber);
      if (!known.has(alias[1])) throw new TextureError(`"${alias[1]}" is not defined yet`, lineNumber);
      program.channels[name] = { node: alias[1], output: alias[2] ? Number(alias[2]) : undefined };
      continue;
    }
    const open = right.indexOf('(');
    if (open < 1 || !right.endsWith(')')) throw new TextureError(`"${right}" is not a node call like fbm(scale_x=4)`, lineNumber);
    const type = right.slice(0, open).trim();
    const def = type === EXPR_TYPE ? EXPR_NODE : type === IMAGE_TYPE ? IMAGE_NODE : type === SIM_TYPE ? SIM_NODE : library[type];
    if (!def) throw new TextureError(`unknown texture node "${type}"`, lineNumber);
    if (known.has(name)) throw new TextureError(`"${name}" is defined twice`, lineNumber);
    const args = split(right.slice(open + 1, -1), ',').map((a) => a.trim()).filter(Boolean);
    const sizeArg = args.find((a) => /^generic_size\s*=/.test(a));
    const size = sizeArg ? Number(sizeArg.split('=')[1]) : 1;
    const model = def.shader_model && def.type !== 'graph' ? expandGeneric(def.shader_model, size) : undefined;
    const inputDefs: InputDef[] = model?.inputs ?? (def.type === 'graph' ? graphInputs(def) : []);
    // A subgraph node exposes numbers (its panel); colours, enums and curves are set on single nodes.
    const paramDefs: ParamDef[] = model?.parameters ?? Object.keys(def.parameters ?? {}).map((key) => ({ name: key, type: 'float' }));
    const node: TexNode = { id: name, type };
    const params: Record<string, ParamValue> = {};
    let positional = 0;
    for (const arg of args) {
      const named = /^([A-Za-z_][A-Za-z0-9_]*)\s*=(?!=)([\s\S]*)$/.exec(arg);
      const link = (inputName: string, text2: string) => {
        const r = reference.exec(text2.trim());
        if (!r || !known.has(r[1])) throw new TextureError(`input ${inputName} of ${type} must be a node defined above (got "${text2.trim()}")`, lineNumber);
        node.inputs = { ...node.inputs, [inputName]: { node: r[1], output: r[2] ? Number(r[2]) : undefined } };
      };
      if (!named) {
        const slot = inputDefs[positional++];
        if (!slot) throw new TextureError(`${type} takes ${inputDefs.length} input(s) (${inputDefs.map((i) => i.name).join(', ') || 'none'}); "${arg}" is one too many`, lineNumber);
        link(slot.name, arg);
        continue;
      }
      const [, key, rest] = named;
      if (key === 'generic_size') { params.generic_size = size; continue; }
      if (key === 'seed') { params.seed = Number(rest); continue; }
      const input = inputDefs.find((i) => i.name === key);
      if (input) { link(key, rest); continue; }
      const param = paramDefs.find((p) => p.name === key);
      if (!param) {
        const options = [...inputDefs.map((i) => `${i.name} (input)`), ...paramDefs.map((p) => p.name)];
        throw new TextureError(`${type} has no parameter or input "${key}"; it has ${options.join(', ') || 'none'}`, lineNumber);
      }
      params[key] = value(param, rest, type);
    }
    if (Object.keys(params).length) node.params = params;
    program.nodes.push(node);
    known.add(name);
    if (channelOf(name)) program.channels[name] = { node: name };
  }
  if (!program.channels.albedo && !program.channels.height && !program.channels.emissive && !program.channels.roughness && !program.channels.metallic) {
    throw new TextureError(`the program fills no channel; end with albedo = <node> (channels: ${CHANNELS.join(', ')})`);
  }
  return program;
}

/** The input names of a subgraph node (the ports of its `gen_inputs` node). */
function graphInputs(def: NodeDef): InputDef[] {
  const nodes = ((def as unknown as { nodes?: unknown }).nodes ?? []) as { name: string; ports?: { name: string; type: string; default?: string }[] }[];
  return (nodes.find((n) => n.name === 'gen_inputs')?.ports ?? []).map((p) => ({ name: p.name, type: p.type, default: p.default ?? '0.0' }));
}
