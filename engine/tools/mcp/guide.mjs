// What the MCP server tells a model about the language, in layers so a model reads only what it needs.
//
//   get_guide()                short core: the syntax, the workflow, and the list of topics (about 3 KB)
//   get_guide(topic="physics") one topic with its examples, taken from the full guide text
//   list_operators()           the families and the operator names in each (about 2 KB)
//   list_operators(family=..)  one family: each operator's signature and one line of help
//   describe_operator(id)      every parameter of one operator, with unit, range, default and help
//
// Before this, one get_guide call returned 31 KB and one list_operators call 100 KB, in every session.

/** The topics cut out of the full guide text: where each starts (the first words of its paragraph) and what it covers. */
export const SECTIONS = [
  { topic: 'shapes', start: 'Smooth shapes: a few', about: 'profiles, solid primitives, moving/mirroring/arraying, rounding, kits (a vase, a table)' },
  { topic: 'curves', start: 'Curved paths (initial bounded subset)', about: 'curves, sweeps along a path, lofts between profiles' },
  { topic: 'deformers', start: 'Bounded mesh deformations refine', about: 'twist, taper, bend, shear, wave, noise, lattice' },
  { topic: 'physics', start: 'Physics (play mode):', about: 'bodies, joints, collisions, gravity, a walking viewer' },
  { topic: 'world', start: 'Landscapes: terrain.heightfield', about: 'terrain, placing things on it, water, sky' },
  { topic: 'polygons', start: 'Polygon editing (faces, edges, vertices)', about: 'editing meshes: select, inset, extrude, bevel, subdivide, UVs' },
  { topic: 'reuse', start: 'Groups (build once, use many times)', about: 'groups, rigid alignment, assembly sockets, shared placements (instances)' },
  { topic: 'appearance', start: 'Colour and finish: solid.material', about: 'colour, opacity, procedural textures' },
  { topic: 'animation', start: 'Animation: anim.track@1', about: 'keyframed motion, waves, colour/glow/visible tracks' },
  { topic: 'rigging', start: 'Bendy things (skinning)', about: 'bones, skinning, inverse kinematics' },
  { topic: 'assemblies', start: 'Assemblies: anim.parent@1', about: 'parts that follow other parts, glow' },
  { topic: 'behaviours', start: 'Behaviours: behavior.on@1', about: 'reacting to clicks and proximity, flags and logic, grabbing, clips, timers, sockets' },
];

/** Topics written for this guide rather than cut out of the full text. */
export const EXTRA_TOPICS = {
  characters: {
    about: 'make_character: a rigged, animated character or creature in one call',
    text: `make_character builds a whole animated character. It writes the graph for you (a smooth body from tapered bones, a rig, skinning,
walk / run / idle clips, toon shading), so you do not build limbs one node at a time.

  make_character(plan="biped", sliders={size: 1.7, head: 1.2, legs: 1.1, color: [0.95, 0.8, 0.7]})
  make_character(plan="quadruped", sliders={size: 2.4, horns: true, wings: true}, name="dragon")
  make_character(plan="serpent", sliders={size: 3, tail: 1.3})

plan: biped, quadruped or serpent. sliders: size in metres (a serpent's is its length), head, limbs, torso, legs, neck, tail as
multipliers 0.5 to 2, color [r, g, b], horns and wings for a quadruped. name keeps several characters apart (up to 13 lowercase
letters and digits); at = [x, y, z] places one. Call it again with another name for more characters.

For a body no plan covers, give creature = { name, color, bones: [...], blobs: [...] }. A bone is { name, parent, head, tail, radius:
[at head, at tail], role, pair, phase }. The creature stands on y = 0 and faces +z. role (leg, arm, spine, neck, head, tail, wing, free)
picks how it moves. pair=true means you wrote the +x side and the left and right copies are made (name_l, name_r). A bone's parent must
come earlier in the list; at most 32 rig bones (a pair counts twice), chains at most 8 deep. blobs are rounded lumps with no bone
(a head, a chest): { name, center, radii }. Errors name the bone or slider to change.

Afterwards: frame_view, and solid.style on the part named after the character to change its look (see topic styles).`,
  },
  styles: {
    about: 'looks (anime, painterly, ink...), cel shading, and textures that follow the shape',
    text: `Looks. solid.style@1(solid, style="anime"|"realistic"|"painterly"|"flat"|"retro"|"ink", outline_scale=1) sets shading, outline and
texture finishing in one word and keeps colour, roughness and textures. realistic removes an earlier style. solid.toon@1 gives
exact control of the cel bands (bands, feather, shadow colour, rim, spec, outline). Put either after solid.material / solid.texture.

Textures that follow the shape. A solid.texture program can use surface nodes, which read the part's own geometry:
  surface_convex(gain)   1 on sharp outer edges: edge wear, chipped paint
  surface_cavity(gain)   1 in creases and dents: dirt, grime
  surface_up(power)      1 on tops, 0 on walls: moss, snow, dust
  surface_ao(strength)   ambient occlusion; surface_curvature(gain) signed; surface_height(low, high) height above the ground
Example (stone with pale worn edges, dark creases and moss on top):
  solid.texture@1(block, program="ao = surface_ao(strength=0.35); stone = colorize(ao, gradient=[0:#4a4038, 1:#a8978a]); moss = colorize(ao, gradient=[0:#2e4a1c, 1:#78a83e]); wear = colorize(ao, gradient=[0:#a79b8a, 1:#efe6d4]); up = surface_up(power=3); edge = surface_convex(gain=7); a1 = blend(moss, stone, up, blend_type=normal, amount=0.9); albedo = blend(wear, a1, edge, blend_type=normal, amount=0.85)")
IMPORTANT: blend(top, bottom, mask) draws the FIRST colour where the mask is 1. blend(moss, stone, up) is moss on the tops.
Use 3D noise nodes (tex3d_fbm ...) with surface nodes: a 2D noise shows seams between the part's UV charts.`,
  },
};

