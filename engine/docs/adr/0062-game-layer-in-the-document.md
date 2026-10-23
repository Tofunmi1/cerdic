# ADR 0062: The game lives in the document (game nodes over `ve-game`)

- Status: accepted (2026-10-22, owner accepted the proposal that the game is nodes in the document)
- Date: 2026-10-22
- Amends: ADR 0027 (part behaviours), ADR 0029 (scene flags and conditions: its "not in this slice" list), ADR 0035 and 0037 (physics, joints and triggers)
- Part of: `plans/game-world-system.md` sections 7.3 and 8

## Context

A world a model builds must also be a *game*: things to pick up, doors that open for keys, goals, a way to win or lose, feedback. Today the document has part-local logic only (`behavior.on` with triggers `click`, `proximity`, `grab`, `release`, `hold`, `socket`, `flag`; named on/off flags, up to 32; no numbers, timers, OR conditions or initial values) and a separate crate `ve-game` (a deterministic rules core with variables, kinds, spawns, rules, goals and a text form, designed from Ludii and VGDL) that is partial (clippy fails, acceptance unmet) and connected to nothing. Evidence (2026): LLMs write entities well and rules and goals badly (Spec2Game: 46 and 44 out of 100), improve with detailed sectioned specs, and complete games only when a play-and-repair loop closes (GameCraft-Bench, PlaytestArena, ScriptDoctor).

Two ways to attach `ve-game`: a separate game text next to the document, or game nodes inside it. A separate text would bypass undo, diff, labels, caching, the `apply_text` atomicity and every check, and could not reference scene parts by name.

## Decision

