// Geometry signals for texture programs: what a surface point is like (convex edge, crease, facing up, open to the sky),
// so a texture can put wear on edges, dirt in creases and moss on top without painting it.
//
// The baker draws the mesh into UV space three times (positions, smooth normals, occlusion) and a fourth pass (`Baker.surfaceMap`)
// combines them per texel into the signal map the surface nodes read:
//   r  curvature: positive on a convex edge or bump, negative in a crease or dent. It is the mean curvature of the surface
//      read off the normals' change per texel, so it is exact on a big flat face (zero) and sharp on a small fillet,
//      whatever the triangles look like. 1 / 0.05 m = about 1: a 5 cm radius edge gives 1.
//   g  ambient occlusion: 1 in the open, 0 fully enclosed (the scene's baked occlusion, 1 when there is none)
//   b  slope: the vertical part of the normal, +1 facing up, -1 facing down
import { cornerNormals } from '../normals';

/** Three floats per triangle corner that the baker draws as a map: the occlusion in the first, zeros in the others. */
export function cornerOcclusion(indices: Uint32Array, occlusion?: Float32Array): Float32Array {
  const out = new Float32Array(indices.length * 3);
  for (let c = 0; c < indices.length; c++) {
    const value = occlusion && occlusion.length > indices[c] ? Math.max(0, Math.min(1, occlusion[indices[c]])) : 1;
    out[c * 3] = value;
  }
  return out;
}

/** The inputs of the surface map for a mesh: smooth corner normals and corner occlusion. */
export function surfaceInputs(positions: Float32Array, indices: Uint32Array, occlusion?: Float32Array): { normals: Float32Array; occlusion: Float32Array } {
  return { normals: cornerNormals(positions, indices), occlusion: cornerOcclusion(indices, occlusion) };
}
