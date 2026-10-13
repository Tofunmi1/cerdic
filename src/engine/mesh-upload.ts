import type { Runtime } from './bridge';
import type { MeshPart, Token } from './worker-types';

const TRIANGLES_PER_CHUNK = 512;
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => resolve()));

// Bounded render packing. One small chunk per frame keeps copies/asset creation below the byte cap.
export async function uploadResult(runtime: Runtime, token: Token, parts: MeshPart[], current: () => boolean) {
  const text = JSON.stringify(token);
  if (!current() || !runtime.eval_projection_begin(text)) return false;
  for (const [partIndex,part] of parts.entries()) {
    const group=part.transforms?.length ? partIndex+1 : 0;
    if (group && part.transforms && part.instance_indices) {
      for (let offset=0; offset<part.instance_indices.length; offset+=512) {
        if (!current() || !runtime.eval_projection_instances(text,group,part.node,
          part.transforms.subarray(offset*16,(offset+512)*16),part.instance_indices.subarray(offset,offset+512))) return false;
        await frame();
      }
    }
    if (part.material && !runtime.eval_projection_material(text, part.node, JSON.stringify(part.material))) return false;
    const normals = part.indices.length / 3 <= SMOOTH_TRIANGLE_LIMIT ? cornerNormals(part.positions, part.indices) : null;
    for (let offset = 0; offset < part.indices.length; offset += TRIANGLES_PER_CHUNK * 3) {
      if (!current()) return false;
      const end = Math.min(offset + TRIANGLES_PER_CHUNK * 3, part.indices.length);
      const data = pack(part, offset, end, normals);
      if (group ? !runtime.eval_projection_chunk_group(text,group,part.node,part.name,data) :
          !runtime.eval_projection_chunk(text, part.node, part.name, data)) return false;
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

// Faces meeting at less than this angle share a smooth normal; sharper edges stay sharp. A curved
// surface made of flat facets (a revolved profile, a sphere) then shades as one surface while box
// edges and rims keep their crisp look.
const CREASE_DEGREES = 40;
// Above this many triangles the packing stays flat-shaded: smoothing is computed on the window thread.
const SMOOTH_TRIANGLE_LIMIT = 500_000;

/** One normal per triangle corner (three floats each, in index order), smoothed across shallow edges. */
export function cornerNormals(positions: Float32Array, indices: Uint32Array, creaseDegrees = CREASE_DEGREES): Float32Array {
  const faces = indices.length / 3;
  const unit = new Float32Array(faces * 3);    // unit face normals
  const angle = new Float32Array(indices.length); // the angle each corner makes inside its triangle
  const edge = (from: number, to: number) => [positions[to] - positions[from], positions[to + 1] - positions[from + 1], positions[to + 2] - positions[from + 2]];
  const between = (u: number[], v: number[]) => Math.atan2(Math.hypot(u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]), u[0] * v[0] + u[1] * v[1] + u[2] * v[2]);
  for (let f = 0; f < faces; f++) {
    const a = indices[f * 3] * 3, b = indices[f * 3 + 1] * 3, c = indices[f * 3 + 2] * 3;
    const ab = edge(a, b), ac = edge(a, c), bc = edge(b, c);
    const nx = ab[1] * ac[2] - ab[2] * ac[1], ny = ab[2] * ac[0] - ab[0] * ac[2], nz = ab[0] * ac[1] - ab[1] * ac[0];
    const length = Math.hypot(nx, ny, nz) || 1;
    unit[f * 3] = nx / length; unit[f * 3 + 1] = ny / length; unit[f * 3 + 2] = nz / length;
    angle[f * 3] = between(ab, ac);
    angle[f * 3 + 1] = between([-ab[0], -ab[1], -ab[2]], bc);
    angle[f * 3 + 2] = Math.PI - angle[f * 3] - angle[f * 3 + 1];
  }
  // the corners around each vertex, as a compressed list
  const vertices = positions.length / 3;
  const start = new Uint32Array(vertices + 1);
  for (const index of indices) start[index + 1]++;
  for (let v = 0; v < vertices; v++) start[v + 1] += start[v];
  const fill = start.slice(0, vertices), around = new Uint32Array(indices.length);
  for (let corner = 0; corner < indices.length; corner++) around[fill[indices[corner]]++] = corner;
  const limit = Math.cos((creaseDegrees * Math.PI) / 180);
  const out = new Float32Array(indices.length * 3);
  for (let corner = 0; corner < indices.length; corner++) {
    const f = Math.floor(corner / 3), v = indices[corner];
    let x = 0, y = 0, z = 0;
    for (let k = start[v]; k < start[v + 1]; k++) {
      const other = around[k], g = Math.floor(other / 3);
      if (unit[f * 3] * unit[g * 3] + unit[f * 3 + 1] * unit[g * 3 + 1] + unit[f * 3 + 2] * unit[g * 3 + 2] >= limit) {
        x += unit[g * 3] * angle[other]; y += unit[g * 3 + 1] * angle[other]; z += unit[g * 3 + 2] * angle[other];
      }
    }
    const length = Math.hypot(x, y, z) || 1;
    out[corner * 3] = x / length; out[corner * 3 + 1] = y / length; out[corner * 3 + 2] = z / length;
  }
  return out;
}

function pack(part: MeshPart, start: number, end: number, normals: Float32Array | null): Float32Array {
  const out = new Float32Array((end - start) * 6);
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
    });
  }
  return out;
}
