// The `image` texture node: a CC0 photograph (one of the maps beside the page in `outfits/`) as a node of a texture program, tiled over the surface.
//
//   brick = image(name="Bricks113A_Color", repeat_x=4, repeat_y=6)
//   rough = image(name="Bricks113A_Roughness", repeat_x=4, repeat_y=6)
//   wear  = blend(brick, dirt, mask, blend_type=multiply)      // layer the procedural wear over the photograph
//   albedo = wear; roughness = rough
//
// Like `expr`, a graph node of type `image` becomes a node of its own in the library before the graph is compiled, so it composes with every other texture node. Its pixels are a WebGL texture
// the baker binds under the uniform `uImg_<name>` (see `Baker.bake`); the page decodes the pictures first (`preloadOutfitImages`), so baking itself stays synchronous.
import type { Library, NodeDef, TexGraph } from './types';

export const IMAGE_TYPE = 'image';

/** What a texture program sees of the `image` node: the picture's name and how often it repeats over the unit square. */
export const IMAGE_NODE: NodeDef = {
  name: IMAGE_TYPE,
  shader_model: {
    name: IMAGE_TYPE,
    shortdesc: 'Picture',
    longdesc: 'A photograph from the shipped CC0 library (name="Concrete034_Color": the material and its map), repeated repeat_x by repeat_y times over the texture. Feed it to blend, colorize or a channel.',
    inputs: [],
    outputs: [{ type: 'rgba', rgba: '' }],
    parameters: [
      { name: 'name', type: 'string', label: 'Name', default: '' },
      { name: 'repeat_x', type: 'float', label: 'Repeat X', default: 1, min: 0.01, max: 256 },
      { name: 'repeat_y', type: 'float', label: 'Repeat Y', default: 1, min: 0.01, max: 256 },
    ],
  },
};

const NAME = /^[A-Za-z0-9_]{1,64}$/;
const literal = (n: number) => (Number.isInteger(n) ? n.toFixed(1) : String(Number(n.toPrecision(7))));

/** Replaces each `image` node of the graph with a generated library node that samples the picture's texture. */
export function expandImages(library: Library, graph: TexGraph): { library: Library; graph: TexGraph } {
  if (!graph.nodes.some((node) => node.type === IMAGE_TYPE)) return { library, graph };
  const expanded: Library = { ...library };
  const nodes = graph.nodes.map((node) => {
    if (node.type !== IMAGE_TYPE) return node;
    const name = node.params?.name;
    if (typeof name !== 'string' || !NAME.test(name)) throw new Error(`image node "${node.id}" needs name="Concrete034_Color" (letters, digits and underscores)`);
    const rx = Number(node.params?.repeat_x ?? 1);
    const ry = Number(node.params?.repeat_y ?? 1);
    if (!(rx > 0 && rx <= 256 && ry > 0 && ry <= 256)) throw new Error(`image node "${node.id}": repeat_x and repeat_y must be between 0 and 256`);
    const type = `image_${node.id.replace(/\W/g, '_')}`;
    expanded[type] = {
      name: type,
      shader_model: {
        name: type,
        inputs: [],
        outputs: [{ type: 'rgba', rgba: '$(name)_pic($uv)' }],
        instance: [`vec4 $(name)_pic(vec2 uv) {`, `  return texture(uImg_${name}, uv * vec2(${literal(rx)}, ${literal(ry)}));`, '}'],
      },
    };
    const { name: _name, repeat_x: _x, repeat_y: _y, ...params } = node.params ?? {};
    return { ...node, type, params };
  });
  return { library: expanded, graph: { ...graph, nodes } };
}

/** The picture names a compiled shader reads (its `uImg_<name>` uniforms). */
export function imagesIn(source: string): string[] {
  return [...new Set([...source.matchAll(/\buImg_([A-Za-z0-9_]+)/g)].map((m) => m[1]))];
}