/** Where `text` starts: the index of the first of `starts`' first words, or -1. */
function find(text, start) {
  return text.indexOf(start);
}

/**
 * Cuts the full guide text into its topics. A topic runs from its start phrase to the next topic's. Returns the sections found
 * and the start phrases that were not found (the guide text and `SECTIONS` have drifted apart).
 */
export function splitGuide(full) {
  const found = [];
  const missing = [];
  for (const section of SECTIONS) {
    const at = find(full, section.start);
    if (at < 0) missing.push(section.topic);
    else found.push({ ...section, at });
  }
  found.sort((a, b) => a.at - b.at);
  const workflow = full.indexOf('How to work: call get_text');
  const topics = {};
  found.forEach((section, i) => {
    const end = i + 1 < found.length ? found[i + 1].at : (workflow > section.at ? workflow : full.length);
    topics[section.topic] = full.slice(section.at, end).trim();
  });
  return { topics, missing };
}

/** The topic list a model sees at the end of the core guide: name, what it covers, and how big it is. */
export function topicIndex(topics) {
  const all = { ...Object.fromEntries(SECTIONS.map(s => [s.topic, { about: s.about, text: topics[s.topic] ?? '' }])), ...EXTRA_TOPICS };
  return Object.entries(all).map(([name, t]) => `  ${name.padEnd(11)} ${t.about} (${Math.ceil((t.text?.length ?? 0) / 1000)} KB)`).join('\n');
}

export function coreGuide(topics) {
  return `The document is a graph of operators, written one node per line:

  name = family.op@1(input_node, ..., key=value, ...)
  output part_name = node_name        (or: output node_name)

Inputs are earlier node names, in port order (or port=name). Values are numbers, "text", true/false and [lists]. Lengths are metres
(suffixes mm and cm work); angles are degrees unless you write rad (angle=90deg, 1.57rad). Y is up. Lines starting with # are comments.

Example (a vase with a 3 mm minimum wall that the engine enforces):
  outer = profile.polygon@1(points=[[0,0],[0.06,0],[0.11,0.12],[0.07,0.26],[0.05,0.3],[0,0.3]])
  inner = profile.polygon@1(points=[[0,0.01],[0.056,0.01],[0.104,0.12],[0.064,0.26],[0.04,0.31],[0,0.31]])
  wall = check.min_wall@1(outer, inner, min=3mm)
  body = solid.revolve@1(outer, angle=360deg, segments=64)
  cavity = solid.revolve@1(inner, angle=360deg, segments=64)
  vase = solid.difference@1(body, cavity)
  output vase

How to work:
1. Find operators: list_operators() shows the families and their names; list_operators(family="solid") gives each operator's signature
   (inputs, parameters with defaults) and one line of help; describe_operator(id) gives every parameter's unit, range and help.
   texture_nodes searches the procedural texture catalog. Use only operators that exist.
2. Read how-tos with get_guide(topic=...): worked examples for each area (topics below). Read only the ones you need.
3. get_text shows the document. apply_text ADDS the nodes of a program (it never replaces; it may read nodes already there by their
   label, and "output name = node" re-points an output). To change an existing node use apply_commands (set_param ...) with ids from
   get_nodes. check_text validates a program without applying it. An edit is atomic: a refused one changes nothing, and its diagnostic
   (code, line, column, suggestion) says what to change. Read it and fix that line.
4. Check what you made: measure gives volumes, sizes and triangle counts: use it instead of estimating. Then frame_view so the person
   watching sees it.
For people and creatures use make_character (topic characters): one call builds a rigged, animated body.

Topics for get_guide(topic):
${topicIndex(topics)}`;
}

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
