// Texture nodes that read what the surface is like: edges, creases, facing up, open to the sky, height. They work like any
// other node (`wear = surface_convex(gain=2)`), but need the mesh, so a program that uses one is baked onto a part's own UVs.
// The signals come from `signals.ts` (curvature, occlusion, slope) and the baker's position map (height).
import type { Library, NodeDef, ParamDef } from './types';

const number = (name: string, label: string, def: number, min: number, max: number, step: number, longdesc: string): ParamDef => ({
  name, label, type: 'float', default: def, min, max, step, longdesc,
} as ParamDef);

function node(name: string, title: string, description: string, expression: string, parameters: ParamDef[]): [string, NodeDef] {
  return [name, {
    name,
    type: 'shader',
    parameters: Object.fromEntries(parameters.map((p) => [p.name, p.default])),
    shader_model: {
      name: title,
      code: [],
      global: '',
      instance: '',
      inputs: [],
      outputs: [{ f: expression, type: 'f', shortdesc: description }],
      parameters,
      shortdesc: description,
      longdesc: description,
    },
  } as NodeDef];
}

/** The mesh-reading nodes, by name. Signals are 0 to 1 greyscale unless noted. */
export const SURFACE_NODES: Library = Object.fromEntries([
  node('surface_convex', 'Surface convex', 'How much the surface is a convex edge or bump: 1 on a sharp outer edge, 0 on flat or hollow ground. Use it for edge wear and chipped paint.',
    'clamp($gain*max(texture(uSurface, vUv).r, 0.0), 0.0, 1.0)',
    [number('gain', 'Gain', 1, 0, 16, 0.1, 'Multiplies the signal; higher makes a wider, stronger mask.')]),
  node('surface_cavity', 'Surface cavity', 'How much the surface is a crease or dent: 1 deep in a crack, 0 on flat or raised ground. Use it for dirt and grime.',
    'clamp($gain*max(-texture(uSurface, vUv).r, 0.0), 0.0, 1.0)',
    [number('gain', 'Gain', 1, 0, 16, 0.1, 'Multiplies the signal.')]),
  node('surface_curvature', 'Surface curvature', 'Signed curvature remapped so 0.5 is flat, towards 1 convex and towards 0 concave.',
    'clamp(0.5 + 0.5*$gain*texture(uSurface, vUv).r, 0.0, 1.0)',
    [number('gain', 'Gain', 1, 0, 16, 0.1, 'Multiplies the curvature before it is centred on 0.5.')]),
  node('surface_ao', 'Surface occlusion', 'Ambient occlusion: 1 in the open, towards 0 where the surface is enclosed. Use it to darken corners and under things.',
    'mix(1.0, texture(uSurface, vUv).g, $strength)',
    [number('strength', 'Strength', 1, 0, 1, 0.05, '0 ignores the occlusion, 1 uses all of it.')]),
  node('surface_up', 'Surface facing up', 'How much the surface faces the sky: 1 on a flat top, 0 on a wall or underside. Use it for moss, snow and dust.',
    'pow(clamp(texture(uSurface, vUv).b, 0.0, 1.0), $power)',
    [number('power', 'Power', 1, 0.1, 16, 0.1, 'Above 1 limits the mask to nearly flat tops; below 1 lets it creep down slopes.')]),
  node('surface_height', 'Surface height', 'The height of the surface point above the ground (y = 0), remapped so `low` gives 0 and `high` gives 1. Use it for wet lower parts or snow lines.',
    'clamp((texture(uPosition, vUv).y - $low) / max($high - $low, 0.0001), 0.0, 1.0)',
    [number('low', 'Low', 0, -100, 100, 0.01, 'Height in metres that maps to 0.'), number('high', 'High', 2, -100, 100, 0.01, 'Height in metres that maps to 1.')]),
]);

/** Names of the nodes that need the mesh's signal map or position map. */
export const SURFACE_NODE_NAMES = Object.keys(SURFACE_NODES);

/** `library` with the surface nodes added. */
export function withSurfaceNodes(library: Library): Library {
  return { ...library, ...SURFACE_NODES };
}
