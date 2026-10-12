// The three control surfaces, each with the same vocabulary, conventions and worked examples.
//   dsl   : the line-per-node text language
//   tools : typed commands (add / set_param / set_inputs / remove / output) as schema-constrained JSON
//   code  : a JavaScript builder (loops, variables and arithmetic allowed) run in a sandbox
// Every surface lowers to the same IR, so one evaluator and one set of checks judge all three.
import { opsTable, NODES } from './lang.mjs';
import { irToDsl, dslToIr, irToJs, irToCommands, ID_RE } from './ir.mjs';
import { runScript } from './sandbox.mjs';

const EX_TABLE = {
  nodes: [
    { name: 'top', kind: 'box', inputs: [], params: { size: [1.2, 0.05, 0.7] } },
    { name: 'top_up', kind: 'move', inputs: ['top'], params: { by: [0, 0.775, 0] } },
    { name: 'top_oak', kind: 'material', inputs: ['top_up'], params: { name: 'oak', color: [0.45, 0.28, 0.14], metallic: 0, roughness: 0.55 } },
    { name: 'top_part', kind: 'part', inputs: ['top_oak'], params: { name: 'tabletop' } },
    { name: 'leg', kind: 'box', inputs: [], params: { size: [0.05, 0.775, 0.05] } },
    { name: 'leg_a', kind: 'move', inputs: ['leg'], params: { by: [0.55, 0.3875, 0.3] } },
    { name: 'legs_x', kind: 'mirror', inputs: ['leg_a'], params: { axis: 'x' } },
    { name: 'legs', kind: 'mirror', inputs: ['legs_x'], params: { axis: 'z' } },
    { name: 'legs_steel', kind: 'material', inputs: ['legs'], params: { name: 'steel', color: [0.6, 0.6, 0.62], metallic: 1, roughness: 0.35 } },
    { name: 'legs_part', kind: 'part', inputs: ['legs_steel'], params: { name: 'legs' } },
  ],
  outputs: ['top_part', 'legs_part'],
};
const EX_MUG = {
  nodes: [
    { name: 'outer', kind: 'polygon', inputs: [], params: { points: [[0, 0], [0.04, 0], [0.045, 0.09], [0, 0.09]] } },
    { name: 'inner', kind: 'polygon', inputs: [], params: { points: [[0, 0.005], [0.036, 0.005], [0.04, 0.1], [0, 0.1]] } },
    { name: 'body', kind: 'revolve', inputs: ['outer'], params: { angle: 360, segments: 48 } },
    { name: 'cavity', kind: 'revolve', inputs: ['inner'], params: { angle: 360, segments: 48 } },
    { name: 'cup', kind: 'difference', inputs: ['body', 'cavity'], params: {} },
    { name: 'ring', kind: 'circle', inputs: [], params: { radius: 0.005, segments: 16 } },
    { name: 'ring_out', kind: 'move', inputs: ['ring'], params: { by: [0.025, 0, 0] } },
    { name: 'handle', kind: 'revolve', inputs: ['ring_out'], params: { angle: 360, segments: 24 } },
    { name: 'handle_flat', kind: 'rotate', inputs: ['handle'], params: { degrees: [90, 0, 0] } },
    { name: 'handle_side', kind: 'move', inputs: ['handle_flat'], params: { by: [0.045, 0.045, 0] } },
    { name: 'mug', kind: 'union', inputs: ['cup', 'handle_side'], params: {} },
    { name: 'mug_glaze', kind: 'material', inputs: ['mug'], params: { name: 'glaze', color: [0.8, 0.1, 0.1], metallic: 0, roughness: 0.25 } },
    { name: 'mug_part', kind: 'part', inputs: ['mug_glaze'], params: { name: 'mug' } },
  ],
  outputs: ['mug_part'],
};

const CONVENTIONS = `Units are metres, angles in degrees, Y is up, the ground is the plane y=0. Stand objects on the ground.
Every operator below is the complete vocabulary: do not invent operators or parameters.

Geometry conventions:
- box, sphere, cylinder, cone, torus, capsule are centred on the origin; cylinder/cone/torus/capsule stand along Y unless axis is "x" or "z".
- 2D profiles (polygon, rect, circle) live in the XY plane. revolve spins a profile with x >= 0 about the Y axis, and the profile's y becomes the height.
  extrude stands a profile up from y=0 to y=height (the profile's y maps to -z). revolve and extrude need a closed, non-self-intersecting profile.
- move/rotate/scale apply to the whole input; rotate is about the origin, X then Y then Z. mirror keeps the original and adds the mirrored copy.
- radial copies a solid or profile around the Y axis (or the profile origin); repeat steps a copy by "step" and "rotate_step" per copy (stairs, spirals); array makes a grid.
- Booleans: union joins, difference subtracts later inputs from the first. Parts that must be one connected piece have to overlap by about a millimetre, otherwise they stay separate.
- Appearance: every output is one named part with one material. Typical values: steel color [0.6,0.6,0.62] metallic 1 roughness 0.35; brass [0.8,0.6,0.25] metallic 1 roughness 0.3;
  gold [0.9,0.7,0.2] metallic 1 roughness 0.25; copper [0.72,0.45,0.2] metallic 1 roughness 0.2; cast iron [0.15,0.15,0.16] metallic 0.8 roughness 0.6;
  wood [0.45,0.28,0.14] metallic 0 roughness 0.55; stone [0.55,0.52,0.48] roughness 0.9; glazed ceramic metallic 0 roughness 0.2;
  clear glass color [0.85,0.9,0.95] roughness 0.05 opacity 0.15 (tint by changing color); glowing things use emissive [r,g,b] (values above 1 glow), e.g. flame [1.0,0.55,0.15] times 3.
- Plan first: decide the parts, their real dimensions, and where each sits, then build. Keep to under 150 nodes and 100,000 triangles per part.`;

