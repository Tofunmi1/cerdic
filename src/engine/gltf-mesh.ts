import type { MeshPart } from './worker-types';
import { cornerNormals } from './mesh-upload';
import { boxChartUvs } from './texture/uv';

/** Welds triangle corners with identical position and normal into an indexed mesh. */
export function weld(part: MeshPart, creaseDegrees: number, needTangents = false, mikkTangents?: (positions: Float32Array, normals: Float32Array, uvs: Float32Array, indices: Uint32Array) => Float32Array) {
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

