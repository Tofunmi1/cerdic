// Throwaway spike: line-per-node geometry language, registry + parser + validator.
//   name = kind(arg, arg, key=value, ...)     inputs are node names, params are key=value
//   output name
// Types: solid (closed mesh), sdf (implicit), curve (polyline/spline), profile (2D loop), inst (instances)

const V3 = 'vec3', N = 'num', I = 'int', S = 'str', B = 'bool', V2 = 'vec2', L = 'list';

// kind: { out, inputs: [types] | {variadic: type, min}, params: {name: [type, default|REQ]} }
const REQ = Symbol('required');
export const NODES = {
  // --- solid primitives
  box:      { out: 'solid', inputs: [], params: { size: [V3, REQ] }, doc: 'axis-aligned box centred at origin, size in metres' },
  sphere:   { out: 'solid', inputs: [], params: { radius: [N, REQ], segments: [I, 24] } },
  cylinder: { out: 'solid', inputs: [], params: { radius: [N, REQ], height: [N, REQ], segments: [I, 24], axis: [S, 'y'] }, doc: 'axis x|y|z, default y (the long/ring-normal direction)' },
  cone:     { out: 'solid', inputs: [], params: { radius: [N, REQ], height: [N, REQ], top_radius: [N, 0], segments: [I, 24], axis: [S, 'y'] } },
  torus:    { out: 'solid', inputs: [], params: { major: [N, REQ], minor: [N, REQ], segments: [I, 32], axis: [S, 'y'] } },
  capsule:  { out: 'solid', inputs: [], params: { radius: [N, REQ], height: [N, REQ], axis: [S, 'y'] } },
  // --- profiles/curves
  rect:     { out: 'profile', inputs: [], params: { size: [V2, REQ], corner: [N, 0] } },
  circle:   { out: 'profile', inputs: [], params: { radius: [N, REQ], segments: [I, 24] } },
  polygon:  { out: 'profile', inputs: [], params: { points: [L, REQ] }, doc: 'list of [x,y]' },
  line:     { out: 'curve', inputs: [], params: { points: [L, REQ] }, doc: 'list of [x,y,z]' },
  spline:   { out: 'curve', inputs: [], params: { points: [L, REQ], samples: [I, 32] }, doc: 'smooth curve through points' },
  // --- profile/curve -> solid
  extrude:  { out: 'solid', inputs: ['profile'], params: { height: [N, REQ] } },
  revolve:  { out: 'solid', inputs: ['profile'], params: { angle: [N, 360], segments: [I, 32] }, doc: 'profile x>0 revolved around Y' },
  sweep:    { out: 'solid', inputs: ['profile', 'curve'], params: { twist: [N, 0], scale_end: [N, 1] } },
  loft:     { out: 'solid', inputs: { variadic: 'profile', min: 2 }, params: { spacing: [N, REQ] } },
  // --- booleans (variadic)
  union:     { out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {} },
  difference:{ out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {}, doc: 'first minus the rest' },
  intersect: { out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {} },
  // --- transforms (work on solid, sdf, inst)
  move:     { out: '=0', inputs: ['solid|sdf|inst'], params: { by: [V3, REQ] } },
  rotate:   { out: '=0', inputs: ['solid|sdf|inst'], params: { degrees: [V3, REQ] } },
  scale:    { out: '=0', inputs: ['solid|sdf|inst'], params: { by: [V3, REQ] } },
  mirror:   { out: '=0', inputs: ['solid'], params: { axis: [S, REQ] }, doc: 'axis x|y|z; keeps original and adds mirrored copy' },
  // --- repetition
  array:    { out: 'solid', inputs: ['solid'], params: { count: [V3, REQ], spacing: [V3, REQ] }, doc: 'GRID: count per axis (ints) x spacing, first copy at origin, merged into one solid' },
  repeat:   { out: 'solid', inputs: ['solid'], params: { count: [I, REQ], step: [V3, REQ], rotate_step: [V3, [0, 0, 0]] }, doc: 'copy i = original rotated by rotate_step*i degrees (about origin) then moved by step*i. Lines, stairs, spirals/helices' },
  radial:   { out: 'solid', inputs: ['solid'], params: { count: [I, REQ], radius: [N, 0] }, doc: 'copies around Y axis' },
  scatter:  { out: 'inst',  inputs: ['solid'], params: { area: [V2, REQ], count: [I, REQ], seed: [I, 1] } },
  merge:    { out: 'solid', inputs: ['inst'], params: {} },
  // --- mesh ops
  bevel:    { out: 'solid', inputs: ['solid'], params: { radius: [N, REQ] } },
  smooth:   { out: 'solid', inputs: ['solid'], params: { iterations: [I, 2] } },
  simplify: { out: 'solid', inputs: ['solid'], params: { ratio: [N, REQ] } },
  hollow:   { out: 'solid', inputs: ['solid'], params: { thickness: [N, REQ] } },
  // --- SDF family
  sd_sphere:{ out: 'sdf', inputs: [], params: { radius: [N, REQ] } },
  sd_box:   { out: 'sdf', inputs: [], params: { size: [V3, REQ], round: [N, 0] } },
  sd_capsule:{ out: 'sdf', inputs: [], params: { a: [V3, REQ], b: [V3, REQ], radius: [N, REQ] } },
  sd_union: { out: 'sdf', inputs: { variadic: 'sdf', min: 2 }, params: { smooth: [N, 0] } },
  sd_subtract:{ out: 'sdf', inputs: ['sdf', 'sdf'], params: { smooth: [N, 0] } },
  sd_noise: { out: 'sdf', inputs: ['sdf'], params: { amount: [N, REQ], frequency: [N, REQ], seed: [I, 1] } },
  sd_expr:  { out: 'sdf', inputs: [], params: { expr: [S, REQ] }, doc: 'closed-form f(x,y,z) < 0 inside; functions: sin cos abs min max sqrt length' },
  mesh_sdf: { out: 'solid', inputs: ['sdf'], params: { bounds: [V3, REQ], resolution: [I, 48] } },
  // --- parts / logic hooks
  part:     { out: '=0', inputs: ['solid'], params: { name: [S, REQ] }, doc: 'names a sub-shape so scene logic can address it' },
  socket:   { out: '=0', inputs: ['solid'], params: { name: [S, REQ], at: [V3, REQ], normal: [V3, [0, 1, 0]] }, doc: 'attach point for other objects' },
  material: { out: '=0', inputs: ['solid'], params: { slot: [S, REQ] } },
};