const HEAD = `You build 3D objects for a procedural modeling engine. ${CONVENTIONS}

Operators (name(inputs; parameters) -> output; type:default):
${opsTable()}
`;

export const MODELS = { sonnet: 'sonnet', haiku: 'haiku' };

export const SURFACES = {
  dsl: {
    system: `${HEAD}
You write a program in a small line-per-node language. One node per line: name = kind(input_node, ..., param=value, ...).
Inputs are earlier node names; parameters are literals (numbers, "strings", [x,y,z] lists). Name every output with: output node_name (one line per part).
There are no loops or variables: use repeat, radial, array and mirror to repeat things. Reply with the program in one code block and nothing else.

Example 1:
\`\`\`
${irToDsl(EX_TABLE).text}
\`\`\`
Example 2:
\`\`\`
${irToDsl(EX_MUG).text}
\`\`\``,
    editNote: 'Return the complete revised program (not a diff), keeping every unchanged line and node name exactly as it is.',
    stateText: (s) => s.text,
    create: (task) => ({ prompt: `Build this:\n${task.prompt}` }),
    parse(reply) {
      const text = fence(reply);
      const r = dslToIr(text);
      if (!r.ok) return { ok: false, errors: r.errors.map((e) => `line ${e.ln}: ${e.msg}`), state: { text } };
      return { ok: true, ir: r.ir, dsl: text, state: { text }, compiled: r.compiled };
    },
  },
  tools: {
    system: `${HEAD}
You edit a node graph with typed commands, returned as JSON {"commands": [...]}. Commands:
- {"op":"add","id":"<name>","kind":"<operator>","inputs":["<earlier id>", ...],"params":{...}}  (ids are letters, digits and underscore, unique; inputs must already exist)
- {"op":"set_param","id":"<id>","param":"<name>","value":<value>}
- {"op":"set_inputs","id":"<id>","inputs":["<id>", ...]}
- {"op":"remove","id":"<id>"}   (only a node nothing reads)
- {"op":"output","id":"<id>"}   (declares a part's final node as an output; every output is a part-named material-bearing node)
Order matters: a node is added after the nodes it reads. Outputs are declared with output commands.

Example 1 commands:
${JSON.stringify({ commands: irToCommands(EX_TABLE) })}
Example 2 commands:
${JSON.stringify({ commands: irToCommands(EX_MUG) })}`,
    schema: {
      type: 'object',
      properties: {
        commands: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              op: { type: 'string', enum: ['add', 'set_param', 'set_inputs', 'remove', 'output'] },
              id: { type: 'string' },
              kind: { type: 'string', enum: Object.keys(NODES) },
              inputs: { type: 'array', items: { type: 'string' } },
              params: { type: 'object' },
              param: { type: 'string' },
              value: {},
            },
            required: ['op', 'id'],
          },
        },
      },
      required: ['commands'],
    },
    stateText: (s) => JSON.stringify({ nodes: s.ir.nodes.map((n) => ({ id: n.name, kind: n.kind, inputs: n.inputs, params: n.params })), outputs: s.ir.outputs }),
    editNote: 'Return only the commands that change the graph (keep every existing id; do not re-add nodes that already exist).',
    create: (task) => ({ prompt: `Build this with add and output commands:\n${task.prompt}` }),
    parse(reply, prev) {
      const commands = reply?.commands;
      if (!Array.isArray(commands)) return { ok: false, errors: ['the reply has no commands array'], state: prev ?? { ir: { nodes: [], outputs: [] } } };
      const ir = structuredClone(prev?.ir ?? { nodes: [], outputs: [] });
      const errors = [];
      commands.forEach((c, i) => { const e = apply(ir, c); if (e) errors.push(`command ${i + 1} (${c.op} ${c.id}): ${e}`); });
      if (errors.length) return { ok: false, errors, state: prev ?? { ir } };
      const lowered = irToDsl(ir);
      if (lowered.error) return { ok: false, errors: [lowered.error], state: prev ?? { ir } };
      const parsed = dslToIr(lowered.text);
      if (!parsed.ok) return { ok: false, errors: parsed.errors.map((e) => `node near line ${e.ln}: ${e.msg}`), state: prev ?? { ir } };
      return { ok: true, ir, dsl: lowered.text, state: { ir }, compiled: parsed.compiled };
    },
  },
  code: {
    system: `${HEAD}
You write a JavaScript program that builds the object. Each operator is a function: kind(input, input, ..., {param: value, ...}); inputs are the values returned by other calls,
the last argument is an object of parameters. Declare outputs with output(node) (one call per part). You may use variables, functions, loops, arrays and Math.* to avoid repeating yourself.
Only these functions exist (no imports, no require, no globals, no timers). Reply with the program in one code block and nothing else.

Example 1:
\`\`\`js
${irToJs(EX_TABLE)}
\`\`\`
Example 2:
\`\`\`js
${irToJs(EX_MUG)}
\`\`\``,
    stateText: (s) => s.text,
    editNote: 'Return the complete revised program (not a diff), keeping every unchanged line exactly as it is.',
    create: (task) => ({ prompt: `Build this:\n${task.prompt}` }),
    parse(reply) {
      const text = fence(reply);
      const r = runScript(text);
      if (!r.ok) return { ok: false, errors: r.errors.map((e) => `line ${e.ln}: ${e.msg}`), state: { text } };
      const lowered = irToDsl(r.ir);
      if (lowered.error) return { ok: false, errors: [lowered.error], state: { text } };
      const parsed = dslToIr(lowered.text);
      if (!parsed.ok) {
        const lineOf = new Map(r.sites);
        const names = lowered.text.split('\n');
        return { ok: false, errors: parsed.errors.map((e) => { const n = names[e.ln - 1]?.split(' ')[0]; return `script line ${lineOf.get(n) ?? '?'}: ${e.msg}`; }), state: { text } };
      }
      return { ok: true, ir: r.ir, dsl: lowered.text, state: { text }, compiled: parsed.compiled, sites: r.sites };
    },
  },
};

