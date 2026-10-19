// Faces meeting at less than this angle share a smooth normal; sharper edges stay sharp. A curved
// surface made of flat facets (a revolved profile, a sphere) then shades as one surface while box
// edges and rims keep their crisp look.
export const CREASE_DEGREES = 40;

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
