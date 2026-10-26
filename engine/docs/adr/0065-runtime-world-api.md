# ADR 0065: A runtime world API over Bevy reflection

- Status: proposed
- Date: 2026-10-25
- Decision owner: the owner (the goal: an LLM can build and change anything in a running world, not only what a node catalogue offers)

## Context

The language builds a world; it cannot change one that is running. A game rule can play a sound or move an entity, a state machine can pick clips, but a model (or a person, or a script) cannot say "make this crate glow when touched" or "list everything near the player" unless a node or a rule effect was written for exactly that. The goal is open ended, so the running world has to be reachable by name: every component of every entity, readable and writable, with the same typed discipline as the rest of the engine (limits, errors a model can act on, undo).

Bevy already has the mechanism: components that derive `Reflect` and register in the type registry can be found by type name, serialised to JSON and applied from JSON without compile-time types (`bevy_reflect`). `bevy_remote` packages that as JSON-RPC methods (`world.get_components`, `world.insert_components`, `world.mutate_components`, `world.query`, `world.spawn_entity`, `world.despawn_entity`, `registry.schema` ...).

## Measurements (2026-10-25, `tools/reflect-spike.mjs`, `crates/ve-runtime/src/reflect_spike.rs`)

In the browser's wasm (development profile, opt-level 1, desktop Chromium), one `Transform` per entity, median of 15 passes:

| entities | read to JSON | write from JSON | direct typed write |
|---|---|---|---|
| 100 | 0.4 ms | 0.5 ms | about 0 |
| 1,000 | 1.9 ms | 2.3 ms | 0.1 ms |
| 5,000 | 9.4 ms | 10.3 ms | 0.1 ms |

About 2 microseconds per component each way, linear in the count, 20 times the cost of a typed write. Not measured: a Quest (its CPU is slower, probably 3 to 5 times) and an optimised build.

`bevy_remote` depends unconditionally on `bevy_dev_tools`, which depends on `bevy_audio`, `bevy_animation` and `bevy_picking`: build time and wasm size for a feature we use a small part of.

## Decision

1. **A thin reflection API of our own** in `ve-runtime` (`world_api.rs`), on `bevy_reflect` and `bevy_ecs` only, **keeping `bevy_remote`'s method names and JSON shapes** where they exist, so its documentation and tooling carry over: `registry.list`, `world.list_components`, `world.get_components`, `world.insert_components`, `world.query`, `world.spawn_entity`, `world.despawn_entity`, plus `world.parts` and `world.find` that name the authored parts (an entity's authored name and node are what a model knows it by, not its id).
2. **Entities are addressed by a string of their id bits** (a JSON number cannot hold 64 bits) **or by authored part name**.
3. **The page calls it asynchronously**: `world_call(method, params_json)` returns a request id, the runtime answers at the start of the next frame (`world_take(id)`). A budget of 64 requests and 1,000 query rows per frame keeps a request from stalling a frame; every answer is `{ok, result}` or `{ok: false, error}` with the message naming what to change.
4. **Reflection is the long tail, not the hot path.** At about 2 microseconds a component it suits tools, models and event-driven edits (hundreds a frame), not per-frame bulk updates of thousands of entities, which belong on typed fast paths (batched transform writes, the animation parameters that already exist). Any script runtime built on this API must offer both.
5. **Only registered components are reachable.** Which ones are exposed is a list we keep (Bevy registers its own: transform, visibility, lights, camera; ours are added as they get `Reflect`). Resources and events are a later step.
6. **Safety and undo come in the command layer on top** (the typed commands of the language already give undo and limits for authored changes); the API itself refuses malformed JSON, unknown names, unregistered types and oversize requests with typed errors. A log of what changed (entity, component, before, after) is kept for the first slice so edits can be reverted and replayed; persistence of those edits is the next ADR (live world editing).

## Consequences

- A model can inspect and change a running world with one tool (`world` in the MCP server) and, later, a script can do the same in process.
- The cost is bounded and measured; the fast path is a design requirement of the script layer, not an afterthought.
- We do not adopt `bevy_remote` or its HTTP transport (browser only, no sidecar). If its dependencies become cheap (a split of `bevy_dev_tools`), swapping our handlers for its methods is possible, because the shapes match.
- Components that do not derive `Reflect` (ours, mostly: `VeMaterial`, the animation state) need it added before they can be reached; that is incremental work per component, listed in the status of the plan.

## Not decided here

The script runtime (JavaScript in a worker or Luau) and its host API; persistence and replay of edits; permissions (a model's edits against a game's own rules); resources and events.
