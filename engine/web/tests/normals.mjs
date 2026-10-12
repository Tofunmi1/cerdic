// Smooth shading across shallow edges, sharp shading across real ones (src/engine/mesh-upload.ts).
import assert from 'node:assert/strict';
import { cornerNormals } from '../../../src/engine/mesh-upload.ts';

const near = (a, b, tolerance = 1e-5) => Math.abs(a - b) <= tolerance;
const normalAt = (normals, corner) => [normals[corner * 3], normals[corner * 3 + 1], normals[corner * 3 + 2]];

// A cube with 8 shared vertices: every face is a hard edge from its neighbours.
const cube = {
  positions: new Float32Array([0,0,0, 1,0,0, 1,1,0, 0,1,0, 0,0,1, 1,0,1, 1,1,1, 0,1,1]),
  indices: new Uint32Array([0,2,1, 0,3,2, 4,5,6, 4,6,7, 0,1,5, 0,5,4, 3,7,6, 3,6,2, 0,4,7, 0,7,3, 1,2,6, 1,6,5]),
};
const cubeNormals = cornerNormals(cube.positions, cube.indices);
for (let corner = 0; corner < cube.indices.length; corner++) {
  const n = normalAt(cubeNormals, corner);
  assert(n.filter((c) => near(Math.abs(c), 1)).length === 1 && n.filter((c) => near(c, 0)).length === 2, `cube corner ${corner} keeps its face normal ${n}`);
}

// A 24-sided cylinder, 1 m tall, radius 0.5, with a top cap: the side is one smooth surface and the cap
// is flat with a hard rim.
const sides = 24, positions = [], indices = [];
for (let ring = 0; ring < 2; ring++) for (let i = 0; i < sides; i++) {
  const a = (i / sides) * Math.PI * 2; positions.push(0.5 * Math.cos(a), ring, 0.5 * Math.sin(a));
}
positions.push(0, 1, 0);
const centre = sides * 2;
for (let i = 0; i < sides; i++) {
  const j = (i + 1) % sides;
  indices.push(i, i + sides, j + sides, i, j + sides, j);       // side quad, outward
  indices.push(centre, j + sides, i + sides);                    // cap fan, facing up
}
const cylinderIndices = new Uint32Array(indices), cylinderPositions = new Float32Array(positions);
const cylinder = cornerNormals(cylinderPositions, cylinderIndices);
let sideCorners = 0, capCorners = 0;
for (let face = 0; face < cylinderIndices.length / 3; face++) {
  const isCap = cylinderIndices[face * 3] === centre;
  for (let k = 0; k < 3; k++) {
    const corner = face * 3 + k, n = normalAt(cylinder, corner), v = cylinderIndices[corner];
    if (isCap) { assert(near(n[1], 1) && near(n[0], 0) && near(n[2], 0), `cap corner ${corner} is flat: ${n}`); capCorners++; }
    else {
      const x = cylinderPositions[v * 3], z = cylinderPositions[v * 3 + 2], radial = Math.hypot(x, z);
      assert(near(n[1], 0, 1e-4) && near(n[0], x / radial, 2e-3) && near(n[2], z / radial, 2e-3), `side corner ${corner} points straight out: ${n}`);
      sideCorners++;
    }
  }
}
assert(sideCorners === sides * 6 && capCorners === sides * 3);
console.log('normals test passed');
