import type { Runtime } from './bridge';
import { bakeMaterialMaps, type BakedMaterialMaps } from './texture/apply';
import { Baker } from './texture/bake';
import type { Compiled } from './texture/compile';
import { LIVE_SIZE, LiveTextures, animatedAlbedo } from './texture/live';
import { DEFAULT_MAP_SIZE, fitToBudget, snapMapSize, type MapName } from './texture/budget';
import { loadBrowserTextureLibrary } from './texture/browser-library';
import { preloadOutfitImages } from './texture/outfit-images';
import { SURFACE_NODE_NAMES } from './texture/surface-nodes';
import { cornerNormals } from './normals';
import { worldTileUvs } from './texture/uv';
import type { EvaluationWarning, MeshPart, SceneEnvironment, SceneLight, Token } from './worker-types';

const TRIANGLES_PER_CHUNK = 512;
/**
 * Side of the baked maps shown for a part: the size its material asks for (`solid.texture` size), else 1024, snapped to one of
 * the renderer's three map sizes (1024, 512, 256). A box part's six faces share one map in a 3 x 2 grid, so each face gets a
 * sixth of it (170 x 256 pixels at 512, blurry on big walls; 340 x 512 at 1024), which is why large is the default. The page
 * may override it for every part (`__veMapSize`, used by tests); a slow software GPU may not go below 1024 either, only bake slower.
 */
function viewportMapSize(part?: MeshPart): number {
  // A live texture is drawn and sent every frame, so its map is small whatever was asked for.
  if (part && animatedParts.has(part)) return LIVE_SIZE;
  const forced = Number((globalThis as { __veMapSize?: number }).__veMapSize);
  if (Number.isInteger(forced) && forced >= 64 && forced <= 2048) return snapMapSize(forced);
  const asked = part?.material?.texture_size;
  return snapMapSize(asked && asked > 0 ? asked : DEFAULT_MAP_SIZE);
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

// Parts whose texture reads the clock, with their compiled albedo shaders: drawn live every frame once the scene is shown (texture/live.ts).
const animatedParts = new WeakMap<MeshPart, Compiled>();
let liveTextures: LiveTextures | undefined;
/** The live textures of the scene now shown (undefined when it has none); for tests and the console. */
export function liveTextureStats(): { count: number; accepted: number; refused: number } {
  return { count: liveTextures?.count ?? 0, accepted: liveTextures?.accepted ?? 0, refused: liveTextures?.refused ?? 0 };
}

/** Replaces the live textures with those of `parts` (the scene that was just committed). */
function startLiveTextures(runtime: Runtime, parts: MeshPart[]): void {
  liveTextures?.stop();
  liveTextures = undefined;
  const live = parts.filter(part => animatedParts.has(part));
  if (live.length === 0) return;
  try {
    const textures = new LiveTextures(runtime);
    for (const part of live) textures.add(part.node, part.region ?? 0, animatedParts.get(part) as Compiled, LIVE_SIZE);
    textures.start();
    liveTextures = textures;
  } catch (error) {
    console.warn('live textures could not start; the surfaces show their still maps:', error);
  }
}

// Baked maps by program and mesh, so an edit that leaves a textured part alone does not bake it again.
const bakedCache = new Map<string, BakedMaterialMaps>();
const BAKED_CACHE_LIMIT = 8;

function bakeKey(part: MeshPart): string {
  let checksum = 0;
  const step = Math.max(1, Math.floor(part.positions.length / 512));
  for (let i = 0; i < part.positions.length; i += step) checksum = (checksum * 31 + Math.round(part.positions[i] * 1e4)) | 0;
  return `${part.material?.texture_program}|${part.material?.texture_style ?? ''}|${part.material?.texture_tile ?? 0}|${viewportMapSize(part)}|${part.indices.length}|${part.uvs?.length ?? 0}|${checksum}`;
}

/** Bakes the maps of every textured part for the viewport. A part that cannot be baked is shown without them. */
async function bakeForViewport(parts: MeshPart[], notes: EvaluationWarning[] = []): Promise<Map<MeshPart, BakedMaterialMaps>> {
  const baked = new Map<MeshPart, BakedMaterialMaps>();
  const wanted = parts.filter(part => part.material?.texture_program);
  if (wanted.length === 0) return baked;
  let baker: Baker | undefined;
  const inScene = new Map<string, BakedMaterialMaps>();
  const failed = new Set<string>();
  try {
    const { library, helpers } = await loadBrowserTextureLibrary();
    // Imported clothes and hair name pictures instead of nodes (`image(albedo="...")`): fetch and decode them first, so baking stays synchronous.
    try {
      await preloadOutfitImages(wanted.map(part => part.material?.texture_program ?? undefined));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.warn('outfit pictures could not be loaded:', error);
      notes.push({ code: 'texture.bake_failed', node: wanted[0]?.node ?? '', message: `a texture picture could not be loaded: ${message}` });
    }
    // Programs that read the clock are drawn live (see texture/live.ts); their maps are small, so say so before the sizes are chosen.
    for (const part of wanted) {
      const compiled = animatedAlbedo(library, helpers, part.material?.texture_program ?? '');
      if (compiled) animatedParts.set(part, compiled);
    }
    for (const part of wanted) {
     // One part's bad program must not take the others' textures with it, and it must be reported: a model never sees the console.
     try {
      // A tiled part's pixels depend only on its program, tile and size, so every part with the same ones shares them
      // (and a renderer layer); only the UVs are its own.
      const tile = part.material?.texture_tile ?? 0;
      // A program that reads the surface (edges, creases, height) depends on the part's own mesh, so it is never shared.
      const readsSurface = SURFACE_NODE_NAMES.some(name => part.material?.texture_program?.includes(name));
      const shareable = tile > 0 && !part.uvs?.length && !readsSurface;
      const size = viewportMapSize(part);
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
     } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.warn('texture bake for the viewport failed; showing the surface without maps:', error);
      if (!failed.has(`${part.node}|${message}`)) {
        failed.add(`${part.node}|${message}`);
        notes.push({ code: 'texture.bake_failed', node: part.node, message: `the texture program could not be baked, so the surface is drawn without its maps: ${message}` });
      }
     }
    }
  } catch (error) {
    console.warn('texture library could not be loaded; showing surfaces without maps:', error);
    notes.push({ code: 'texture.bake_failed', node: wanted[0]?.node ?? '', message: `the texture library could not be loaded: ${error instanceof Error ? error.message : String(error)}` });
  } finally {
    baker?.dispose();
  }
  return baked;
}

