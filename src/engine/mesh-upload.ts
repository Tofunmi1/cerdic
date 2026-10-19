import type { Runtime } from './bridge';
import { bakeMaterialMaps, type BakedMaterialMaps } from './texture/apply';
import { Baker } from './texture/bake';
import { fitToLayerBudget, VIEWPORT_LAYER_LIMIT } from './texture/budget';
import { loadBrowserTextureLibrary } from './texture/browser-library';
import { SURFACE_NODE_NAMES } from './texture/surface-nodes';
import { cornerNormals } from './normals';
import { worldTileUvs } from './texture/uv';
import type { EvaluationWarning, MeshPart, SceneEnvironment, SceneLight, Token } from './worker-types';

const TRIANGLES_PER_CHUNK = 512;
/** Side of the baked maps shown in the viewport: the renderer's layer size. */
// A box part's six faces share one map in a 3 x 2 grid, so each face gets a sixth of it (170 x 256 pixels at 512, blurry on big walls;
// 340 x 512 at 1024). The renderer's texture array has fixed layers of `ve_render::LAYER_SIZE` (1024), 32 layers at most, so this
// must match it; a slow software GPU (the test browser) may not go below it either, only bake slower.
const DEFAULT_MAP_SIZE = 1024;
function viewportMapSize(): number {
  const wanted = Number((globalThis as { __veMapSize?: number }).__veMapSize);
  return Number.isInteger(wanted) && wanted >= 64 && wanted <= 2048 ? wanted : DEFAULT_MAP_SIZE;
}
const NO_MAP = new Uint8Array(0);
/** Resamples an RGBA square with pixel-centred bilinear filtering. */
export function resampleRgba(source: Uint8Array, size: number, targetSize: number): Uint8Array {
  if (size === targetSize) return source;
  const out = new Uint8Array(targetSize * targetSize * 4);
  for (let y = 0; y < targetSize; y++) for (let x = 0; x < targetSize; x++) {
    const sx = (x / (targetSize - 1)) * (size - 1), sy = (y / (targetSize - 1)) * (size - 1);
    const x0 = Math.floor(sx), y0 = Math.floor(sy), x1 = Math.min(x0 + 1, size - 1), y1 = Math.min(y0 + 1, size - 1);
    const fx = sx - x0, fy = sy - y0, at = (y * targetSize + x) * 4;
    for (let c = 0; c < 4; c++) {
      const a = source[(y0 * size + x0) * 4 + c] * (1 - fx) + source[(y0 * size + x1) * 4 + c] * fx;
      const b = source[(y1 * size + x0) * 4 + c] * (1 - fx) + source[(y1 * size + x1) * 4 + c] * fx;
      out[at + c] = Math.round(a * (1 - fy) + b * fy);
    }
  }
  return out;
}
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => resolve()));

// Baked maps by program and mesh, so an edit that leaves a textured part alone does not bake it again.
const bakedCache = new Map<string, BakedMaterialMaps>();
const BAKED_CACHE_LIMIT = 8;

function bakeKey(part: MeshPart): string {
  let checksum = 0;
  const step = Math.max(1, Math.floor(part.positions.length / 512));
  for (let i = 0; i < part.positions.length; i += step) checksum = (checksum * 31 + Math.round(part.positions[i] * 1e4)) | 0;
  return `${part.material?.texture_program}|${part.material?.texture_style ?? ''}|${part.material?.texture_tile ?? 0}|${part.indices.length}|${part.uvs?.length ?? 0}|${checksum}`;
}

