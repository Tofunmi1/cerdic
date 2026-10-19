// glTF binary (.glb) export of an evaluated model: the way a model leaves the engine for a game engine,
// a DCC tool or a web viewer. Everything is generated from the document; nothing is imported.
//
// Units are metres and the axes are Y up, matching glTF. Normals use the same crease rule as the
// viewer (faces meeting at less than 40 degrees share a smooth normal), so the export shades like the
// screen. A part's colour, metal and roughness become a PBR metallic-roughness material; procedural
// surface recipes are shader-side in the viewer and are not exported. Instanced parts become one node
// per instance sharing one mesh.
import type { AnimationTrack, MeshPart, PartMaterial } from './worker-types';
import { cornerNormals } from './mesh-upload';
import { boxChartUvs } from './texture/uv';
import { sampleClip, sampleLook, type SampledClip } from './animation';
import { Baker, encodePng } from './texture/bake';
import { bakeMaterialMaps } from './texture/apply';
import { loadBrowserTextureLibrary } from './texture/browser-library';

export type GlbOptions = {
  /** Faces meeting at less than this angle share a smooth normal (degrees). Default 40, as the viewer. */
  creaseDegrees?: number;
  /** The scene name. */
  name?: string;
  /** Encoded PNG maps, keyed by source part index. Used by `buildGlbWithTextures`. */
  textureImages?: ReadonlyMap<number, GlbTexturePngs>;
  /** MikkTSpace generator returning one tangent per triangle corner. */
  mikkTangents?: (positions: Float32Array, normals: Float32Array, uvs: Float32Array, indices: Uint32Array) => Float32Array;
};

export type GlbTexturePngs = { albedo?: Uint8Array; metallicRoughness?: Uint8Array; normal?: Uint8Array; occlusion?: Uint8Array; emissive?: Uint8Array };

type Json = Record<string, unknown>;

const srgbToLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const FLOAT = 5126, UNSIGNED_SHORT = 5123, UNSIGNED_INT = 5125, ARRAY_BUFFER = 34962, ELEMENT_ARRAY_BUFFER = 34963;
/** Most copies of a placed set whose motion is written into the file (each one costs its own channels). */
const MAX_ANIMATED_COPIES = 256;
const GLB_MAGIC =0x46546c67, JSON_CHUNK = 0x4e4f534a, BIN_CHUNK = 0x004e4942;

function materialJson(material: PartMaterial | null | undefined, maps: Partial<Record<keyof GlbTexturePngs, { index: number }>> = {}): Json {
  const color = material?.color ?? [0.68, 0.73, 0.87];
  const json: Json = {
    pbrMetallicRoughness: {
      baseColorFactor: [...color.map(srgbToLinear), material?.opacity ?? 1],
      metallicFactor: material?.metallic ?? 0,
      roughnessFactor: material?.roughness ?? 0.5,
    },
  };
  if ((material?.opacity ?? 1) < 1) json.alphaMode = 'BLEND';
  if (maps.albedo || maps.metallicRoughness) {
    const pbr = json.pbrMetallicRoughness as Json;
    if (maps.albedo) pbr.baseColorTexture = maps.albedo;
    if (maps.metallicRoughness) pbr.metallicRoughnessTexture = maps.metallicRoughness;
  }
  const emissive = material?.emissive ?? [0, 0, 0];
  const emissiveStrength = Math.max(1, ...emissive);
  if (emissive.some(v => v > 0)) {
    json.emissiveFactor = emissive.map(v => v / emissiveStrength);
    if (emissiveStrength > 1) json.extensions = { KHR_materials_emissive_strength: { emissiveStrength } };
  } else if (maps.emissive) json.emissiveFactor = [1, 1, 1];
  if (maps.normal) json.normalTexture = maps.normal;
  if (maps.occlusion) json.occlusionTexture = maps.occlusion;
  if (maps.emissive) json.emissiveTexture = maps.emissive;
  return json;
}

