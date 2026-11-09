# Game foundations: the plan to be able to make games (v1, 2026-11-08)

Status: approved by the owner on 2026-11-08 ("these are it; draft a plan and start work"). Executes sections 4 to 6 of `game-maker-plan.md` (workstream ids F1 to F6 are the same) under the rules of `game-world-system.md` (principles, five-part slice, tests first, status rows). Target is the desktop browser (keyboard, mouse, gamepad). Every game named here is a **demo built on the engine**, never part of it (memory: games are examples).

## 1. Why this, and what is already there

The world and look side is far ahead of the game side. Verified in the code on 2026-11-08:

- **Have:** `game.*` nodes (var, kind, spawn, input, rule, goal, hud, agent, nav, ground, terrain, surface, script); rule events (start, tick, touch, destroyed, grab, release, use, sight, lost, catch, stun, recover, swing, out, pocket, `var:`, `emit:`, `timer:`, `input:`, `host:`); `game.script` (JavaScript in the fixed step: find, spawn, entity props, effects, win and lose, `state` that survives save and undo) with two standard libraries, `rpg` (attributes, tags, effects, abilities, damage) and `combat` (melee timelines, buffering, combos, dodge, parry, hit-stop); physics, ragdolls, hands, navmesh, NPC minds, a first-person walker (`phys.world walker=true`), a save snapshot (`Snapshot`, `request_save`, restore), bots (`ve_game::playtest`).
- **Missing (the owner's list):** F1 controllers and cameras (only first person), F3 game flow (title, pause, restart, levels, checkpoints, lives, save slots), F4 a UI kit (only a pixel-text HUD), F2 ranged combat and generic health, F5 game feel (music, juice), F6 an action map across devices, then genre modules (inventory and crafting, dialogue and quests, economy, grid and turns, stealth, vehicles, waves), verification (reachability, softlocks, persona bots, play-and-look) and shipping (start screen, bundle).

The stale docs (`scripting-and-rpg-plan.md` says nothing is built; `game-world-system.md` section 6 lists `ve-game` as unconnected) are corrected as each workstream lands.

## 2. Design rules for every workstream

1. **Five-part slice** per capability: the node or language construct, a tool that lets a model see it (`game_describe`, a check, a view), diagnostics with a `fix`, a worked example in the guide, tests (contract tests written first by me).
2. **Pure logic first.** Behaviour lives in `ve-game` (deterministic, no engine types, replayable, bot-testable); the runtime only feeds it input and draws the result. A feature the bots cannot drive does not ship.
3. **Generic mechanisms, no genre words in Rust.** "Platformer" is a preset of controller parameters, not a code path.
4. **Determinism.** The same inputs give the same game (the world hash already tests this); no clock, seeded random.
5. **Each workstream ends with demo games** (examples only): the smallest games that need it, run by a bot and reviewed by eye.

## 3. Workstreams, in build order

### F1 Controllers and cameras (first; everything else needs a body and a view)
- **`game.controller@1`**: moves a kind's entities from input axes with real dynamics: acceleration and deceleration, air control, turn rate, gravity, jump (variable height, coyote time, jump buffer, extra jumps, wall slide). Modes are parameter presets: `walk` (3D ground), `platformer` (movement confined to a plane), `top_down` (4 or 8 directions, no gravity). Ground comes from the existing ground field, flat `ground`, and the tops of `solid` boxes (platforms); physics colliders follow in a second step. Moves can be camera-relative (`relative="camera"`).
- **`game.camera@1`**: modes `first`, `third` (orbit with mouse or stick, shoulder offset, spring arm that shortens against physics geometry, smoothing), `top_down`, `side` (dead-zone follow, bounds), `iso`, `fixed` and `cinematic`; blends between cameras when a rule switches (`camera:<name>`, blend seconds).
- Animation drive: speed, grounded and jumping flags go to the character's animation machine through the existing `anim_set` and `anim_flag` effects.
- **Tests first:** controller maths (acceleration curves, jump height and time, coyote and buffer windows, platform landing, determinism over a replay, bot-drivable); camera rig (follow, collision shortening, bounds, blend continuity).
- **Demos:** a third-person walker on terrain, a side-scrolling platformer, a top-down room, each a few lines of nodes.

### F6 Input and onboarding (alongside F1, because F1 needs named actions)
- **Action map**: `game.input` actions bound per device (keyboard, mouse, gamepad), axes and presses, dead zones, rebinding saved in settings; the runtime supplies `Inputs` from the active device.
- On-screen prompts that follow the device (glyph names, no art assets: procedural), contextual tutorial prompts (`game.hint`), a controls screen.

### F3 Game flow and state
- **`game.flow@1`**: states (`title`, `playing`, `paused`, `game_over`, `win`, and named custom ones) and the transitions between them; pause stops the fixed step; `restart`, `next_level`, `checkpoint`, `lives`.
- **Levels**: a level sequence (documents or named sets of parts, gates between them), checkpoints that re-spawn the hero with state.
- **Saving**: save and load slots on the page (IndexedDB) on top of `Snapshot`; an undo stack (snapshots are cheap); settings persistence.
- Tests: transitions, pause purity, save and restore round trip with the world hash, softlock-free restart.

### F4 UI kit
- Decision needed first (ADR): the UI is a screen-space layer drawn by the page from a document-defined tree (DOM or canvas, accessible, device-independent), driven by `ve-game` state, not geometry in the world. In-world panels stay for diegetic UI.
- **`game.ui@1`** tree: text, bar (bound to a variable or an `rpg` attribute), icon shapes, button (action), image-free panels, layout (anchor, stack, grid), themes; built on it: menus, dialogue box with choices, inventory grid, damage numbers, minimap, prompts, tooltips.
- A procedural, scalable font (the pixel font is the fallback) because assets are not pulled.
- Tests: layout rules, bindings update, actions route to rules, keyboard and gamepad focus navigation.

### F2 Combat and health (ranged and generic)
- Melee exists (`combat` library). Add: projectiles (speed, lifetime, gravity, pierce, splash, homing), hitscan (physics raycast), weapons (ammo, reload, cooldown, spread, recoil), aiming (camera ray), enemies that fight back (agent attack behaviours), knockback and invulnerability frames, hit feedback and damage numbers (via F4 and F5).
- Health stays in the `rpg` library; `game.health` is only sugar that declares the attributes and a bar.
- Tests: projectile paths, hit resolution order, ammo and reload, determinism.

### F5 Game feel
- **`game.music`**: procedural layered and adaptive music driven by game variables (intensity, danger) from `ve-audio`; stingers on events.
- Screen effects (flash, vignette, chromatic aberration), camera-shake presets with a strength and a reduced-motion setting, trails and hit-stop presets bundled as `juice` presets.

### Genre modules (after F1 to F6; each unlocks a family; each ships as a library of rules and nodes plus an example)
inventory, crafting, loot and build mode; dialogue graphs, quests and visual-novel staging; economy (resources, production, upgrades, big numbers); grid and board with turns and undo; stealth (sight cones, noise, alert); vehicles (the wheeled vehicle in `ve-physics` gets a node first); waves and tower defence; then RTS, cards, rhythm, sports.

### Verification and shipping (grows with every workstream)
- Bots: scripted, random, greedy navmesh, persona; `goal_reachable`, `no_softlock`, `pacing` checks; a loop that plays a game, looks at screenshots and reports.
- M7 model evaluation: one-line prompts to finished games, scored by checks, bot outcomes and review.
- Ship: start screen, load progress, quality auto-select, shareable bundle.

## 4. Known engine limits that games will hit (tracked here, fixed in `game-world-system.md` M8 and M9)
Documents that take over about a minute to evaluate hit the 60 s upload limit (tile streaming needed); drawn triangles count every copy (near and far tree levels, impostors); the `ve-graph` physics node file has owner clippy errors.

## 5. Definition of done for this plan
A model, given a one-line prompt, builds a small game of three different camera types (third-person, side-scroll, top-down) with menus, saving, a HUD and feel, through the tool calls alone; a bot completes each; the checks pass; each looks presentable. Genre modules and the model evaluation (M7) then extend it.

## 6. Status (newest first)
| Date | Entry |
| --- | --- |
| 2026-11-08 | **F6, F3, F4 and F2 landed (logic and tests; browser check pending).** F6: `input` keys/pad/label bindings, `game_input_map`, `actionMap.ts` (resolve, rebinding, prompts, menu keys). F3: `state`/`level` statements, rule `state=`, actions go/restart/level/checkpoint/respawn, events `state:`/`level:`, frozen states, `saveSlots.ts` (slots, undo, settings; no screen yet). F4: ADR 0074, the `ui` tree (`ve-game/src/ui/tree.rs`, `layout.rs`: stack, grid, bars, buttons with focus, tooltips, minimap, `{var}`/`{key:ACTION}`/`{kind.prop}` bindings), node `game.ui@1`. F2: `health`/`projectile`/`weapon`, hurt/heal/fire/reload, nodes, arena demo. Also `fall_limit`, `game.camera` spring arm against solids, the animation feed, wall slide. Tests: `ve-game` (camera 20, controller 19+, flow 11, bindings 5, ui_tree 16, ranged 22), `ve-modeling` scene tests (walker, platformer 6, toproom, arena 3, nodes 10), bun (actionMap 16, saveSlots 11). **Open:** F5, genre modules, save/rebind screens, damage numbers, browser verification, shipping. |
| 2026-11-08 | **F1 step 2: cameras and the playable wiring.** `camera NAME ...` statement and `CameraRig` (`ve-game/src/camera.rs`, pure): modes first, third (orbit, shoulder, spring arm), top_down, side (dead zone, bounds), iso, fixed; smoothing; pitch limits; blends between cameras (smoothstep, shortest-way yaw); the rule action `effect:camera:NAME[:SECONDS]`; node `game.camera@1`. Runtime: `game_camera.rs` writes the active camera into the viewer rig each frame (the viewer's look is the orbit state), `camera.yaw` is fed to every step, the page sends `move.x`/`move.z`, a `jump` press and a `jump` held axis (a press alone is a release, so a held axis keeps the full jump). Tests: `ve-game/tests/camera.rs` (19), controller held-axis test, `ve-modeling/tests/walker_scene.rs` (a bot climbs the ledges of `web/tests/scenes/walker/walker.txt`), node tests; guide paragraph PLAYABLE HERO. **Not done in F1:** spring arm against real walls (the rig takes a raycast, the runtime passes none yet), hero animation flags, wall slide, physics-collider ground, platformer and top-down demos, browser verification of the walker (bundle rebuild in progress). |
| 2026-11-08 | **F1 step 1 landed: the character controller.** `controller KIND ...` statement and `ControllerDef` (`ve-game/src/parse/controller.rs`), the step (`ve-game/src/world/state/controller.rs`: acceleration and deceleration, air control, gravity, jump with variable height, extra jumps, coyote time and jump buffer, camera-relative movement, axis lock, turn rate, platforms from `solid` boxes with step-up and side blocking, props `grounded` and `speed`), the node `game.controller@1` (`ve-graph` spec, `ve-modeling/src/game_nodes/agents.rs`). Contract tests written first: `ve-game/tests/controller.rs` (16) and `ve-modeling/tests/game_controller_node.rs` (5); all of `ve-game` and the game node tests pass, clippy clean on the new files, the wasm runtime compiles. **Not done yet in F1:** the runtime feeds no keyboard axes, jump or `camera.yaw` to a controlled hero, so it cannot be played; `game.camera` (third person, top-down, side, iso, fixed, blends); wall slide; physics-collider ground (only ground field, flat ground and `solid` boxes); the guide topic and a worked example; the demo games; bots driving a controller. |
| 2026-11-08 | Plan written and approved. |