/** Bakes the maps of every textured part for the viewport. A part that cannot be baked is shown without them. */
async function bakeForViewport(parts: MeshPart[]): Promise<Map<MeshPart, BakedMaterialMaps>> {
  const baked = new Map<MeshPart, BakedMaterialMaps>();
  const wanted = parts.filter(part => part.material?.texture_program);
  if (wanted.length === 0) return baked;
  let baker: Baker | undefined;
  const inScene = new Map<string, BakedMaterialMaps>();
  try {
    const { library, helpers } = await loadBrowserTextureLibrary();
    for (const part of wanted) {
      // A tiled part's pixels depend only on its program, tile and size, so every part with the same ones shares them
      // (and a renderer layer); only the UVs are its own.
      const tile = part.material?.texture_tile ?? 0;
      // A program that reads the surface (edges, creases, height) depends on the part's own mesh, so it is never shared.
      const readsSurface = SURFACE_NODE_NAMES.some(name => part.material?.texture_program?.includes(name));
      const shareable = tile > 0 && !part.uvs?.length && !readsSurface;
      const size = viewportMapSize();
      const key = shareable ? `tiled|${part.material?.texture_program}|${part.material?.texture_style ?? ''}|${tile}|${size}` : bakeKey(part);
      let maps = inScene.get(key) ?? bakedCache.get(key);
      if (!maps) {
        baker ??= new Baker(new OffscreenCanvas(1, 1));
        maps = bakeMaterialMaps(library, helpers, part, baker, size);
        if (bakedCache.size >= BAKED_CACHE_LIMIT) bakedCache.delete(bakedCache.keys().next().value as string);
        bakedCache.set(key, maps);
      }
      inScene.set(key, maps);
      baked.set(part, shareable ? { ...maps, uvs: worldTileUvs(part.positions, part.indices, tile) } : maps);
    }
  } catch (error) {
    console.warn('texture bake for the viewport failed; showing the surface without maps:', error);
  } finally {
    baker?.dispose();
  }
  return baked;
}

// Bounded render packing. One small chunk per frame keeps copies/asset creation below the byte cap.
export async function uploadResult(runtime: Runtime, token: Token, parts: MeshPart[], lights: SceneLight[], environment: SceneEnvironment | null, current: () => boolean, notes: EvaluationWarning[] = []) {
  const text = JSON.stringify(token);
  if (!current() || !runtime.eval_projection_begin(text)) return false;
  if (!runtime.eval_projection_lights(text, JSON.stringify(lights))) return false;
  if (environment && !runtime.eval_projection_environment(text, JSON.stringify(environment))) return false;
  const baked = await bakeForViewport(parts);
  // The viewport's texture array is shared and bounded: later surfaces lose their least useful maps first.
  const entries = [...baked.entries()];
  const fitted = fitToLayerBudget(entries.map(([, maps]) => maps));
  entries.forEach(([part], i) => {
    baked.set(part, fitted.kept[i]);
    if (fitted.dropped[i].length > 0) {
      notes.push({
        code: 'texture.viewport_budget', node: part.node,
        message: `The viewport holds ${VIEWPORT_LAYER_LIMIT} texture maps in all; "${part.name}" is shown without its ${fitted.dropped[i].join(', ')} map(s). The exported .glb still has them. Use fewer textured parts or drop channels you do not need.`,
      });
    }
  });
  for (const [partIndex,part] of parts.entries()) {
    const group=part.transforms?.length ? partIndex+1 : 0;
    if (group && part.transforms && part.instance_indices) {
      for (let offset=0; offset<part.instance_indices.length; offset+=512) {
        if (!current() || !runtime.eval_projection_instances(text,group,part.node,
          part.transforms.subarray(offset*16,(offset+512)*16),part.instance_indices.subarray(offset,offset+512))) return false;
        await frame();
      }
    }
    const region = part.region ?? 0;
    if (part.material && !runtime.eval_projection_material_region(text, part.node, region, JSON.stringify(part.material))) return false;
    if (part.animation?.length && !runtime.eval_projection_animation(text, part.node, JSON.stringify(part.animation))) return false;
    if (part.parent && !group && !runtime.eval_projection_parent(text, part.node, part.parent)) return false;
    if (part.ik && !group && !runtime.eval_projection_ik(text, part.node, JSON.stringify(part.ik))) return false;
    if (part.skin && part.skin_joints?.length && part.skin_weights?.length && !group && !part.region
      && !runtime.eval_projection_skin(text, part.node, JSON.stringify(part.skin.bones), part.indices, part.skin_joints, part.skin_weights, part.skin.blend === 'dual_quaternion')) return false;
    if (part.behavior && !group && !runtime.eval_projection_behavior(text, part.node, JSON.stringify(part.behavior))) return false;
    if (part.body && !group) {
      // Only shapes built from the part's own vertices need them sent; the others fit the bounding box.
      const needs = ['auto', 'convex', 'mesh'].includes(part.body.shape);
      if (!runtime.eval_projection_body(text, part.node, JSON.stringify(part.body), needs ? part.positions : new Float32Array(0), needs ? part.indices : new Uint32Array(0))) return false;
    }
    const maps = baked.get(part);
    if (maps || part.baked?.normal || part.baked?.ao) {
      const normal = maps?.normal ?? (part.baked?.normal && region === 0 ? resampleRgba(part.baked.normal, part.baked.size, viewportMapSize()) : NO_MAP);
      if (!runtime.eval_projection_maps(text, part.node, region, maps?.albedo ?? NO_MAP, normal, maps?.metallicRoughness ?? NO_MAP, maps?.emissive ?? NO_MAP)) return false;
    }
    const normals = part.indices.length / 3 <= SMOOTH_TRIANGLE_LIMIT ? cornerNormals(part.positions, part.indices) : null;
    // A baked normal map is sampled with the part's own UVs (one pair per triangle corner), so they travel with its vertices.
    const ownUvs = part.baked?.normal && !maps?.normal && region === 0 && part.uvs?.length === part.indices.length * 2 ? part.uvs : undefined;
    const uvs = maps?.uvs ?? ownUvs;
    for (let offset = 0; offset < part.indices.length; offset += TRIANGLES_PER_CHUNK * 3) {
      if (!current()) return false;
      const end = Math.min(offset + TRIANGLES_PER_CHUNK * 3, part.indices.length);
      const data = pack(part, offset, end, normals, uvs);
      if (!runtime.eval_projection_chunk_part(text, group, region, part.node, part.name, data, !!uvs)) return false;
      await frame();
    }
  }
  if (!current() || !runtime.eval_projection_finish(text)) return false;
  while (current()) {
    const projection = JSON.parse(runtime.eval_projection_status()) as { committed: Token | null };
    if (projection.committed?.request === token.request && projection.committed.generation === token.generation) return true;
    await frame();
  }
  return false;
}