1. **The game is nodes in the document.** New operators in family `game.*` (each a typed node, ADR 0002 text form, one output type `Game` that carries a rule fragment): `game.var`, `game.kind`, `game.spawn`, `game.input`, `game.rule`, `game.goal`, `game.flow`, `game.hud`. A `game.collect` node joins the fragments into one `GameDef` (the game's output); several fragments from several recipes can be joined (a key-and-door kit and a coin kit in one game). The canonical text form of the nodes is the same line-per-node language as everything else; `ve-game`'s own line text (`game`, `var`, `kind`, `rule`...) remains its internal and diagnostic form and is printed by `game_describe`.
2. **Binding to the world is by name and by sets, not by coordinates.**
   - `game.kind(name, part=<output name or recipe>, tags, props, size, speed)` binds a kind to scene geometry; the collider shape comes from the part's physics body or a generated collider.
   - `game.spawn(kind, at=<point> | in=<instance or point set>, count)` makes entities from a placed set (the coins scattered on a tile are the coin entities, in element order) or from points; per-element attributes (ADR 0060) become entity props.
   - Events come from the engine: `touch` from `ve-physics` sensors and contacts, `grab`/`release`/`socket` from `ve-interact`, `tick` and timers from the fixed step, `var:<name>` changes, custom `emit`. No script can raise an event.
   - Effects (`sfx`, `particles`, `camera_shake`, `message`) are *returned* by the rules core and executed by audio and particle nodes and the HUD, so the core stays pure and the same game runs headless for bots.
3. **Rules stay data.** Actions: `set`, `add`, `spawn`, `destroy`, `move`, `setprop`, `emit`, `timer`, `win`, `lose`, `effect`. Conditions are the small expression grammar of `ve-game` (numbers, names, comparisons, `and`, `or`, `not`, arithmetic) and may read ADR 0060 attributes of the entities involved. No loops, no recursion, no user functions. Budgets from `ve-game` (2,048 entities, 256 rules, 64 kinds, 4,096 actions per step, chain depth 8) are enforced at estimate time with `budget.*` diagnostics.
4. **Flags and `behavior.on` remain** as the part-local form (a door's clip, a lamp following a switch). Both compile to the same event mechanism; flags become variables with values 0 and 1 in the shared state, so a `needs` of `behavior.on` can read a `game.var` and a `game.rule` can set a flag. ADR 0029's "not in this slice" items (numbers, timers, OR, initial values) are delivered by `game.*`.
5. **Level flow is a node.** `game.flow` declares zones (named regions or parts), connections between them and gates (a gate names the variable or kind that opens it). Layout generators (structure kit and room graphs, workstream B) consume the flow graph; design checks traverse it. A flow with a gate nobody can open is a `game.validate` error before anything is built.
6. **HUD binds variables.** `game.hud` places a world-space panel (text and meters bound to variables and messages) as a node; panel rendering and gaze/pointer interaction are the in-world UI task (E8).
7. **Validation before play.** `GameDef::validate` and the document checks report: rules that can never fire (no entity has the tag), goals that cannot be reached (reachability on the flow graph and navmesh), gates with no key, variables never read or written, softlocks (a state from which no goal is reachable, found by bounded search over the abstract state of variables, pickups and gates).
8. **Playtest bots run headless.** `playtest` (ADR 0061) drives `ve-game` with scripted, random, greedy-on-navmesh and persona policies (cautious, greedy, explorer, speedrunner) against the same deterministic step, reporting win/lose, time, coverage of rules and kinds, softlocks and unreachable goals. Same definition, seed and policy give the same hash on native and WASM.
9. **Genre kits are recipes** (ADR 0060 recipes) written in the language and kept in the guide: collect-and-escape, key-and-door, arena survival, socket puzzles, social hub, exploration with landmarks. The engine knows none of those words.

## Consequences
- A model writes the game with the same tools it writes walls with; undo, diff, labels, dry runs and checks cover it; the world can carry its own acceptance (check nodes) and a bot proves it is completable.
- `ve-game` must be finished first (handoff `handoff-codex-game.md`: tests written first by the owner-side session, clippy clean, deterministic snapshot and restore) and then wired: operators in `ve-graph`, evaluator in `ve-modeling` producing a `GameDef`, a runtime system in `ve-runtime` stepping the world at 1/60 s with physics events in and effects out.
- The text form gains `Game` as a value type and the `fix`/`suggestion` rules of ADR 0061 apply to every `game.*` diagnostic (with line and column from the document text, not from `ve-game`'s internal text, so the model edits what it wrote).
- Costs: new port type, eight operators, runtime wiring, bots, checks, and the guide's sectioned game-spec template (core loop, kinds, rules, goals, feedback, flow) that raises rule quality.
- Not in this ADR: networked state, save slots (the world snapshot and hash of `ve-game` make them possible later), AI behaviour beyond navmesh goals (behaviour trees would be a new ADR).

## Phases
1. Finish `ve-game` (parser, events, actions, selectors, validation, determinism, snapshot and restore; three example games as tests).
2. `game.var`, `game.kind`, `game.rule`, `game.goal`, `game.input`, `game.collect` and the runtime system; `game_describe`, `game_validate`; headless play in tests.
3. `game.spawn` from placed sets, physics events in, effect nodes (audio, particles), `game_state`.
4. `playtest` bots and the design checks; `game.flow` and the flow-aware validation.
5. `game.hud` with the in-world UI task.

## Implementation notes (2026-10-22, what the first slices taught)
- **The player is the hero.** `input ... follow=viewer target=tag:hero` places the tagged entity at the viewer's feet each step (desktop eye minus 1.6 m; in a headset, the head over the floor of the tracking space); on load the viewer (or the tracking origin) starts at the hero's spawn. Entities touch as spheres (`size`), so VR needs no button to pick things up.
- **Kinds draw as scene parts and block as boxes.** `kind part=<scene output>` shows each entity with that authored part (the authored one is a hidden prototype for as long as a kind binds it, alive or not); `kind solid=w,h,d` makes a box the hero cannot enter. Nothing blocks the *player's body* yet; that is locomotion with collision (M3).
- **Spawns come from sets.** `game.spawn(<scatter.place/inst.array ...>, kind=...)` makes one entity per placement on the exact ground height (up to 2,048); `at`/`count`/`spread` remain for points.
- **HUD is plain geometry.** `hud text="Score {score}"` (a 5x7 procedural pixel font, no font assets, rebuilt only when text changes), anchored in front of the head with a slow yaw lag, plus an automatic YOU WON / YOU LOST banner. The in-world UI question beyond text panels (pointing, buttons) is still open.
- **Effects return to the host.** The runtime numbers each effect with the place it happened (captured before the step, so a destroyed target keeps its position); the page plays `sfx` effects through the procedural sound generator, positional, unlocked by the first click or key. Particles are not wired.
- **Bots play like a person.** A hero that follows the viewer is played by walking a virtual viewer at 3 m/s; `seek:<hero>:<tag,tag...>` goes for the first tag in the list that still has a live entity. Bots found real design flaws (an unlockable door, a stuck exit) that unit tests did not.
- **Validation hooks that exist:** `check` reports kinds naming missing scene parts and unknown sound names; `GameDef::validate` and the node-level diagnostics name the node that wrote the bad item.