/** Welds triangle corners with identical position and normal into an indexed mesh. */
function weld(part: MeshPart, creaseDegrees: number, needTangents = false, mikkTangents?: GlbOptions['mikkTangents']) {
  const normals = cornerNormals(part.positions, part.indices, creaseDegrees);
  const vertexColors = (part as MeshPart & { vertexColors?: Float32Array }).vertexColors;
  if (vertexColors && vertexColors.length !== part.positions.length / 3 * 4) throw new Error('vertex colours must contain four floats per vertex');
  const cornerUvs = part.uvs?.length ? part.uvs : boxChartUvs(part.positions, part.indices);
  if (cornerUvs.length !== part.indices.length * 2) throw new Error('mesh UVs must contain two floats per triangle corner');
  if (cornerUvs.some(value => !Number.isFinite(value))) throw new Error('mesh UVs must be finite');
  const seen = new Map<string, number>();
  const positions: number[] = [], outNormals: number[] = [], outUvs: number[] = [], outColors: number[] = [], indices: number[] = [], sourceOf: number[] = [];
  const bits = new Uint32Array(1), view = new Float32Array(bits.buffer);
  const key = (values: number[]) => values.map(v => { view[0] = v; return bits[0].toString(16); }).join('.');
  for (let corner = 0; corner < part.indices.length; corner++) {
    const p = part.indices[corner] * 3;
    const source = part.indices[corner];
    const colors = vertexColors?.subarray(source * 4, source * 4 + 4);
    const values = [part.positions[p], part.positions[p + 1], part.positions[p + 2], normals[corner * 3], normals[corner * 3 + 1], normals[corner * 3 + 2], cornerUvs[corner * 2], cornerUvs[corner * 2 + 1], ...(colors ?? [])];
    const id = key(values);
    let index = seen.get(id);
    if (index === undefined) {
      index = positions.length / 3;
      seen.set(id, index);
      positions.push(values[0], values[1], values[2]);
      outNormals.push(values[3], values[4], values[5]);
      outUvs.push(values[6], values[7]);
      if (colors) outColors.push(...colors);
      sourceOf.push(source); // the part's own vertex, which skin weights belong to
    }
    indices.push(index);
  }
  const mesh = { positions: new Float32Array(positions), normals: new Float32Array(outNormals), uvs: new Float32Array(outUvs), ...(vertexColors ? { colors: new Float32Array(outColors) } : {}), indices: new Uint32Array(indices), sources: new Uint32Array(sourceOf) };
  if (!needTangents) return { ...mesh, tangents: undefined };
  // glTF's v runs down the image (row 0 is v = 0) but a normal map's +Y is up, and the tangent basis must
  // have its bitangent along +Y: generate it on (u, 1 - v), as exporters of v-up tools do.
  const upwardUvs = Float32Array.from(mesh.uvs, (value, i) => (i % 2 === 1 ? 1 - value : value));
  if (!mikkTangents) return { ...mesh, tangents: tangents(mesh.positions, mesh.normals, upwardUvs, mesh.indices) };
  const corners = mikkTangents(mesh.positions, mesh.normals, upwardUvs, mesh.indices);
  if (corners.length !== mesh.indices.length * 4 || corners.some(value => !Number.isFinite(value))) throw new Error('MikkTSpace returned invalid tangent data');
  const split = new Map<string, number>();
  const splitPositions: number[] = [], splitNormals: number[] = [], splitUvs: number[] = [], splitColors: number[] = [], outTangents: number[] = [], splitIndices: number[] = [], splitSources: number[] = [];
  for (let corner = 0; corner < mesh.indices.length; corner++) {
    const source = mesh.indices[corner], t = Array.from(corners.subarray(corner * 4, corner * 4 + 4));
    const key = `${source}:${t.join(',')}`;
    let target = split.get(key);
    if (target === undefined) {
      target = splitPositions.length / 3; split.set(key, target);
      splitPositions.push(...mesh.positions.subarray(source * 3, source * 3 + 3));
      splitNormals.push(...mesh.normals.subarray(source * 3, source * 3 + 3));
      splitUvs.push(...mesh.uvs.subarray(source * 2, source * 2 + 2)); outTangents.push(...t);
      if (mesh.colors) splitColors.push(...mesh.colors.subarray(source * 4, source * 4 + 4));
      splitSources.push(mesh.sources[source]);
    }
    splitIndices.push(target);
  }
  return { positions: new Float32Array(splitPositions), normals: new Float32Array(splitNormals), uvs: new Float32Array(splitUvs), ...(mesh.colors ? { colors: new Float32Array(splitColors) } : {}), indices: new Uint32Array(splitIndices), tangents: new Float32Array(outTangents), sources: new Uint32Array(splitSources) };
}

