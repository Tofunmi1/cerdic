import type { MeshPart } from '../worker-types';
import { compileTexture } from './compile';
import { Baker, normalFromHeight, type PositionMap } from './bake';
import { parseTextureProgram, type Channel } from './program';
import { surfaceInputs } from './signals';
import { SURFACE_NODE_NAMES } from './surface-nodes';
import type { Library } from './types';
import { boxChartUvs, cornerPositions, worldTileUvs } from './uv';

export type BakedMaterialMaps = {
  /** One UV per triangle corner, suitable for a mesh's non-indexed texture attributes. */
  uvs: Float32Array;
  albedo?: Uint8Array;
  metallicRoughness?: Uint8Array;
  normal?: Uint8Array;
  emissive?: Uint8Array;
  size: number;
};

/**
 * Parses and bakes a material's authored program. Object-space nodes share a position map built from the
 * same corner UVs. Caller owns the Baker and must dispose it when the application/export session ends.
 */
export function bakeMaterialMaps(
  library: Library,
  helpers: string,
  part: MeshPart,
  baker: Baker,
  size = 512,
  heightStrength = 1,
): BakedMaterialMaps {
  const source = part.material?.texture_program;
  if (!source) throw new Error(`part "${part.name}" has no procedural texture program`);
  if (!Number.isInteger(size) || size < 16 || size > 2048) throw new Error('texture size must be an integer from 16 to 2048');
  if (!Number.isFinite(heightStrength) || heightStrength < 0 || heightStrength > 32) throw new Error('height strength must be from 0 to 32');
  const program = parseTextureProgram(source, library);
  // A program that reads the surface needs every surface point to have its own texel, so tiled UVs (which repeat) are
  // not used: such a part gets authored UVs or the unique box-chart atlas.
  const usesSurface = program.nodes.some((node) => SURFACE_NODE_NAMES.includes(node.type));
  const tile = usesSurface ? 0 : part.material?.texture_tile ?? 0;
  const uvs = part.uvs?.length ? part.uvs : tile > 0 ? worldTileUvs(part.positions, part.indices, tile) : boxChartUvs(part.positions, part.indices);
  if (uvs.length !== part.indices.length * 2) throw new Error('mesh UVs must contain two floats per triangle corner');
  const positions = cornerPositions(part.positions, part.indices);
  const maps: BakedMaterialMaps = { uvs, size };
  let positionMap: PositionMap | undefined;
  let surfaceMap: PositionMap | undefined;
  let roughness: Uint8Array | undefined;
  let metallic: Uint8Array | undefined;
  let height: Uint8Array | undefined;
  try {
    for (const channel of Object.keys(program.channels) as Channel[]) {
      const output = program.channels[channel];
      if (!output) continue;
      const graph = { nodes: program.nodes, output };
      const compiled = compileTexture(library, graph, helpers);
      if ((compiled.needs.position || compiled.needs.surface) && !positionMap) positionMap = baker.positionMap(uvs, positions, size);
      // The signal map is made from three more maps of the mesh drawn in UV space: smooth normals and occlusion, with the positions.
      if (compiled.needs.surface && !surfaceMap && positionMap) {
        const inputs = surfaceInputs(part.positions, part.indices, part.occlusion);
        const normalMap = baker.positionMap(uvs, inputs.normals, size);
        const occlusionMap = baker.positionMap(uvs, inputs.occlusion, size);
        surfaceMap = baker.surfaceMap(positionMap, normalMap, occlusionMap, size);
        baker.disposePositionMap(normalMap);
        baker.disposePositionMap(occlusionMap);
      }
      const pixels = baker.bake(compiled, size, positionMap, surfaceMap);
      if (channel === 'albedo') maps.albedo = pixels;
      else if (channel === 'roughness') roughness = pixels;
      else if (channel === 'metallic') metallic = pixels;
      else if (channel === 'height') height = pixels;
      else if (channel === 'emissive') maps.emissive = pixels;
    }
  } finally {
    if (positionMap) baker.disposePositionMap(positionMap);
    if (surfaceMap) baker.disposePositionMap(surfaceMap);
  }
  if (roughness || metallic) {
    const packed = new Uint8Array(size * size * 4);
    for (let i = 0; i < packed.length; i += 4) {
      packed[i] = 255;
      packed[i + 1] = roughness?.[i] ?? 255;
      packed[i + 2] = metallic?.[i] ?? 0;
      packed[i + 3] = 255;
    }
    maps.metallicRoughness = packed;
  }
  if (height) maps.normal = normalFromHeight(height, size, heightStrength);
  // A style (`solid.style`) finishes the colour map: painted over, pixelated or inked.
  const finish = part.material?.texture_style;
  if (maps.albedo && (finish === 'painterly' || finish === 'retro' || finish === 'ink')) maps.albedo = baker.finish(maps.albedo, size, finish);
  return maps;
}