/** Lets the page draw between pieces of work, but only after about 8 ms of work since the last frame: a chunk per frame made a scene of
 *  hundreds of chunks last as long as hundreds of frames, which on a machine drawing a heavy scene at a few frames a second passed a minute. */
function pacer(runtime: Runtime): () => Promise<void> {
  let since = performance.now();
  return async () => {
    // The renderer turns queued chunks into meshes a few milliseconds per frame, so stop sending while a lot is waiting.
    const queued = (JSON.parse(runtime.eval_projection_status()) as { queued_bytes?: number }).queued_bytes ?? 0;
    if (queued < 3 * 1024 * 1024 && performance.now() - since < 8) return;
    await frame();
    since = performance.now();
  };
}

let lastRefused = '';
/** The renderer call that last said no, so a rejected scene can say where (empty when the scene was only replaced while it uploaded). */
export function lastRefusal(): string { return lastRefused; }

/** The runtime with each `eval_projection_*` call watched: the one that answers false is remembered by name. */
function traced(runtime: Runtime): Runtime {
  return new Proxy(runtime, {
    get(target, key) {
      const value = Reflect.get(target, key);
      if (typeof value !== 'function' || typeof key !== 'string' || !key.startsWith('eval_projection_')) return value;
      return (...args: unknown[]) => {
        const result = value.apply(target, args);
        if (result === false) lastRefused = key;
        return result;
      };
    },
  });
}

