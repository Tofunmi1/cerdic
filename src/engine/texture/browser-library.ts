import { helperFunctions } from './compile';
import { parseLenientJson } from './parse';
import { withSurfaceNodes } from './surface-nodes';
import type { Library, NodeDef } from './types';

export type BrowserTextureLibrary = { library: Library; helpers: string };
let cached: Promise<BrowserTextureLibrary> | undefined;

/** Loads the engine-build's static, same-origin copy of the vendored Material Maker node definitions. */
export function loadBrowserTextureLibrary(baseUrl = './texture-library/'): Promise<BrowserTextureLibrary> {
  if (cached) return cached;
  const loading = (async () => {
    // The engine's files (this library included) sit next to the engine bundle: on the plain viewer page that is next to this code; in the studio
    // the page names the folder (`__veEngineBase`), because this code is then inside the app's own bundle.
    const home = (globalThis as { __veEngineBase?: string }).__veEngineBase ?? import.meta.url;
    const base = new URL(baseUrl, home);
    const indexResponse = await fetch(new URL('index.json', base));
    if (!indexResponse.ok) throw new Error(`texture library index unavailable (${indexResponse.status})`);
    const names = await indexResponse.json() as string[];
    if (!Array.isArray(names) || names.length < 300 || names.length > 1000 || names.some(name => !/^[a-z0-9_]+$/.test(name))) {
      throw new Error('texture library index is malformed');
    }
    const loaded: [string, NodeDef][] = [];
    for (let offset = 0; offset < names.length; offset += 16) {
      const batch = await Promise.all(names.slice(offset, offset + 16).map(async name => {
        const response = await fetch(new URL(`nodes/${name}.mmg`, base));
        if (!response.ok) throw new Error(`texture node "${name}" unavailable (${response.status})`);
        return [name, parseLenientJson<NodeDef>(await response.text())] as [string, NodeDef];
      }));
      loaded.push(...batch);
    }
    const helperResponse = await fetch(new URL('shader_functions.tres', base));
    if (!helperResponse.ok) throw new Error(`texture shader helpers unavailable (${helperResponse.status})`);
    const helperText = await helperResponse.text();
    return { library: withSurfaceNodes(Object.fromEntries(loaded)), helpers: helperFunctions(helperText) };
  })();
  cached = loading;
  void loading.catch(() => { if (cached === loading) cached = undefined; });
  return loading;
}
