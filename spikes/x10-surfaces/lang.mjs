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
  extrude:  { out: 'solid', inputs: ['profile'], params: { height: [N, REQ], twist: [N, 0], scale_top: [N, 1], divisions: [I, 1] }, doc: 'stands along Y from y=0 to y=height; twist in degrees over the height; scale_top scales the top relative to the base; divisions = vertical slabs (use >=16 with twist)' },
  offset:   { out: 'profile', inputs: ['profile'], params: { delta: [N, REQ] }, doc: 'grow (delta>0) or shrink (delta<0) a profile by delta metres, mitred corners' },
  pdifference:{ out: 'profile', inputs: { variadic: 'profile', min: 2 }, params: {}, doc: 'first profile minus the rest (2D)' },
  punion:   { out: 'profile', inputs: { variadic: 'profile', min: 2 }, params: {}, doc: '2D union' },
  revolve:  { out: 'solid', inputs: ['profile'], params: { angle: [N, 360], segments: [I, 32] }, doc: 'profile x>0 revolved around Y' },
  sweep:    { out: 'solid', inputs: ['profile', 'curve'], params: { twist: [N, 0], scale_end: [N, 1] } },
  loft:     { out: 'solid', inputs: { variadic: 'profile', min: 2 }, params: { spacing: [N, REQ] } },
  // --- booleans (variadic)
  union:     { out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {} },
  difference:{ out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {}, doc: 'first minus the rest' },
  intersect: { out: 'solid', inputs: { variadic: 'solid', min: 2 }, params: {} },
  // --- transforms (work on solid, sdf, inst)
  move:     { out: '=0', inputs: ['solid|sdf|inst|profile'], params: { by: [V3, REQ] }, doc: 'on a profile only by[0], by[1] are used' },
  rotate:   { out: '=0', inputs: ['solid|sdf|inst|profile'], params: { degrees: [V3, REQ] }, doc: 'on a profile only degrees[2] is used (counter-clockwise in the XY plane)' },
  scale:    { out: '=0', inputs: ['solid|sdf|inst|profile'], params: { by: [V3, REQ] }, doc: 'on a profile only by[0], by[1] are used' },
  mirror:   { out: '=0', inputs: ['solid'], params: { axis: [S, REQ] }, doc: 'axis x|y|z; keeps original and adds mirrored copy' },
  // --- repetition
  array:    { out: 'solid', inputs: ['solid'], params: { count: [V3, REQ], spacing: [V3, REQ] }, doc: 'GRID: count per axis (ints) x spacing, first copy at origin, merged into one solid' },
  repeat:   { out: '=0', inputs: ['solid|profile'], params: { count: [I, REQ], step: [V3, REQ], rotate_step: [V3, [0, 0, 0]] }, doc: 'copy i = original rotated by rotate_step*i degrees (about origin) then moved by step*i. Lines, stairs, spirals/helices' },
  radial:   { out: '=0', inputs: ['solid|profile'], params: { count: [I, REQ], radius: [N, 0] }, doc: 'solid: copies about the Y axis, each moved radius out along +X first. profile: copies about the origin of the XY plane, each moved radius out along +X first. Evenly spaced angles.' },
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
  material: { out: '=0', inputs: ['solid'], params: { name: [S, REQ], color: [V3, REQ], metallic: [N, 0], roughness: [N, 0.5], emissive: [V3, [0, 0, 0]], opacity: [N, 1] }, doc: 'physically based surface: color = linear base colour [r,g,b] in 0..1; metallic 0 (dielectric) to 1 (metal); roughness 0 (mirror) to 1 (matte); emissive = light colour [r,g,b] (values above 1 glow); opacity 1 solid .. 0 clear' },
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
  const outputs = [];
  const lines = src.split('\n');
  lines.forEach((raw, i) => {
    const ln = i + 1;
    const line = raw.replace(/#.*$/, '').trim();
    if (!line) return;
    let m;
    if ((m = line.match(/^output\s+([A-Za-z_]\w*)$/))) { outputs.push({ name: m[1], ln }); return; }
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
  if (!outputs.length) errors.push({ ln: 0, msg: 'missing "output <node>" line' });
  for (const o of outputs) {
    if (!nodes.has(o.name)) errors.push({ ln: o.ln, msg: `output "${o.name}" is not defined` });
    else if (outType.get(o.name) !== 'solid') errors.push({ ln: o.ln, msg: `output must be a solid, "${o.name}" is ${outType.get(o.name)} (use mesh_sdf / merge / extrude etc.)` });
  }

  // unused nodes -> warnings
  const used = new Set(outputs.map(o => o.name));
  for (const n of [...nodes.values()].reverse()) if (used.has(n.name)) n.inputs.forEach(r => used.add(r));
  const warnings = [...nodes.keys()].filter(k => !used.has(k)).map(k => `node "${k}" is not used by any output`);

  return { ok: errors.length === 0, errors, warnings, nodes: [...nodes.values()], outputs: outputs.map(o => o.name), count: nodes.size };
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

// One line per operator, shared by every surface so each sees the same vocabulary.
export function opsTable() {
  return Object.entries(NODES).map(([k, s]) => {
    const ins = Array.isArray(s.inputs) ? s.inputs.join(', ') : `${s.inputs.variadic}... (min ${s.inputs.min})`;
    const ps = Object.entries(s.params).map(([p, [t, d]]) => `${p}:${t}${d === REQ ? '' : '=' + JSON.stringify(d)}`).join(', ');
    return `${k}(${[ins && `in[${ins}]`, ps].filter(Boolean).join('; ')}) -> ${s.out === '=0' ? 'same as input' : s.out}${s.doc ? '   # ' + s.doc : ''}`;
  }).join('\n');
}
