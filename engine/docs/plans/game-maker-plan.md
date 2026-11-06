# Game maker plan: any kind of game from a prompt (v1, 2026-11-06)

Status: proposed, for the owner's review. Sits above `game-world-system.md` (the systems and the five-part rule), `game-generation-pipeline.md` (prompt to shipped game) and `graphics-plan.md` (the look). Target: desktop browser, keyboard, mouse and gamepad (VR is left out of this plan on purpose). Rules that apply to every item: everything through the language and tool calls, generic mechanisms with no genre words in Rust, procedural only (no font files, no imported art), tests first, a game is an example and never part of the engine, each capability lands with its five parts (node, tool to see it, diagnostics with fixes, an example, a test).

## 1. Goal and what "any kind of game" has to mean

The owner's goal: a model, given one prompt, builds a complete, playable, good-looking browser game of any genre through the tool calls alone. "Any" cannot mean every game ever made; it means every game that is **a point in a small space** we cover with composable layers:

`game = camera + controller + verbs (mechanics) + systems (economy, progression, AI) + rules (win, lose, flow) + levels + UI + art style + feel`

A genre is then a preset of those choices (a *kit*, a recipe in the language), never code. Out of scope for now: networked multiplayer (netcode, matchmaking), persistent online worlds, console or store submission. Local hot-seat is possible later.

## 2. What the evidence says (what to build for, and what fails)

