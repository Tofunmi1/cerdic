/**
 * What changed between two screenshots of the same view: how much of the picture, where, and by how much. Pure, so it is
 * tested without a browser. The numbers are meant for a model, so `describeDiff` turns them into one or two plain sentences.
 */
export type ImageDiff = {
  /** Fraction (0 to 1) of pixels whose colour moved by more than the threshold in some channel. */
  changedFraction: number;
  /** Box around every changed pixel as fractions of the image [left, top, right, bottom], or null when nothing changed. */
  bbox: [number, number, number, number] | null;
  /** Fraction of changed pixels in each of 3 x 3 cells, row by row from the top-left. */
  grid: number[];
  /** Mean absolute colour difference over the whole image (0 to 1). */
  meanShift: number;
};

/** A channel must move by more than this (of 255) for a pixel to count as changed; it ignores dithering and tiny shading noise. */
export const CHANGE_THRESHOLD = 16;

const CELL_NAMES = ['top left', 'top', 'top right', 'left', 'centre', 'right', 'bottom left', 'bottom', 'bottom right'];

/** Compares two RGBA images of the same size. Throws when the sizes differ, because the pixels would not correspond. */
export function diffImages(a: ArrayLike<number>, b: ArrayLike<number>, width: number, height: number, threshold = CHANGE_THRESHOLD): ImageDiff {
  if (width <= 0 || height <= 0 || a.length !== width * height * 4 || b.length !== a.length) {
    throw new Error(`images must both be ${width} x ${height} RGBA`);
  }
  const cellCount = new Array<number>(9).fill(0), cellChanged = new Array<number>(9).fill(0);
  let changed = 0, shift = 0, left = width, top = height, right = -1, bottom = -1;
  for (let y = 0; y < height; y++) {
    const row = Math.min(2, Math.floor((y * 3) / height));
    for (let x = 0; x < width; x++) {
      const at = (y * width + x) * 4;
      let largest = 0;
      for (let c = 0; c < 3; c++) {
        const d = Math.abs(a[at + c] - b[at + c]);
        shift += d;
        if (d > largest) largest = d;
      }
      const cell = row * 3 + Math.min(2, Math.floor((x * 3) / width));
      cellCount[cell]++;
      if (largest > threshold) {
        changed++;
        cellChanged[cell]++;
        if (x < left) left = x;
        if (x > right) right = x;
        if (y < top) top = y;
        if (y > bottom) bottom = y;
      }
    }
  }
  const pixels = width * height;
  return {
    changedFraction: changed / pixels,
    bbox: changed === 0 ? null : [left / width, top / height, (right + 1) / width, (bottom + 1) / height],
    grid: cellChanged.map((n, i) => (cellCount[i] ? n / cellCount[i] : 0)),
    meanShift: shift / (pixels * 3 * 255),
  };
}

/** One or two sentences a model can act on. */
export function describeDiff(diff: ImageDiff): string {
  if (diff.bbox === null) return 'the view is unchanged';
  const percent = (diff.changedFraction * 100).toFixed(diff.changedFraction < 0.01 ? 2 : 1);
  const busiest = diff.grid
    .map((fraction, i) => ({ fraction, name: CELL_NAMES[i] }))
    .filter(cell => cell.fraction > 0.02)
    .sort((x, y) => y.fraction - x.fraction)
    .slice(0, 3)
    .map(cell => cell.name);
  const where = busiest.length > 0 ? `, mostly ${busiest.join(', ')}` : '';
  const box = diff.bbox.map(v => (v * 100).toFixed(0)).join(', ');
  return `${percent}% of the view changed${where}; the change fits in the box [${box}] (left, top, right, bottom, as % of the view)`;
}
