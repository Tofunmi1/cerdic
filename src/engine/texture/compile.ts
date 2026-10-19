// Compiles a texture graph (Material Maker nodes, MIT) into one GLSL ES 3.00 fragment shader.
//
// Each node's `shader_model` is a template. `$name` and `$(name)` stand for the node's parameters, its inputs
// (`$in(uv)` evaluates whatever feeds `in` at `uv`, or the input's default expression when nothing does),
// `$uv` (the coordinate being evaluated), `$(name)` (a node-unique name) and `$(name_uv)` (a name unique to the
// node and the coordinate). Variables are replaced from the last `$` to the first, so a call's arguments are
// resolved before the call, as Material Maker does it (see `gen_shader.gd` in the vendored tree).
//
// Subgraphs, generic nodes, buffers and switches are expanded first (`flatten.ts`). Inputs flagged `function`
// become GLSL functions. Not supported (a clear error): attributes such as `.texture`, buffers that need a
// render pass, iteration, polygons, splines, pixel and lattice parameters.
import { expandExpressions } from './expr/node';
import { flatten, type FlatNode, type FlatRef } from './flatten';
import type { InputDef, Library, TexGraph } from './types';

const WORD = /[A-Za-z0-9_?]/;

/** The pixel type of each Material Maker value type, and how a value converts to the others (`io_types.mmt`). */
const TYPES: Record<string, { glsl: string; params: string; arg: string; convert?: Record<string, string> }> = {
  f: { glsl: 'float', params: 'vec2 uv', arg: 'uv', convert: { rgb: 'vec3($(value))', rgba: 'vec4(vec3($(value)), 1.0)' } },
  rgb: { glsl: 'vec3', params: 'vec2 uv', arg: 'uv', convert: { f: '(dot($(value), vec3(1.0))/3.0)', rgba: 'vec4($(value), 1.0)' } },
  rgba: { glsl: 'vec4', params: 'vec2 uv', arg: 'uv', convert: { f: '(dot(($(value)).rgb, vec3(1.0))/3.0)', rgb: '(($(value)).rgb)' } },
  sdf2d: { glsl: 'float', params: 'vec2 uv', arg: 'uv' },
  sdf3d: { glsl: 'float', params: 'vec3 p', arg: 'p', convert: { sdf3dc: 'vec2($(value), 0.0)' } },
  sdf3dc: { glsl: 'vec2', params: 'vec3 p', arg: 'p', convert: { sdf3d: '($(value)).x' } },
  tex3d_gs: { glsl: 'float', params: 'vec4 p', arg: 'p', convert: { tex3d: 'vec3($(value))' } },
  tex3d: { glsl: 'vec3', params: 'vec4 p', arg: 'p', convert: { tex3d_gs: '(dot($(value), vec3(1.0))/3.0)' } },
  v4v4: { glsl: 'vec4', params: 'vec4 p', arg: 'p' },
  fill: { glsl: 'vec4', params: 'vec2 uv', arg: 'uv' },
};

/** The picture an output type draws as: an expression over its value (`$(value)`) giving an RGBA colour. */
const PREVIEW: Record<string, string> = {
  f: 'vec4(vec3($(value)), 1.0)',
  rgb: 'vec4($(value), 1.0)',
  rgba: '$(value)',
  sdf2d: 'vec4(vec3($(value)), 1.0)',
  tex3d_gs: 'vec4(vec3($(value)), 1.0)',
  tex3d: 'vec4($(value), 1.0)',
  sdf3d: 'vec4(vec3($(value)), 1.0)',
  sdf3dc: 'vec4(vec3(($(value)).x), 1.0)',
};

const lines = (value: string[] | string | undefined) => (Array.isArray(value) ? value.join('\n') : value ?? '');
/** A GLSL float literal in parentheses (so a negative value is safe inside any expression). */
const float = (n: number) => `(${Number.isInteger(n) ? n.toFixed(1) : String(Number(n.toPrecision(9)))})`;
const f9 = (n: number) => n.toFixed(9);
/** A GLSL identifier for a node: no run of underscores (`__` is reserved in GLSL). */
const identifier = (id: string) => `o${id.replace(/\W/g, '_').replace(/_+/g, '_')}`;