function tangents(positions: Float32Array, normals: Float32Array, uvs: Float32Array, indices: Uint32Array): Float32Array {
  const tan = new Float64Array(positions.length), bitan = new Float64Array(positions.length);
  for (let i = 0; i < indices.length; i += 3) {
    const ids = [indices[i], indices[i + 1], indices[i + 2]];
    const p = ids.map(index => index * 3), t = ids.map(index => index * 2);
    const e1 = [positions[p[1]] - positions[p[0]], positions[p[1] + 1] - positions[p[0] + 1], positions[p[1] + 2] - positions[p[0] + 2]];
    const e2 = [positions[p[2]] - positions[p[0]], positions[p[2] + 1] - positions[p[0] + 1], positions[p[2] + 2] - positions[p[0] + 2]];
    const du1 = uvs[t[1]] - uvs[t[0]], dv1 = uvs[t[1] + 1] - uvs[t[0] + 1];
    const du2 = uvs[t[2]] - uvs[t[0]], dv2 = uvs[t[2] + 1] - uvs[t[0] + 1];
    const determinant = du1 * dv2 - du2 * dv1;
    if (Math.abs(determinant) < 1e-12) continue;
    const reciprocal = 1 / determinant;
    const s = [(e1[0] * dv2 - e2[0] * dv1) * reciprocal, (e1[1] * dv2 - e2[1] * dv1) * reciprocal, (e1[2] * dv2 - e2[2] * dv1) * reciprocal];
    const b = [(e2[0] * du1 - e1[0] * du2) * reciprocal, (e2[1] * du1 - e1[1] * du2) * reciprocal, (e2[2] * du1 - e1[2] * du2) * reciprocal];
    for (const id of ids) for (let axis = 0; axis < 3; axis++) { tan[id * 3 + axis] += s[axis]; bitan[id * 3 + axis] += b[axis]; }
  }
  const out = new Float32Array((positions.length / 3) * 4);
  for (let i = 0; i < positions.length / 3; i++) {
    const at = i * 3, outAt = i * 4;
    const n = [normals[at], normals[at + 1], normals[at + 2]];
    let x = tan[at], y = tan[at + 1], z = tan[at + 2];
    const projection = x * n[0] + y * n[1] + z * n[2];
    x -= projection * n[0]; y -= projection * n[1]; z -= projection * n[2];
    let length = Math.hypot(x, y, z);
    if (length < 1e-12) {
      const axis = Math.abs(n[0]) < 0.8 ? [1, 0, 0] : [0, 1, 0];
      [x, y, z] = [n[1] * axis[2] - n[2] * axis[1], n[2] * axis[0] - n[0] * axis[2], n[0] * axis[1] - n[1] * axis[0]];
      length = Math.hypot(x, y, z) || 1;
    }
    x /= length; y /= length; z /= length;
    const cross = [n[1] * z - n[2] * y, n[2] * x - n[0] * z, n[0] * y - n[1] * x];
    const handed = cross[0] * bitan[at] + cross[1] * bitan[at + 1] + cross[2] * bitan[at + 2] < 0 ? -1 : 1;
    out.set([x, y, z, handed], outAt);
  }
  return out;
}

const pad4 = (n: number) => (n + 3) & ~3;

