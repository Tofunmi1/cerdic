// Calls checked against the generated operator declarations (src/engine/operators.d.ts). Lines marked `@ts-expect-error` must be REJECTED by the compiler; every other line must be accepted.
import ve, { type Node } from '../../src/engine/operators';

// A solid, moved: one input first, then the parameters; the result keeps the input's kind.
const wall = ve.solid.box({ size: [4, 2.4, 0.3] });
const lifted = ve.xform.move(wall, { by: [0, 1, 0] });
const kindStaysSolid: Node<'Solid'> = ve.xform.move(wall, { by: [0, 1, 0] });

// A polygon mesh through polygon operators, with a choice parameter and a selection.
const grid = ve.poly.grid({ width: 1, height: 1, columns: 8, rows: 8 });
const top = ve.poly.select(grid, { element: 'vertex', by: 'extreme', axis: 'y', side: 'max' });

// A number node can fill a number parameter.
const half = ve.value.math({ op: 'div', a: 3.5, b: 2 });
const sized = ve.solid.box({ size: [4, 1, 0.3] });
const radius = ve.value.number({ value: 0.5 });
const ball = ve.solid.sphere({ radius });

// An operator with several inputs takes an object of ports; optional ports may be left out.
const plants = ve.scatter.place({ in: wall, surface: grid }, { radius: 3 });

// Operators with required parameters.
const bone = ve.rig.bone(wall, { head: [0, 1, 0], tail: [0, 0, 0] });

// New operators from this phase are there too.
const rd = ve.terrain.biomes(ve.terrain.tile({}), { rules: 'forest: slope 0..30' });
const fracture = ve.poly.fracture(wall, { pieces: 8, piece: 3 });

export { lifted, kindStaysSolid, top, half, sized, ball, plants, bone, rd, fracture };

// --- mistakes the types must catch ---
// @ts-expect-error a polygon mesh is not a solid
ve.solid.union(grid, wall);
// @ts-expect-error not one of the choices
ve.poly.select(grid, { element: 'triangle' });
// @ts-expect-error a required parameter is missing
ve.rig.bone(wall, { head: [0, 1, 0] });
// @ts-expect-error a size is three numbers
ve.solid.box({ size: [1, 2] });
// @ts-expect-error there is no such parameter
ve.solid.box({ sizes: [1, 1, 1] });
// @ts-expect-error there is no such operator
ve.solid.cube({});
// @ts-expect-error a solid is not a selection
ve.poly.delete_faces(grid, wall);
// @ts-expect-error a number parameter is not text
ve.solid.sphere({ radius: 'big' });
