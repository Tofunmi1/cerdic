// Pure planning for a hand-held tool striking a rolling thing toward a target.
export function planShot({ ball, cup, profile, arrive = 1, cap = 16 }) {
  const dx = cup[0] - ball[0], dz = cup[2] - ball[2];
  const distance = Math.hypot(dx, dz);
  let v2 = arrive * arrive;
  for (let i = profile.length - 1; i > 0; i--) {
    const a = profile[i - 1], b = profile[i], ds = b.s - a.s;
    if (ds <= 0) continue;
    const slope = a.height != null && b.height != null ? (b.height - a.height) / ds : 0;
    const decel = (a.decel + b.decel) / 2;
    v2 += 2 * Math.max(0.05, decel + (5 / 7) * 9.8 * slope) * ds;
  }
  return { dir: distance > 0 ? [dx / distance, dz / distance] : [0, 1], distance,
    speed: Math.max(0.5, Math.min(cap, Math.sqrt(Math.max(v2, 0.25)))),
    voids: profile.filter(point => point.height == null).length,
    surfaces: [...new Set(profile.map(point => point.surface_name))] };
}

export function nextScale(scale, { start, ball, cup }) {
  const dx = cup[0] - start[0], dz = cup[2] - start[2];
  const distance2 = dx * dx + dz * dz;
  const reach = distance2 > 0 ? ((ball[0] - start[0]) * dx + (ball[2] - start[2]) * dz) / distance2 : 1;
  const reset = Math.hypot(ball[0] - start[0], ball[2] - start[2]) < 0.05;
  return scale * (reset ? 0.7 : reach > 1.05 ? 0.8 : reach < 0.9 ? 1.15 : 1);
}

export function swingFrames({ ball, dir, speed, fps = 60 }) {
  if (!Number.isFinite(speed) || speed <= 0 || !Number.isFinite(fps) || fps <= 0) throw new Error('speed and fps must be positive finite numbers');
  const at = t => [ball[0] + dir[0] * t, ball[1], ball[2] + 0.9 + dir[1] * t];
  const frames = Array.from({ length: 8 }, () => at(-0.6));
  for (let t = -0.6; t < 0.5;) {
    t += speed / fps;
    frames.push(at(t));
  }
  return frames;
}

export function parseAimPolicy(policy) {
  if (typeof policy !== 'string') return null;
  const match = /^aim:([^\s:,]+):([^\s:,]+(?:,[^\s:,]+)*)$/.exec(policy);
  if (!match) return null;
  const targets = match[2].split(',');
  return { thing: match[1], target: targets[0], ...(targets.length > 1 ? { targets } : {}) };
}