/** Splits a call's arguments at the commas that are not inside parentheses. */
function splitArguments(text: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '(') depth++;
    else if (text[i] === ')') depth--;
    else if (text[i] === ',' && depth === 0) {
      out.push(text.slice(start, i).trim());
      start = i + 1;
    }
  }
  out.push(text.slice(start).trim());
  return out;
}

/** Godot's shader language lets an int literal stand where a float goes, GLSL ES does not: `max(0, x)` -> `max(0.0, x)`. */
function floatLiterals(source: string): string {
  const call = /\b(?:clamp|max|min|pow|mix|mod|step|smoothstep)\s*\(/g;
  const isInt = (a: string) => /^\d+$/.test(a);
  let out = source;
  let from = 0;
  for (;;) {
    call.lastIndex = from;
    const m = call.exec(out);
    if (!m) return out;
    const open = m.index + m[0].length - 1;
    from = open + 1;
    const close = matching(out, open);
    if (close === -1) continue;
    const args = splitArguments(out.slice(open + 1, close));
    // A call that mixes in an int-typed argument is an int overload: leave it.
    if (!args.some(isInt) || args.some((a) => /\bint\s*\(|\bivec\d|\buint\s*\(/.test(a))) continue;
    out = `${out.slice(0, open + 1)}${args.map((a) => (isInt(a) ? `${a}.0` : a)).join(', ')}${out.slice(close)}`;
  }
}

/** Converts integer literals only where a local GLSL function explicitly declares a float parameter. */
function typedFunctionLiterals(source: string): string {
  const signature = /\b(?:float|vec[234]|int|ivec[234]|bool)\s+(\w+)\s*\(([^)]*)\)/g;
  const floatParameters = new Map<string, boolean[]>();
  for (const match of source.matchAll(signature)) {
    floatParameters.set(match[1], match[2].split(',').map(parameter => /^\s*float\b/.test(parameter)));
  }
  const call = /\b(\w+)\s*\(/g;
  const replacements: { start: number; end: number; text: string }[] = [];
  for (const match of source.matchAll(call)) {
    const parameters = floatParameters.get(match[1]);
    if (!parameters) continue;
    const open = match.index! + match[0].lastIndexOf('(');
    const close = matching(source, open);
    if (close < 0) continue;
    const args = splitArguments(source.slice(open + 1, close));
    let changed = false;
    const next = args.map((argument, index) => {
      if (parameters[index] && /^\s*[+-]?\d+\s*$/.test(argument)) { changed = true; return `${argument.trim()}.0`; }
      return argument;
    });
    if (changed) replacements.push({ start: open + 1, end: close, text: next.join(', ') });
  }
  let out = source;
  for (const item of replacements.reverse()) out = out.slice(0, item.start) + item.text + out.slice(item.end);
  return out;
}

/** Index of the parenthesis closing the one at `i`, or -1. */
export function matching(text: string, i: number): number {
  let depth = 0;
  for (let k = i; k < text.length; k++) {
    if (text[k] === '(') depth++;
    else if (text[k] === ')' && --depth === 0) return k;
  }
  return -1;
}

type Value = string | ((suffix: string, params: string) => string);
type Code = { code: string; values: Record<string, string>; type: string };
/** One scope of generated statements: the main function, or the body of one generated function. */
type Scope = { seen: Map<string, number>; counts: Map<string, number>; hoisted: string };
const newScope = (): Scope => ({ seen: new Map(), counts: new Map(), hoisted: '' });

/** Material Maker's `get_variant`: -1 the first time a key is seen for a node, its index after that. */
function variant(s: Scope, node: string, key: string): number {
  const id = `${node}|${key}`;
  const known = s.seen.get(id);
  if (known !== undefined) return known;
  const count = s.counts.get(node) ?? 0;
  s.counts.set(node, count + 1);
  s.seen.set(id, count);
  return -1;
}

/** A GLSL ES 3.00 array initialiser: `float a[3] = {1.0, 2.0, 3.0};` becomes `= float[](1.0, 2.0, 3.0);`. */
function arrayInitialisers(source: string): string {
  return source.replace(/\b(\w+)\s+(\w+)\s*\[([^\]]*)\]\s*=\s*\{([^}]*)\}/g, '$1 $2[$3] = $1[]($4)');
}

export type Compiled = {
  source: string;
  /** `3d` shaders read an object-space position map (`uPosition`, one position per texel) instead of the UV. */
  space: '2d' | '3d';
  /** Which maps of the mesh the shader reads: the position map (`uPosition`) and the surface signal map (`uSurface`). */
  needs: { position: boolean; surface: boolean };
};

class Compiler {
  private globals = new Map<string, string>();
  private defs: string[] = [];
  private declared = new Set<string>();

  constructor(private lib: Library, private nodes: Map<string, FlatNode>) {}

  private node(id: string): FlatNode {
    const node = this.nodes.get(id);
    if (!node) throw new Error(`flattened graph has no node "${id}"`);
    return node;
  }

  /** Evaluates output `ref.output` of node `ref.node` at the coordinate expression `uv`. */
  code(s: Scope, ref: FlatRef, uv: string): Code {
    const f = this.node(ref.node);
    const output = f.model.outputs?.[ref.output];
    if (!output) throw new Error(`node "${f.type}" has no output ${ref.output}`);
    const name = identifier(f.id);
    const vars: Record<string, Value> = { uv, time: '0.0', name, seed: float(f.seed), node_id: '0', '?1': '0.0', '?2': '0.0', '?3': '0.0', '?4': '0.0' };
    for (const [key, value] of Object.entries(f.env)) if (typeof value === 'number') vars[key] = float(value);
    const declare = !this.declared.has(f.id);
    this.declared.add(f.id);
    this.parameters(f, vars, name, declare);
    for (const input of f.model.inputs ?? []) vars[input.name] = (suffix, params) => this.input(s, f, input, suffix, params, vars);
    if (declare) this.declarations(f, vars, name);
    const type = output.type ?? '';
    const expression = String(output[type] ?? '');
    // Statements go to the scope in the order Material Maker appends them: the inputs of the node's code,
    // the code, the inputs of the output expression, the output assignment.
    if (f.model.code && !expression.includes('@NOCODE')) {
      const first = variant(s, f.id, uv) === -1;
      vars.name_uv = `${name}_${variant(s, f.id, uv)}`;
      if (first) {
        const code = this.replace(lines(f.model.code), vars);
        s.hoisted += `${code}\n`;
      }
    }
    const key = `${uv},${ref.output}`;
    let index = variant(s, f.id, key);
    const assign = index === -1;
    if (assign) index = variant(s, f.id, key);
    const t = TYPES[type];
    if (!t) throw new Error(`node "${f.type}" has output type "${type}", which this compiler does not support`);
    const variable = `${name}_${ref.output}_${index}_${type}`;
    if (assign) {
      const text = expression.replace('@NOCODE', '').replace('@KEEPTYPE', '').trim();
      const value = this.replace(text, vars);
      s.hoisted += `${t.glsl} ${variable} = ${value};\n`;
    }
    const values: Record<string, string> = { [type]: variable };
    for (const [target, template] of Object.entries(t.convert ?? {})) values[target] = template.replace('$(value)', variable);
    return { code: '', values, type };
  }

  /** Globals, includes, input functions and instance functions: once per node. */
  private declarations(f: FlatNode, vars: Record<string, Value>, name: string) {
    for (const include of f.model.includes ?? []) {
      const global = lines(this.lib[include]?.shader_model.global);
      if (global) this.globals.set(`include:${include}`, global);
    }
    const global = lines(f.model.global);
    if (global.trim()) this.globals.set(`global:${global}`, global);
    for (const input of f.model.inputs ?? []) {
      const link = f.inputs[input.name];
      if (!input.function || !link) continue;
      const t = TYPES[input.type];
      const local = newScope();
      const body = this.code(local, link, t.arg);
      const value = body.values[input.type];
      if (value === undefined) throw new Error(`cannot convert ${body.type} to ${input.type} for input ${input.name} of ${f.type}`);
      this.defs.push(`${t.glsl} ${name}_input_${input.name}(${t.params}, float _seed_variation_, vec4 _controlled_variation_) {\n${local.hoisted}${body.code}return ${value};\n}\n`);
    }
    const instance = lines(f.model.instance);
    if (instance.trim()) this.defs.push(`// instance: ${f.type}\n${this.replace(instance, vars)}\n`);
  }

  private parameters(f: FlatNode, vars: Record<string, Value>, name: string, declare: boolean) {
    let offset = 0;
    for (const def of f.model.parameters ?? []) {
      const value = f.params[def.name] ?? def.default;
      switch (def.type) {
        case 'float':
          vars[def.name] = typeof value === 'number' ? float(value) : `(${this.random(String(value), f.seed, offset)})`;
          offset += 17;
          break;
        case 'size':
          vars[def.name] = (2 ** Number(value)).toFixed(1);
          break;
        case 'boolean':
          vars[def.name] = value ? 'true' : 'false';
          break;
        case 'enum': {
          const index = typeof value === 'number' && value >= 0 && value < (def.values?.length ?? 0) ? value : 0;
          vars[def.name] = def.values?.[index]?.value ?? '';
          break;
        }
        case 'color': {
          const c = value as { r: number; g: number; b: number; a?: number };
          vars[def.name] = `vec4(${[c.r, c.g, c.b, c.a ?? 1].map(f9).join(', ')})`;
          break;
        }
        case 'gradient':
          vars[def.name] = this.gradient(`${name}_${def.name}`, value as Record<string, unknown>, declare);
          break;
        case 'curve':
          vars[def.name] = this.curve(`${name}_${def.name}`, value as Record<string, unknown>, declare);
          break;
        default:
          throw new Error(`parameter ${def.name} of ${f.type} has type ${def.type}, which this compiler does not support`);
      }
    }
  }

  /** `$rnd(min, max)` and `$rndi(min, max)` in a parameter expression: a value drawn from the node's seed. */
  private random(text: string, seed: number, offset: number): string {
    let out = text;
    for (const fn of ['rndi', 'rnd']) {
      for (;;) {
        const at = out.indexOf(`$${fn}(`);
        if (at === -1) break;
        const close = matching(out, at + fn.length + 1);
        const args = out.slice(at + fn.length + 2, close);
        const shift = fn === 'rnd' ? Math.sin(at + offset) * 0.5 + 0.5 : Math.sin(at) + offset;
        out = `${out.slice(0, at)}param_${fn}(${args}, ${float(seed)}+${float(shift)})${out.slice(close + 1)}`;
      }
    }
    return out;
  }

  private gradient(name: string, value: Record<string, unknown>, declare: boolean): string {
    if (!declare) return `${name}_gradient_fct`;
    const points = [...((value.points as { pos: number; r: number; g: number; b: number; a: number }[]) ?? [])].sort((a, b) => a.pos - b.pos);
    const interpolation = Number(value.interpolation ?? 1); // 0 constant, 1 linear, 2 smoothstep, 3 cubic (drawn as smoothstep)
    const colour = (i: number) => `vec4(${f9(points[i].r)},${f9(points[i].g)},${f9(points[i].b)},${f9(points[i].a)})`;
    let body = '  return vec4(0.0, 0.0, 0.0, 1.0);\n';
    if (points.length > 0) {
      body = `  if (x < ${f9(points[0].pos)}) return ${colour(0)};\n`;
      for (let i = 0; i < points.length - 1; i++) {
        const t = `(x-${f9(points[i].pos)})/(${f9(points[i + 1].pos)}-${f9(points[i].pos)})`;
        const blend = interpolation === 1 ? t : `(0.5-0.5*cos(3.14159265359*${t}))`;
        body += interpolation === 0
          ? `  if (x < ${f9(points[i + 1].pos)}) return ${colour(i)};\n`
          : `  if (x < ${f9(points[i + 1].pos)}) return mix(${colour(i)}, ${colour(i + 1)}, ${blend});\n`;
      }
      body += `  return ${colour(points.length - 1)};\n`;
    }
    this.defs.push(`vec4 ${name}_gradient_fct(float x) {\n${body}}\n`);
    return `${name}_gradient_fct`;
  }

  private curve(name: string, value: Record<string, unknown>, declare: boolean): string {
    if (!declare) return `${name}_curve_fct`;
    const points = (value.points as { x: number; y: number; ls: number; rs: number }[]) ?? [];
    let body = '';
    for (let i = 0; i < points.length - 1; i++) {
      const a = points[i];
      const b = points[i + 1];
      const guard = i < points.length - 2 ? `if (x <= ${f9(b.x)}) ` : '';
      body += `${guard}{ float d = ${f9(b.x - a.x)}; float t = (x-${f9(a.x)})/d; float omt = 1.0-t; d /= 3.0;\n`
        + `  float yac = ${f9(a.y)} + d*${f9(a.rs)}; float ybc = ${f9(b.y)} - d*${f9(b.ls)};\n`
        + `  return ${f9(a.y)}*omt*omt*omt + yac*omt*omt*t*3.0 + ybc*omt*t*t*3.0 + ${f9(b.y)}*t*t*t; }\n`;
    }
    this.defs.push(`float ${name}_curve_fct(float x) {\n${body}  return ${points.length ? f9(points[points.length - 1].y) : '0.0'};\n}\n`);
    return `${name}_curve_fct`;
  }

  /** What `$input(params)` becomes: the input's default, a call to its generated function, or its inlined value. */
  private input(s: Scope, f: FlatNode, input: InputDef, suffix: string, params: string, vars: Record<string, Value>): string {
    if (suffix && suffix !== 'variation') throw new Error(`input attribute ".${suffix}" of ${f.type} is not supported`);
    const link = f.inputs[input.name];
    // `params` keeps its parentheses, `(uv)`. A `.variation(uv, seed)` call carries the variation as a second argument.
    const list = suffix === 'variation' ? splitArguments(params.slice(1, -1)) : [params.slice(1, -1)];
    const coordinate = suffix === 'variation' ? `(${list[0]})` : params;
    if (!link) {
      const saved = vars.uv;
      vars.uv = coordinate;
      let text = this.replace(input.default, vars);
      vars.uv = saved;
      // Some sdf3d inputs are declared with a vec2 default, a value of the sdf3dc type.
      if (input.type === 'sdf3d' && text.trim().startsWith('vec2')) text = `(${text}).x`;
      return text;
    }
    if (input.function) {
      const tail = suffix === 'variation' && list.length === 2 ? ', _controlled_variation_' : suffix === 'variation' && list.length > 2 ? '' : ', _seed_variation_, _controlled_variation_';
      return `${identifier(f.id)}_input_${input.name}(${list.join(', ')}${tail})`;
    }
    const source = this.code(s, link, coordinate);
    const value = source.values[input.type];
    if (value === undefined) throw new Error(`cannot convert ${source.type} to ${input.type} for input ${input.name} of ${f.type}`);
    return value;
  }

  /** Replaces `$variables`, last to first. */
  replace(template: string, vars: Record<string, Value>): string {
    let text = template;
    for (;;) {
      const dollar = text.lastIndexOf('$');
      if (dollar === -1) return text;
      let end: number;
      let variable: string;
      if (text[dollar + 1] === '(') {
        end = matching(text, dollar + 1) + 1;
        variable = text.slice(dollar + 2, end - 1);
      } else {
        end = dollar + 1;
        while (end < text.length && WORD.test(text[end])) end++;
        variable = text.slice(dollar + 1, end);
      }
      const head = text.slice(0, dollar);
      let tail = text.slice(end);
      const value = vars[variable];
      if (value === undefined) throw new Error(`template variable $${variable} is not defined`);
      if (typeof value === 'string') {
        text = head + value + tail;
        continue;
      }
      let suffix = '';
      const trimmed = tail.trimStart();
      if (trimmed[0] === '.') {
        const rest = trimmed.slice(1).trimStart();
        let k = 0;
        while (k < rest.length && WORD.test(rest[k])) k++;
        suffix = rest.slice(0, k);
        tail = rest.slice(k);
      }
      const call = tail.trimStart();
      let params = '';
      if (call[0] === '(') {
        const close = matching(call, 0);
        params = call.slice(0, close + 1);
        if (params[1] === '(' && matching(params, 1) === params.length - 2) params = params.slice(1, -1);
        tail = call.slice(close + 1);
      }
      text = head + value(suffix, params) + tail;
    }
  }

  assemble(functions: string, root: FlatRef): Compiled {
    const rootType = this.node(root.node).model.outputs?.[root.output]?.type ?? '';
    const preview = PREVIEW[rootType];
    if (!preview) throw new Error(`output type ${rootType} cannot be drawn`);
    const is3d = TYPES[rootType].arg === 'p';
    const position = TYPES[rootType].params.startsWith('vec4') ? 'vec4(texture(uPosition, vUv).xyz, 0.0)' : 'texture(uPosition, vUv).xyz';
    const main = newScope();
    const result = this.code(main, root, is3d ? 'p' : 'uv');
    const colour = preview.replace('$(value)', result.values[rootType]);
    // Nodes that read the mesh (the surface nodes) name their maps in their expressions; the shader declares what is used.
    const body = [main.hoisted, result.code, ...this.defs, ...this.globals.values()].join('\n');
    const needs = { position: is3d || body.includes('uPosition'), surface: body.includes('uSurface') };
    const source = [
      '#version 300 es', 'precision highp float;', 'precision highp int;',
      // Godot built-ins the nodes use, and the variation inputs nodes take (no variation here).
      'const float PI = 3.14159265359;', 'const float TAU = 6.28318530718;',
      'float _seed_variation_ = 0.0;', 'vec4 _controlled_variation_ = vec4(0.0);', 'const float elapsed_time = 0.0;',
      functions, ...this.globals.values(), ...this.defs,
      'in vec2 vUv;', 'out vec4 fragColor;',
      needs.position ? 'uniform sampler2D uPosition;' : '',
      needs.surface ? 'uniform sampler2D uSurface;' : '',
      'void main() {',
      is3d ? `  ${TYPES[rootType].params} = ${position};` : '  vec2 uv = vUv;',
      main.hoisted, result.code, `  fragColor = ${colour};`, '}', '',
    ].join('\n');
    return { source: typedFunctionLiterals(floatLiterals(arrayInitialisers(source))), space: is3d ? '3d' : '2d', needs };
  }
}

/**
 * Compiles a graph to a GLSL ES 3.00 fragment shader. `functions` is Material Maker's shared GLSL helper code
 * (`rand`, `rgb2hsv`, ...), taken from `shader_functions.tres`. The shader reads the texture coordinate `vUv`
 * (0 to 1) and writes the output node's colour.
 */
export function compileTexture(library: Library, graph: TexGraph, functions: string): Compiled {
  // `expr` nodes (shader expressions) become ordinary nodes of an extended library, with their helper functions added once.
  const expressions = expandExpressions(library, graph);
  const flat = flatten(expressions.library, expressions.graph);
  return new Compiler(expressions.library, flat.nodes).assemble(expressions.helpers ? `${functions}\n${expressions.helpers}\n` : functions, flat.root);
}

/** The helper code from a `shader_functions.tres` file. */
export function helperFunctions(tres: string): string {
  const start = tres.indexOf('text = "');
  if (start === -1) throw new Error('shader_functions.tres has no text');
  const body = tres.slice(start + 8, tres.lastIndexOf('"'));
  return body.replace(/\\"/g, '"').replace(/\\\\/g, '\\');
}
