# ADR 0032: Inverse kinematics for chains of bone parts

Status: proposed (owner confirmation pending). Built and tested 2026-10-17.

## Context
A skeleton (ADR 0030) is bone parts joined by `anim.parent`. Animating each joint by hand is slow, and the interesting
cases, an arm that follows a moving or grabbed object, a leg that plants a foot, are naturally stated as "the tip goes
there". Blender's `iksolver` is vendored, but its output was not yet bit-identical between native and WASM.

## Decision
1. `rig.ik@1(in, bones, iterations)` makes its input the **target**. `bones` lists the output names of bone parts from
   root to tip (1 to 8). The target may be animated, carried (grab) or fixed; the chain follows where its middle is at
   each frame.
2. After evaluation each chain is resolved: every bone must carry `rig.bone`, each must be `anim.parent`-ed to the one
   before it, and each must start (within 1 cm) where the previous ends. A chain that fails is dropped with a warning
   (`rig.ik_bone_missing`, `rig.ik_unparented`, `rig.ik_gap`); at most 8 chains are solved (`rig.ik_budget`).
3. **Solver: our own deterministic cyclic coordinate descent** (`ve_modeling::ik`), in `f64` with only `+ - * /` and
   `sqrt`, so a given input gives the same result on every machine. It starts from the rest pose each frame (no state),
   works from the tip joint to the root, and swings each joint so the tip moves toward the goal, stopping within 0.1 mm or
   after `iterations` passes (default 32, at most 64). A goal out of reach leaves the chain straight and pointing at it.
   A straight chain with a goal inside reach on its own line (every swing angle zero) is bent a fixed 0.15 rad to a fixed
   side so it can fold. Bone lengths are preserved exactly (poses are rotations about each bone's head).
4. The solver replaces the bones' own `anim.track` poses while a target exists; parts parented to the bones follow them.
5. Blender's `iksolver` stays vendored and unbuilt. It may replace the solver later behind the same node once its output
   is bit-identical on native and WASM; our solver is the oracle-free default until then.
6. glTF export does not contain IK (the file has no constraint); bones export at their rest pose and tracks.

## Not in this slice
Joint limits and twist, pole targets (which way the elbow points), matching the target's orientation, chains that hang from
another chain's bones, multiple targets for one chain, smoothing between frames.

## Acceptance (met)
Native analytic tests (reach within 1e-3, rigid bone lengths, out-of-reach straightening, goal at rest tip moves nothing,
moved and rotated parents, three bones, a goal on the chain's own line, identical results every run); evaluator tests for the
chain checks; a browser test measuring pixels: the arm tip within 15 px of the ball in reach, and pointing within 0.1
degrees of it out of reach.
