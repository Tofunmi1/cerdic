// The viewport shares one texture array between all textured surfaces (see `MaterialTextures` in ve-render):
// at most this many maps in all. When a scene needs more, the last surfaces lose their least useful maps
// first, and the author is told which; the exported GLB is not affected.
export const VIEWPORT_LAYER_LIMIT = 32;

export type MapName = 'albedo' | 'normal' | 'metallicRoughness' | 'emissive';
/** What goes first when the array is full: glow, then shininess, then bumps; colour is kept longest. */
const DROP_ORDER: MapName[] = ['emissive', 'metallicRoughness', 'normal', 'albedo'];

type Maps = Partial<Record<MapName, unknown>>;

/**
 * Trims `items` (in scene order) to `limit` maps in all. Returns the trimmed copies and, per item, the maps
 * that were dropped. Later items lose maps before earlier ones.
 */
export function fitToLayerBudget<T extends Maps>(items: T[], limit = VIEWPORT_LAYER_LIMIT): { kept: T[]; dropped: MapName[][] } {
  const kept = items.map(item => ({ ...item }));
  const dropped: MapName[][] = items.map(() => []);
  // Parts that share a map (tiled parts with one program) share its layer, so only distinct maps count.
  const count = () => {
    const distinct = new Set<unknown>();
    for (const item of kept) for (const name of DROP_ORDER) if (item[name] !== undefined) distinct.add(item[name]);
    return distinct.size;
  };
  for (const name of DROP_ORDER) {
    for (let i = kept.length - 1; i >= 0 && count() > limit; i--) {
      if (kept[i][name] !== undefined) {
        delete kept[i][name];
        dropped[i].push(name);
      }
    }
  }
  return { kept, dropped };
}
