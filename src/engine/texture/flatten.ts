// Flattening: Material Maker nodes can be subgraphs (a `.mmg` of type "graph" holding inner nodes, wires, a
// parameter panel and input/output ports). The compiler only understands single shader nodes, so this turns a
// texture graph into a flat set of shader nodes: subgraphs are expanded, `buffer` and `reroute` nodes are
// wires, `switch` nodes pick their source, and the panel parameters of a subgraph are copied onto the inner
// nodes they control (the `remote` node's widgets).
import { expandGeneric } from './generic';
import type { Library, NodeDef, ParamValue, ShaderModel, TexGraph } from './types';

export type FlatRef = { node: string; output: number };
export type FlatNode = {
  id: string;
  /** Library name, for messages. */
  type: string;
  model: ShaderModel;
  params: Record<string, unknown>;
  /** Named parameters of the enclosing subgraph, available to parameter expressions as `$name`. */
  env: Record<string, unknown>;
  seed: number;
  inputs: Record<string, FlatRef>;
};
export type Flat = { nodes: Map<string, FlatNode>; root: FlatRef };

type Inner = {
  name: string;
  type: string;
  parameters?: Record<string, unknown>;
  shader_model?: ShaderModel;
  ports?: { name: string; type: string; default?: string }[];
  widgets?: Widget[];
  generic_size?: number;
  seed_int?: number;
  nodes?: Inner[];
  connections?: Wire[];
};
type Wire = { from: string; from_port: number; to: string; to_port: number };
type Widget = {
  name: string;
  type: string;
  default?: unknown;
  linked_widgets?: { node: string; widget: string }[];
  configurations?: Record<string, { node: string; widget: string; value: unknown }[]>;
};
type Graph = NodeDef & { nodes: Inner[]; connections: Wire[] };

const DEFAULT_OF_TYPE: Record<string, string> = { f: '0.0', rgb: 'vec3(0.0)', rgba: 'vec4(0.0, 0.0, 0.0, 1.0)', sdf2d: '1.0' };
const SPECIAL = new Set(['ios', 'remote', 'buffer', 'switch', 'comment', 'reroute', 'shader', 'graph']);

class Flattener {
  readonly nodes = new Map<string, FlatNode>();
  private made = new Map<string, FlatRef>();

  constructor(private lib: Library) {}

  private add(node: FlatNode) {
    this.nodes.set(node.id, node);
    return node;
  }

  /** A node that stands for an unconnected subgraph input: its default expression. */
  private constant(id: string, type: string, expression: string): FlatRef {
    const key = `const:${id}`;
    if (!this.made.has(key)) {
      const model: ShaderModel = { outputs: [{ type, [type]: expression }] };
      this.add({ id: key, type: 'default', model, params: {}, env: {}, seed: 0, inputs: {} });
      this.made.set(key, { node: key, output: 0 });
    }
    return this.made.get(key) as FlatRef;
  }

  /** A shader node (library, inline or top level) wired to `links`. Returns the flat node's id. */
  shader(id: string, type: string, model: ShaderModel, params: Record<string, unknown>, env: Record<string, unknown>, seed: number, size: number, links: (name: string, index: number) => FlatRef | undefined): string {
    const expanded = expandGeneric(model, size);
    const inputs: Record<string, FlatRef> = {};
    (expanded.inputs ?? []).forEach((input, index) => {
      const link = links(input.name, index);
      if (link) inputs[input.name] = link;
    });
    this.add({ id, type, model: expanded, params, env, seed, inputs });
    return id;
  }

  /** Instantiates a library node by name; returns what each of its outputs resolves to. */
  instantiate(id: string, type: string, params: Record<string, unknown>, env: Record<string, unknown>, seed: number, links: (name: string, index: number) => FlatRef | undefined, size: number): (port: number) => FlatRef {
    const def = this.lib[type];
    if (!def) throw new Error(`unknown texture node "${type}"`);
    if (def.type === 'graph') return this.subgraph(id, def as Graph, params, seed, links);
    const merged = { ...(def.parameters ?? {}), ...params };
    this.shader(id, type, def.shader_model, merged, env, seed, size, links);
    return (port) => ({ node: id, output: port });
  }

  private subgraph(prefix: string, def: Graph, given: Record<string, unknown>, seed: number, outer: (name: string, index: number) => FlatRef | undefined): (port: number) => FlatRef {
    const params = { ...(def.parameters ?? {}), ...given };
    const byName = new Map(def.nodes.map((n) => [n.name, n]));
    const wires = new Map(def.connections.map((w) => [`${w.to}:${w.to_port}`, w]));
    const env: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(params)) if (typeof value === 'number') env[key] = value;
    const overrides = new Map<string, Record<string, unknown>>();
    const set = (node: string, widget: string, value: unknown) => overrides.set(node, { ...overrides.get(node), [widget]: value });
    for (const remote of def.nodes.filter((n) => n.type === 'remote')) {
      for (const w of remote.widgets ?? []) {
        const value = params[w.name] ?? w.default;
        if (w.type === 'named_parameter') env[w.name] = value;
        else if (w.type === 'config_control' && w.configurations) {
          const choice = Object.values(w.configurations)[Number(value) || 0] ?? [];
          for (const c of choice) set(c.node, c.widget, c.value);
        } else for (const l of w.linked_widgets ?? []) set(l.node, l.widget, value);
      }
    }
    const inputPorts = byName.get('gen_inputs')?.ports ?? [];
    const outputPorts = byName.get('gen_outputs')?.ports ?? [];
    const cache = new Map<string, FlatRef>();

