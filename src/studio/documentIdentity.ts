/** Names used by the engine's document storage. */
export const documentName = (raw: string) => raw.replace(/\.[a-z0-9]+$/i, '').replace(/[^a-z0-9_-]+/gi, '-').replace(/^-+|-+$/g, '').slice(0, 48) || 'untitled';
/** The engine derives document/node IDs from its seed, not its storage name.
 * Keep the seed stable per world rather than assigning every project seed 1.
 * Reserve 0 and 1 for bootstrap/legacy documents. */
export function documentSeed(name: string): number {
  let hash = 2166136261;
  for (const character of documentName(name)) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619);
  const seed = hash >>> 0;
  return seed > 1 ? seed : 2;
}