- **Models fail at completeness, not at isolated mechanics.** GameCraft-Bench (140 Godot tasks, 15 game families, the best agent 41%): agents build recognisable local mechanics but fail to assemble complete, coherent games. Its scoring weights show what matters: core mechanics 0.15, functional visuals 0.15, **content depth 0.35, art and presentation 0.35**; evidence that is label-only, static, placeholder or default widgets is capped at 0.5. Agents that looked at rendered screens fixed more layout and visibility bugs; effort (tool-call volume) did not predict score. ([GameCraft-Bench](https://arxiv.org/html/2606.17861v1))
- GameXpert-Bench: a playable core does not mean a complete game; agents repair known bugs far more reliably than they find hidden ones; 3D fails more than 2D, mostly by load or crash. ([GameXpert-Bench](https://arxiv.org/html/2608.21833v1))
- **Consequences for us:** (1) depth and presentation are 70% of a judge's view, so content-depth minimums, the look presets and the UI are not polish, they are the product; (2) verification must replay and look (screenshots from a bot's play), not read code; (3) every kit must carry a checklist a model can run (levels, enemy types, progression beats, feedback on every action).
- **The 15 families to cover first** (GameCraft-Bench's list, in its task counts): platformer 19, strategy 17, tycoon 16, open world 15, roguelike 14, visual novel 11, puzzle 8, shooter 7, simulation 6, card 5, horror 5, rhythm 5, idle 4, racing 4, sports 4. We add four that reuse the same parts: survival-crafting, tower defence, stealth, fighting.
- **Mechanics vocabulary** we use to check coverage: Adams and Dormans' five categories (physics, internal economy, progression, tactical manoeuvring, social interaction) and Schell's (space, objects, attributes and states, actions, rules, skill, chance). The tabletop encyclopedia of mechanisms (Engelstein and Shalev, 200+ mechanisms) is the reference for board, card and turn-based rules. ([Adams and Dormans via O'Reilly](https://www.oreilly.com/library/view/practical-game-design/9781787121799/8cceb529-abde-4755-be15-4e021b5c4763.xhtml), [Wikiversity list](https://en.wikiversity.org/wiki/Game_mechanics))
- **No-code engines as a checklist.** GDevelop ships these as behaviours: platformer character, top-down movement (4 or 8 directions), 3D physics character, pathfinding (grid and navmesh), physics, car, draggable, anchor, tween, destroy outside screen, multiplayer object. A behaviour is a small reusable mechanic you attach to an object: that is the shape our *mechanic modules* should have. ([GDevelop behaviours](https://wiki.gdevelop.io/gdevelop5/behaviors/all-behaviors/))

## 3. What we have today (verified in the repository, 2026-11-06)

**Language and world.** About 250 ready nodes (counted from the operator list). Geometry and materials: poly (59), field (25), solid (23), transform (16), instances (13), curves (12), body (8, including `body.human` with a 104-bone skeleton and CC0 clothes and hair), terrain (8: heightfield, tile, erode, biomes, flatten, place), river, water, flora, scatter, city, street, building, cloth and fluid simulation, scene sky and environment (haze, clouds, light shafts, depth of field, GI), lights, procedural materials and texture programs, `bake.maps`, `script.js`.

**Game layer (`ve-game`, nodes `game.*`, 13): ** variables, kinds (bound to scene parts, with tags, props, speed, solid boxes), spawns (from placed sets or points), inputs (`follow=viewer` makes the player the hero), rules (events: touch, grab, release, socket, tick, timers, variable changes, custom emit; actions: set, add, spawn, destroy, move, setprop, emit, timer, win, lose, effect), goals, `game.collect`, HUD text with `{variable}`, creatures (`game.agent`: patrol, notice, chase, catch, stun on a navmesh), falling and thrown things, rolling things on a ground field with surfaces and pockets (`game.terrain`, `game.surface`), swinging hand tools, effects returned to the host (sound, particles, shake, message). Deterministic 60 Hz step, snapshot and restore, headless **playtest bots** (idle, random, seek), `check`, `game_describe`.

**Systems.** `ve-physics` (Rapier: six shapes, joints, ragdolls, contacts, raycasts, sensors, layers, kinematic movers, heightfield; `phys.*` nodes), `ve-interact` (hands, grab, sockets, `behavior.on`), `ve-nav` (navmesh, A*, crowds), `ve-npc` (ADR 0063 and 0064: FSM, behaviour trees, utility, HTN, skills; LLM minds with memory, intents from a closed vocabulary, `DialogueSession` with states and a streaming turn stream, fallbacks, budgets, inspectable prompts), `ve-animstate` (state machines, `anim.machine`), animation (tracks, waves, retarget, IK, gait, spring bones), `ve-audio` (procedural sound effects), `ve-particles` (`fx.*`), `ve-gait`, `ve-chain`, `ve-sky`, runtime world API for scripts (ADR 0065), game entities as animated models (ADR 0066), skinned creatures (a zombie with a state machine), save slots in the page (`savegame.js`), MCP tools (`apply_text`, `find`, `check`, `snapshot`, `playtest`, ...).

**Studio (new, 2026-11):** a React app with chat, node graph, outline and inspector, quality and effect switches, overlays, transform gizmo, playback, build progress with parts appearing as they evaluate.

**Examples that exist:** water-jar collect game, key-and-door vault, Blackout and Blackout 2 (creatures, throwing), mini-golf, zombie scene, desert outpost, city districts.

## 4. Gaps, by layer (what we need to add)

Grouped so that each group is one workstream with a demo. "Have" means usable from the language today.

| Layer | Have | Missing |
|---|---|---|
| **F1 Controllers and cameras** | the viewer is the hero (walk, look, fly) | grounded character (gravity, jump, coyote time, double jump, wall slide), top-down 4 and 8 direction, side-scroll with parallax, isometric, third-person follow with collision, fixed and cinematic cameras, camera modes switched by rules, key and gamepad bindings with remap, cursor-pick input (point and click) |
| **F2 Combat and health** | touch events, `destroy`, creatures catch or stun | health and damage, damage types and resistances, death events, melee overlap, projectiles, hitscan, weapons (ammo, reload, cooldown), status effects (burn, slow, stun), knockback, invulnerability frames, hit feedback |
| **F3 Flow and state** | win and lose banner, snapshot and restore | title, pause, game over, restart, level sequence and gates, checkpoints, lives, save and load slots, **undo stack** (snapshot makes it cheap), settings persistence |
| **F4 UI** | pixel-font HUD text | see section 5 |
| **F5 Feel** | sound effects, particles, shake effect | music (procedural, layered, adaptive), hit stop, trails, screen effects (flash, vignette, chromatic), camera shake presets with a strength slider, damage numbers, juice presets with a reduced-motion setting |
| **F6 Input and onboarding** | keyboard and mouse viewer controls, gamepad module | action map (`input` names per device), on-screen prompts that follow the device, contextual tutorial prompts, controls hint |
| **M-GRID** boards, turns, undo | ground field | a grid or board, pieces, turn manager, move rules, win checks, undo, hints |
| **M-CARD** | none | zones (deck, hand, discard, table), draw and play, targeting, effects as rules, turn phases, opponent AI |
| **M-ECON** resources, production, tycoon, idle | variables and timers | resource types with rates, production chains, upgrades and unlock trees, placement on a grid, big-number formatting, offline progress, charts |
| **M-INV** inventory, crafting, building | none | items, stacks, slots, equip, crafting recipes, build mode with snapping, containers, loot tables |
| **M-NARR** dialogue, quests, visual novel | `ve-npc` dialogue and minds | authored dialogue graphs with flags, quest and journal state, scene staging (backdrop, speaker, expression), choices, skip and auto, relationship UI |
| **M-STEALTH** perception | creatures notice and chase | sight cones with occlusion, noise, light level, alert states, search, hiding, a scare director |
| **M-VEHICLE** | physics bodies, joints | wheeled vehicle (suspension, steering, drive), boats, flight, vehicle camera, AI drivers |
| **M-RTS** selection, orders, fog | navmesh crowds | box select, orders (move, attack, build), production queues, fog of war, minimap, formations, utility or HTN opponent |
| **M-WAVES** tower defence, arena | spawns | wave scheduler, lanes, turrets, targeting, economy hooks |
| **M-RHYTHM** | procedural audio | beat clock, chart format, hit windows, judgement, combo, lane UI |
| **M-SPORT** | rolling things, swing | ball physics with spin and walls, fields and goals, team AI, rules of play |
| **Levels** | terrain, rivers, flora, scatter, city, buildings | see section 6 |
| **Verification** | `check`, bots (idle, random, seek) | see section 7 |
| **Ship** | export, page | start screen, load progress, quality auto-select, share bundle |

## 5. UI plan (types, screens, fonts, NPC dialogue)

Research basis: the diegetic, non-diegetic, spatial and meta taxonomy (Fagerholt and Lorentzon, 2009), the Game UI Database's screen catalogue, Steam and Xbox accessibility guidance, and per-genre HUD practice. Evidence is thin for puzzle, rhythm, social and exploration UI, and several figures come from secondary sources.

**UI placement types the language must express** (one element definition, a `space` parameter): screen overlay (non-diegetic), attached to a part (diegetic: ammo on a weapon, a wrist device, a sign), in-world marker (spatial: waypoint, outline, nameplate), and meta effects (damage vignette, low-health pulse). Visibility tiers: always (critical only), semi-persistent (compass, minimap, objective), on demand (inventory, quests, map).

**Genre presets** (what is on screen, and the default placement):

| Genre | Always visible | On demand | Default style |
|---|---|---|---|
| Shooter, action | health, ammo, crosshair, damage direction | inventory, map | non-diegetic corners; ammo on the weapon |
| Platformer | lives, collectibles, progress | level select | light HUD, state through animation |
| Racing | speed, position, lap, track map | results | needle or digital speedometer |
| Puzzle | nothing, or move count | undo, restart, hint (optional, previewed, costed) | minimal; level select with retry, next, map |
| Strategy, tycoon | resources, minimap, selection, command card, alerts | tech tree, stats | dense modular panels; tooltips never the only home of critical facts |
| RPG, open world | health, objective, compass | stats, quests, inventory, map, journal | HUD light, menus deep |
| Survival | one cluster of 4 to 5 status meters with trend arrows, hotbar | crafting, inventory, radial menu | hold-to-open radial menu |
| Horror, stealth | almost nothing; diegetic where possible | hand-held device for map and inventory | time keeps running in menus; scarce inventory |
| Visual novel | text box, speaker, choices | log, save, settings | text box; choices after the line completes; skip and auto |
| Card | hand, mana or energy, turn indicator | deck, discard | drag to play, targeting lines |
| Rhythm | lane, judgement, combo | song select | feedback in sight and sound at once |
| Idle | numbers, upgrades | prestige, stats | UI-first |

**Screen catalogue every game gets** (from the Game UI Database): title, mode and level select, difficulty, load and save, loading, pause, settings (gameplay, display, audio, UI and accessibility, language), results, game over, credits, tutorial prompts.

**Accessibility defaults** (Steam and Xbox guidance): text scalable to at least 38 px tall at 1080p, up to 200%; contrast 4.5:1 normal and 3:1 large; separate subtitle scale with an optional plate and at most 38 characters a line; focus loops in lists but not grids, consistent prompts, input that moves focus on can move it off, any device at any time; camera shake as a slider; reduced motion; critical text about 5% inside the screen edge.

**Fonts (no font files).** Procedural skeleton fonts: each glyph is a few strokes (a skeleton) plus a pen; `ui.font` has axes (weight, width, slant, contrast, roundness, spacing) and presets (pixel, techno, rounded, serif, hand-drawn). For stroke glyphs the exact distance field is computed from the skeleton, so one small shader draws crisp text at any size. Printable ASCII first, other scripts later. The existing 5×7 pixel font becomes one preset. (METAFONT and skeleton-based design are the model; fixed parameter sets proved too restrictive, so the axes are ours.)

**UI nodes:** `ui.theme` (colour, radius, spacing, font, motion tokens, like our studio's `tokens.ts`), `ui.panel`, `ui.text`, `ui.bar`, `ui.icon` (drawn from shapes), `ui.stack` (flex-like layout), `ui.button`, `ui.menu` (focus graph), `ui.list`, `ui.grid` (inventory), `ui.minimap`, `ui.marker`, `ui.dialogue`, all bindable to `{variable}` and expressions, with show-when conditions and animation by the existing wave and track tools. Units are fractions of screen height so text scale and safe margins are global settings.

**NPC and dialogue UI (`ve-npc`).** `DialogueSession` already gives states (idle, listening, thinking, speaking, closed) and a streaming turn stream with fallback and interrupted flags. `ui.dialogue` shows speaker, portrait or nameplate, streaming text with skip, a thinking state, and choices drawn from the NPC's registered intents plus an optional free-text or microphone box. Findings that shape it: delay above about 4 seconds degrades the experience and natural fillers shorten perceived delay (so a thinking gesture and a short filler line, never silence); stream the text; hybrid typed and suggested replies avoid the blank page; shipped games limit LLM chat to ambient characters with text plus a fixed set of game flags, which is what the closed intent vocabulary already is. Memory and relationships feed a journal screen; the mind inspector (retrieved memories, prompt parts, dropped intents, brain state) lives in the studio, not in the game.

## 6. Levels plan

Principle: a level is a generator plus constraints plus a verifier. Constructive generators first (they are controllable and testable); search-based tuning later.

| Generator | Genres | Method | Status |
|---|---|---|---|
| Open terrain, rivers, flora, city | open world, exploration, racing scenery | existing nodes | **have** |
| Room graph (rooms, doors, keys, loops) | roguelike, horror, shooter, adventure | graph grammar from a flow graph (`game.flow`), then room layout | missing |
| Platformer chunks | platformer, runner | a library of hand-authored chunk recipes assembled under reachability rules, as in Spelunky-style generation | missing |
| Track from a spline | racing | closed spline, road, barriers, checkpoints, scenery strips, AI line | missing (`street.road` exists) |
| Arena plus waves | shooter, survival, tower defence | arena shape, cover, spawn points, a wave schedule with a difficulty curve | missing |
| Tile boards and constraint puzzles | puzzle, card tables, tycoon plots | grid or wave-function-collapse tiles; puzzle generation by reverse play from the solved state | missing |
| Interiors | horror, adventure, social | building shells with room subdivision, furniture kits | partial (`building.asset`) |

Each generator takes a seed and a difficulty parameter, returns a level plus its flow graph, and is checked by `goal_reachable` and `no_softlock` (section 7). Sources are thin for racing tracks and arena waves; those are designed from first principles and measured.

## 7. Verification plan

- **Design checks in `check`:** `goal_reachable` (on the flow graph and navmesh), `no_softlock` (a state from which no win is possible, found by search over the deterministic snapshot), `pacing` (time to first reward, rewards per minute, longest quiet stretch), `feedback_coverage` (every action has a sound or visual response), `content_depth` (counts of levels, enemy or obstacle types, progression beats against the kit's minimums), `ui_contrast` and `ui_scale`.
- **Persona bots:** cautious, greedy, explorer, speed-runner, button-masher, on top of idle, random and seek, per genre action set; a bot run produces a replay trace and screenshots.
- **Replay review:** screenshots from the bot's play, judged against a presentation checklist (readable HUD, no placeholder look, consistent style), because the benchmark shows judges weigh depth and presentation at 70%.
- **One `verify` report** a model can act on, with a fix for each finding.

## 8. Architecture decisions

1. **Mechanic modules, not genre code.** Each module (controller, health, grid, cards, economy, inventory, vehicle, perception, rhythm, waves, selection) is a small reusable component with parameters, like a GDevelop behaviour, authored as `game.*` nodes over deterministic Rust in `ve-game`. A genre kit is a recipe in the guide that turns modules on. The engine knows no genre word.
2. **Determinism stays.** Every module runs in the 60 Hz fixed step with a seeded generator, so bots, replays, undo and save work for all of them.
3. **UI renders in the engine.** In-game UI is engine geometry and a small shader (so exports and replays have it); the studio only edits and inspects it.
4. **Few crates.** New algorithm families are modules of `ve-game`, `ve-world` (level generators) and a new `ui` module family; a new crate needs an ADR. Files stay under 400 lines.
5. **ADRs needed before building:** game UI and fonts; mechanic modules and controllers; turn-based and card systems; economy and inventory; level generators; verification suite.

## 9. Roadmap (ordered by what unlocks complete, losable, good-looking games soonest; no dates)

**P0 Foundation (every genre needs it).** F1 controllers and cameras, F2 combat and health, F3 flow, save and undo, F4 UI v1 (fonts, theme, panel, text, bar, button, menu, screens) with focus navigation and accessibility defaults, F5 feel presets and procedural music, F6 input map and prompts, the look presets (clean stylised first). Demo: the zombie scene becomes a game that can be lost, restarted and won, in third person, with menus, in two looks.

**P1 Action genres.** Platformer pack (controller tuning, chunks generator, checkpoints), shooter and arena pack (weapons, waves, arena generator), racing pack (vehicle, track generator, laps, AI). Demos: three games from three prompts.

**P2 Systems genres.** Grid and puzzle (board, turns, undo, hints, level select), card and turn-based (zones, turn manager, opponent AI), economy, tycoon and idle (resources, production, upgrades, placement), narrative and visual novel (dialogue graphs, quests, journal, dialogue UI with `ve-npc`), strategy and tower defence (selection, orders, production, fog, minimap). Demos: one game per family.

**P3 Depth genres.** Roguelike (room graph, loot tables, run and meta state, seeds), survival-crafting (needs, gathering, crafting, build mode), stealth and horror (perception, scare director, diegetic UI), rhythm (beat clock, charts), sports (ball physics with spin, team AI), fighting (hitboxes, combos).

**P4 Verification and evaluation.** The full `verify` suite, persona bots per genre, replay review; then run the briefs across models and tool-call variants and fix the language and tools where they fail (this includes the nodes-versus-code measurement, G7). Target: ten prompts, ten games, a table of pass rates.

**P5 Ship.** Start screen, controls hint, load progress, auto quality, share bundle.

Alongside, as already planned: the graphics phases (reflections, atmosphere), NPC mind wiring, terrain LOD and streaming.

## 10. Coverage check (can the plan make each family?)

| Family | Needs (modules and level generator) | Phase |
|---|---|---|
| Platformer | F1, F2, F3, chunks | P1 |
| Strategy | M-RTS, M-ECON, F4 dense UI | P2 |
| Tycoon, idle | M-ECON, grid placement, UI-first | P2 |
| Open world | F1, M-NARR (quests), terrain (have), save | P0 + P2 |
| Roguelike | F2, room graph, loot tables, meta state | P3 |
| Visual novel | M-NARR, dialogue UI, staging | P2 |
| Puzzle | M-GRID, undo, tile generator | P2 |
| Shooter | F1, F2, arena + waves | P1 |
| Simulation | M-ECON, NPC schedules, M-VEHICLE as needed | P2 to P3 |
| Card | M-CARD, turn manager | P2 |
| Horror, stealth | M-STEALTH, F5 audio, diegetic UI, room graph | P3 |
| Rhythm | M-RHYTHM | P3 |
| Racing | M-VEHICLE, track generator | P1 |
| Sports | M-SPORT | P3 |
| Survival-crafting, tower defence, fighting | M-INV, M-WAVES, hitbox combat | P2 to P3 |

## 11. Risks and open decisions for the owner

1. **Fonts:** procedural skeleton fonts (recommended, matches the no-assets rule) or an exception for open-licence font files.
2. **Order:** this plan puts foundation and UI first, then action genres. Is that the genres you want first?
3. **Dialogue input:** choices first with optional free text (recommended), or free text first; microphone and spoken replies in the first version or later.
4. **Multiplayer:** confirm it stays out of scope for now.
5. **2D games:** decided 2026-11-06, pure 3D. There is no 2D sprite path. A side-scroller or top-down game is a camera and controller mode over the same 3D world.
6. **Risk:** breadth. Mitigation: modules are small and each lands with a demo and a verifier; a genre is declared supported only when a bot completes a generated game of it and the replay review passes.
7. **Risk:** thin sourcing for some genres' UI and level design (puzzle, rhythm, racing tracks, arena waves). Mitigation: prototype and measure, and record findings here.

## 12. Sources
[GameCraft-Bench](https://arxiv.org/html/2606.17861v1) · [GameXpert-Bench](https://arxiv.org/html/2608.21833v1) · [GDevelop behaviours](https://wiki.gdevelop.io/gdevelop5/behaviors/all-behaviors/) · [Game mechanics taxonomies (Wikiversity)](https://en.wikiversity.org/wiki/Game_mechanics) · [Shell's taxonomy and Adams and Dormans](https://www.oreilly.com/library/view/practical-game-design/9781787121799/8cceb529-abde-4755-be15-4e021b5c4763.xhtml) · [Procedural dungeons and levels (Liapis)](https://antoniosliapis.com/articles/pcgbook_dungeons.php) · [Beyond the HUD (Fagerholt and Lorentzon)](https://www.researchgate.net/publication/277202228_Beyond_the_HUD_-_User_Interfaces_for_Increased_Player_Immersion_in_FPS_Games) · [Game UI Database](https://www.gameuidatabase.com/index.php?scrn=1) · [Steam accessibility](https://partner.steamgames.com/doc/accessibility_features) · [Xbox accessibility 102](https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/102) and [112](https://devdocs.xbox.com/gaming/accessibility/xbox-accessibility-guidelines/112) · [LLM agent latency in conversation](https://arxiv.org/abs/2507.22352) · [Long Dark HUD](https://www.gamedeveloper.com/design/long-dark-survival-game-hud-ui) · [Dead Space](https://en.wikipedia.org/wiki/Dead_Space_(2008_video_game)) · [Game camera perspectives (Unity)](https://unity.com/blog/video-game-camera-perspectives) · [Puzzle design](https://gamedesignskills.com/game-design/puzzle/) · [Racing design](https://gamedesignskills.com/game-design/racing/) · [Meta-progression](https://heroiclabs.com/blog/what-is-meta-progression-in-games/)
