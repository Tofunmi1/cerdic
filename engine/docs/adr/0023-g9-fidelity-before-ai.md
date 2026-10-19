# ADR 0023: G9 fixes and visual fidelity before AI evaluation

Date: 2026-10-14  
Status: Proposed

## Context

Review renders of the pistol prop and furnished study room showed that the geometry authoring foundation
was not yet producing convincing scenes. The largest visible gaps were lighting and contact shading,
flat materials, sharp edges, sparse composition, and a distorted default camera. Several practical issues
also made the fixtures difficult to author or evaluate: planner overestimation for chained booleans,
Preview-only page evaluation, concave polygon subdivision failures, a 200-command transaction cap, and
slow scene evaluation without profiling.

The original G9 was AI composition and evaluation, while G10 covered procedural appearance. Starting
provider-backed evaluation before improving those foundations would spend evaluation effort scoring
clearly avoidable failures.

## Decision

Expand G9 and implement it in three ordered parts:

1. **G9a fixes:** planner estimates, Preview-to-Final behavior, concave subdivision, large atomic command
   batches, and profiling/reuse for large scenes.
2. **G9b visual fidelity:** lights and emissive contribution, baked ambient occlusion, ambient/bounce
   lighting, presentation and post-processing defaults, the bounded procedural material subset needed by
   the review fixtures, real edge rounding, composition aids, and a visual review gate.
3. **G9c AI composition and evaluation:** capability selection, bounded scene context, targeted repair,
   reviewable pending edits, deterministic mock tasks, and the frozen 60-task evaluation. Provider-backed
   evaluation follows only when funded credentials are available.

Move only the listed appearance work from G10 into G9b. G10 retains procedural appearance features not
needed for the fidelity gate, the material editor, and broader material coverage. The order is a scope
decision, not a schedule-based reduction. All expensive rendering features need explicit quality tiers;
Quest costs remain unmeasured until the headset run. SwiftShader measurements are not used as Quest
performance evidence.

Build the recommended implementation defaults in this handoff behind this Proposed ADR. No provider
credit is presumed available and no key is reused from earlier chat.

For G9b procedural materials, recipes are bounded choices on `solid.material@1`, alongside a bounded
`texture_scale`. The typed material value carries the recipe and scale through geometry evaluation,
serialization, projection and instancing; shader recipe codes are explicit mappings, never user shader
text. This keeps AI and manual authoring on the same validated command path and lets recipe changes
remain ordinary undoable material edits.

For G9b ambient lighting, the scene environment is an authored, typed graph value
(`scene.environment@1` producing a `Environment` port) rather than a renderer-only global. Its sky and
ground colours are packed as a first-order (L1) spherical-harmonic irradiance band with an optional warm
ground bounce. This is a bounded, deterministic approximation of an environment, not global illumination:
it adds no inter-surface light transport. A document authors at most one environment; a second is refused.

For G9b presentation, viewer grading is a bounded, renderer-only transform (exposure, contrast,
saturation and tint) with a small set of named presets. It is a viewer scope, not an authored scene
value: it never enters a document and so cannot change evaluator output on native or WASI. It is applied
in the forward shader after tonemapping, which keeps windows and XR on the same grade without adding a
pass, target or draw. Bloom and depth of field are deferred rather than shipped unmeasured: neither can
be isolated cleanly in the current straight-copy present path without new targets and passes, and the
handoff makes bloom conditional on that isolation.

For G9b edge treatment, `solid.round@1` and the new `solid.chamfer@1` are whole-solid convex Minkowski
operations (a sphere and an octahedron) bounded to 20,000 input and 250,000 output triangles. They cut
convex edges and vertices only and leave concave edges sharp, so the name does not promise arbitrary CAD
filleting. Selected-edge beveling stays on `poly.bevel_edges@1`, whose v1 contract accepts only disjoint
manifold interior edges at valence-three vertices: a connected edge network or vertex fan refuses as
`poly.bevel_network_unsupported` before any partial result, and boundary, non-manifold and
winding-conflict edges refuse with their own codes. Edge networks are not enabled by this ADR; widening
the contract is a later, separately tested decision.

## Consequences

- The milestone table and G9/G10 acceptance text are updated to reflect the new sequence.
- The G9a batch limit supersedes the provisional 200-command / 256 KiB batch cap in ADR 0011; the
  independent 256 KiB source-text limit remains in force.
- Review fixtures and before/after evidence become part of the G9 visual gate.
- G9c reports model/prompt/schema versions, validity, semantic and visual results, edit locality, latency,
  and provider cost separately. Mock results do not substitute for funded real-model results.
- Release and headset evidence remain explicit gate requirements; this ADR does not claim those tests
  have run.

## Evidence and open limits

The initial review set is `pistol_prop.txt` and `study_room.txt`. Their review renders and the feature
acceptance details are recorded in `handoff-g9-fidelity-and-ai.md`. This ADR remains Proposed until the
owner accepts the scope. G8 acceptance is still open according to its current report; completing G8 remains
a prerequisite for declaring the G9-G10 release sequence complete.
