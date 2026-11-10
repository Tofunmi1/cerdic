// glTF binary (.glb) export of an evaluated model: the way a model leaves the engine for a game engine,
// a DCC tool or a web viewer. Everything is generated from the document; nothing is imported.
//
// Units are metres and the axes are Y up, matching glTF. Normals use the same crease rule as the
// viewer (faces meeting at less than 40 degrees share a smooth normal), so the export shades like the
// screen. A part's colour, metal and roughness become a PBR metallic-roughness material; procedural
// surface recipes are shader-side in the viewer and are not exported. Instanced parts become one node
// per instance sharing one mesh.
import type { AnimationTrack, MeshPart, PartMaterial } from './worker-types';
import { weld } from './gltf-mesh';
import { cornerNormals } from './normals';
import { sampleClip, sampleLook, type SampledClip } from './animation';
import { Baker, encodePng } from './texture/bake';
import { bakeMaterialMaps } from './texture/apply';
import { loadBrowserTextureLibrary } from './texture/browser-library';
import { rgbaPng } from './png';

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
  // Foliage (leaves, grass, fences) is two-sided, and a cutoff makes the albedo texture's alpha a mask. glTF reads the alpha of
  // the base colour texture times the factor, so the cutoff applies as it is; with no texture there is nothing to cut.
  const foliage = material?.foliage;
  if (foliage && (foliage.alpha_cutoff > 0 || foliage.sway > 0 || foliage.flutter > 0)) {
    json.doubleSided = true;
    if (foliage.alpha_cutoff > 0 && maps.albedo && json.alphaMode !== 'BLEND') {
      json.alphaMode = 'MASK';
      json.alphaCutoff = foliage.alpha_cutoff;
    }
  }
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
  // A bone part (from `rig.bone`) that a skin names is a joint, not geometry: its small placeholder cube is hidden in our own viewer, so
  // it is written as an empty node. Any other viewer would show it as a floating cube at every joint.
  const jointNames = new Set(parts.flatMap(p => (p.skin && p.skin_joints?.length ? p.skin.bones : [])));
  const isJoint = (p: MeshPart) => Boolean(p.bone) && !p.skin && !p.transforms?.length && jointNames.has(p.name || p.node);
  // Parts that get a node: a skin may only name bones that exist in the file.
  const drawn = new Set(parts.filter(p => isJoint(p) || (p.indices.length >= 3 && p.positions.length >= 9 && !p.transforms?.length)).map(p => p.name || p.node));
  const lodPattern = /^(.*)_lod([0-7])$/i;
  const lodParts = new Map<number, { base: string; level: number; count: number }>();
  const lodGroups = new Map<string, { index: number; level: number; node: number; skinned: boolean; bones: string[] }[]>();
  for (const [index, part] of parts.entries()) {
    if (part.transforms?.length || part.indices.length < 3 || part.positions.length < 9) continue;
    const match = lodPattern.exec(part.name || part.node);
    if (!match) continue;
    const base = match[1], level = Number(match[2]);
    const entries = lodGroups.get(base) ?? [];
    entries.push({ index, level, node: -1, skinned: Boolean(part.skin), bones: part.skin?.bones ?? [] });
    lodGroups.set(base, entries);
  }
  for (const [base, entries] of lodGroups) {
    const count = entries.length;
    for (const entry of entries) lodParts.set(entry.index, { base, level: entry.level, count });
  }
  const lodOrder = new Map<number, number[]>();
  for (const entries of lodGroups.values()) {
    const ordered = [...entries].sort((a, b) => a.level - b.level || a.index - b.index).map(entry => entry.index);
    for (const entry of entries) lodOrder.set(entry.index, ordered);
  }
  const orderedIndices: number[] = [], emitted = new Set<number>();
  for (let index = 0; index < parts.length; index++) {
    const group = lodOrder.get(index);
    if (group) {
      for (const member of group) if (!emitted.has(member)) { orderedIndices.push(member); emitted.add(member); }
    } else orderedIndices.push(index);
  }
  const lodWarnings: string[] = [];
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
  for (const partIndex of orderedIndices) {
    const part = parts[partIndex];
    if (isJoint(part)) {
      nodes.push({ name: part.name || part.node });
      placed.push({ part, node: nodes.length - 1 });
      const motion = part.animation?.filter(track => !['glow', 'visible', 'color'].includes(track.property)) ?? [];
      for (const name of clipsIn(motion)) animated.push({ node: nodes.length - 1, clip: sampleClip(inClip(motion, name)), name });
      continue;
    }
    if (part.indices.length < 3 || part.positions.length < 9) continue;
    const pngs = options.textureImages?.get(partIndex);
    const morphTargets = part.morph_targets ?? [];
    const basePositions = part.positions.slice();
    for (const target of morphTargets) {
      if (target.positions.length !== basePositions.length) throw new Error(`shape key ${target.name} does not match ${part.name || part.node}`);
      for (let i = 0; i < basePositions.length; i++) basePositions[i] -= target.positions[i] * target.weight;
    }
    const mesh = weld(morphTargets.length ? { ...part, positions: basePositions } : part, crease, !!pngs?.normal, options.mikkTangents);
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
    const morphJson: Json[] = morphTargets.map(target => {
      const deltas = new Float32Array(mesh.sources.length * 3);
      for (let i = 0; i < mesh.sources.length; i++) {
        const source = mesh.sources[i] * 3;
        deltas.set(target.positions.subarray(source, source + 3), i * 3);
      }
      const view = push(new Uint8Array(deltas.buffer, deltas.byteOffset, deltas.byteLength), ARRAY_BUFFER);
      const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
      for (let i = 0; i < deltas.length; i += 3) {
        for (let axis = 0; axis < 3; axis++) {
          min[axis] = Math.min(min[axis], deltas[i + axis]);
          max[axis] = Math.max(max[axis], deltas[i + axis]);
        }
      }
      accessors.push({ bufferView: view, componentType: FLOAT, count: deltas.length / 3, type: 'VEC3', min, max });
      const targetPositions = basePositions.slice();
      for (let i = 0; i < targetPositions.length; i++) targetPositions[i] += target.positions[i];
      const cornerTargetNormals = cornerNormals(targetPositions, part.indices, crease);
      const sums = new Float64Array(mesh.positions.length);
      for (let corner = 0; corner < mesh.indices.length; corner++) {
        const targetVertex = mesh.indices[corner] * 3;
        sums[targetVertex] += cornerTargetNormals[corner * 3];
        sums[targetVertex + 1] += cornerTargetNormals[corner * 3 + 1];
        sums[targetVertex + 2] += cornerTargetNormals[corner * 3 + 2];
      }
      const normalDeltas = new Float32Array(mesh.positions.length);
      for (let vertex = 0; vertex < mesh.positions.length / 3; vertex++) {
        const at = vertex * 3;
        const length = Math.hypot(sums[at], sums[at + 1], sums[at + 2]) || 1;
        normalDeltas[at] = sums[at] / length - mesh.normals[at];
        normalDeltas[at + 1] = sums[at + 1] / length - mesh.normals[at + 1];
        normalDeltas[at + 2] = sums[at + 2] / length - mesh.normals[at + 2];
      }
      const normalView = push(new Uint8Array(normalDeltas.buffer, normalDeltas.byteOffset, normalDeltas.byteLength), ARRAY_BUFFER);
      accessors.push({ bufferView: normalView, componentType: FLOAT, count: normalDeltas.length / 3, type: 'VEC3' });
      return { POSITION: accessors.length - 2, NORMAL: accessors.length - 1 };
    });
    meshes.push({
      name: part.name || part.node,
      ...(morphTargets.length ? { weights: morphTargets.map(target => target.weight), extras: { targetNames: morphTargets.map(target => target.name) } } : {}),
      primitives: [{ attributes: { POSITION: positionAccessor, NORMAL: normalAccessor, TEXCOORD_0: uvAccessor, ...(colorAccessor === undefined ? {} : { COLOR_0: colorAccessor }), ...(tangentAccessor === undefined ? {} : { TANGENT: tangentAccessor }), ...skinAttributes }, ...(morphJson.length ? { targets: morphJson } : {}), indices: indexAccessor, material, mode: 4 }],
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
      const lod = lodParts.get(partIndex);
      const node: Json = { name: lod ? `${lod.base}_LOD${lod.level}` : part.name || part.node, mesh: meshIndex };
      if (lod) node.extras = { lod: { base: lod.base, level: lod.level, levels: lod.count, triangles: mesh.indices.length / 3 } };
      nodes.push(node);
      if (lod) lodGroups.get(lod.base)?.find(entry => entry.index === partIndex) && (lodGroups.get(lod.base)!.find(entry => entry.index === partIndex)!.node = nodes.length - 1);
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
  const lodRoots: number[] = [];
  for (const [base, entries] of lodGroups) {
    const levels = entries.map(entry => entry.level);
    if (new Set(levels).size !== levels.length) lodWarnings.push(`LOD group ${base} has duplicate levels`);
    const unique = [...new Set(levels)].sort((a, b) => a - b);
    if (unique.some((level, i) => i > 0 && level !== unique[i - 1] + 1)) lodWarnings.push(`LOD group ${base} is missing one or more levels`);
    const root = entries.find(entry => entry.level === 0);
    if (!root) continue;
    lodRoots.push(root.node);
    const sameSkin = entries.every(entry => entry.skinned === root.skinned && (!entry.skinned || JSON.stringify(entry.bones) === JSON.stringify(root.bones)));
    const lod = (nodes[root.node].extras as Json).lod as Json;
    lod.screenCoverage = unique.map(level => 0.5 ** level); // Hint for engines that choose a level by projected screen coverage.
    if (sameSkin) nodes[root.node].extensions = { ...((nodes[root.node].extensions as Json | undefined) ?? {}), MSFT_lod: { ids: entries.filter(entry => entry.level !== 0).map(entry => entry.node) } };
    else lodWarnings.push(`LOD group ${base} has incompatible skinning`);
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
  if (lodWarnings.length) (json.asset as Json).extras = { warnings: lodWarnings };
  if (animations.length) json.animations = animations;
  if (skins.length) json.skins = skins;
  const used: string[] = [];
  if (lookAnimated.length > 0 || visibilityAnimated.length > 0) used.push('KHR_animation_pointer');
  if (lookAnimated.some(entry => entry.glow) || materials.some(material => Boolean((material.extensions as Json | undefined)?.KHR_materials_emissive_strength))) used.push('KHR_materials_emissive_strength');
  if (visibilityAnimated.length > 0) used.push('KHR_node_visibility');
  if (lodRoots.some(node => Boolean((nodes[node].extensions as Json | undefined)?.MSFT_lod))) used.push('MSFT_lod');
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
export async function buildGlbWithTextures(parts: MeshPart[], options: GlbOptions = {}, textureSize = 512): Promise<Uint8Array> {
  if (!parts.some(part => part.material?.texture_program || part.baked?.normal || part.baked?.ao)) return buildGlb(parts, options);
  const hasProcedural = parts.some(part => part.material?.texture_program);
  const resources = hasProcedural ? await loadBrowserTextureLibrary() : undefined;
  const baker = hasProcedural ? new Baker(new OffscreenCanvas(1, 1)) : undefined;
  const maps = new Map<number, GlbTexturePngs>(options.textureImages);
  try {
    for (const [index, part] of parts.entries()) {
      if (!part.baked) continue;
      const pngs: GlbTexturePngs = { ...(maps.get(index) ?? {}) };
      if (part.baked.normal) pngs.normal = await rgbaPng(part.baked.normal, part.baked.size);
      if (part.baked.ao) pngs.occlusion = await rgbaPng(part.baked.ao, part.baked.size);
      maps.set(index, pngs);
    }
    for (const [index, part] of parts.entries()) {
      if (!part.material?.texture_program) continue;
      if (!resources || !baker) continue;
      const baked = bakeMaterialMaps(resources.library, resources.helpers, part, baker, textureSize);
      const pngs: GlbTexturePngs = maps.get(index) ?? {};
      if (baked.albedo) pngs.albedo = await encodePng(baked.albedo, baked.size);
      if (baked.metallicRoughness) pngs.metallicRoughness = await encodePng(baked.metallicRoughness, baked.size);
      if (baked.normal && !pngs.normal) pngs.normal = await encodePng(baked.normal, baked.size);
      if (baked.emissive) pngs.emissive = await encodePng(baked.emissive, baked.size);
      maps.set(index, pngs);
    }
  } finally {
    baker?.dispose();
  }
  return buildGlb(parts, { ...options, textureImages: maps });
}
