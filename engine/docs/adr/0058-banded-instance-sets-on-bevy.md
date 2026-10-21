# ADR 0058: Banded instance sets on Bevy (near entities, mid merged chunks, far impostors)

- Status: proposed
- Date: 2026-10-21
- Amends: ADR 0054 (retained instance sets, chunk culling), ADR 0053 (the draw budget)

## Context

ADR 0054 made big sets cheap in the engine's own renderer: one retained buffer, chunk culling, levels by distance. On the Bevy renderer (the one that ships) that path is not carried over: `ve-bevy/src/sets.rs` spawns one child entity per copy, with no levels of detail and no per-copy variation, and the language refuses more than 16,384 copies (`MAX_INSTANCES` in `ve-modeling`, `placements.rs` in `ve-runtime`).

Measured 2026-10-21 in the browser harness (Apple GPU, Chromium, 1,000 to 15,625 identical spheres, CPU slowed 6 times with the DevTools throttle as a rough, uncalibrated stand-in for Quest's CPU):

| Copies | Frame, no throttle | Frame, 6x throttle, in view | looking away |
| --- | --- | --- | --- |
| 1,000 | 16.7 ms (vsync cap) | 33 ms | 33 ms |
| 8,000 | 16.7 ms | 33 to 50 ms | 33 ms |
| 15,625 | 16.7 ms | 50 to 67 ms | 33 to 50 ms |

The cost grows with the number of entities even when nothing is in view, at about 1.5 to 2 ms per 1,000 copies under the throttle; a scene of 100,000 entities cannot run on a headset. The triangles are the second limit: a 7,000 triangle tree times 1,000 is 7 million. Bevy 0.20 (vendored) batches same-mesh entities into instanced draws itself, but per-copy data reaches the shader only through that batching (a uniform array on WebGL2, no storage buffers, no compute culling), and there is no supported way to make one entity draw N instances short of writing a custom render pipeline, which would not give us its lit PBR path. Not measured: the draw cost of merged chunk meshes on a headset (the language's `inst.realize` is capped at 200,000 triangles, so the test could not be made with it), mesh upload time, and the visual pop between levels.

## Decision

Keep to stock Bevy (entities, `Mesh3d`, `MeshMaterial3d`, `VisibilityRange`) and spend the entity budget where the eye is. A set of copies is cut into spatial chunks (the Z-order chunks of `ve-render/src/set_chunks.rs`, 32 to 512 copies each) and each chunk lives in one of three bands, chosen each frame from the distance to the nearest eye with hysteresis:

1. **Near (full detail): one entity per copy.** Real entities sharing the set's mesh and material, so Bevy batches them, picking works per copy, and per-copy animation and wind stay possible. A hard budget (default 3,000 entities, a quality setting) is shared by all sets, nearest chunks first.
2. **Mid (reduced detail): one entity per chunk.** The copies of the chunk are merged into one mesh built from the set's reduced level (`ve-world` `lod::build_lods`), with per-copy colour and scale variation baked into vertex colours. Built lazily, a few chunks a frame, cached by (set, chunk, level), freed when the chunk leaves the band.
3. **Far (impostor): one entity per chunk.** Merged crossed cards (the last level `build_lods` makes, textured by the set's own material) at about 2 to 8 triangles per copy, so 100,000 copies cost a few hundred thousand vertices. Beyond a set's last range the chunk is not drawn.

Bevy's `VisibilityRange` cross-fades the bands where it can; where it cannot (a band that has to be built first) the older band stays until the new one is ready, so nothing pops to empty. Frustum culling is Bevy's, per chunk entity. Shadows: near and mid cast, far does not.

The per-set copy cap becomes a budget instead of a constant: the evaluator estimates triangles and vertices of the *banded* result (near budget + mid + far), not copies times mesh size, so 200,000 grass tufts or 100,000 trees plan within the memory budget. The 16,384 constants are replaced by that estimate (and a hard ceiling of 1,000,000 copies per set).

## Consequences

- Entity cost is bounded by the near budget whatever the world size; triangle cost is bounded by the bands' reduced levels. A forest, a field of grass and a crowd become possible in principle; the 8 million drawn triangles of the forest demo fall to the sum of the three bands.
- Merged chunk meshes copy geometry per chunk and level: memory scales with the copies in the mid and far bands, which the estimate must count (ADR 0052's texture and memory budget applies).
- A set whose copies move (animated per copy) stays all-near, as before.
- The far band needs per-set cards made from the set's own surface; `ve-world`'s `impostor_spec` describes them but nothing renders them yet.
- Costs: new band scheduler in `ve-bevy`, a chunk merger (CPU, in a worker if it shows in frame time), level generation for every instanced set (not only `flora.tree`), per-copy attributes through vertex colours and `MeshTag`, and an estimate rewrite. Each ships with tests; the first measurement to repeat once it exists is the table above plus the merged-chunk draw cost, on the headset.
- Not chosen: a custom instanced render pipeline (one buffer, one draw; the ADR 0054 form). It would scale further but loses Bevy's lit material path and has to be written against Bevy internals on WebGL2. Revisit if the bands do not meet the headset budget.
