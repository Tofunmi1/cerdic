# ADR 0037: Joints and collision triggers

Status: proposed (owner confirmation pending). Built and tested 2026-10-17. (ADR 0036 is the material opacity record.)

## Context
ADR 0035 gave bodies that fall, stack and can be thrown. Mechanisms (doors, pendulums, drawers, bridges, windmills) need
joints, and scene logic needs to react to things touching: a ball ringing a bell, a door slamming.

## Decision
1. **`phys.joint`** is a pass-through marker added to a body (after `phys.body`), up to four per part. It names the other
   part by output name (empty means the world), a kind (hinge, slider, ball, fixed, spring), the anchor and axis in world
   coordinates at the starting pose, optional limits (degrees for hinges, metres for sliders), friction or spring damping, a
   motor (speed and force cap) and, for springs, a second anchor and rest length. A joint naming a part that is not a body
   is dropped with `phys.joint_dropped`; one before `phys.body` is refused.
2. **Hinges, sliders, ball sockets and welds** are Rapier generic joints built from frames (anchor plus an orientation that puts
   the free axis on X) expressed in each body's own frame, so bodies may start rotated. Contacts between joined bodies are off
   (a door overlaps its post), and the solver runs 8 iterations instead of 4 so chains do not stretch. A world joint attaches
   to a fixed body at the origin made on first use.
3. **Springs are our own forces**, applied at the start of each step: acceleration-based (the same stiffness swings a feather and
   a boulder alike), with damping along the spring. Rapier's spring joint ignored its rest length in this setup.
4. **Mass ratios.** Impulse solvers stretch joints between bodies of very different mass; the guide tells authors to keep
   linked parts within a few to one (link density 10 to 30 against a weight of 1 to 5). The demo and the model prompts follow it.
5. **`behavior.on trigger="collide"`**, optionally with `accepts` naming the one part whose touch counts. Physics notes each
   pair that began touching during a step; behaviours fire on those pairs in the next step, in the same node-name order as
   every other trigger, and can set flags or signal other parts. The part must be a body and may not carry its own tracks (a
   body with tracks is dropped), so the visible reaction goes through a flag or a signal.
6. Limits: 512 joints per world; joint numbers must be finite, anchors within 1,000 m, low not above high.

## Consequences
- A model can build doors, swings, pendulums, drawers, plank bridges, windmills, seesaws and spring toys, and wire a collision to
  a lamp, all in typed nodes; native and WASI results are identical (corpus `ve-physics`).
- Not built: joints derived from assembly sockets, a hand that drives a joint, scene gravity, sensors without a collision
  response, a character controller, breakable joints.
