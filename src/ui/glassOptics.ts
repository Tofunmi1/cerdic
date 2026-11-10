/** An edge-only rounded lens. The center transmits without distortion. */
export function lensNormal(
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
): [number, number] {
  const r = Math.min(radius, width / 2, height / 2);
  const dx = x - width / 2;
  const dy = y - height / 2;
  const qx = Math.abs(dx) - (width / 2 - r);
  const qy = Math.abs(dy) - (height / 2 - r);
  const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
  const distance = outside + Math.min(Math.max(qx, qy), 0) - r;
  const rim = Math.max(1, Math.min(18, r * 0.65));
  if (distance > 0 || distance < -rim) return [0, 0];
  const strength = Math.sin((Math.PI * -distance) / rim) * 0.8;
  if (qx > 0 && qy > 0)
    return [
      ((Math.sign(dx) * qx) / outside) * strength,
      ((Math.sign(dy) * qy) / outside) * strength,
    ];
  return qx > qy
    ? [Math.sign(dx) * strength, 0]
    : [0, Math.sign(dy) * strength];
}
