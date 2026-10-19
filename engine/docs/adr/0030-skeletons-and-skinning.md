# ADR 0030: Skeletons and skinning on top of parts

Status: proposed (owner confirmation pending). Items 1 to 5 are built and tested (2026-10-17); item 6 (IK) is not.

## Context
Rigid assemblies (ADR 0028) cover machines. Bendy things (an arm with skin, a tail, a creature, a tube) need a mesh that
deforms with moving bones. Blender's armature deformation, automatic weights and IK solver are the reference, but its
action, constraint and dependency-graph stack stays out (plan B). The model must stay one graph that a model and a person
author through the same commands, and the viewer and the glTF exporter must agree.

## Decision
1. **A bone is a part.** Bones are ordinary named parts joined by `anim.parent` and moved by `anim.track`, `anim.wave`
   and behaviours, so everything built so far already animates a skeleton. A bone part is usually small and may be
   hidden (`visible` track or an invisible marker). All parts are authored at their rest pose in shared world
   coordinates, so a bone's rest transform is the identity and **no inverse-bind matrices are needed**: the deformation
   of a vertex is `sum(weight_i * pose_i * vertex)`, where `pose_i` is the bone part's pose this frame (the same pose the
   viewer already computes through the parent chain).
2. `rig.bone@1(in, head, tail)` records the bone's rest segment (two points in world coordinates) on its part. It is
   what weights are measured against. A part without it is a point bone at its pivot.
3. `rig.skin@1(in, bones, falloff, influences)` turns the shape into a skinned part: `bones` lists the output names of
   the bone parts (at most 32), `falloff` shapes how quickly influence drops with distance (default 3), `influences`
   is how many bones may move one vertex (1 to 4, default 4). After evaluation each vertex gets weights from its
   distance to each listed bone's segment, `w_i ~ 1 / (d_i + epsilon)^falloff`, keeping the strongest `influences`,
   normalised to sum to 1. The method is deterministic (no search order or hashing), bit-identical on native and WASM, and
   is checked against Blender's own weights as an oracle only where it makes sense (a straight limb: weights blend across
   the joint). Blender's heat-diffusion weights (`meshlaplacian`) may replace the distance rule later behind the same node.
4. A part that is skinned is no longer posed as a rigid body: its own transform stays at the identity, and each frame
   the viewer deforms its vertices (position and normal) on the CPU from the bones' poses and re-uploads the mesh. A
   skinned part may not also be a bone, a parent or a child, and cannot be an instance set. Missing or non-bone names
   in `bones` are dropped with a warning `rig.bone_missing`; a skin with no valid bones is dropped with `rig.skin_empty`.
5. **glTF export** writes a real skin: `JOINTS_0` and `WEIGHTS_0` on the mesh, a `skin` whose joints are the bone
   part nodes and whose inverse bind matrices are the identity (rest pose is the identity). The animation channels
   are the ones already written for parts. The Khronos validator must report no errors.
6. **IK** (separate slice): `rig.ik@1(in, chain, target)` drives a chain of bone parts so the end of the chain reaches a
   target part, solved each fixed step by Blender's `iksolver` (already vendored under `intern/iksolver`) once its output
   is bit-identical on native and WASM; until then a deterministic CCD solver of our own is the fallback.

## Limits
32 bones per skin, 4 influences per vertex, 65,536 skinned vertices per scene (CPU skinning budget), 8 skinned parts.

## Acceptance
An analytic test: a cylinder skinned to two bones bends at the joint, and every deformed vertex equals the blended
transform to within 1e-6; weights sum to 1; native and WASM weights are bit-identical (corpus). A browser test: an arm's
forearm bends at the elbow and the mesh stays connected (no gap at the joint), measured from the picture; the exported
glb validates and its skin has the bones. A real-model prompt set (arm, tail, tentacle).

## Not yet
Dual-quaternion blending, user-painted weights, shape keys, bone constraints, physics bones.