// Above this many triangles the packing stays flat-shaded: smoothing is computed on the window thread.
const SMOOTH_TRIANGLE_LIMIT = 500_000;

// The corner normals live in `normals.ts` (the texture baker needs them too); other modules still import them from here.
export { cornerNormals };

/** Seven floats per vertex: position, normal, baked occlusion; two more (texture coordinates) for parts with baked maps. */
function pack(part: MeshPart, start: number, end: number, normals: Float32Array | null, uvs?: Float32Array): Float32Array {
  const out = new Float32Array((end - start) * (uvs ? 9 : 7));
  let write = 0;
  const p = part.positions;
  for (let face = start; face < end; face += 3) {
    const a = part.indices[face] * 3, b = part.indices[face + 1] * 3, c = part.indices[face + 2] * 3;
    let flat: number[] = [];
    if (!normals) {
      const ab = [p[b] - p[a], p[b+1] - p[a+1], p[b+2] - p[a+2]];
      const ac = [p[c] - p[a], p[c+1] - p[a+1], p[c+2] - p[a+2]];
      const n = [ab[1]*ac[2] - ab[2]*ac[1], ab[2]*ac[0] - ab[0]*ac[2], ab[0]*ac[1] - ab[1]*ac[0]];
      const length = Math.hypot(...n) || 1;
      flat = [n[0]/length, n[1]/length, n[2]/length];
    }
    [a, b, c].forEach((index, k) => {
      out[write++] = p[index]; out[write++] = p[index+1]; out[write++] = p[index+2];
      if (normals) { const at = (face + k) * 3; out[write++] = normals[at]; out[write++] = normals[at + 1]; out[write++] = normals[at + 2]; }
      else { out[write++] = flat[0]; out[write++] = flat[1]; out[write++] = flat[2]; }
      out[write++] = part.occlusion[index / 3] ?? 1;
      if (uvs) { out[write++] = uvs[(face + k) * 2]; out[write++] = uvs[(face + k) * 2 + 1]; }
    });
  }
  return out;
}
