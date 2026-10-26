# ADR 0066: game entities are drawn as animated models

Status: proposed (2026-10-25)

## Context

A game entity (`ve-game`) is drawn by copying the mesh of the part its kind names (`kind guard part=guard_body`), at the entity's position. The copy has no rotation, no bones and no animation state machine, so a guard that walks slides like a statue, and an `effect:anim:...` rule has nothing to act on. Meanwhile the animation system already makes whole hierarchies move: parts that follow parts (`Animated.parent`), state machines (`MachineDrive`), IK and gait chains.

## Decision

A kind whose part is the root of an animated hierarchy is a **model kind**. Each entity of a model kind is drawn as its own **instance** of the hierarchy:

- The runtime clones every entity of the hierarchy (the parts that share the root of the kind's part, found through `Animated.parent`) once per game entity, with names made unique by a suffix `#<entity id>` on the part name, the node, the parent, and the names an IK or gait chain refers to. A clone has its own `MachineDrive` (fresh state, same graph) and its own gait state.
- Each instance has a **placement**, a matrix the game writes every frame from the entity's position and heading (`ve-game` `Entity::heading`). `resolve_worlds` puts the placement in front of the chain of parents, so feet planted by the gait, IK goals and the machine's speed all see the world motion.
- The speed the machine reads is the entity's own displacement per second, so walk and run clips follow the game's movement.
- `effect:anim`, `anim_set`, `anim_flag` aimed at an entity reach that entity's instance only (the part name becomes `<carrier>#<id>`); with no target they reach every machine as before.
- The authored hierarchy stays in the scene, hidden, as the prototype; it is not animated for nothing (its machine keeps running; that is accepted for now).
- Plain kinds (a crate, a coin) keep the old path: one mesh copy per entity.

## Not covered (first version)

- **Skinned meshes** (a `Skinned` part in the hierarchy): joints are looked up by bone name; a hierarchy that contains one is drawn the old way. A later ADR adds per-instance skins.
- Behaviours (`Reactive`), lights and physics bodies on parts of a model are not cloned.
- Per-instance material changes (use `world.insert_components` on the cloned parts, which carry the entity id in their `Label`).

## Consequences

- Cost per instance is one entity per part of the model; animation cost grows by the same factor. The game limit on entities applies; a model of N parts at M entities is N x M draws until instancing of animated parts exists.
- `ve-game` stays free of rendering: it reports position and heading, the runtime does the rest.
