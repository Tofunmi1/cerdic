// Generic nodes: Material Maker nodes whose inputs, parameters or outputs carry a `#` in their name (for
// example `blend2`, with inputs `l#` and `a#`) are written once and repeated `size` times. This is the
// same expansion as `expand_generic` in the vendored `gen_shader.gd`.
import type { InputDef, OutputDef, ParamDef, ShaderModel } from './types';

const text = (value: string[] | string | undefined) => (Array.isArray(value) ? value.join('\n') : value ?? '');

/** First index and one-past-last index of the entries whose name contains `#`; -1 when there are none. */
function range<T>(items: T[], name: (item: T) => string): { first: number; last: number } {
  let first = -1;
  let last = -1;
  items.forEach((item, i) => {
    if (name(item).includes('#')) {
      if (first === -1) first = i;
      else if (last !== -1) throw new Error('generic entries must be contiguous');
    } else if (first !== -1 && last === -1) last = i;
  });
  if (first !== -1 && last === -1) last = items.length;
  return { first, last };
}

/** The entries of `items` with the generic block repeated `size` times, `#` replaced by 1..size. */
function repeat<T>(items: T[], name: (item: T) => string, size: number, copy: (item: T, n: string) => T): T[] {
  const { first, last } = range(items, name);
  if (first === -1) return items;
  const out = items.slice(0, first);
  for (let g = 1; g <= size; g++) for (let i = first; i < last; i++) out.push(copy(items[i], String(g)));
  return out.concat(items.slice(last));
}

/** Expands `#count` and `#for ... #end` blocks of a code string. */
function expandCode(code: string, size: number): string {
  let rest = code.replaceAll('#count', String(size));
  let out = '';
  while (rest !== '') {
    const start = rest.indexOf('#for');
    if (start === -1) return out + rest;
    out += rest.slice(0, start);
    rest = rest.slice(start + 4);
    const end = rest.indexOf('#end');
    const block = end === -1 ? rest : rest.slice(0, end);
    rest = end === -1 ? '' : rest.slice(end + 4);
    for (let g = 1; g <= size; g++) out += block.replaceAll('#', String(g));
  }
  return out;
}

export function isGeneric(model: ShaderModel): boolean {
  return (model.parameters ?? []).some((p) => p.name.includes('#')) || (model.inputs ?? []).some((i) => i.name.includes('#'));
}

/** The model with its generic inputs, parameters, outputs and code repeated `size` times. */
export function expandGeneric(model: ShaderModel, size: number): ShaderModel {
  if (!isGeneric(model)) return model;
  const swap = (s: string | undefined, n: string) => s?.replaceAll('#', n);
  const inputs = repeat<InputDef>(model.inputs ?? [], (i) => i.name, size, (i, n) => ({ ...i, name: i.name.replaceAll('#', n), label: swap(i.label, n) }));
  const parameters = repeat<ParamDef>(model.parameters ?? [], (p) => p.name, size, (p, n) => ({ ...p, name: p.name.replaceAll('#', n) }));
  const outputs = repeat<OutputDef>(
    model.outputs ?? [],
    (o) => String(o[o.type ?? ''] ?? ''),
    size,
    (o, n) => ({ ...o, [o.type ?? '']: String(o[o.type ?? '']).replaceAll('#', n) }),
  );
  return {
    ...model,
    inputs,
    parameters,
    outputs,
    code: model.code === undefined ? undefined : [expandCode(text(model.code), size)],
    instance: model.instance === undefined ? undefined : [expandCode(text(model.instance), size)],
  };
}
