// The `reaction_diffusion` texture node: a simulated pattern as a node of a texture program, tiled over the surface.
//
//   skin = reaction_diffusion(preset="stripes", size=128, steps=3000, repeat_x=2, repeat_y=2)
//   albedo = colorize(skin, gradient=[0:#2b1a0e, 1:#d9a441])
//
// Like `image`, a graph node of this type becomes a node of its own in the library before the graph is compiled, sampling a picture the baker binds under `uImg_<name>`. The picture is the
// result of `grayScott` (simulate.ts) for the node's settings, run here once and kept (the same settings are the same picture, so a program that uses it twice or is baked again costs nothing more).
import { registerRawPicture, hasPicture } from './outfit-images';
import { RD_PRESETS, grayScott, refusal, toPixels } from './simulate';
import type { Library, NodeDef, TexGraph } from './types';

export const SIM_TYPE = 'reaction_diffusion';

/** What a texture program sees of the `reaction_diffusion` node. */
export const SIM_NODE: NodeDef = {
  name: SIM_TYPE,
  shader_model: {
    name: SIM_TYPE,
    shortdesc: 'Reaction-diffusion',
    longdesc: `A pattern grown by simulation (Gray-Scott): spots, stripes, mazes, coral, worms or holes, like an animal's coat or a coral. preset picks the kind (${Object.keys(RD_PRESETS).join(', ')}); feed and kill (above 0) override the preset for your own pattern. The model runs steps times on a size by size grid (size 16 to 512, steps up to 30000, kept under 400 million cell updates), seeded by seed, and the picture tiles. Grey from 0 to 1: feed it to colorize or a channel.`,
    inputs: [],
    outputs: [{ type: 'rgba', rgba: '' }],
    parameters: [
      { name: 'preset', type: 'string', label: 'Preset', default: 'spots' },
      { name: 'size', type: 'float', label: 'Size', default: 128, min: 16, max: 512 },
      { name: 'steps', type: 'float', label: 'Steps', default: 3000, min: 1, max: 30000 },
      { name: 'feed', type: 'float', label: 'Feed', default: 0, min: 0, max: 0.2 },
      { name: 'kill', type: 'float', label: 'Kill', default: 0, min: 0, max: 0.1 },
      { name: 'seed', type: 'float', label: 'Seed', default: 1, min: 0, max: 1000000 },
      { name: 'repeat_x', type: 'float', label: 'Repeat X', default: 1, min: 0.01, max: 256 },
      { name: 'repeat_y', type: 'float', label: 'Repeat Y', default: 1, min: 0.01, max: 256 },
    ],
  },
};

const literal = (n: number) => (Number.isInteger(n) ? n.toFixed(1) : String(Number(n.toPrecision(7))));

/** A short stable name for one set of settings (FNV-1a over their text). */
function pictureName(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193) >>> 0;
  return `rd_${h.toString(16).padStart(8, '0')}`;
}

/** Replaces each `reaction_diffusion` node with a generated library node that samples the simulated picture (running the simulation if it has not been run for these settings). */
export function expandSimulations(library: Library, graph: TexGraph): { library: Library; graph: TexGraph } {
  if (!graph.nodes.some((node) => node.type === SIM_TYPE)) return { library, graph };
  const expanded: Library = { ...library };
  const nodes = graph.nodes.map((node) => {
    if (node.type !== SIM_TYPE) return node;
    const p = node.params ?? {};
    const preset = String(p.preset ?? 'spots');
    const base = RD_PRESETS[preset];
    if (!base) throw new Error(`reaction_diffusion node "${node.id}": preset "${preset}" is not one of ${Object.keys(RD_PRESETS).join(', ')}`);
    const feed = Number(p.feed) > 0 ? Number(p.feed) : base.feed;
    const kill = Number(p.kill) > 0 ? Number(p.kill) : base.kill;
    const settings = { size: Number(p.size ?? 128), steps: Number(p.steps ?? 3000), feed, kill, seed: Number(p.seed ?? 1) };
    const why = refusal(settings);
    if (why) throw new Error(`reaction_diffusion node "${node.id}": ${why}`);
    const rx = Number(p.repeat_x ?? 1), ry = Number(p.repeat_y ?? 1);
    if (!(rx > 0 && rx <= 256 && ry > 0 && ry <= 256)) throw new Error(`reaction_diffusion node "${node.id}": repeat_x and repeat_y must be between 0 and 256`);
    const picture = pictureName(JSON.stringify(settings));
    if (!hasPicture(picture)) registerRawPicture(picture, settings.size, settings.size, toPixels(grayScott(settings)));
    const type = `${SIM_TYPE}_${node.id.replace(/\W/g, '_')}`;
    expanded[type] = {
      name: type,
      shader_model: {
        name: type,
        inputs: [],
        outputs: [{ type: 'rgba', rgba: '$(name)_rd($uv)' }],
        instance: [`vec4 $(name)_rd(vec2 uv) {`, `  return texture(uImg_${picture}, uv * vec2(${literal(rx)}, ${literal(ry)}));`, '}'],
      },
    };
    return { ...node, type, params: {} };
  });
  return { library: expanded, graph: { ...graph, nodes } };
}
