# ADR 0046: Distance-based skinning weights
- Status: proposed
- Date: 2026-10-18

## Context

Generated creature bodies need automatic skin weights. The current implementation assigns weights from each vertex's distance to each rig bone segment. This is simple and deterministic, but can produce less suitable transitions at complex joints than methods that account for surface or volume connectivity.

## Decision

Keep the existing `rig.skin` distance falloff as the current behavior. In `crates/ve-modeling/src/rig.rs::weigh_vertex`, each bone receives inverse `(distance + epsilon)` raised to the configured falloff; the strongest configured influences are retained and normalized. The generated creature rig currently requests two influences and falloff three (`creature/rig.rs`). Painted weight stamps can modify the distance-derived weights.

This record leaves the automatic-weight method open. Alternatives identified for research are heat-diffusion weights (Blender's `meshlaplacian.cc`), geodesic voxel binding, and bounded biharmonic weights. No replacement or selection among these alternatives has been made.

## Consequences

- Existing `rig.skin` graphs retain their current distance-based behavior and parameters.
- Distance falloff is straightforward to calculate per vertex and supports the current bounded influence representation, but its result is based on proximity to bone segments rather than mesh connectivity or a volumetric solve.
- The choice of a more advanced automatic-weight method remains open; any future change should preserve the `rig.skin` interface if practical and be evaluated on generated bodies and joints.

## Alternatives considered

- Heat-diffusion weights from Blender's `meshlaplacian.cc` are mentioned as a possible future method in `docs/adr/0030-skeletons-and-skinning.md` and `docs/plan-animation-and-blender.md`.
- Geodesic voxel binding and bounded biharmonic weights are alternatives named in the handoff for this record. Their implementation details and comparative evaluation are not present in the inspected repository; this specific research is not verified here.
- No alternative has been selected or implemented.

## Evidence

`crates/ve-modeling/src/rig.rs` documents and implements segment-distance weights and includes weight-focused unit tests. `crates/ve-modeling/tests/g11_creature.rs` checks generated skinned bodies and rigs. The prior discussion of Blender heat-diffusion weights is in ADR 0030 and `docs/plan-animation-and-blender.md`. Tests were inspected, not run for this documentation task.
