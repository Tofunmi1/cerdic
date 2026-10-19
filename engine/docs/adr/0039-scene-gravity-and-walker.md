# ADR 0039: Scene gravity and a walking viewer

Status: proposed (owner confirmation pending). Built and tested 2026-10-17. (ADR 0038 is the water and sky record.)

## Context
Play mode flies a camera and applies Earth's gravity to props. Scenes on the Moon, in zero gravity, or a walk-through of a
house or an obstacle course need scene gravity and a person who stands on the floor.

## Decision
1. **`phys.world`** is a pass-through marker added to one body (the floor is natural): `gravity` (a vector, so a sideways pull or zero
   gravity are possible), and `walker`, `height`, `radius`, `speed`, `jump`. It rides on the body rather than being a scene-level
   value so no new result type has to be threaded through the worker, the page and the runtime; the cost is that it needs a
   body to sit on. The first body that carries it wins; a second is dropped with `phys.world_duplicate`; one before
   `phys.body` is refused.
2. **The walker** is a capsule with no collider of its own (it pushes nothing), moved each fixed step by Rapier's kinematic character
   controller: gravity from the scene, slopes up to 50 degrees, auto-stepping over 30 cm kerbs, snapping to the ground, and
   jumping with a start speed. Its result is native and WASI identical (corpus).
3. **The viewer** is walked by physics in play mode when the desktop viewer is on and a presenting headset is not: `ViewerRig`
   gets a `walking` flag that stops the keys flying the camera, `walk_input` gives physics the keys' wishes (forward, strafe, the up key
   as jump, shift as run), and physics places the eye 10 cm below the top of the capsule. Looking is untouched. Stopping play gives the
   keys back to flying; the camera stays where it ended.
4. The walker starts under the camera; if it falls below y = -100 it is put back where it started.

## Consequences
- Walk-through scenes, moon and zero-g demos, and obstacle courses are authored in typed text.
- Not built: headset locomotion (thumbstick walking), pushing dynamic bodies by walking into them, crouching, ladders, swimming.
