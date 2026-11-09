# Game creation and extensibility plan

Status: active execution plan (2026-11-09). Bevy remains the runtime core. This plan joins the existing game foundation, scripting/RPG, and game-maker plans; it does not replace their lower-level specifications.

## Goal

Generate complete, playable games from prompts across a broad set of genres by composing shared systems, content packs, and procedural world tools. “Any game” means a growing, explicit coverage set with reusable mechanisms, not that every possible game is automatically supported.

A game stays one authored document: world and models, materials, input, game systems, UI, goals, and checks. The model works through the language and tools, then iterates against diagnostics, measurements, bot playtests, and fixed browser captures.

## Current foundation found in source

- **Runtime:** vendored Bevy 0.20 in `ve-bevy` is the renderer and runtime core. React Studio is the authoring shell. `ve-runtime` connects browser input, game UI, camera, animation, audio, effects, and game state.
- **Simulation:** `ve-game` owns the deterministic fixed-step game world, rules, state flow, levels, snapshots, and playtest bots. `ve-physics` provides Rapier-backed simulation; `ve-nav` provides navigation.
- **Language:** the registered game family already includes variables, kinds, spawns, rules, input, controllers, cameras, flow states, levels, UI, health, projectiles, weapons, brains, targeting, abilities, navigation, terrain, surfaces, scripts, goals, and collection.
- **Working examples in the repository:** platformer, top-down arena shooter, third-person Wukong boss fight, party encounter, duel, and rolling/minigolf. These prove authored-language expressiveness; they do not yet prove reliable one-prompt generation.
- **Existing script extension point:** QuickJS runs scripts in the fixed simulation step. Script state is JSON-backed and included in save snapshots. The `rpg` and `combat` standard libraries already provide attributes/effects/tags/damage and melee timelines.

## Design contract for every new system

1. Add a reusable mechanism, not an `rpg_*`, `fps_*`, or genre-specific engine branch. Genre kits compose mechanisms as content.
2. Prefer typed language declarations for common configuration. Keep scripts for unique behavior; when a repeated script pattern stabilizes, promote it to a typed node or library function.
3. Simulation state belongs to `ve-game`, advances at fixed 60 Hz, uses the seeded random stream, and can be snapshotted and replayed. Bevy renders and hosts input; it does not own game rules.
4. Each module has a stable id/version, bounded inputs, cross-reference validation, actionable diagnostics, deterministic iteration, and an explicit save-state policy.
5. Every capability lands with a worked game example, model-facing guide/tool description, and focused validation. A genre is called supported only after a browser-playable example and a bot/evaluation run succeed.

The first modules can follow the existing `game.script` standard-library mechanism. Do not add a dynamic plugin/WASM ABI prematurely: register module implementations statically in the engine, and keep game content/configuration serialized in the document.

## Work order

### 0. Reconcile and lock the baseline

- Update the stale status in `game-foundations-plan.md`, `game-maker-plan.md`, `game-generation-pipeline.md`, and `scripting-and-rpg-plan.md` against source and current browser evidence.
- Run and record the current platformer, arena, party, and Wukong flows in the actual Bevy browser runtime. Separate source presence, headless logic coverage, browser verification, and headset verification.
- Keep WebGL2 as the broad browser/XR fallback and WebGPU as an available runtime path.

**Exit:** a capability matrix says exactly what is implemented and which target has actually been exercised.

### 1. RPG/survival item foundation (starting now)

- Add a deterministic inventory module with item definitions, stack limits, slot/weight limits, add/remove/count/query operations, and state stored in the simulation snapshot.
- Make operations safe to call from scripts and reusable by pickup rules, loot, shops, crafting, equipment, and quest rewards.
- Add typed `game.item` and `game.inventory` declarations after the API semantics are stable; do not make the LLM encode all item metadata in hand-written JavaScript.
- Build a small playable pickup/use/reward example and expose contents through `game_describe`/inspection and UI bindings.

