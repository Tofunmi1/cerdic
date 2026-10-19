# ADR 0054: Retained instance sets, culled by chunk

- Status: accepted
- Date: 2026-10-19
- Amends: ADR 0014 (instance sets)

## Context

Every placement of a set was its own entity, and every frame the renderer extracted all of them, sorted them by mesh, rebuilt 192 bytes of instance data for each and uploaded the whole buffer, whether or not anything had moved or
anything was on screen. Nothing was culled: all three shadow cascades and the occlusion depth pass drew everything too. Measured with a software GPU (so the vertex work is on the CPU as well) at 320 by 200 pixels, 16,000 boxes (192,000
triangles a pass) cost 433 ms a frame looking at empty sky, against 117 ms for 2,000 spheres of more triangles: the cost followed the number of placements, about 23 microseconds each, ahead of the number of triangles. Uploading 16,000
placements took 24 s, since each became an entity, 512 a frame. A forest, a field of grass or a crowd cannot be built this way.

## Decision

1. **One entity per set, one retained buffer.** A placement set with 256 or more copies, none animated and an opaque surface, is spawned as one entity with `VeInstanceSet` (`ve-render/src/instance_sets.rs`): the matrices in an `Arc`, the mesh levels, a
   bounding sphere of the mesh. The entity's own `VeMaterial` is the set's surface, so hovering or selecting it highlights it. The render world builds the instance data once, when the set or its material changes.
2. **Sorted into chunks.** The copies are sorted along a Z-order curve and cut into runs of 32 to 512 (about 48 chunks a set), each with a bounding sphere (`set_chunks.rs`). Neighbours in space are neighbours in the buffer, so a chunk is a tight sphere
   and a run of chunks is a single draw call.
3. **Culled per chunk, per view.** Each frame the chunks are tested against the camera's frustum (every eye, in stereo) and against each shadow cascade's four sides (`frustum.rs`, `set_visibility.rs`). A cascade ignores its near and far planes, so a caster
   between the sun and the slice still shades it. Adjacent visible chunks merge into one draw from the set's own buffer; the set joins the frame's batch lists, so every pass (forward, outline, shadow, depth) draws it through the same code as before.
4. **Levels of detail are in the model, not yet in use.** `VeInstanceSet.meshes` is a list of meshes with the distance each reaches; a chunk picks its level from its distance to the camera and is not drawn past the last. The runtime spawns one level today. Generating the
   levels is the next step.
5. **Picking is by copy, selection by set.** `ve_interact::BoxSet` finds the nearest copy a ray hits, rejecting most copies with a world sphere before the exact affine-box test. Hover and selection treat the set as one object (no instance index).
   Sets that are animated per copy, transparent, or smaller than 256 keep the entity-per-copy form and its per-copy picking.

## Consequences

- The retained set draws the identical picture as the entity form, shadows included (browser test `g11-instance-sets.mjs`: 0 of 416,000 pixels differ), draws 0 of 10 chunks when behind the camera, and hovers when pointed at.
- The same probe at 16,000 boxes, software GPU, 320 by 200: looking at empty sky 433 ms down to 150 ms (13 of 49 chunks drawn, 52,000 main-pass triangles against 192,000, shadow 168,000 against 576,000); looking over the forest 550 ms down to 300 ms; upload 24 s down to 8 s.
  On a hardware GPU the triangles cost far less and the per-copy CPU work is what was removed, so the gain is larger.
- A set moves only by replacing its component; per-copy animation falls back to entities. Changing the set's material (the hover fade) rebuilds its buffer for the frames the colour changes.
- Chunks are coarse for a big set (up to 512 copies): culling and level choice are per chunk, not per copy. Per-copy culling inside the nearest chunks is possible later.
- Not done: the 16,384 placement cap, uploading placements faster than 512 a frame, levels of detail from the evaluator, impostors, and a per-frame triangle budget.