/** Builds a binary glTF 2.0 file for the parts. Parts with no triangles are skipped. */
export function buildGlb(parts: MeshPart[], options: GlbOptions = {}): Uint8Array {
  const crease = options.creaseDegrees ?? 40;
  const chunks: Uint8Array[] = [];
  let length = 0;
  const bufferViews: Json[] = [], accessors: Json[] = [], meshes: Json[] = [], nodes: Json[] = [], materials: Json[] = [], images: Json[] = [], textures: Json[] = [];
  const materialIds = new Map<string, number>();
  // Each entry belongs to one named clip ("" is the main clip); every clip becomes its own glTF animation.
  const animated: { node: number; clip: SampledClip; name: string }[] = [];
  const placed: { part: MeshPart; node: number }[] = [];
  const skinnedNodes: { node: number; bones: string[] }[] = [];
  const visibilityAnimated: { node: number; tracks: NonNullable<MeshPart['animation']>; name: string }[] = [];
  const lookAnimated: { material: number; tracks: NonNullable<MeshPart['animation']>; strength: number; glow: boolean; name: string }[] = [];
  const clipsIn = (tracks: AnimationTrack[]) => [...new Set(tracks.map(track => track.clip ?? ''))];
  const inClip = (tracks: AnimationTrack[], name: string) => tracks.filter(track => (track.clip ?? '') === name);
  // Parts that get a node: a skin may only name bones that exist in the file.
  const drawn = new Set(parts.filter(p => p.indices.length >= 3 && p.positions.length >= 9 && !p.transforms?.length).map(p => p.name || p.node));
  const push = (bytes: Uint8Array, target?: number) => {
    const offset = length;
    chunks.push(bytes);
    const padded = pad4(bytes.byteLength);
    if (padded > bytes.byteLength) chunks.push(new Uint8Array(padded - bytes.byteLength));
    length += padded;
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: bytes.byteLength, ...(target === undefined ? {} : { target }) });
    return bufferViews.length - 1;
  };
  const addTexture = (png?: Uint8Array) => {
    if (!png) return undefined;
    if (png.length < 8 || png[0] !== 137 || png[1] !== 80 || png[2] !== 78 || png[3] !== 71) throw new Error('texture image must be a PNG');
    const image = images.length;
    images.push({ bufferView: push(png), mimeType: 'image/png' });
    const index = textures.length;
    textures.push({ sampler: 0, source: image });
    return { index };
  };
  for (const [partIndex, part] of parts.entries()) {
    if (part.indices.length < 3 || part.positions.length < 9) continue;
    const pngs = options.textureImages?.get(partIndex);
    const mesh = weld(part, crease, !!pngs?.normal, options.mikkTangents);
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < mesh.positions.length; i += 3) {
      for (let k = 0; k < 3; k++) { min[k] = Math.min(min[k], mesh.positions[i + k]); max[k] = Math.max(max[k], mesh.positions[i + k]); }
    }
    const positionView = push(new Uint8Array(mesh.positions.buffer, mesh.positions.byteOffset, mesh.positions.byteLength), ARRAY_BUFFER);
    accessors.push({ bufferView: positionView, componentType: FLOAT, count: mesh.positions.length / 3, type: 'VEC3', min, max });
    const positionAccessor = accessors.length - 1;
    const normalView = push(new Uint8Array(mesh.normals.buffer, mesh.normals.byteOffset, mesh.normals.byteLength), ARRAY_BUFFER);
    accessors.push({ bufferView: normalView, componentType: FLOAT, count: mesh.normals.length / 3, type: 'VEC3' });
    const normalAccessor = accessors.length - 1;
    const uvView = push(new Uint8Array(mesh.uvs.buffer, mesh.uvs.byteOffset, mesh.uvs.byteLength), ARRAY_BUFFER);
    accessors.push({ bufferView: uvView, componentType: FLOAT, count: mesh.uvs.length / 2, type: 'VEC2' });
    const uvAccessor = accessors.length - 1;
    let colorAccessor: number | undefined;
    if (mesh.colors) {
      const colorView = push(new Uint8Array(mesh.colors.buffer, mesh.colors.byteOffset, mesh.colors.byteLength), ARRAY_BUFFER);
      accessors.push({ bufferView: colorView, componentType: FLOAT, count: mesh.colors.length / 4, type: 'VEC4' });
      colorAccessor = accessors.length - 1;
    }
    let tangentAccessor: number | undefined;
    if (mesh.tangents) {
      const tangentView = push(new Uint8Array(mesh.tangents.buffer, mesh.tangents.byteOffset, mesh.tangents.byteLength), ARRAY_BUFFER);
      accessors.push({ bufferView: tangentView, componentType: FLOAT, count: mesh.tangents.length / 4, type: 'VEC4' });
      tangentAccessor = accessors.length - 1;
    }
    const IndexArray = mesh.positions.length / 3 <= 65536 && mesh.indices.every(index => index <= 65535) ? Uint16Array : Uint32Array;
    const exportedIndices = IndexArray.from(mesh.indices);
    const indexView = push(new Uint8Array(exportedIndices.buffer, exportedIndices.byteOffset, exportedIndices.byteLength), ELEMENT_ARRAY_BUFFER);
    accessors.push({ bufferView: indexView, componentType: IndexArray === Uint16Array ? UNSIGNED_SHORT : UNSIGNED_INT, count: mesh.indices.length, type: 'SCALAR' });
    const indexAccessor = accessors.length - 1;
    const maps = {
      albedo: addTexture(pngs?.albedo), metallicRoughness: addTexture(pngs?.metallicRoughness),
      normal: addTexture(pngs?.normal), occlusion: addTexture(pngs?.occlusion), emissive: addTexture(pngs?.emissive),
    };
    const materialJsonText = JSON.stringify(materialJson(part.material, maps));
    // A part whose glow or colour changes over time gets a material of its own, since the clip would otherwise change
    // every part that shares the same look.
    const looks = !part.transforms?.length && part.animation?.some(track => track.property === 'glow' || track.property === 'color');
    let material = looks ? undefined : materialIds.get(materialJsonText);
    if (material === undefined) {
      material = materials.length;
      const json = JSON.parse(materialJsonText) as Json;
      if (looks) {
        const emissive = (part.material?.emissive ?? [0, 0, 0]).slice(0, 3);
        const peak = Math.max(0, ...emissive);
        if (peak > 0 && part.animation?.some(track => track.property === 'glow')) {
          // Glow scales the emissive strength, which can be above 1 (the factor itself stays within 0 to 1).
          json.emissiveFactor = emissive.map(v => v / Math.max(1, peak));
          json.extensions = { KHR_materials_emissive_strength: { emissiveStrength: Math.max(1, peak) } };
          for (const name of clipsIn(part.animation.filter(track => track.property === 'glow'))) {
            lookAnimated.push({ material, tracks: inClip(part.animation, name), strength: Math.max(1, peak), glow: true, name });
          }
        }
        for (const name of clipsIn((part.animation ?? []).filter(track => track.property === 'color'))) {
          lookAnimated.push({ material, tracks: inClip(part.animation ?? [], name), strength: 1, glow: false, name });
        }
      }
      materials.push(json);
      if (!looks) materialIds.set(materialJsonText, material);
    }
    // A skinned part carries its bone indices and weights per exported vertex (looked up through the vertex each came from).
    const skinned = Boolean(part.skin && part.skin_joints?.length && part.skin_weights?.length && !part.transforms?.length
      && part.skin.bones.length <= 32 && part.skin.bones.every(name => drawn.has(name)));
    const skinAttributes: Json = {};
    if (skinned && part.skin_joints && part.skin_weights) {
      const count = mesh.sources.length;
      const jointData = new Uint16Array(count * 4), weightData = new Float32Array(count * 4);
      for (let i = 0; i < count; i++) {
        for (let k = 0; k < 4; k++) {
          jointData[i * 4 + k] = part.skin_joints[mesh.sources[i] * 4 + k];
          weightData[i * 4 + k] = part.skin_weights[mesh.sources[i] * 4 + k];
        }
      }
      const jointView = push(new Uint8Array(jointData.buffer, jointData.byteOffset, jointData.byteLength), ARRAY_BUFFER);
      accessors.push({ bufferView: jointView, componentType: UNSIGNED_SHORT, count, type: 'VEC4' });
      skinAttributes.JOINTS_0 = accessors.length - 1;
      const weightView = push(new Uint8Array(weightData.buffer, weightData.byteOffset, weightData.byteLength), ARRAY_BUFFER);
      accessors.push({ bufferView: weightView, componentType: FLOAT, count, type: 'VEC4' });
      skinAttributes.WEIGHTS_0 = accessors.length - 1;
    }
    meshes.push({
      name: part.name || part.node,
      primitives: [{ attributes: { POSITION: positionAccessor, NORMAL: normalAccessor, TEXCOORD_0: uvAccessor, ...(colorAccessor === undefined ? {} : { COLOR_0: colorAccessor }), ...(tangentAccessor === undefined ? {} : { TANGENT: tangentAccessor }), ...skinAttributes }, indices: indexAccessor, material, mode: 4 }],
    });
    const meshIndex = meshes.length - 1;
    if (part.transforms && part.transforms.length >= 16) {
      // Instances: one node per placement, all sharing the mesh. The 4 x 4 matrices are column-major,
      // as glTF wants.
      const wanted = part.instance_indices && part.instance_indices.length > 0
        ? Array.from(part.instance_indices) : Array.from({ length: part.transforms.length / 16 }, (_, i) => i);
      for (const instance of wanted) {
        const matrix = Array.from(part.transforms.subarray(instance * 16, instance * 16 + 16));
        if (matrix.length !== 16) continue;
        // A copy of a placed set with motion tracks gets translation, rotation and scale channels that already include its
        // placement and phase (a node with a matrix cannot be animated). Big sets stay static to keep the file small.
        const motion = wanted.length <= MAX_ANIMATED_COPIES
          ? (part.animation ?? []).filter(track => ['position', 'rotation', 'scale'].includes(track.property)) : [];
        const name = `${part.name || part.node}.${instance}`;
        if (motion.length === 0) {
          nodes.push({ name, mesh: meshIndex, matrix });
          continue;
        }
        nodes.push({ name, mesh: meshIndex });
        for (const clipName of clipsIn(motion)) {
          animated.push({ node: nodes.length - 1, clip: sampleClip(inClip(motion, clipName), { base: matrix, index: instance }), name: clipName });
        }
      }
    } else {
      nodes.push({ name: part.name || part.node, mesh: meshIndex });
      placed.push({ part, node: nodes.length - 1 });
      if (skinned && part.skin) skinnedNodes.push({ node: nodes.length - 1, bones: part.skin.bones });
      if (part.animation?.some(track => track.property === 'visible')) {
        // Showing and hiding goes through KHR_node_visibility; the node starts as the clip's first state.
        const first = sampleLook(part.animation, 'visible');
        nodes[nodes.length - 1].extensions = { KHR_node_visibility: { visible: (first?.values[0] ?? 1) >= 0.5 } };
        for (const name of clipsIn(part.animation.filter(track => track.property === 'visible'))) {
          visibilityAnimated.push({ node: nodes.length - 1, tracks: inClip(part.animation, name), name });
        }
      }
      // Glow has no glTF node channel (it would need an extension); only motion is written.
      const motion = part.animation?.filter(track => !['glow', 'visible', 'color'].includes(track.property)) ?? [];
      for (const name of clipsIn(motion)) animated.push({ node: nodes.length - 1, clip: sampleClip(inClip(motion, name)), name });
    }
  }
  // A part that follows another becomes its child node: the child's own channels act first and the parent's
  // carry it along, which is how the viewer composes them. Only parentless nodes are scene roots.
  const nodeOfName = new Map(placed.map(({ part, node }) => [part.name || part.node, node]));
  const children = new Set<number>();
  for (const { part, node } of placed) {
    const parent = part.parent ? nodeOfName.get(part.parent) : undefined;
    if (parent === undefined || parent === node) continue;
    ((nodes[parent].children ??= []) as number[]).push(node);
    children.add(node);
  }
  // The current rig representation stores bone segments in world space and applies skinning in the rest pose.
  // Emit explicit identity inverse-bind matrices. All joints of a skin must hang under one root, so when the bones are
  // separate roots they are gathered under a plain armature node.
  const parentOf = new Map<number, number>();
  nodes.forEach((entry, parent) => (entry.children as number[] | undefined)?.forEach(child => parentOf.set(child, parent)));
  const rootOf = (node: number): number => { let at = node; while (parentOf.has(at)) at = parentOf.get(at) as number; return at; };
  const skins: Json[] = [];
  for (const { node, bones } of skinnedNodes) {
    const joints = bones.map(name => nodeOfName.get(name) as number);
    const roots = [...new Set(joints.map(rootOf))];
    if (roots.length > 1) {
      nodes.push({ name: 'armature', children: roots });
      const armature = nodes.length - 1;
      for (const root of roots) { parentOf.set(root, armature); children.add(root); }
    }
    const inverseBindMatrices = new Float32Array(joints.length * 16);
    for (let i = 0; i < joints.length; i++) {
      inverseBindMatrices[i * 16] = 1; inverseBindMatrices[i * 16 + 5] = 1;
      inverseBindMatrices[i * 16 + 10] = 1; inverseBindMatrices[i * 16 + 15] = 1;
    }
    const inverseBindView = push(new Uint8Array(inverseBindMatrices.buffer));
    accessors.push({ bufferView: inverseBindView, componentType: FLOAT, count: joints.length, type: 'MAT4' });
    skins.push({ joints, inverseBindMatrices: accessors.length - 1 });
    nodes[node].skin = skins.length - 1;
  }
  // One animation per clip holds every moving part's channels for that clip, so the whole scene plays that clip
  // together. The main clip is named `clip`; the others carry their own names.
  const animations: Json[] = [];
  const accessor = (data: Float32Array, components: number, type: string, bounds?: Json) => {
    const view = push(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    accessors.push({ bufferView: view, componentType: FLOAT, count: data.length / components, type, ...bounds });
    return accessors.length - 1;
  };
  const clipNames = [...new Set([...animated, ...lookAnimated, ...visibilityAnimated].map(entry => entry.name))]
    .sort((a, b) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)));
  for (const clipName of clipNames) {
    const samplers: Json[] = [], channels: Json[] = [];
    for (const { node, clip } of animated.filter(entry => entry.name === clipName)) {
      const input = accessor(clip.times, 1, 'SCALAR', { min: [clip.times[0]], max: [clip.times[clip.times.length - 1]] });
      const outputs: [string, Float32Array, number, string][] = [
        ['translation', clip.translations, 3, 'VEC3'], ['rotation', clip.rotations, 4, 'VEC4'], ['scale', clip.scales, 3, 'VEC3'],
      ];
      for (const [path, data, components, type] of outputs) {
        samplers.push({ input, output: accessor(data, components, type), interpolation: 'LINEAR' });
        channels.push({ sampler: samplers.length - 1, target: { node, path } });
      }
    }
    // Glow and colour clips animate material properties through KHR_animation_pointer (the glTF animation of a
    // material has no node to target).
    for (const { material, tracks, strength, glow } of lookAnimated.filter(entry => entry.name === clipName)) {
      const look = sampleLook(tracks, glow ? 'glow' : 'color');
      if (!look) continue;
      const input = accessor(look.times, 1, 'SCALAR', { min: [look.times[0]], max: [look.times[look.times.length - 1]] });
      let output: number, pointer: string;
      if (glow) {
        output = accessor(look.values.map(v => v * strength), 1, 'SCALAR');
        pointer = `/materials/${material}/extensions/KHR_materials_emissive_strength/emissiveStrength`;
      } else {
        const rgba = new Float32Array((look.values.length / 3) * 4);
        for (let i = 0; i < look.values.length / 3; i++) {
          for (let k = 0; k < 3; k++) rgba[i * 4 + k] = srgbToLinear(look.values[i * 3 + k]);
          rgba[i * 4 + 3] = 1;
        }
        output = accessor(rgba, 4, 'VEC4');
        pointer = `/materials/${material}/pbrMetallicRoughness/baseColorFactor`;
      }
      samplers.push({ input, output, interpolation: 'LINEAR' });
      channels.push({ sampler: samplers.length - 1, target: { path: 'pointer', extensions: { KHR_animation_pointer: { pointer } } } });
    }
    for (const { node, tracks } of visibilityAnimated.filter(entry => entry.name === clipName)) {
      const look = sampleLook(tracks, 'visible');
      if (!look) continue;
      const input = accessor(look.times, 1, 'SCALAR', { min: [look.times[0]], max: [look.times[look.times.length - 1]] });
      samplers.push({ input, output: accessor(look.values, 1, 'SCALAR'), interpolation: 'STEP' });
      channels.push({ sampler: samplers.length - 1, target: { path: 'pointer', extensions: { KHR_animation_pointer: { pointer: `/nodes/${node}/extensions/KHR_node_visibility/visible` } } } });
    }
    const clipTracks = parts.flatMap(part => part.animation ?? []).filter(track => (track.clip ?? '') === clipName);
    animations.push({ name: clipName || 'clip', samplers, channels, extras: { loop: clipTracks.length > 0 && clipTracks.every(track => track.looped) } });
  }
  const json: Json = {
    asset: { version: '2.0', generator: 'Spatial Lab engine' },
    scene: 0,
    scenes: [{ name: options.name ?? 'model', nodes: nodes.map((_, i) => i).filter(i => !children.has(i)), extras: { units: 'metres' } }],
    nodes, meshes, materials, accessors, bufferViews,
    buffers: [{ byteLength: length }],
  };
  if (animations.length) json.animations = animations;
  if (skins.length) json.skins = skins;
  const used: string[] = [];
  if (lookAnimated.length > 0 || visibilityAnimated.length > 0) used.push('KHR_animation_pointer');
  if (lookAnimated.some(entry => entry.glow) || materials.some(material => Boolean((material.extensions as Json | undefined)?.KHR_materials_emissive_strength))) used.push('KHR_materials_emissive_strength');
  if (visibilityAnimated.length > 0) used.push('KHR_node_visibility');
  if (used.length) json.extensionsUsed = used;
  if (images.length) {
    json.images = images;
    json.textures = textures;
    json.samplers = [{ magFilter: 9729, minFilter: 9987, wrapS: 10497, wrapT: 10497 }];
  }
  if (length === 0) delete json.buffers; // a buffer must hold at least one byte
  for (const key of ['nodes', 'meshes', 'materials', 'accessors', 'bufferViews'] as const) {
    if ((json[key] as unknown[]).length === 0) delete json[key];
  }
  if (nodes.length === 0) delete (json.scenes as Json[])[0].nodes;
  const jsonBytes = new TextEncoder().encode(JSON.stringify(json));
  const jsonLength = pad4(jsonBytes.byteLength);
  const total = 12 + 8 + jsonLength + (length > 0 ? 8 + length : 0);
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  view.setUint32(0, GLB_MAGIC, true); view.setUint32(4, 2, true); view.setUint32(8, total, true);
  view.setUint32(12, jsonLength, true); view.setUint32(16, JSON_CHUNK, true);
  out.set(jsonBytes, 20);
  out.fill(0x20, 20 + jsonBytes.byteLength, 20 + jsonLength); // JSON is padded with spaces
  if (length > 0) {
    const at = 20 + jsonLength;
    view.setUint32(at, length, true); view.setUint32(at + 4, BIN_CHUNK, true);
    let write = at + 8;
    for (const chunk of chunks) { out.set(chunk, write); write += chunk.byteLength; }
  }
  return out;
}

