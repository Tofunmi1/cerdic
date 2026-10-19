// The catalog of texture nodes a model or person can use: generated from the vendored Material Maker
// definitions (names, inputs, parameters with ranges, defaults and choices), so it never drifts from what the
// compiler accepts. The guide shows the curated set; `describe` gives the full detail of any node.
import { expandGeneric } from './generic';
import type { Library, NodeDef, ParamDef } from './types';

/** The nodes shown in the guide: the useful, verified core of the 350-odd that compile. */
export const CURATED = [
  // noises
  'perlin', 'fbm', 'voronoi', 'noise_white', 'color_noise', 'wavelet_noise', 'clouds_noise', 'directional_noise', 'noise_anisotropic', 'scratches', 'dirt', 'crystal',
  // patterns and gradients
  'bricks', 'beehive', 'cairo', 'weave', 'diagonal_weave', 'truchet', 'pattern', 'sine_wave', 'radial_gradient', 'circular_gradient', 'spiral_gradient', 'tiler', 'splatter', 'shape',
  // colour
  'colorize', 'tones', 'tones_step', 'tones_map', 'brightness_contrast', 'adjust_hsv', 'invert', 'greyscale', 'quantize', 'remap', 'colormap', 'palettize', 'multigradient', 'blend', 'math', 'combine', 'decompose',
  // transforms and warps
  'translate', 'rotate', 'scale', 'mirror', 'kaleidoscope', 'warp', 'directional_warp', 'swirl', 'twist', 'spherize', 'tile2x2', 'pixelize',
  // filters
  'gaussian_blur', 'sharpen', 'emboss', 'edge_detect',
  // the surface itself: wear on edges, dirt in creases, moss on top (they need the part's mesh)
  'surface_convex', 'surface_cavity', 'surface_curvature', 'surface_ao', 'surface_up', 'surface_height',
  // 3D (object space): wrap a mesh with no seams
  'tex3d_fbm', 'tex3d_voronoi', 'tex3d_pattern', 'tex3d_colorize', 'tex3d_tonality', 'tex3d_math', 'tex3d_blend', 'tex3d_distort', 'tex3d_warp', 'tex3d_scale', 'tex3d_rotate', 'tex3d_translate', 'tex3d_uniform', 'tex3d_from2d',
] as const;

export type ParamInfo = { name: string; type: string; default?: unknown; min?: number; max?: number; options?: string[]; description?: string };
export type NodeInfo = {
  name: string;
  title: string;
  description: string;
  /** `2d` nodes are drawn in UV space, `3d` nodes in object space. */
  space: '2d' | '3d';
  inputs: { name: string; type: string; description?: string }[];
  outputs: { type: string; description?: string }[];
  params: ParamInfo[];
  /** False for nodes that need things the engine does not have (buffers, meshes, iteration). */
  generic: boolean;
};

const slug = (s: string) => s.trim().toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');

function paramInfo(def: ParamDef, defaults: Record<string, unknown>): ParamInfo {
  const raw = defaults[def.name] ?? def.default;
  const info: ParamInfo = { name: def.name, type: def.type, description: (def as { longdesc?: string }).longdesc };
  if (def.type === 'enum') {
    info.options = (def.values ?? []).map((v) => slug(v.name));
    info.default = typeof raw === 'number' ? info.options[raw] : raw;
  } else {
    info.default = raw;
    if (def.min !== undefined) info.min = def.min;
    if (def.max !== undefined) info.max = def.max;
  }
  return info;
}

/** What a node is and how to call it. */
export function nodeInfo(name: string, def: NodeDef): NodeInfo | undefined {
  const model = def.shader_model ? expandGeneric(def.shader_model, 1) : undefined;
  if (!model || def.type === 'graph') {
    // A subgraph exposes its ports and panel numbers.
    const nodes = ((def as unknown as { nodes?: { name: string; ports?: { name: string; type: string }[] }[] }).nodes ?? []);
    const inputs = (nodes.find((n) => n.name === 'gen_inputs')?.ports ?? []).map((p) => ({ name: p.name, type: p.type }));
    const outputs = (nodes.find((n) => n.name === 'gen_outputs')?.ports ?? []).map((p) => ({ type: p.type }));
    const params = Object.entries(def.parameters ?? {}).filter(([, v]) => typeof v === 'number').map(([key, v]) => ({ name: key, type: 'float', default: v }));
    const label = (def as unknown as { label?: string; shortdesc?: string; longdesc?: string });
    return { name, title: label.label ?? label.shortdesc ?? name, description: label.longdesc ?? '', space: '2d', inputs, outputs, params, generic: false };
  }
  const defaults = def.parameters ?? {};
  const first = model.outputs?.[0]?.type ?? 'f';
  return {
    name,
    title: model.shortdesc ?? name,
    description: model.longdesc ?? '',
    space: /^(tex3d|sdf3d)/.test(first) || (model.inputs ?? []).some((i) => /^(tex3d|sdf3d)/.test(i.type)) ? '3d' : '2d',
    inputs: (model.inputs ?? []).map((i) => ({ name: i.name, type: i.type, description: (i as { longdesc?: string }).longdesc })),
    outputs: (model.outputs ?? []).map((o) => ({ type: o.type ?? 'f', description: (o as { longdesc?: string }).longdesc })),
    params: (model.parameters ?? []).map((p) => paramInfo(p, defaults)),
    generic: (def.shader_model.inputs ?? []).some((i) => i.name.includes('#')),
  };
}

const show = (v: unknown) => (typeof v === 'object' && v !== null ? '…' : String(v));

/** One line per node: the guide's form. */
export function summary(info: NodeInfo): string {
  const params = info.params.map((p) => {
    if (p.options) return `${p.name}=${p.options.join('|')}`;
    if (p.type === 'gradient' || p.type === 'curve' || p.type === 'color') return `${p.name}=<${p.type}>`;
    const range = p.min !== undefined && p.max !== undefined ? `${p.min}..${p.max}` : p.type;
    return `${p.name}=${range}(${show(p.default)})`;
  });
  const inputs = info.inputs.map((i) => `${i.name}:${i.type}`).join(', ');
  const outs = info.outputs.map((o) => o.type).join(',');
  return `${info.name}(${inputs}) -> ${outs}${params.length ? ` [${params.join(' ')}]` : ''} - ${info.title}`;
}

/** The detail of one node, with descriptions of its parameters. */
export function describe(library: Library, name: string): string {
  const def = library[name];
  const info = def ? nodeInfo(name, def) : undefined;
  if (!info) return `unknown texture node "${name}"`;
  const lines = [summary(info), info.description];
  for (const p of info.params) if (p.description) lines.push(`  ${p.name}: ${p.description}`);
  return lines.filter(Boolean).join('\n');
}

/** Names of library nodes matching a search word in their name, title or description. */
export function search(library: Library, word: string, limit = 40): string[] {
  const w = word.toLowerCase();
  const out: string[] = [];
  for (const [name, def] of Object.entries(library)) {
    const info = nodeInfo(name, def);
    if (info && `${name} ${info.title} ${info.description}`.toLowerCase().includes(w)) out.push(name);
    if (out.length >= limit) break;
  }
  return out;
}
