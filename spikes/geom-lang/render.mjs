// Throwaway: tiny software rasteriser -> PNG contact sheet (flat shading, z-buffer, 3/4 view).
import zlib from 'node:zlib';
import fs from 'node:fs';

function crc32(buf) { let c, crc = ~0; for (let n = 0; n < buf.length; n++) { c = (crc ^ buf[n]) & 0xff; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; crc = (crc >>> 8) ^ c; } return ~crc >>> 0; }
function chunk(type, data) { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td)); return Buffer.concat([len, td, crc]); }
export function writePng(path, w, h, rgb) {
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) { raw[y * (w * 3 + 1)] = 0; rgb.copy(raw, y * (w * 3 + 1) + 1, y * w * 3, (y + 1) * w * 3); }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  fs.writeFileSync(path, Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]));
}

// draw one object into rgb buffer region (ox,oy,size)
export function drawObject(rgb, W, ox, oy, S, r, yaw = 35, pitch = 25) {
  const { verts, numProp, tri, bbox } = r;
  const c = bbox.min.map((v, i) => (v + bbox.max[i]) / 2); const ext = Math.max(...bbox.max.map((v, i) => v - bbox.min[i])) || 1;
  const cy = Math.cos(yaw * Math.PI / 180), sy = Math.sin(yaw * Math.PI / 180), cp = Math.cos(pitch * Math.PI / 180), sp = Math.sin(pitch * Math.PI / 180);
  const nv = verts.length / numProp, P = new Float32Array(nv * 3);
  for (let i = 0; i < nv; i++) {
    let x = verts[i * numProp] - c[0], y = verts[i * numProp + 1] - c[1], z = verts[i * numProp + 2] - c[2];
    [x, z] = [x * cy + z * sy, -x * sy + z * cy]; [y, z] = [y * cp - z * sp, y * sp + z * cp];
    const s = (S * 0.82) / ext; P[i * 3] = ox + S / 2 + x * s; P[i * 3 + 1] = oy + S / 2 - y * s; P[i * 3 + 2] = z * s;
  }
  const zb = new Float32Array(S * S).fill(1e9); const L = [0.4, 0.8, 0.45]; const ll = Math.hypot(...L);
  for (let t = 0; t < tri.length; t += 3) {
    const a = tri[t] * 3, b = tri[t + 1] * 3, d = tri[t + 2] * 3;
    const ax = P[a], ay = P[a + 1], az = P[a + 2], bx = P[b], by = P[b + 1], bz = P[b + 2], dx = P[d], dy = P[d + 1], dz = P[d + 2];
    const u = [bx - ax, by - ay, bz - az], v = [dx - ax, dy - ay, dz - az];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]; const nl = Math.hypot(...n) || 1;
    // screen y flipped => normal z sign; shade with two-sided light
    const lit = Math.abs((-n[0] * L[0] + n[1] * L[1] * -1 + -n[2] * L[2]) / (nl * ll)); const k = 0.28 + 0.72 * Math.min(1, lit);
    const back = n[2] < 0; const col = back ? [200 * k, 120 * k, 120 * k] : [150 * k, 190 * k, 235 * k];
    const minx = Math.max(ox, Math.floor(Math.min(ax, bx, dx))), maxx = Math.min(ox + S - 1, Math.ceil(Math.max(ax, bx, dx)));
    const miny = Math.max(oy, Math.floor(Math.min(ay, by, dy))), maxy = Math.min(oy + S - 1, Math.ceil(Math.max(ay, by, dy)));
    const den = (by - dy) * (ax - dx) + (dx - bx) * (ay - dy); if (Math.abs(den) < 1e-9) continue;
    for (let y = miny; y <= maxy; y++) for (let x = minx; x <= maxx; x++) {
      const w0 = ((by - dy) * (x + 0.5 - dx) + (dx - bx) * (y + 0.5 - dy)) / den, w1 = ((dy - ay) * (x + 0.5 - dx) + (ax - dx) * (y + 0.5 - dy)) / den, w2 = 1 - w0 - w1;
      if (w0 < 0 || w1 < 0 || w2 < 0) continue;
      const z = w0 * az + w1 * bz + w2 * dz; const zi = (y - oy) * S + (x - ox); if (z >= zb[zi]) continue; zb[zi] = z;
      const o = (y * W + x) * 3; rgb[o] = col[0]; rgb[o + 1] = col[1]; rgb[o + 2] = col[2];
    }
  }
}

export function sheet(path, results, cols = 6, S = 220) {
  const rows = Math.ceil(results.length / cols), W = cols * S, H = rows * S;
  const rgb = Buffer.alloc(W * H * 3, 30);
  results.forEach((r, i) => {
    const ox = (i % cols) * S, oy = Math.floor(i / cols) * S;
    for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) { const o = ((oy + y) * W + ox + x) * 3; const g = 38 + (y * 14) / S; rgb[o] = g; rgb[o + 1] = g; rgb[o + 2] = g + 6; if (x === 0 || y === 0) { rgb[o] = rgb[o + 1] = rgb[o + 2] = 70; } }
    if (r.ok) drawObject(rgb, W, ox, oy, S, r);
  });
  writePng(path, W, H, rgb);
}
