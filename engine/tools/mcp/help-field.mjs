// help_field: the exact documentation of one operator parameter (or one operator, or one family) by a dotted path, in a few lines.
// A model that wants to know what `scale_end` does on a sweep should not have to read a 180 KB operator list or even a whole operator.
//
//   solid.sweep.scale_end    one parameter: type, range, unit, default, options and the full documentation
//   solid.sweep@1.scale_end  the same (the version is optional)
//   solid.sweep              the operator: inputs, output, what it does, every parameter on one line
//   solid                    the family: one line per operator
//   scale_end                a bare parameter name: every operator that has one
//
// A path that matches nothing answers with the nearest real names, so a typo or a guessed name leads to the right one.
import { familyOf, paramText, portsText, shortDoc } from './operator-index.mjs';

const base = id => String(id).split('@')[0];

/** Edit distance between two short strings. */
export function distance(a, b) {
  const row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let previous = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const saved = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, previous + (a[i - 1] === b[j - 1] ? 0 : 1));
      previous = saved;
    }
  }
  return row[b.length];
}

const number = n => (Math.abs(n) >= 1e5 ? n.toExponential(0).replace('+', '') : String(n));

/** One parameter in full: its line, then what the type allows, the default and the whole documentation. */
export function paramBlock(op, param) {
  const ty = param.ty ?? {};
  const lines = [`${base(op.id)}.${param.name}  (${op.id})`, `  ${paramText(param)}`];
  if (ty.type === 'choice') lines.push(`  one of: ${(ty.options ?? []).join(', ')}`);
  if ((ty.type === 'num' || ty.type === 'int') && ty.min !== undefined) {
    if (ty.unit === 'angle') {
      // The engine stores angles in radians but the text language writes them in degrees (a plain number is degrees).
      const degrees = radians => Math.round((radians * 180 * 100) / Math.PI) / 100;
      lines.push(`  range ${degrees(ty.min)} to ${degrees(ty.max)} degrees (write a plain number or a deg suffix; the text below may say radians, which is how it is stored)`);
    } else {
      lines.push(`  range ${number(ty.min)} to ${number(ty.max)} (${ty.unit && ty.unit !== 'none' ? ty.unit : 'no unit'})`);
    }
  }
  if (param.default === null || param.default === undefined) lines.push('  required: no default');
  lines.push(`  ${String(param.doc ?? '').replace(/\s+/g, ' ').trim() || 'no documentation'}`);
  return lines.join('\n');
}

function operatorBlock(op) {
  const output = op.output?.kind === 'fixed' ? op.output.port : `same as ${op.output?.port ?? 'input'}`;
  const lines = [
    `${op.id}(${portsText(op)}) -> ${output}`,
    `  ${String(op.doc ?? '').replace(/\s+/g, ' ').trim()}`,
    '  parameters (ask help_field "<operator>.<name>" for one in full):',
    ...(op.params ?? []).map(param => `    ${paramText(param)} | ${shortDoc(param.doc, 110)}`),
  ];
  if (!(op.params ?? []).length) lines.pop(), lines.pop();
  return lines.join('\n');
}

/** The nearest real paths to a wrong one, best first: a path that shares the last word and the family beats one that only looks alike. */
export function nearest(ops, wrong, limit = 5) {
  const clean = wrong.replace(/@\d+/g, '');
  const leaf = clean.split('.').pop();
  const family = familyOf(clean);
  const names = [];
  for (const op of ops) {
    names.push(base(op.id));
    for (const param of op.params ?? []) names.push(`${base(op.id)}.${param.name}`);
  }
  return [...new Set(names)]
    .map(name => {
      const nameLeaf = name.split('.').pop();
      const score = Math.min(distance(name, clean), distance(nameLeaf, leaf) + 1) + (familyOf(name) === family ? 0 : 2) + (nameLeaf === leaf ? -2 : 0);
      return { name, score };
    })
    .sort((a, b) => a.score - b.score || a.name.localeCompare(b.name))
    .slice(0, limit)
    .map(entry => entry.name);
}

/** Answers a path: `{ ok, text }`. `ops` is the operator list the engine reports (`list_operators`). */
export function helpField(ops, rawPath) {
  const path = String(rawPath ?? '').trim().replace(/@\d+/g, '').replace(/\s+/g, '');
  if (!path) return { ok: false, text: 'give a path: "solid.sweep.scale_end" (one parameter), "solid.sweep" (one operator), "solid" (a family) or just a parameter name' };
  const operator = ops.find(op => base(op.id) === path);
  if (operator) return { ok: true, text: operatorBlock(operator) };
  const owner = ops.find(op => path.startsWith(`${base(op.id)}.`));
  if (owner) {
    const name = path.slice(base(owner.id).length + 1);
    const param = (owner.params ?? []).find(p => p.name === name);
    if (param) return { ok: true, text: paramBlock(owner, param) };
    const close = (owner.params ?? []).map(p => p.name).sort((a, b) => distance(a, name) - distance(b, name)).slice(0, 5);
    return { ok: false, text: `${base(owner.id)} has no parameter "${name}". Its parameters: ${(owner.params ?? []).map(p => p.name).join(', ') || 'none'}.${close.length ? ` Nearest: ${close.join(', ')}.` : ''}` };
  }
  const members = ops.filter(op => familyOf(op.id) === path);
  if (members.length) {
    return { ok: true, text: [`${path}: ${members.length} operators (help_field "<operator>" for one)`, ...members.map(op => `${op.id} | ${shortDoc(op.doc, 110)}`)].join('\n') };
  }
  if (!path.includes('.')) {
    const having = ops.filter(op => (op.params ?? []).some(p => p.name === path));
    if (having.length) {
      return { ok: true, text: [`"${path}" is a parameter of ${having.length} operators:`, ...having.slice(0, 40).map(op => `${base(op.id)}.${path} | ${paramText(op.params.find(p => p.name === path))}`), having.length > 40 ? `... and ${having.length - 40} more` : ''].filter(Boolean).join('\n') };
    }
  }
  return { ok: false, text: `nothing is called "${path}". Nearest: ${nearest(ops, path).join(', ')}. Paths look like solid.sweep.scale_end; list_operators shows every operator.` };
}
