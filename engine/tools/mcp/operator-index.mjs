// Compact views of the operator list for a model. The full list is every operator with every parameter's documentation
// (about 180 KB); a model rarely needs all of it. These functions give it a one-line-per-operator index (about a tenth
// of the size), a filtered view, and the full spec of the few operators it asks for: progressive disclosure.

const sentence = /^(.+?[.!?])(\s|$)/s;

/** The first sentence of `doc`, cut at `max` characters with an ellipsis. */
export function shortDoc(doc, max = 150) {
  const text = String(doc ?? '').replace(/\s+/g, ' ').trim();
  const first = sentence.exec(text)?.[1] ?? text;
  return first.length <= max ? first : `${first.slice(0, max - 1).trimEnd()}…`;
}

const number = n => (Math.abs(n) >= 1e5 ? n.toExponential(0).replace('+', '') : String(n));
const unitMark = { length: 'm', angle: 'deg', none: '' };
// Angles are stored in radians but written in degrees in the text language, so the index shows degrees.
const degrees = radians => Math.round((radians * 180 * 100) / Math.PI) / 100;

/** One parameter in a few characters: `radius:num 0..1e6 m`, `segments:int 6..24=12`, `mode:a|b|c=a`, `size:vec3 m`. */
export function paramText(param) {
  const ty = param.ty ?? {};
  const angle = ty.unit === 'angle';
  let kind;
  switch (ty.type) {
    case 'num': kind = angle
      ? `num ${degrees(ty.min)}..${degrees(ty.max)} deg`
      : `num ${number(ty.min)}..${number(ty.max)} ${unitMark[ty.unit] ?? ty.unit ?? ''}`.trim(); break;
    case 'int': kind = `int ${ty.min}..${ty.max}`; break;
    case 'bool': kind = 'bool'; break;
    case 'str': kind = 'text'; break;
    case 'choice': kind = (ty.options ?? []).join('|'); break;
    case 'vec2': case 'vec3': kind = `${ty.type} ${unitMark[ty.unit] ?? ''}`.trim(); break;
    default: kind = ty.type ?? '?';
  }
  const given = angle && typeof param.default === 'number' ? degrees(param.default) : param.default;
  const fallback = given === null || given === undefined ? '' : `=${JSON.stringify(given).replace(/"/g, '')}`;
  return `${param.name}:${kind}${fallback}`;
}

/** The input ports: `in`, `surface`, optional ones as `name?`, ones that take several links as `name...`. */
export function portsText(op) {
  return (op.inputs ?? []).map(port => {
    const mark = port.arity?.kind === 'optional' ? '?' : port.arity?.kind === 'variadic' ? '...' : '';
    return `${port.name}${mark}`;
  }).join(', ');
}

/** One operator on one line: id, inputs, output type, parameters, then its first sentence. */
export function indexLine(op) {
  const output = op.output?.kind === 'fixed' ? op.output.port : `same as ${op.output?.port ?? 'input'}`;
  const params = (op.params ?? []).map(paramText).join(', ');
  return `${op.id}(${portsText(op)}) -> ${output}${params ? ` | ${params}` : ''} | ${shortDoc(op.doc)}`;
}

/** The family of an operator id: `solid` for `solid.box@1`. */
export const familyOf = id => String(id).split('.')[0];

/** Operators matching `query` (a word in the id or the documentation) and `family`; both optional. */
export function select(ops, { query, family } = {}) {
  const words = String(query ?? '').toLowerCase().split(/\s+/).filter(Boolean);
  return ops.filter(op => {
    if (family && familyOf(op.id) !== family) return false;
    const hay = `${op.id} ${op.doc ?? ''}`.toLowerCase();
    return words.every(word => hay.includes(word));
  });
}

/** The index text: a header with the counts, then the operators grouped by family, one line each; kits last. */
export function indexText(ops, filters = {}) {
  const kits = ops.filter(op => op.category === 'kit');
  const operators = select(ops.filter(op => op.category !== 'kit'), filters);
  const shownKits = select(kits, filters);
  const families = new Map();
  for (const op of operators) families.set(familyOf(op.id), [...(families.get(familyOf(op.id)) ?? []), op]);
  const lines = [
    `${operators.length} operators in ${families.size} families${shownKits.length ? ` and ${shownKits.length} kits` : ''}.`
    + ' Each line: id(inputs; ? optional, ... several) -> output | parameters (type range unit =default) | what it does.'
    + ' Ask for full documentation of any with ids=["solid.box@1", ...]; ask for a kit\'s source with ids=["kit.door"].',
  ];
  for (const [family, members] of families) {
    lines.push('', `# ${family}`, ...members.map(indexLine));
  }
  if (shownKits.length) {
    lines.push('', '# kits (ready-made groups: paste the source once, then call name(color=[r, g, b]); ids=["kit.name"] returns the source)');
    lines.push(...shownKits.map(kit => `${kit.id} | ${shortDoc(kit.doc, 200)}`));
  }
  return lines.join('\n');
}

/** The full specs of the named operators and kits, and the ids that matched nothing. */
export function fullSpecs(ops, ids) {
  const wanted = new Set(ids);
  const found = ops.filter(op => wanted.has(op.id));
  const missing = ids.filter(id => !found.some(op => op.id === id));
  return { found, missing };
}
