# ADR 0035: Rigid-body physics

Status: proposed (owner confirmation pending; the owner chose Rapier with a thin interface of our own in the session
that preceded it). Built and tested 2026-10-17.

## Context
Behaviours, clips and IK move parts on a script. Scenes that feel real in a headset also need things that fall, stack,
roll, bounce and can be thrown. The engine must stay deterministic (the same scene plays the same on every device, which
tests and replays rely on), run in the browser as WASM, and be authored by a model and a person through the same typed
nodes.

## Decision
1. **Rapier 0.22** with `enhanced-determinism`, behind our own crate `ve-physics`. The rest of the engine sees `World`,
   `BodyDesc`, `Shape`, `Kind`, `Pose` and `BodyId`, never Rapier types, so the engine can swap or extend it (Bullet for
   soft bodies remains an option, W21). Native and `wasm32-wasip1` runs of a tower, a mesh hill, capsule, convex rock and a
   kinematic pusher print identical bits (`ve-physics` example `corpus`).
2. **One node, `phys.body`**, a pass-through marker like `behavior.on`: `kind` (static, dynamic, kinematic), `shape`
   (auto/convex from the vertices, box, sphere, capsule, cylinder from the bounding box, mesh for static only), `density`,
   `friction`, `restitution`, damping. A part with its own tracks, parent, bones, skin, or that is an instance set cannot
   be a body: it is dropped with `phys.body_dropped` and a reason. A moving mesh shape is refused at the node.
3. **Fixed step shared with behaviours.** Physics takes exactly as many 1/60 s steps as behaviours did that frame
   (`Behaviors::stepped`), so the same frames give the same states; poses are written to each chunk entity of the part as
   `Transform = (rotation, position - rotation * centre)`, so a part whose vertices are authored in world coordinates turns
   about its own centre.
4. **Play mode only.** Entering play builds the world from the scene at its authored pose; leaving drops it and every part
   returns. Edit mode shows the starting pose. Gravity is Earth's, fixed for now.
5. **Hands.** Any non-static body can be picked up in play mode (it becomes kinematic and follows the hand's transform,
   which is what the interaction machine already writes); on release it is dynamic again with the hand's speed over the
   last six steps, so it can be thrown. Behaviours still work on the same part.
6. **Bounce** combines as the larger of the two bodies' restitution, so a rubber ball bounces on a floor that says nothing
   about bounce. Friction averages (Rapier's default).
7. Budgets: 256 bodies per scene, 256 hull points per convex body (extremes along 26 directions plus an even stride), 65,536
   points and 200,000 triangles for a static mesh.

## Consequences
- A model can build a bouncing ball, a tower of crates, a ramp with a rolling ball, and the person can throw them.
- Determinism is checked at three levels: native vs WASI bits, two plays in the browser compared at the same step, and
  the unit test that two worlds give the same bits.
- Not built (plan W20): joints from the assembly model, grab driving joints, collide triggers for behaviours, character
  controller, scene gravity, sensors, soft bodies (W21), physics export to glTF (`KHR_physics_rigid_bodies`, none yet).
