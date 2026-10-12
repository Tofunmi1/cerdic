import type { Runtime } from './bridge';
import type { MeshPart, Token } from './worker-types';

const TRIANGLES_PER_CHUNK = 512;
const frame = () => new Promise<void>(resolve => requestAnimationFrame(() => resolve()));

// Bounded render packing. One small chunk per frame keeps copies/asset creation below the byte cap.
export async function uploadResult(runtime: Runtime, token: Token, parts: MeshPart[], current: () => boolean) {
  const text = JSON.stringify(token);
  if (!current() || !runtime.eval_projection_begin(text)) return false;
  for (const part of parts) {
    for (let offset = 0; offset < part.indices.length; offset += TRIANGLES_PER_CHUNK * 3) {
      if (!current()) return false;
      const end = Math.min(offset + TRIANGLES_PER_CHUNK * 3, part.indices.length);
      const data = pack(part, offset, end);
      if (!runtime.eval_projection_chunk(text, part.node, part.name, data)) return false;
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

function pack(part: MeshPart, start: number, end: number): Float32Array {
  const out = new Float32Array((end - start) * 6);
  let write = 0;
  for (let face = start; face < end; face += 3) {
    const a = part.indices[face] * 3, b = part.indices[face + 1] * 3, c = part.indices[face + 2] * 3;
    const p = part.positions;
    const ab = [p[b] - p[a], p[b+1] - p[a+1], p[b+2] - p[a+2]];
    const ac = [p[c] - p[a], p[c+1] - p[a+1], p[c+2] - p[a+2]];
    const n = [ab[1]*ac[2] - ab[2]*ac[1], ab[2]*ac[0] - ab[0]*ac[2], ab[0]*ac[1] - ab[1]*ac[0]];
    const length = Math.hypot(...n) || 1;
    for (const index of [a,b,c]) {
      out[write++] = p[index]; out[write++] = p[index+1]; out[write++] = p[index+2];
      out[write++] = n[0]/length; out[write++] = n[1]/length; out[write++] = n[2]/length;
    }
  }
  return out;
}
