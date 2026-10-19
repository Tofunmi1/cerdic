<!-- Addendum 2026-10-17: `interpolation` also accepts `ease_in`, `ease_out`, `bezier` (Blender's automatic clamped
F-curve handles, one curve through all keys), `back`, `bounce` and `elastic` (Blender's BLI_easing). Both are ports
checked against Blender: the Bezier against values printed by Blender's own `evaluate_fcurve` on the same keys, the
easing against `BLI_easing` called through the kernel (`ve_blender_ease`). The F-curve evaluator itself is not vendored
(about 320 files for one function); only `easing.cc`, already part of blenlib, is linked. -->
# ADR 0026: Keyframed transform tracks on authored parts

- **Status:** Accepted
- **Date:** 2026-10-17
- **Decision owners:** Spatial Lab maintainers

## Context

Documents describe still scenes. The product goal includes scenes that move and respond (a door that
swings, a fan that spins), and the modeling-tools plan lists `Clip` and `Behavior` as later document types
(Package D). Motion has to be authorable by a person and by a model through the same typed commands, survive
save, reload and undo, be checkable without a GPU, and leave the scene in other tools (glTF).

## Decision

Add `anim.track@1(in, property, keys, interpolation, loop, pivot)`. It passes its input through unchanged
and appends one track. `property` is `position` (metres), `rotation` (Euler degrees, order Y, X, Z) or
`scale`; `keys` is a bounded string, `seconds:x,y,z` entries separated by `;` (2 to 64 keys, strictly
increasing times, at most one hour, values bounded); `interpolation` is `linear`, `smooth` (smoothstep) or
`step`; `loop` repeats after the last key or holds it; `pivot` is the point, in the part's own space, that
rotation and scale act about. At most 16 tracks per part. Chained nodes accumulate tracks, which play
together and act in listed order in world space.

Tracks are plain data on the evaluated part (`Part.animation`), like `Material.texture_program`: the
evaluator validates and carries them, it does not simulate. Sampling is one pure function (`Track::sample`),
mirrored in TypeScript for export.

- The viewer holds one shared clock (play, pause, jump) and poses every animated part from it each frame.
  Instanced placements move as a set and carry no tracks.
- glTF export samples each part's tracks into translation, rotation and scale channels (key times plus eight
  points inside every segment that is not a straight translation) in one animation named `clip`.
- Behaviors (triggers that start clips, state, rules) are a separate decision that will build on this.

## Consequences

- Motion is authored, saved, undone and diffed like any other node; a model writes it as text.
- Native and WASM evaluation agree because tracks are data and sampling is pure.
- Smooth segments are approximated with linear glTF samples; very long clips cap at 1,024 samples.
- Rotation uses Euler angles for authoring simplicity; gimbal limits apply and quaternions are produced only
  at export.
- Skeletons, skinning, IK and morph targets are out of scope here.