function typeOk(t, v) {
  switch (t) {
    case N: return typeof v === 'number';
    case I: return Number.isInteger(v);
    case S: return typeof v === 'string';
    case B: return typeof v === 'boolean';
    case V3: return Array.isArray(v) && v.length === 3 && v.every(x => typeof x === 'number');
    case V2: return Array.isArray(v) && v.length === 2 && v.every(x => typeof x === 'number');
    case L: return Array.isArray(v) && v.length >= 2 && v.every(p => Array.isArray(p) && p.every(x => typeof x === 'number'));
  }
  return false;
}

// split on top-level commas (outside [] and quotes)
function splitArgs(s) {
  const out = []; let depth = 0, q = null, cur = '';
  for (const ch of s) {
    if (q) { cur += ch; if (ch === q) q = null; continue; }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (ch === '[') depth++;
    if (ch === ']') depth--;
    if (ch === ',' && depth === 0) { out.push(cur.trim()); cur = ''; continue; }
    cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

function parseValue(s) {
  s = s.trim();
  if (/^["'].*["']$/.test(s)) return s.slice(1, -1);
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (/^-?\d+(\.\d+)?([eE]-?\d+)?$/.test(s)) return Number(s);
  if (s.startsWith('[')) { try { return JSON.parse(s); } catch { return undefined; } }
  return { ref: s };
}

export function compile(src) {
  const errors = [];
  const nodes = new Map();
  let output = null;
  const lines = src.split('\n');
  lines.forEach((raw, i) => {
    const ln = i + 1;
    const line = raw.replace(/#.*$/, '').trim();
    if (!line) return;
    let m;
    if ((m = line.match(/^output\s+([A-Za-z_]\w*)$/))) { output = { name: m[1], ln }; return; }
    m = line.match(/^([A-Za-z_]\w*)\s*=\s*([a-z_0-9]+)\((.*)\)$/);
    if (!m) { errors.push({ ln, msg: `cannot parse: "${line}" (expected  name = kind(...)  or  output name)` }); return; }
    const [, name, kind, argstr] = m;
    if (nodes.has(name)) { errors.push({ ln, msg: `node "${name}" defined twice` }); return; }
    const spec = NODES[kind];
    if (!spec) { errors.push({ ln, msg: `unknown node kind "${kind}"` + suggest(kind) }); return; }
    const inputs = [], params = {};
    for (const a of splitArgs(argstr)) {
      const eq = a.match(/^([a-z_0-9]+)\s*=\s*(.+)$/);
      if (eq) {
        const pv = parseValue(eq[2]);
        if (pv === undefined) errors.push({ ln, msg: `${name}: bad value for ${eq[1]}` });
        else params[eq[1]] = pv;
      } else {
        const v = parseValue(a);
        if (v && v.ref) inputs.push(v.ref);
        else errors.push({ ln, msg: `${name}: positional argument "${a}" must be a node name` });
      }
    }
    nodes.set(name, { name, kind, spec, inputs, params, ln });
  });

  // resolve + type check in definition order (forward refs are errors => acyclic by construction)
  const outType = new Map();
  for (const n of nodes.values()) {
    const { spec, ln, name } = n;
    // params
    for (const k of Object.keys(n.params)) {
      if (!spec.params[k]) errors.push({ ln, msg: `${name}: ${n.kind} has no parameter "${k}" (has: ${Object.keys(spec.params).join(', ') || 'none'})` });
      else if (n.params[k].ref !== undefined) errors.push({ ln, msg: `${name}: parameter ${k} must be a literal, got node name "${n.params[k].ref}"` });
      else if (!typeOk(spec.params[k][0], n.params[k])) errors.push({ ln, msg: `${name}: parameter ${k} must be ${spec.params[k][0]}` });
    }
    for (const [k, [t, d]] of Object.entries(spec.params)) {
      if (d === REQ && !(k in n.params)) errors.push({ ln, msg: `${name}: missing required parameter "${k}" (${t})` });
    }
    // inputs
    const want = Array.isArray(spec.inputs) ? spec.inputs : null;
    const types = n.inputs.map(r => {
      if (!nodes.has(r)) { errors.push({ ln, msg: `${name}: input "${r}" is not defined` }); return null; }
      if (!outType.has(r)) { errors.push({ ln, msg: `${name}: input "${r}" is defined later; define inputs first` }); return null; }
      return outType.get(r);
    });
    if (want) {
      if (n.inputs.length !== want.length) errors.push({ ln, msg: `${name}: ${n.kind} takes ${want.length} input node(s), got ${n.inputs.length}` });
      else want.forEach((w, j) => { if (types[j] && !w.split('|').includes(types[j])) errors.push({ ln, msg: `${name}: input ${j + 1} ("${n.inputs[j]}") is ${types[j]}, ${n.kind} needs ${w}` }); });
    } else {
      if (n.inputs.length < spec.inputs.min) errors.push({ ln, msg: `${name}: ${n.kind} needs at least ${spec.inputs.min} inputs` });
      types.forEach((t, j) => { if (t && t !== spec.inputs.variadic) errors.push({ ln, msg: `${name}: input "${n.inputs[j]}" is ${t}, ${n.kind} needs ${spec.inputs.variadic}` }); });
    }
    outType.set(name, spec.out === '=0' ? (types[0] ?? 'solid') : spec.out);
  }
  if (!output) errors.push({ ln: 0, msg: 'missing "output <node>" line' });
  else if (!nodes.has(output.name)) errors.push({ ln: output.ln, msg: `output "${output.name}" is not defined` });
  else if (outType.get(output.name) !== 'solid') errors.push({ ln: output.ln, msg: `output must be a solid, "${output.name}" is ${outType.get(output.name)} (use mesh_sdf / merge / extrude etc.)` });

  // unused nodes -> warnings
  const used = new Set(output ? [output.name] : []);
  for (const n of [...nodes.values()].reverse()) if (used.has(n.name)) n.inputs.forEach(r => used.add(r));
  const warnings = [...nodes.keys()].filter(k => !used.has(k)).map(k => `node "${k}" is not used by the output`);

  return { ok: errors.length === 0, errors, warnings, nodes: [...nodes.values()], output: output?.name, count: nodes.size };
}

function suggest(kind) {
  const names = Object.keys(NODES);
  const near = names.filter(n => n.includes(kind) || kind.includes(n) || lev(n, kind) <= 2).slice(0, 4);
  return near.length ? ` — did you mean: ${near.join(', ')}?` : '';
}
function lev(a, b) {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) for (let j = 1; j <= b.length; j++)
    d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
  return d[a.length][b.length];
}

export function spec() {
  const rows = Object.entries(NODES).map(([k, s]) => {
    const ins = Array.isArray(s.inputs) ? s.inputs.join(', ') : `${s.inputs.variadic}... (min ${s.inputs.min})`;
    const ps = Object.entries(s.params).map(([p, [t, d]]) => `${p}:${t}${d === REQ ? '' : '=' + JSON.stringify(d)}`).join(', ');
    return `${k}(${[ins && `in[${ins}]`, ps].filter(Boolean).join('; ')}) -> ${s.out === '=0' ? 'same as input' : s.out}${s.doc ? '   # ' + s.doc : ''}`;
  });
  return `LANGUAGE
one node per line:  name = kind(input_node, ..., param=value, ...)
last line:          output name
Units are metres, Y is up. Inputs are earlier node names. Params are literals: numbers, "strings", [x,y,z] lists.
Types: solid, sdf, curve, profile, inst. Output must be a solid. Lines starting with # are comments.

NODES
${rows.join('\n')}

EXAMPLE
top  = box(size=[1.2, 0.05, 0.7])
top2 = move(top, by=[0, 0.75, 0])
leg  = box(size=[0.05, 0.75, 0.05])
leg2 = move(leg, by=[0.55, 0.375, 0.3])
legx = mirror(leg2, axis="x")
legs = mirror(legx, axis="z")
tbl  = union(top2, legs)
tbl2 = part(tbl, name="table")
output tbl2
`;
}
