// import_file: brings the user's OWN mesh file into the document as a poly.import node (G5).
// The file is read from the path the caller names, on the machine running this server; nothing is fetched and no model is retrieved.
// Only plain-text .obj, ascii .ply and ascii .stl are read; the text goes into the node's `data`, so the document carries it.
import { readFileSync, statSync } from 'node:fs';
import { basename, extname, resolve } from 'node:path';

const FORMATS = { '.obj': 'obj', '.ply': 'ply', '.stl': 'stl' };
/** The node's text limit is 1,000,000 characters; stay under it with room for the rest of the line. */
export const MAX_BYTES = 990_000;

/** A node name made from a file name: letters, digits and underscores, starting with a letter, 32 at most. */
export function nodeName(file) {
  const stem = basename(file, extname(file)).toLowerCase().replace(/[^a-z0-9_]+/g, '_').replace(/^_+|_+$/g, '');
  const named = /^[a-z]/.test(stem) ? stem : `mesh_${stem}`;
  return named.slice(0, 32).replace(/_+$/, '') || 'mesh';
}

/** The text-language string for `data`: quotes, backslashes and newlines escaped, carriage returns dropped. */
export function quote(data) {
  return `"${data.replace(/\r/g, '').replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n')}"`;
}

/**
 * Reads the file and returns { text, name, format, bytes } for apply_text, or { error }.
 * `args`: path (required), name, format (obj|ply|stl, default from the extension), scale, up ('y'|'z'), output (default true: also output the part).
 */
export function buildImport(args) {
  const { path, name, format, scale, up, output } = args ?? {};
  if (typeof path !== 'string' || path === '') return { error: 'import_file needs path: the file to read (.obj, ascii .ply or ascii .stl)' };
  const file = resolve(path);
  const kind = format ?? FORMATS[extname(file).toLowerCase()];
  if (!['obj', 'ply', 'stl'].includes(kind)) return { error: `cannot tell the format of ${basename(file)}: pass format as obj, ply or stl (only plain-text files are read)` };
  let size;
  try { size = statSync(file).size; } catch { return { error: `cannot read ${file}: the file does not exist or is not readable` }; }
  if (size > MAX_BYTES) return { error: `${basename(file)} is ${size} bytes; the most a node holds is ${MAX_BYTES}. Decimate or split it in the program that made it, or export fewer details` };
  const buffer = readFileSync(file);
  if (buffer.includes(0)) return { error: `${basename(file)} looks binary; only text files are read. Re-save it as ${kind === 'obj' ? 'a plain OBJ' : `ASCII ${kind.toUpperCase()}`}` };
  let data = buffer.toString('utf8');
  if (kind === 'obj') data = data.split('\n').filter((line) => !line.startsWith('#')).join('\n');
  const label = typeof name === 'string' && name ? name : nodeName(file);
  if (!/^[a-z][a-z0-9_]{0,31}$/.test(label)) return { error: 'name is lowercase letters, digits and underscores, starting with a letter (32 at most)' };
  if (up !== undefined && !['y', 'z'].includes(up)) return { error: 'up is "y" or "z"' };
  if (scale !== undefined && !(typeof scale === 'number' && scale > 0)) return { error: 'scale is a positive number (0.001 for millimetres, 0.01 for centimetres)' };
  const extra = `${scale === undefined ? '' : `, scale=${scale}`}${up === undefined ? '' : `, up="${up}"`}`;
  const text = `${label} = poly.import@1(format="${kind}", data=${quote(data)}${extra})\n${output === false ? '' : `output ${label} = ${label}\n`}`;
  return { text, name: label, format: kind, bytes: size };
}
