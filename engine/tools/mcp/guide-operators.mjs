// Operator descriptions shared by the MCP guide and its operator tools.
// ---- operators ----

/** The first sentence of a doc string, at most `limit` characters. */
export function oneLine(doc, limit = 110) {
  const text = String(doc ?? '').replace(/\s+/g, ' ').trim();
  const stop = text.search(/[.;:](\s|$)/);
  const first = stop > 0 ? text.slice(0, stop) : text;
  return first.length > limit ? `${first.slice(0, limit - 1).trimEnd()}…` : first;
}

const compactValue = (value) => {
  if (value === null || value === undefined) return '?';
  if (Array.isArray(value)) return `[${value.map(compactValue).join(', ')}]`;
  if (typeof value === 'number') return String(Number(value.toPrecision(5)));
  return typeof value === 'string' ? JSON.stringify(value) : String(value);
};

/** `solid.toon@1(in; bands=2, feather=0.08, ...)`: the inputs, then each parameter with its default (`?` when it is required). */
export function signature(op) {
  const inputs = (op.inputs ?? []).map(port => `${port.name}${port.arity?.kind === 'variadic' ? '...' : ''}`).join(', ');
  const params = (op.params ?? []).map(p => `${p.name}=${compactValue(p.default)}`).join(', ');
  return `${op.id}(${inputs}${params ? `; ${params}` : ''})`;
}

const outputOf = (op) => (op.output?.kind === 'same_as_input' ? 'same as input' : op.output?.port ?? '?');

const family = (op) => op.id.split('.')[0];

/** No family: every family with the names in it. A family: each operator's signature and one line of help. */
export function operatorIndex(operators, { family: wanted, query } = {}) {
  const ready = operators.filter(op => op.readiness === 'ready');
  if (!wanted && !query) {
    const groups = new Map();
    for (const op of ready) groups.set(family(op), [...(groups.get(family(op)) ?? []), op.id]);
    const lines = [...groups].map(([name, ids]) => `${name} (${ids.length}): ${ids.map(id => id.slice(name.length + 1)).join(', ')}`);
    return `${ready.length} operators in ${groups.size} families (names are family.name@1). list_operators(family="solid") gives signatures and one-line help; describe_operator(id) gives every parameter.\n\n${lines.join('\n')}`;
  }
  const needle = query?.toLowerCase();
  const chosen = ready.filter(op => (!wanted || family(op) === wanted) && (!needle || `${op.id} ${op.doc}`.toLowerCase().includes(needle)));
  if (chosen.length === 0) return `no operators match${wanted ? ` family "${wanted}"` : ''}${query ? ` query "${query}"` : ''}. Families: ${[...new Set(ready.map(family))].join(', ')}`;
  return chosen.map(op => `${signature(op)} -> ${outputOf(op)}: ${oneLine(op.doc)}`).join('\n');
}

const describeType = (ty = {}) => {
  const unit = ty.unit && ty.unit !== 'none' ? ` ${ty.unit}` : '';
  const range = ty.min !== undefined && ty.max !== undefined ? ` ${ty.min}..${ty.max}` : '';
  const options = ty.options ? ` one of ${ty.options.join(' | ')}` : '';
  return `${ty.type ?? '?'}${unit}${range}${options}`;
};

/** Everything about one operator, as text: what it does, its ports, and each parameter with unit, range, default and help. */
export function describeOperator(operators, id) {
  const wanted = String(id ?? '').trim();
  const op = operators.find(o => o.id === wanted) ?? operators.find(o => o.id === `${wanted}@1`);
  if (!op) {
    const near = operators.filter(o => o.id.includes(wanted.replace(/@\d+$/, ''))).slice(0, 8).map(o => o.id);
    return null ?? `no operator "${wanted}".${near.length ? ` Did you mean: ${near.join(', ')}?` : ' Call list_operators() for the names.'}`;
  }
  const ports = (op.inputs ?? []).map(p => `${p.name}${p.arity?.kind === 'variadic' ? `... (at least ${p.arity.min ?? 1})` : ''} accepts ${p.accepts?.join(' | ')}`).join('; ') || 'none';
  const params = (op.params ?? []).map(p => `  ${p.name} (${describeType(p.ty)}) default ${compactValue(p.default)}: ${p.doc}`).join('\n') || '  none';
  return `${signature(op)} -> ${outputOf(op)}\n${op.doc}\n\ninputs: ${ports}\nparameters:\n${params}`;
}
