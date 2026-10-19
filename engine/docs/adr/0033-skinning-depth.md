# ADR 0033: Skinning depth: dual-quaternion blending, painted weights, budget

Status: proposed (owner confirmation pending). Built and tested 2026-10-17.

## Context
ADR 0030 gave linear blend skinning with distance weights. Two defects show up in practice: a bone that turns about
its own length (a forearm roll, a twisting torso) pinches the mesh to a thread at the joint, and distance weights
cannot be corrected where the automatic result is wrong (a knee, a shoulder). The vertex budget was a guess.

## Decision
1. `rig.skin` gets `blend`: `linear` (default, unchanged) or `dual_quaternion`. Each bone pose becomes a unit dual
   quaternion (rotation plus translation; scale in a pose is dropped), the weighted sum is flipped to the first bone's
   hemisphere and renormalised, and the vertex and normal are moved by the result. The conversion of poses happens once
   per chunk per frame (`rig::prepare_dual`); the per-vertex blend uses cross products rather than quaternion products.
   All arithmetic is `f64` with a fixed order (only `+ - * /` and `sqrt`), so native and WASM agree; the corpus checks
   a direct deformation as well as the weights.
2. `rig.skin` gets `paint`: up to 64 brush stamps `bone x y z radius strength`, applied in order to the distance
   weights before the strongest `influences` are kept. A stamp fades with a smoothstep to nothing at its radius.
   Positive strength blends the whole weight vector toward all weight on that bone; negative strength scales that bone's
   weight down and renormalises; a vertex whose weight would vanish keeps its unpainted weights. Stamps naming a bone
   that is not usable are dropped with `rig.paint_bone_missing`. Semantic selection by name (a model should not count
   vertices) stays the rule: a stamp is a place and a radius, never an index.
3. The glTF export stays linear (glTF has no dual-quaternion skinning); painted weights are exported as they are,
   because they are ordinary weights. A twist skinned as dual quaternion therefore looks different in a third-party
   viewer. `blend` is a viewer-quality choice, documented in the node text.
4. **GPU skinning is not built.** Measured natively in release (`skin_cost_per_hundred_thousand_corners`): linear
   about 1 to 2 ms and dual quaternion about 7 ms per 100,000 corners. The scene budget rises from 65,536 to 131,072
   skinned vertices; a part is skinned per corner, so a budget scene costs a few milliseconds of linear CPU time. The
   decision is revisited when a real GPU or headset frame time shows skinning in the profile (plan W22).

## Consequences
- Twist-heavy limbs keep their volume in the viewer; the cost is about five times the linear blend on the same mesh.
- Weight corrections are text, so a model can fix a bad joint and a person can see the stamps.
- Shape keys (`rig.morph`) and skinned instance sets remain separate work.
