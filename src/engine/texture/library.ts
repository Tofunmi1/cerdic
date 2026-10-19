// Loading the vendored Material Maker node library (engine/third_party/material-maker, MIT).
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { helperFunctions } from './compile';
import { parseLenientJson } from './parse';
import { withSurfaceNodes } from './surface-nodes';
import type { Library, NodeDef } from './types';

export { parseLenientJson } from './parse';

const NODES = 'addons/material_maker/nodes';

/** Reads the `.mmg` node definitions named in `names` (all of them when omitted) from the vendored tree. */
export function loadLibrary(root: string, names?: string[]): Library {
  const dir = join(root, NODES);
  const files = names ? names.map((name) => `${name}.mmg`) : readdirSync(dir).filter((file) => file.endsWith('.mmg'));
  const library: Library = {};
  for (const file of files) {
    const def = parseLenientJson<NodeDef>(readFileSync(join(dir, file), 'utf8'));
    library[file.replace(/\.mmg$/, '')] = def;
  }
  return withSurfaceNodes(library);
}

/** Material Maker's shared GLSL helper functions. */
export function loadHelpers(root: string): string {
  return helperFunctions(readFileSync(join(root, 'addons/material_maker/shader_functions.tres'), 'utf8'));
}
