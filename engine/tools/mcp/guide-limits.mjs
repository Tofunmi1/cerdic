// The hard limits of the engine, in one place, each with the error it raises and what to change. get_guide topic "limits".
//
// Every row names the source file and a pattern that finds its number there, and tools/limits-test.mjs checks that the number written here is the
// number in the code, so this page cannot drift from the engine without a test failing. Add a row when a limit is added.
// `file` is relative to the engine folder; `pattern` has one capture group holding the number (underscores allowed), or none for a literal.

export const LIMITS = [
  // ---- the document ----
  { area: 'document', what: 'nodes in one document', value: '5,000', number: 5000, fix: 'build repeated things with groups, macros (def) or inst.* sets instead of one node per copy', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_NODES: usize = ([\\d_]+)' },
  { area: 'document', what: 'nodes one evaluation expands to once groups and macros are unfolded', value: '20,000', number: 20000, fix: 'fewer or smaller groups; a loop that makes 30 nodes per copy has a copy limit', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_EXPANDED_NODES: usize = ([\\d_]+)' },
  { area: 'document', what: 'named outputs (each is a part the renderer draws)', value: '256', number: 256, fix: 'merge small props with solid.union or inst.realize, or place copies with inst.* instead of one output each', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_OUTPUTS: usize = ([\\d_]+)' },
  { area: 'document', what: 'material definitions', value: '256', number: 256, fix: 'share one solid.material between parts', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_MATERIALS: usize = ([\\d_]+)' },
  { area: 'document', what: 'group definitions (groups may nest at most 8 deep)', value: '256', number: 256, fix: 'flatten nesting; use macros for small repeated patterns', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_GROUPS: usize = ([\\d_]+)' },
  { area: 'document', what: 'typed commands in one batch (a text program costs one per node, per input link and per output)', value: '10,000', number: 10000, fix: 'send a big world as several apply_text calls', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_COMMANDS: usize = ([\\d_]+)' },
  { area: 'document', what: 'undo steps kept', value: '200', number: 200, fix: 'none needed; older steps drop off', file: 'crates/ve-graph/src/limits.rs', pattern: 'MAX_HISTORY: usize = ([\\d_]+)' },
  // ---- geometry and memory ----
  { area: 'geometry', what: 'memory one document\'s geometry and storage may need', value: '1 GiB', number: 1073741824, code: 'budget.memory', fix: 'decimate (poly.decimate), lower detail, or split the scene', file: 'crates/ve-modeling/src/budget.rs', pattern: 'MEMORY_LIMIT: usize = 1024 \\* 1024 \\* 1024' },
  { area: 'geometry', what: 'triangles stored or drawn: NO limit (memory is the bound); above 4,000,000 drawn it warns', value: '4,000,000 (warning only)', number: 4000000, code: 'budget.drawn_heavy', fix: 'lighter placed shapes or fewer placements', file: 'crates/ve-modeling/src/result.rs', pattern: 'DRAWN_TRIANGLES_WARNING: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'triangles in the whole scene for the ambient-occlusion bake (the real triangle ceiling)', value: '1,000,000', number: 1000000, code: 'budget.ao_triangles', fix: 'the error names the heaviest parts: lower their detail first (columns/rows/segments, a larger field.to_solid edge_length, less subdivision)', file: 'crates/ve-modeling/src/ao.rs', pattern: 'triangle_count > ([\\d_]+)' },
  { area: 'geometry', what: 'cells in one field.to_solid grid (the bounds divided by edge_length, cubed)', value: '6,000,000', number: 6000000, code: 'budget.field_cells', fix: 'raise edge_length or shrink min and max; preview already uses twice the edge_length', file: 'crates/ve-modeling/src/plan/fields.rs', pattern: 'MAX_CELLS: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'work units to build one field (cells times the field\'s cost)', value: '1,500,000,000', number: 1500000000, code: 'budget.field_ops', fix: 'raise edge_length or use fewer field primitives; noise and repeats cost the most', file: 'crates/ve-modeling/src/plan/fields.rs', pattern: 'MAX_WORK: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'instructions in one field\'s compiled tape', value: '4,096', number: 4096, code: 'budget.field_tape', fix: 'split the field into parts, or simplify it', file: 'crates/ve-modeling/src/plan/fields.rs', pattern: 'MAX_TAPE: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'points in a field.tube path; a field repeat count', value: '128; 16', number: 128, fix: 'a tube with fewer points; build long repeats from two steps', file: 'crates/ve-geometry/src/field.rs', pattern: 'MAX_TUBE_POINTS: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'placements in one instance set', value: '16,384', number: 16384, code: 'budget.instances', fix: 'lower count, count_x, count_y, count_z; split into several sets', file: 'crates/ve-modeling/src/instances.rs', pattern: 'MAX_INSTANCES: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'lights in one scene (emissive parts that emit light count)', value: '128', number: 128, code: 'budget.lights', fix: 'remove emitters, or use emissive colour without emits_light', file: 'crates/ve-modeling/src/result.rs', pattern: 'MAX_SCENE_LIGHTS: usize = ([\\d_]+)' },
  { area: 'geometry', what: 'maps of baked textures on the GPU (1024, 512 and 256 px sizes share it)', value: '192 MiB: 33 maps of 1024, or 136 of 512, or 546 of 256', number: 201326592, fix: 'give small things size="256" or "512" in solid.texture; share one texture between parts', file: 'crates/ve-render/src/textures.rs', pattern: 'BUDGET_BYTES: usize = 192 \\* 1024 \\* 1024' },
  { area: 'geometry', what: 'a texture program\'s text', value: '8,192 bytes', number: 8192, fix: 'simplify the program, or split it across two solid.texture nodes on two parts', file: '../src/engine/operators.d.ts', pattern: 'Procedural texture DSL[^\\n]*text up to ([\\d]+) bytes' },
  // ---- games ----
  { area: 'game', what: 'entities alive at once', value: '2,048', number: 2048, fix: 'despawn spent things (projectiles, effects); the game stops with an error at the limit', file: 'crates/ve-game/src/world/state.rs', pattern: 'const MAX_ENTITIES: usize = ([\\d_]+)' },
  { area: 'game', what: 'game actions run in one step (rule actions and scripts together)', value: '4,096', number: 4096, fix: 'fewer rules firing every step; use every= on rules; check for a rule that raises its own event', file: 'crates/ve-game/src/world/state.rs', pattern: 'const MAX_ACTIONS: usize = ([\\d_]+)' },
  { area: 'game', what: 'depth of a chain of events raising events', value: '8', number: 8, fix: 'break the loop: an emit that another rule answers with an emit of the first', file: 'crates/ve-game/src/world/state/events.rs', pattern: 'event.depth > ([\\d_]+)' },
  { area: 'game', what: 'kinds in one game; rules in one game', value: '64; 256', number: 64, fix: 'share a kind and tell things apart with tags and props; merge rules with expressions', file: 'crates/ve-game/src/parse.rs', pattern: 'd.kinds.len\\(\\) > ([\\d_]+)' },
  { area: 'game', what: 'game.script nodes; source size of one', value: '16; 64 KiB', number: 16, fix: 'merge scripts; keep data in game.var or state, not in source', file: 'crates/ve-game/src/parse/script.rs', pattern: 'MAX_SCRIPTS: usize = ([\\d_]+)' },
  { area: 'game', what: 'work one script handler may do in one call (interrupt polls, about 10,000 operations each)', value: '2,000 polls', number: 2000, fix: 'the step fails with "a script used up its work budget" and rolls back; loop less per tick, spread work over ticks, or find() once', file: 'crates/ve-game/src/world/state/script.rs', pattern: 'POLLS_PER_CALL: u64 = ([\\d_]+)' },
  { area: 'game', what: 'UI elements shown at once', value: '256', number: 256, fix: 'remove elements you no longer show (ui.remove); build rows with ui.stack', file: 'crates/ve-game/src/ui.rs', pattern: 'MAX_ELEMENTS: usize = ([\\d_]+)' },
  { area: 'game', what: 'worker scripts (the script.js node and the script tool): count, source size, and one frame\'s time', value: '16; 64 KiB; 250 ms', number: 250, fix: 'a frame over 250 ms stops the script; batch world calls with world.batch', file: '../src/engine/scripts.ts', pattern: 'FRAME_DEADLINE_MS = ([\\d_]+)' },
];

/** The `limits` guide topic: grouped, one limit to a line, with the error each raises and what to change. */
export function limitsText() {
  const titles = { document: 'The document', geometry: 'Geometry, memory, textures and lights', game: 'Games and scripts' };
  const lines = [
    'Hard limits, what an error looks like and what to change. Numbers are checked against the engine\'s code by a test.',
    'Budget errors say how far over you are and, for triangles, name the heaviest parts: fix those first. A failed edit changes nothing.',
  ];
  for (const area of Object.keys(titles)) {
    lines.push('', `## ${titles[area]}`);
    for (const row of LIMITS.filter(r => r.area === area)) {
      lines.push(`- ${row.what}: ${row.value}${row.code ? ` (${row.code})` : ''}. To fix: ${row.fix}.`);
    }
  }
  lines.push('',
    '## Things that are not limits but look like them',
    '- Angles are degrees in the text language (a plain number is degrees); some parameter descriptions still say radians, which is how they are stored. Entity headings in game scripts are radians.',
    '- Quality "preview" builds fields twice as coarse as "final"; a scene that fits in preview can still hit a budget in final.',
    '- A texture or a program that fails to bake is a warning in evaluation.warnings, not a failure: look there when something renders flat.',
    '- The studio hides its room when a game sets a camera; a scene without a game camera is looked at inside the studio room.');
  return lines.join('\n');
}

export const LIMITS_TOPIC = { about: 'every hard limit, the error it raises and what to change', text: limitsText() };
