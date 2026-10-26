import { mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

const templateDir = new URL('./templates/', import.meta.url);
const recipeDir = () => resolve(process.env.VE_RECIPES_DIR || new URL('./recipes-local/', import.meta.url).pathname);

function readTemplate(file) {
  const raw = readFileSync(new URL(file, templateDir), 'utf8');
  const header = [];
  const lines = raw.split(/\r?\n/);
  while (lines[0]?.startsWith('#')) header.push(lines.shift().slice(1).trim());
  const fields = Object.fromEntries(header.map(line => {
    const at = line.indexOf(':');
    return at < 0 ? ['', ''] : [line.slice(0, at).trim(), line.slice(at + 1).trim()];
  }).filter(([key]) => key));
  return { name: file.slice(0, -4), about: fields.about || '', kind: fields.kind || 'scene', change: header.filter(line => line.startsWith('change:')).map(line => line.slice(7).trim()), text: lines.join('\n').trim() };
}

export function templateNames() {
  return readdirSync(new URL('./templates/', import.meta.url)).filter(file => file.endsWith('.txt')).sort();
}

export function callStartFrom(args, callPage) {
  const files = templateNames();
  if (args?.template === undefined) return { templates: files.map(file => {
    const { name, about, kind, text } = readTemplate(file);
    return { name, about, nodes: (text.match(/^(?!group |end$|output ).+=/gm) || []).length, kind };
  }) };
  const file = `${args.template}.txt`;
  if (!files.includes(file)) throw new Error(`Unknown template "${args.template}". Choose one of: ${files.map(f => f.slice(0, -4)).join(', ')}.`);
  const body = readTemplate(file);
  if (!args.apply) return { name: body.name, about: body.about, text: body.text, change: body.change, notes: `Working ${body.kind} skeleton. Edit its counts, dimensions, colours, names, or rules to make it your own.` };
  return callPage('apply_text', { text: body.text });
}

const recipePattern = /^[a-z0-9_]{1,32}$/;
const commentAbout = (source) => source.match(/^#\s*about:\s*(.*)$/m)?.[1]?.trim() || '';
const sourceOnly = (raw) => raw.replace(/^#.*(?:\r?\n|$)/, '').trim();

function exposed(source) {
  return [...source.matchAll(/^\s*expose\s+([a-zA-Z_][\w]*)\s*=\s*([\w]+)\.([\w]+)\s*$/gm)].map(([, name, node, property]) => {
    const line = source.match(new RegExp(`^\\s*${node}\\s*=\\s*[^\\n]*\\b${property}\\s*=\\s*`, 'm'));
    if (!line) return { name };
    const start = line.index + line[0].length;
    let depth = 0, quote = '', end = start;
    for (; end < source.length && source[end] !== '\n'; end++) {
      const char = source[end];
      if (quote) { if (char === quote && source[end - 1] !== '\\') quote = ''; continue; }
      if (char === '"' || char === "'") { quote = char; continue; }
      if (char === '[' || char === '{' || char === '(') depth++;
      if (char === ']' || char === '}' || char === ')') { if (depth === 0) break; depth--; }
      if (char === ',' && depth === 0) break;
    }
    const value = source.slice(start, end).trim();
    return { name, value: value && /^[\[\]{}"'\w.+, -]+$/.test(value) ? value : undefined };
  });
}

function parseRecipe(text, name) {
  const source = String(text ?? '').trim();
  const groups = [...source.matchAll(/^\s*group\s+([a-zA-Z_][\w]*)\s*$/gm)];
  const lines = source.split(/\r?\n/).map(line => line.trim());
  if (groups.length !== 1 || !/^group\s+[a-zA-Z_]\w*$/.test(lines[0] ?? '') || lines.at(-1) !== 'end') {
    throw new Error('text must contain exactly one complete `group NAME ... end`. Remove surrounding nodes and add the group end line.');
  }
  if (groups[0][1] !== name) throw new Error(`group name "${groups[0][1]}" must match recipe name "${name}". Rename the group or save it under that name.`);
  return source;
}

function recipePath(name) { return join(recipeDir(), `${name}.txt`); }
function readRecipe(name) {
  try { const raw = readFileSync(recipePath(name), 'utf8'); return { name, about: commentAbout(raw), text: sourceOnly(raw) }; }
  catch { return null; }
}

export function callRecipes(args, kits) {
  const action = args?.action;
  if (!['list', 'save', 'show', 'delete'].includes(action)) throw new Error('action must be list, save, show, or delete. Choose one of those actions.');
  const dir = recipeDir();
  const files = () => { try { return readdirSync(dir).filter(file => file.endsWith('.txt')).sort(); } catch { return []; } };
  if (action === 'list') return { recipes: files().map(file => readRecipe(file.slice(0, -4))).filter(Boolean).map(recipe => ({ name: recipe.name, about: recipe.about, params: exposed(recipe.text).map(p => p.name) })) };
  const name = args?.name;
  if (typeof name !== 'string' || !recipePattern.test(name)) throw new Error('name must use 1–32 lowercase letters, digits, or underscores. Rename it and try again.');
  if (action === 'save') {
    if (kits.includes(name)) throw new Error(`"${name}" is a built-in kit. Choose a different recipe name.`);
    const text = parseRecipe(args.text, name);
    mkdirSync(dir, { recursive: true });
    writeFileSync(recipePath(name), `# about: ${String(args.about ?? '').replace(/[\r\n]+/g, ' ').trim()}\n${text}\n`);
    return { saved: name };
  }
  const recipe = readRecipe(name);
  if (!recipe) throw new Error(`No recipe named "${name}". Call recipes(action="list") or save it first.`);
  if (action === 'show') {
    const params = exposed(recipe.text);
    return { name, about: recipe.about, source: recipe.text, call: `${name}(${params.map(p => p.value === undefined ? p.name : `${p.name}=${p.value}`).join(', ')})` };
  }
  if (action === 'delete') { unlinkSync(recipePath(name)); return { deleted: name }; }
  throw new Error(`unknown action ${action}`);
}
