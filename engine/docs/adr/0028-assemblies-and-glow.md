# ADR 0028: Parent-child assemblies and glow tracks

Status: proposed (owner confirmation pending)

## Context
ADR 0026 moves single parts; ADR 0027 lets them react. Machines, arms, vehicles and lamps need parts that move
together and things that light up. All of it stays data in the typed graph, so a model and a person author it the
same way, and the viewer and the glTF exporter agree.

## Decision
1. `anim.parent@1(in, parent)` records the OUTPUT name of the part the shape follows. Parts are authored at their
   rest positions in shared world coordinates; a child's pivots stay in those coordinates. A child's pose is
   `parent_pose * own_pose` (its own motion acts first about its pivot, then the parent carries it), applied
   up the chain.
2. After evaluation each parent name is resolved against the parts. A name that matches nothing, a loop (including
   a part following itself) or a chain deeper than 8 is dropped and reported as `anim.parent_missing`,
   `anim.parent_cycle` or `anim.parent_too_deep`, so consumers only ever see sound hierarchies. Placed instance
   sets cannot be parents or children.
3. `anim.track` gains the property `glow`: a brightness factor (first value, 0 to 100, keys `seconds:factor`) on
   the part's emissive colour and on the light it emits (`emits_light`). The light also rides with its part's pose.
   Glow does not move the part.
4. glTF export nests a child node under its parent node (scene roots are parentless nodes only), so the file plays
   the same as the viewer. Glow, colour and visibility are exported with `KHR_animation_pointer` (emissive strength through
   `KHR_materials_emissive_strength`, `baseColorFactor`, and `KHR_node_visibility`); a part that glows or changes colour
   gets its own material so a clip never changes parts that share its look. The Khronos validator reports no errors or
   warnings for these files; whether a given viewer plays them depends on its support for those extensions.

## Not yet
Skinned meshes, joints with limits driven by grabbing, inverse kinematics, animated colours and visibility, clips
and sequences, state and conditions, animated instance sets. Each adds properties or nodes to this same model.
