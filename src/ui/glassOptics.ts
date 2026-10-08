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

export function makeLensMap(
  width: number,
  height: number,
  radius: number,
): string {
  const canvas = document.createElement("canvas");
  // The image is regenerated on resize, never on pointer movement or animation.
  const ratio = Math.min(1, 256 / Math.max(width, height));
  canvas.width = Math.max(1, Math.round(width * ratio));
  canvas.height = Math.max(1, Math.round(height * ratio));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Lens texture could not be created");
  const image = context.createImageData(canvas.width, canvas.height);
  for (let y = 0; y < canvas.height; y++)
    for (let x = 0; x < canvas.width; x++) {
      const [nx, ny] = lensNormal(
        (x + 0.5) / ratio,
        (y + 0.5) / ratio,
        width,
        height,
        radius,
      );
      const offset = (y * canvas.width + x) * 4;
      image.data[offset] = Math.round(127.5 + nx * 127.5);
      image.data[offset + 1] = Math.round(127.5 + ny * 127.5);
      image.data[offset + 2] = 128;
      image.data[offset + 3] = 255;
    }
  context.putImageData(image, 0, 0);
  return canvas.toDataURL();
}