    const input = (name: string, index: number): FlatRef | undefined => {
      const wire = wires.get(`${name}:${index}`);
      return wire ? resolve(wire.from, wire.from_port) : undefined;
    };
    const resolve = (name: string, port: number): FlatRef => {
      const key = `${name}:${port}`;
      const known = cache.get(key);
      if (known) return known;
      const node = byName.get(name);
      if (!node) throw new Error(`subgraph ${prefix} has no node "${name}"`);
      let ref: FlatRef | undefined;
      if (node.type === 'ios') {
        const p = inputPorts[port];
        if (!p) throw new Error(`subgraph ${prefix} has no input port ${port}`);
        ref = outer(p.name, port) ?? this.constant(`${prefix}/${p.name}`, p.type, p.default ?? DEFAULT_OF_TYPE[p.type] ?? '0.0');
      } else if (node.type === 'buffer' || node.type === 'reroute') {
        ref = input(name, 0);
        if (!ref) throw new Error(`${node.type} ${name} in ${prefix} has nothing connected`);
      } else if (node.type === 'switch') {
        const outputs = Number(node.parameters?.outputs ?? 2);
        const source = Number(overrides.get(name)?.source ?? node.parameters?.source ?? 0);
        ref = input(name, source * outputs + port);
        if (!ref) throw new Error(`switch ${name} in ${prefix} has nothing connected to the chosen source`);
      } else if (node.type === 'shader' || node.type === 'graph' || !SPECIAL.has(node.type)) {
        ref = this.inner(prefix, node, overrides.get(name) ?? {}, env, seed, input)(port);
      } else throw new Error(`node type "${node.type}" in ${prefix} is not supported`);
      cache.set(key, ref);
      return ref;
    };
    return (port) => {
      const wire = wires.get(`gen_outputs:${port}`);
      if (!wire) throw new Error(`subgraph ${prefix} has nothing connected to output ${port} (${outputPorts[port]?.name ?? '?'})`);
      return resolve(wire.from, wire.from_port);
    };
  }

  private inner(prefix: string, node: Inner, override: Record<string, unknown>, env: Record<string, unknown>, seed: number, input: (name: string, index: number) => FlatRef | undefined): (port: number) => FlatRef {
    const id = `${prefix}/${node.name}`;
    const own = { ...(node.parameters ?? {}), ...override };
    const innerSeed = (seed + (node.seed_int ?? 0) / 65536) % 1;
    const links = (_name: string, index: number) => input(node.name, index);
    if (node.type === 'shader') {
      if (!node.shader_model) throw new Error(`shader node ${id} has no model`);
      this.shader(id, node.name, node.shader_model, own, env, innerSeed, node.generic_size ?? 1, links);
      return (port) => ({ node: id, output: port });
    }
    if (node.type === 'graph') return this.subgraph(id, node as unknown as Graph, own, innerSeed, links);
    return this.instantiate(id, node.type, own, env, innerSeed, links, node.generic_size ?? 1);
  }
}

/** Expands a texture graph into shader nodes only. */
export function flatten(library: Library, graph: TexGraph): Flat {
  const f = new Flattener(library);
  const byId = new Map(graph.nodes.map((n) => [n.id, n]));
  const made = new Map<string, (port: number) => FlatRef>();
  const build = (id: string): ((port: number) => FlatRef) => {
    const known = made.get(id);
    if (known) return known;
    const node = byId.get(id);
    if (!node) throw new Error(`texture graph has no node "${id}"`);
    const def = library[node.type];
    if (!def) throw new Error(`unknown texture node "${node.type}"`);
    const names = def.type === 'graph' ? ((def as Graph).nodes.find((n) => n.name === 'gen_inputs')?.ports ?? []).map((p) => p.name) : undefined;
    const links = (name: string, index: number): FlatRef | undefined => {
      const link = node.inputs?.[name] ?? (names ? undefined : node.inputs?.[String(index)]);
      return link ? build(link.node)(link.output ?? 0) : undefined;
    };
    const { generic_size, seed, ...params } = (node.params ?? {}) as Record<string, ParamValue>;
    const outputs = f.instantiate(`n:${id}`, node.type, params, {}, typeof seed === 'number' ? seed : 0, links, typeof generic_size === 'number' ? generic_size : 1);
    made.set(id, outputs);
    return outputs;
  };
  const root = build(graph.output.node)(graph.output.output ?? 0);
  return { nodes: f.nodes, root };
}