/** Bakes authored procedural programs in the browser and embeds their PNG maps in the exported GLB. */
export async function buildGlbWithTextures(parts: MeshPart[], options: Omit<GlbOptions, 'textureImages'> = {}, textureSize = 512): Promise<Uint8Array> {
  if (!parts.some(part => part.material?.texture_program)) return buildGlb(parts, options);
  const { library, helpers } = await loadBrowserTextureLibrary();
  const baker = new Baker(new OffscreenCanvas(1, 1));
  const maps = new Map<number, GlbTexturePngs>();
  try {
    for (const [index, part] of parts.entries()) {
      if (!part.material?.texture_program) continue;
      const baked = bakeMaterialMaps(library, helpers, part, baker, textureSize);
      const pngs: GlbTexturePngs = {};
      if (baked.albedo) pngs.albedo = await encodePng(baked.albedo, baked.size);
      if (baked.metallicRoughness) pngs.metallicRoughness = await encodePng(baked.metallicRoughness, baked.size);
      if (baked.normal) pngs.normal = await encodePng(baked.normal, baked.size);
      if (baked.emissive) pngs.emissive = await encodePng(baked.emissive, baked.size);
      maps.set(index, pngs);
    }
  } finally {
    baker.dispose();
  }
  return buildGlb(parts, { ...options, textureImages: maps });
}