// Bounded render packing. One small chunk per frame keeps copies/asset creation below the byte cap.
export async function uploadResult(plainRuntime: Runtime, token: Token, parts: MeshPart[], lights: SceneLight[], environment: SceneEnvironment | null, current: () => boolean, notes: EvaluationWarning[] = [], draft = false) {
  lastRefused = '';
  const runtime = traced(plainRuntime);
  const pace = pacer(plainRuntime);
  const text = JSON.stringify(token);
  if (!current() || !runtime.eval_projection_begin(text)) return false;
  if (!runtime.eval_projection_lights(text, JSON.stringify(lights))) return false;
  if (environment && !runtime.eval_projection_environment(text, JSON.stringify(environment))) return false;
  // A draft is shown quickly and replaced soon, so it skips the texture baking the finished scene does.
  const baked = draft ? new Map<MeshPart, Awaited<ReturnType<typeof bakeForViewport>> extends Map<MeshPart, infer V> ? V : never>() : await bakeForViewport(parts, notes);
  // What each part sends to the renderer: its baked maps, or a normal map baked by bake.maps resampled to the part's size.
  type Sent = Partial<Record<MapName, Uint8Array>>;
  const sending = new Map<MeshPart, Sent>();
  for (const part of parts) {
    const maps = baked.get(part);
    const normal = maps?.normal ?? (part.baked?.normal && (part.region ?? 0) === 0 ? resampleRgba(part.baked.normal, part.baked.size, viewportMapSize(part)) : undefined);
    if (maps || normal) sending.set(part, { albedo: maps?.albedo, normal, metallicRoughness: maps?.metallicRoughness, emissive: maps?.emissive });
  }
  // The viewport's texture arrays share one byte budget: the largest maps shrink first (last surfaces first), and only when
  // everything is already the smallest size do the last surfaces lose their least useful maps.
  const entries = [...sending.entries()];
  const fitted = fitToBudget(entries.map(([, maps]) => maps));
  entries.forEach(([part], i) => {
    sending.set(part, fitted.kept[i]);
    if (fitted.dropped[i].length > 0) {
      notes.push({
        code: 'texture.viewport_budget', node: part.node,
        message: `The viewport's texture memory is full; "${part.name}" is shown without its ${fitted.dropped[i].join(', ')} map(s). The exported .glb still has them. Use fewer textured parts, a smaller solid.texture size, or drop channels you do not need.`,
      });
    } else if (fitted.shrunk[i].length > 0) {
      const list = fitted.shrunk[i].map(({ name, from, to }) => `${name} ${from} to ${to}`).join(', ');
      notes.push({
        code: 'texture.viewport_shrunk', node: part.node,
        message: `The viewport's texture memory is nearly full; "${part.name}" is shown with smaller maps (${list}). The exported .glb keeps full size. Give less important parts a smaller solid.texture size.`,
      });
    }
  });
  for (const [partIndex,part] of parts.entries()) {
    const group=part.transforms?.length ? partIndex+1 : 0;
    if (group && part.transforms && part.instance_indices) {
      for (let offset=0; offset<part.instance_indices.length; offset+=512) {
        if (!current() || !runtime.eval_projection_instances(text,group,part.node,
          part.transforms.subarray(offset*16,(offset+512)*16),part.instance_indices.subarray(offset,offset+512),
          Boolean(part.visible_range),part.visible_range?.[0] ?? 0,part.visible_range?.[1] ?? 0,part.visible_fade ?? 0)) return false;
        await pace();
      }
    }
    const region = part.region ?? 0;
    if (part.material && !runtime.eval_projection_material_region(text, part.node, region, JSON.stringify(part.material))) {
      console.error(`the renderer refused the material of ${part.name}`, JSON.stringify(part.material));
      return false;
    }
    if (part.animation?.length && !runtime.eval_projection_animation(text, part.node, JSON.stringify(part.animation))) return false;
    if (part.parent && !group && !runtime.eval_projection_parent(text, part.node, part.parent)) return false;
    if (part.ik && !group && !runtime.eval_projection_ik(text, part.node, JSON.stringify(part.ik))) return false;
    if (part.ragdoll && !group && !runtime.eval_projection_ragdoll(text, part.node, JSON.stringify(part.ragdoll))) {
      console.error(`the runtime refused the ragdoll of ${part.name}`, JSON.stringify(part.ragdoll));
      return false;
    }
    if (part.fluid && !group && !runtime.eval_projection_fluid(text, part.node, JSON.stringify(part.fluid))) {
      console.error(`the runtime refused the liquid of ${part.name}`, JSON.stringify(part.fluid));
      return false;
    }
    if (part.sim && !group && !runtime.eval_projection_sim(text, part.node, JSON.stringify(part.sim), part.positions, part.indices)) {
      console.error(`the runtime refused the simulation of ${part.name}`, JSON.stringify(part.sim));
      return false;
    }
    if (part.cloth && !group && !runtime.eval_projection_cloth(text, part.node, JSON.stringify(part.cloth), part.positions, part.indices)) {
      console.error(`the runtime refused the cloth of ${part.name}`, JSON.stringify(part.cloth));
      return false;
    }
    if (part.fx && !group && !runtime.eval_projection_fx(text, part.node, JSON.stringify(part.fx))) {
      console.error(`the renderer refused the particle effect of ${part.name}`, JSON.stringify(part.fx));
      return false;
    }
    if (part.skin && part.skin_joints?.length && part.skin_weights?.length && !group && !part.region
      && !runtime.eval_projection_skin(text, part.node, JSON.stringify(part.skin.bones), part.indices, part.skin_joints, part.skin_weights, part.skin.blend === 'dual_quaternion')) return false;
    if (part.behavior && !group && !runtime.eval_projection_behavior(text, part.node, JSON.stringify(part.behavior))) return false;
    if (part.state_machine && !group && !runtime.eval_projection_machine(text, part.node, JSON.stringify(part.state_machine))) return false;
    if (part.body && !group) {
      // Only shapes built from the part's own vertices need them sent; the others fit the bounding box.
      const needs = ['auto', 'convex', 'mesh'].includes(part.body.shape);
      if (!runtime.eval_projection_body(text, part.node, JSON.stringify(part.body), needs ? part.positions : new Float32Array(0), needs ? part.indices : new Uint32Array(0))) return false;
    }
    const maps = baked.get(part);
    const sent = sending.get(part);
    if (maps || part.baked?.normal || part.baked?.ao) {
      if (!runtime.eval_projection_maps(text, part.node, region, sent?.albedo ?? NO_MAP, sent?.normal ?? NO_MAP, sent?.metallicRoughness ?? NO_MAP, sent?.emissive ?? NO_MAP)) return false;
    }
    const explicit = part.normals && part.normals.length === part.positions.length ? cornersFromVertices(part.indices, part.normals) : null;
    const normals = explicit ?? (part.indices.length / 3 <= SMOOTH_TRIANGLE_LIMIT ? cornerNormals(part.positions, part.indices) : null);
    // A baked normal map is sampled with the part's own UVs (one pair per triangle corner), so they travel with its vertices.
    const ownUvs = part.baked?.normal && !maps?.normal && region === 0 && part.uvs?.length === part.indices.length * 2 ? part.uvs : undefined;
    const uvs = maps?.uvs ?? ownUvs;
    // Per-vertex colours (water: flow, depth, foam) travel only when there is one per vertex.
    const colors = part.colors && part.colors.length === (part.positions.length / 3) * 4 ? part.colors : undefined;
    // The runtime accepts at most 64 KiB per chunk, so a vertex with more floats (texture coordinates, a colour) means fewer triangles per chunk.
    const floatsPerVertex = (uvs ? 9 : 7) + (colors ? 4 : 0);
    const trianglesPerChunk = Math.min(TRIANGLES_PER_CHUNK, Math.floor(65536 / (floatsPerVertex * 3 * 4)));
    for (let offset = 0; offset < part.indices.length; offset += trianglesPerChunk * 3) {
      if (!current()) return false;
      const end = Math.min(offset + trianglesPerChunk * 3, part.indices.length);
      const data = pack(part, offset, end, normals, uvs, colors);
      const layout = (uvs ? 1 : 0) | (colors ? 2 : 0);
      if (!runtime.eval_projection_chunk_part(text, group, region, part.node, part.name, data, layout)) {
        const worst = data.reduce((m, v) => Math.max(m, Number.isFinite(v) ? Math.abs(v) : Infinity), 0);
        console.error(`the renderer refused a chunk of ${part.name}: layout ${layout}, ${data.length} floats, largest value ${worst}, colours ${colors?.length}, positions ${part.positions.length}`);
        return false;
      }
      await pace();
    }
  }
  // A draft (the parts finished so far) replaces the scene on screen but never commits the document: it is done when the renderer has taken it.
  if (draft) {
    if (!current() || !runtime.eval_projection_finish_draft(text)) return false;
    await frame();
    while (current()) {
      const projection = JSON.parse(runtime.eval_projection_status()) as { staging_meshes: number; queued_bytes: number };
      if (projection.staging_meshes === 0 && projection.queued_bytes === 0) return true;
      await frame();
    }
    return false;
  }
  if (!current() || !runtime.eval_projection_finish(text)) return false;
  while (current()) {
    const projection = JSON.parse(runtime.eval_projection_status()) as { committed: Token | null };
    if (projection.committed?.request === token.request && projection.committed.generation === token.generation) {
      startLiveTextures(plainRuntime, parts);
      return true;
    }
    await frame();
  }
  return false;
}