**Exit:** a bot collects, consumes, and receives quest reward items; save/restore preserves inventory; invalid item ids, overflow, and capacity failures are explicit.

### 2. RPG campaign slice

- Add data-driven quest definitions: objectives, prerequisites, accepted/active/completed/failed states, event subscriptions, rewards, and journal text.
- Connect the existing `ve-npc` dialogue/intents to game events through a restricted vocabulary. Dialogue choices may request registered actions; LLM text never mutates game state directly.
- Add equipment slots and stat modifiers using the existing `rpg` attribute/effect pipeline; add deterministic loot tables and persistent campaign flags.
- Build a hub, two encounters, an NPC quest, a boss, checkpoints, and a save/load loop from language modules and content packs.

**Exit:** a complete short action-RPG quest is browser-playable, bot/flow checks pass, and save/load resumes the campaign correctly.

### 3. FPS kit

- Reuse first-person camera, action maps, hitscan/projectile weapons, health, enemies, and game UI already in the engine.
- Add the missing reusable feel/content systems: crosshair/aim modes, weapon switching, recoil and spread models, reload/fire presentation, impact feedback, enemy cover/engagement behaviors, encounter waves, and round/checkpoint flow.
- Build one compact FPS example with keyboard/mouse and gamepad mappings, then evaluate visibility, aim correctness, enemy behavior, and performance.

**Exit:** a small FPS loop has title, combat, ammo/reload, enemies that can kill the player, restart, and a bot/browser validation path.

### 4. Shared genre systems

Implement in order of reuse:

1. Dialogue, quests, inventory, equipment, loot, saveable progression (RPG, adventure, roguelike, survival).
2. Room/encounter generation and reachability checks (RPG, horror, shooter, roguelike).
3. Crafting, resources, gathering, placement and build mode (survival, sandbox, tycoon).
4. Vehicle controller, track/checkpoint generation, lap rules and opponent driving (racing, flight).
5. Grid/turn manager, selection/orders, economy and visibility/fog (puzzle, card, tactics, RTS).
6. Perception/noise/alert, wave scheduler, rhythm clock and sports/team rules where examples require them.

Each system should unlock more than one genre. Genre names live in recipes/packs, not engine control flow.

### 5. Prompt-to-game compiler and evaluation

- Convert a brief into a compact typed plan: camera, session loop, setting, look, systems, content budget, goals, and acceptance checks.
- Generate through existing graph fragments and reusable packs; use small edits and stable node ids instead of rewriting entire scene files.
- Keep a frozen prompt suite by family. Each run records tool calls, diagnostics, build/evaluation time, bot outcomes, state hashes, budgets, screenshots, and visual scores.
- Add failure-driven `game_validate`/`verify` fixes: unreachable goal, unwinnable encounter, empty weapon, invalid quest edge, missing save state, broken input binding, excessive world budget.
- Treat screenshot review as a required quality signal; numeric checks alone do not establish a good game.

**Exit:** multiple models can build and repair a representative game per supported family; results are repeatable and comparable.

### 6. Ship and expand

- Provide start/pause/settings/save screens, controls/accessibility help, load progress, quality selection, and a self-contained shareable game bundle.
- Add network multiplayer only as a separate architecture phase; it requires authoritative state, replication, latency handling, and a different verification strategy.

## First implementation slice

The first code landing is the `inventory` standard library in `ve-game`, following the existing `rpg`/`combat` library pattern. It provides deterministic, JSON-serializable inventory state usable from the fixed-step script API. Landed in this pass: item definitions, capacity-limited containers, stack add/remove/count/query, and discoverable `game.script` metadata. The next landing should surface item and inventory declarations as typed game nodes, then exercise them in a small browser-playable RPG/survival example.

## Completion bar

The system is broadly extensible when a new game family can be expressed by composing existing modules and a small content pack; its simulation can be replayed and saved; bots can test its rules; browser captures can review the result; and the LLM can discover the correct tools and repair named failures without inventing operators or weakening acceptance checks.
