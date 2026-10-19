/**
 * Bounded box-chart UV fallback for triangle meshes. Each dominant normal direction gets a separate
 * atlas cell, so a closed mesh does not collapse all six sides onto the same UVs. A real seam-aware
 * unwrap is still preferred for authored UVs and is required for production-quality distortion control.
 */
export function boxChartUvs(positions: Float32Array, indices: Uint32Array, gutter = 0.04): Float32Array {
  if (positions.length % 3 !== 0 || indices.length % 3 !== 0) throw new Error('mesh positions and triangle indices are malformed');
  if (!Number.isFinite(gutter) || gutter < 0 || gutter >= 0.25) throw new Error('UV chart gutter must be in [0, 0.25)');
  const count = positions.length / 3;
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i += 3) for (let axis = 0; axis < 3; axis++) {
    const value = positions[i + axis];
    if (!Number.isFinite(value)) throw new Error('mesh positions must be finite');
    min[axis] = Math.min(min[axis], value); max[axis] = Math.max(max[axis], value);
  }
  for (const index of indices) if (index >= count) throw new Error(`mesh index ${index} is out of range`);
  const out = new Float32Array(indices.length * 2);
  const cells: [number, number][] = [[0, 0], [1, 0], [2, 0], [0, 1], [1, 1], [2, 1]];
  const coordinate = (point: number, axis: number) => {
    const extent = max[axis] - min[axis];
    return extent > 1e-12 ? (positions[point * 3 + axis] - min[axis]) / extent : 0.5;
  };
  for (let face = 0; face < indices.length; face += 3) {
    const ids = [indices[face], indices[face + 1], indices[face + 2]];
    const a = ids[0] * 3, b = ids[1] * 3, c = ids[2] * 3;
    const ab = [positions[b] - positions[a], positions[b + 1] - positions[a + 1], positions[b + 2] - positions[a + 2]];
    const ac = [positions[c] - positions[a], positions[c + 1] - positions[a + 1], positions[c + 2] - positions[a + 2]];
    const normal = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]];
    let axis = 0;
    if (Math.abs(normal[1]) > Math.abs(normal[axis])) axis = 1;
    if (Math.abs(normal[2]) > Math.abs(normal[axis])) axis = 2;
    const positive = normal[axis] >= 0;
    const chart = axis * 2 + (positive ? 0 : 1);
    const [column, row] = cells[chart];
    const axes: [number, number] = axis === 0 ? [2, 1] : axis === 1 ? [0, 2] : [0, 1];
    for (let corner = 0; corner < 3; corner++) {
      const point = ids[corner];
      let u = coordinate(point, axes[0]);
      let v = coordinate(point, axes[1]);
      if (!positive) u = 1 - u;
      out[(face + corner) * 2] = (column + gutter + u * (1 - 2 * gutter)) / 3;
      out[(face + corner) * 2 + 1] = (row + gutter + v * (1 - 2 * gutter)) / 2;
    }
  }
  return out;
}

/**
 * World-space tiling UVs: each triangle is projected along its dominant normal axis and the coordinates are metres divided by
 * `tile`, so one small tileable map repeats every `tile` metres across any surface (the renderer's sampler repeats). Faces
 * with the same orientation line up across the whole part, so a wall or a floor shows one continuous pattern.
 */
export function worldTileUvs(positions: Float32Array, indices: Uint32Array, tile: number): Float32Array {
  if (positions.length % 3 !== 0 || indices.length % 3 !== 0) throw new Error('mesh positions and triangle indices are malformed');
  if (!Number.isFinite(tile) || tile <= 0) throw new Error('texture tile must be a positive number of metres');
  const count = positions.length / 3;
  for (const index of indices) if (index >= count) throw new Error(`mesh index ${index} is out of range`);
  const out = new Float32Array(indices.length * 2);
  for (let face = 0; face < indices.length; face += 3) {
    const a = indices[face] * 3, b = indices[face + 1] * 3, c = indices[face + 2] * 3;
    const ab = [positions[b] - positions[a], positions[b + 1] - positions[a + 1], positions[b + 2] - positions[a + 2]];
    const ac = [positions[c] - positions[a], positions[c + 1] - positions[a + 1], positions[c + 2] - positions[a + 2]];
    const normal = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]];
    let axis = 0;
    if (Math.abs(normal[1]) > Math.abs(normal[axis])) axis = 1;
    if (Math.abs(normal[2]) > Math.abs(normal[axis])) axis = 2;
    // Walls (X or Z facing) run along their horizontal axis with up as v; floors and ceilings (Y facing) use X and Z.
    const [first, second] = axis === 0 ? [2, 1] : axis === 1 ? [0, 2] : [0, 1];
    for (let corner = 0; corner < 3; corner++) {
      const point = indices[face + corner] * 3;
      out[(face + corner) * 2] = positions[point + first] / tile;
      out[(face + corner) * 2 + 1] = positions[point + second] / tile;
    }
  }
  return out;
}

/** Expands indexed positions into triangle-corner order for the browser texture position-map baker. */
export function cornerPositions(positions: Float32Array, indices: Uint32Array): Float32Array {
  const out = new Float32Array(indices.length * 3);
  for (let corner = 0; corner < indices.length; corner++) {
    const source = indices[corner] * 3;
    out.set(positions.subarray(source, source + 3), corner * 3);
  }
  return out;
}