// Above this many triangles the packing stays flat-shaded: smoothing is computed on the window thread.
const SMOOTH_TRIANGLE_LIMIT = 500_000;

// The corner normals live in `normals.ts` (the texture baker needs them too); other modules still import them from here.
export { cornerNormals };

/** The per-corner normals of a mesh whose vertices carry their own: corner `i` takes the normal of vertex `indices[i]`. */
function cornersFromVertices(indices: Uint32Array, vertex: Float32Array): Float32Array {
  const out = new Float32Array(indices.length * 3);
  for (let corner = 0; corner < indices.length; corner++) {
    const from = indices[corner] * 3;
    out[corner * 3] = vertex[from];
    out[corner * 3 + 1] = vertex[from + 1];
    out[corner * 3 + 2] = vertex[from + 2];
  }
  return out;
}

/** Seven floats per vertex: position, normal, baked occlusion; two more (texture coordinates) for parts with baked maps; four more (a vertex colour) for parts that carry one. */
function pack(part: MeshPart, start: number, end: number, normals: Float32Array | null, uvs?: Float32Array, colors?: Float32Array): Float32Array {
  const out = new Float32Array((end - start) * ((uvs ? 9 : 7) + (colors ? 4 : 0)));
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
      if (colors) { const at = (index / 3) * 4; out[write++] = colors[at]; out[write++] = colors[at + 1]; out[write++] = colors[at + 2]; out[write++] = colors[at + 3]; }
    });
  }
  return out;
}
