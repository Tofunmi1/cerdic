// 2D simplex noise (Ian McEwan, Ashima Arts, MIT licence), as the CPU twin of the GLSL `snoise` in `glsl.ts`.

const C = [0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439];

const mod = (x: number, y: number): number => x - y * Math.floor(x / y);
const permute = (x: number): number => mod((x * 34 + 1) * x, 289);

/** Smooth gradient noise, about -1 to 1, with value 0 at integer lattice corners of the skewed grid. */
export function simplex(x: number, y: number): number {
  const skew = (x + y) * C[1];
  let ix = Math.floor(x + skew);
  let iy = Math.floor(y + skew);
  const unskew = (ix + iy) * C[0];
  const x0 = x - ix + unskew;
  const y0 = y - iy + unskew;
  const [i1x, i1y] = x0 > y0 ? [1, 0] : [0, 1];
  const x1 = x0 + C[0] - i1x;
  const y1 = y0 + C[0] - i1y;
  const x2 = x0 + C[2];
  const y2 = y0 + C[2];
  ix = mod(ix, 289);
  iy = mod(iy, 289);
  const p = [
    permute(permute(iy) + ix),
    permute(permute(iy + i1y) + ix + i1x),
    permute(permute(iy + 1) + ix + 1),
  ];
  const corners = [[x0, y0], [x1, y1], [x2, y2]];
  let sum = 0;
  for (let k = 0; k < 3; k++) {
    const [cx, cy] = corners[k];
    let m = Math.max(0.5 - (cx * cx + cy * cy), 0);
    m = m * m * m * m;
    const g = 2 * (p[k] * C[3] - Math.floor(p[k] * C[3])) - 1;
    const h = Math.abs(g) - 0.5;
    const a0 = g - Math.floor(g + 0.5);
    m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
    sum += m * (a0 * cx + h * cy);
  }
  return 130 * sum;
}
