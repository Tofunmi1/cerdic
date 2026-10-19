// The viewport shares three texture arrays (1024, 512 and 256 texels square) between all textured surfaces (see `MaterialTextures`
// in ve-render). What a scene may hold is a byte budget, mip chains counted, so a scene of small maps holds many more than one of
// large maps. When a scene needs more, the largest maps are shrunk first (halved, last surfaces first), and only when everything
// is already at the smallest size do the last surfaces lose their least useful maps. The author is told which; the exported
// GLB is not affected.

/** What all the maps of a scene may take on the GPU: `ve_render::BUDGET_BYTES`. */
export const VIEWPORT_BUDGET_BYTES = 192 * 1024 * 1024;
/** The map sizes the renderer holds, largest first: `ve_render::TIER_SIZES`. */
export const MAP_SIZES = [1024, 512, 256] as const;
export const MIN_MAP_SIZE = 256;
export const DEFAULT_MAP_SIZE = 1024;

export type MapName = 'albedo' | 'normal' | 'metallicRoughness' | 'emissive';
/** What goes first when the budget is spent: glow, then shininess, then bumps; colour is kept longest. */
const DROP_ORDER: MapName[] = ['emissive', 'metallicRoughness', 'normal', 'albedo'];

type Maps = Partial<Record<MapName, Uint8Array>>;

/** The renderer's map size nearest to (and not above) `wanted`; at least 256, at most 1024. */
export function snapMapSize(wanted: number): number {
  if (!Number.isFinite(wanted)) return DEFAULT_MAP_SIZE;
  return MAP_SIZES.find(size => wanted >= size) ?? MIN_MAP_SIZE;
}

/** Bytes a square RGBA8 map of `size` takes on the GPU with its whole mip chain. */
export function mapBytes(size: number): number {
  let texels = 0;
  for (let edge = size; ; edge = Math.floor(edge / 2)) {
    texels += edge * edge;
    if (edge <= 1) break;
  }
  return texels * 4;
}

/** The side of a square RGBA8 map. */
export function mapSide(map: Uint8Array): number {
  return Math.round(Math.sqrt(map.length / 4));
}

/** Halves a square RGBA8 image with a box filter (each new pixel is the mean of four). */
export function halveRgba(source: Uint8Array, size: number): Uint8Array {
  const half = Math.max(1, size >> 1);
  const out = new Uint8Array(half * half * 4);
  for (let y = 0; y < half; y++) for (let x = 0; x < half; x++) {
    const x0 = Math.min(x * 2, size - 1), x1 = Math.min(x * 2 + 1, size - 1);
    const y0 = Math.min(y * 2, size - 1), y1 = Math.min(y * 2 + 1, size - 1);
    for (let c = 0; c < 4; c++) {
      const sum = source[(y0 * size + x0) * 4 + c] + source[(y0 * size + x1) * 4 + c]
        + source[(y1 * size + x0) * 4 + c] + source[(y1 * size + x1) * 4 + c];
      out[(y * half + x) * 4 + c] = (sum + 2) >> 2;
    }
  }
  return out;
}

export type Fitted<T> = {
  kept: T[];
  /** Per item, the maps removed because even the smallest size did not fit. */
  dropped: MapName[][];
  /** Per item, the maps shrunk and the size each ended at. */
  shrunk: { name: MapName; from: number; to: number }[][];
};

/**
 * Fits `items` (in scene order, each a set of square RGBA8 maps of a renderer size) into `budget` bytes. Returns copies; the
 * inputs and their pixel arrays are not changed. Maps shared between items (the same array) count once and shrink together.
 */
export function fitToBudget<T extends Maps>(items: T[], budget = VIEWPORT_BUDGET_BYTES): Fitted<T> {
  const kept = items.map(item => ({ ...item }));
  const dropped: MapName[][] = items.map(() => []);
  const shrunk: Fitted<T>['shrunk'] = items.map(() => []);
  const cost = () => {
    const distinct = new Set<Uint8Array>();
    for (const item of kept) for (const name of DROP_ORDER) { const map = item[name]; if (map) distinct.add(map); }
    let total = 0;
    for (const map of distinct) total += mapBytes(mapSide(map));
    return total;
  };
  // Shrink: one halving at a time, always the largest size present, the least useful channel first, the last item first.
  const original = new Map<Uint8Array, number>();
  for (const item of kept) for (const name of DROP_ORDER) { const map = item[name]; if (map && !original.has(map)) original.set(map, mapSide(map)); }
  while (cost() > budget) {
    let largest = 0;
    for (const item of kept) for (const name of DROP_ORDER) { const map = item[name]; if (map) largest = Math.max(largest, mapSide(map)); }
    if (largest <= MIN_MAP_SIZE) break;
    let target: Uint8Array | undefined;
    search: for (const name of DROP_ORDER) {
      for (let i = kept.length - 1; i >= 0; i--) {
        const map = kept[i][name];
        if (map && mapSide(map) === largest) { target = map; break search; }
      }
    }
    if (!target) break;
    // Every item holding this array gets the same smaller one, so a shared map stays shared.
    const smaller = halveRgba(target, largest);
    original.set(smaller, original.get(target) ?? largest);
    for (const item of kept) for (const name of DROP_ORDER) if (item[name] === target) (item as Maps)[name] = smaller;
  }
  kept.forEach((item, i) => {
    for (const name of DROP_ORDER) {
      const map = item[name];
      const from = map ? original.get(map) : undefined;
      if (map && from !== undefined && from !== mapSide(map)) shrunk[i].push({ name, from, to: mapSide(map) });
    }
  });
  // Drop: only when everything is already as small as it goes.
  for (const name of DROP_ORDER) {
    for (let i = kept.length - 1; i >= 0 && cost() > budget; i--) {
      if (kept[i][name] !== undefined) {
        delete kept[i][name];
        dropped[i].push(name);
      }
    }
  }
  return { kept, dropped, shrunk };
}