function fence(reply) {
  const m = String(reply).match(/```(?:\w+)?\n([\s\S]*?)```/);
  return (m ? m[1] : String(reply)).trim() + '\n';
}

// One typed command applied to the IR; returns an error message or null.
function apply(ir, c) {
  const find = (id) => ir.nodes.find((n) => n.name === id);
  if (c.op === 'add') {
    if (typeof c.id !== 'string' || !ID_RE.test(c.id)) return `"${c.id}" is not a valid id`;
    if (find(c.id)) return `id "${c.id}" already exists`;
    if (!NODES[c.kind]) return `unknown operator "${c.kind}"`;
    for (const r of c.inputs ?? []) if (!find(r)) return `input "${r}" does not exist yet`;
    ir.nodes.push({ name: c.id, kind: c.kind, inputs: [...(c.inputs ?? [])], params: { ...(c.params ?? {}) } });
    return null;
  }
  const n = find(c.id);
  if (c.op === 'output') { if (!n) return `no node "${c.id}"`; if (!ir.outputs.includes(c.id)) ir.outputs.push(c.id); return null; }
  if (!n) return `no node "${c.id}"`;
  if (c.op === 'set_param') { if (typeof c.param !== 'string') return 'missing param name'; n.params[c.param] = c.value; return null; }
  if (c.op === 'set_inputs') { for (const r of c.inputs ?? []) if (!find(r)) return `input "${r}" does not exist`; n.inputs = [...(c.inputs ?? [])]; return null; }
  if (c.op === 'remove') {
    const user = ir.nodes.find((m) => m.inputs.includes(c.id));
    if (user) return `"${c.id}" is still read by "${user.name}"`;
    ir.nodes = ir.nodes.filter((m) => m !== n); ir.outputs = ir.outputs.filter((o) => o !== c.id); return null;
  }
  return `unknown op "${c.op}"`;
}

export function repairPrompt(surface, task, phase, state, attempt, errors) {
  const where = phase === 'edit' ? `The change requested was: ${task.edit.prompt}` : `The request was: ${task.prompt}`;
  const shown = surface === 'tools' ? `Your commands were:\n${JSON.stringify(attempt)}` : `Your program was:\n${attempt}`;
  return `${where}\n\n${shown}\n\nIt was rejected with these problems:\n${errors.map((e) => `- ${e}`).join('\n')}\n\nFix every problem and reply again in the same form${phase === 'edit' && surface !== 'tools' ? ' (the complete revised program)' : ''}${surface === 'tools' && phase === 'create' ? ' (the complete command list from scratch)' : ''}.`;
}

export function editPrompt(surface, task, state) {
  const s = SURFACES[surface];
  return `Here is the current ${surface === 'tools' ? 'graph' : 'program'}:\n${surface === 'dsl' || surface === 'code' ? '```\n' + s.stateText(state) + '```' : s.stateText(state)}\n\nChange it: ${task.edit.prompt}\n${s.editNote}`;
}
