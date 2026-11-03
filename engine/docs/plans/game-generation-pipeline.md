# Game generation pipeline (2026-11-03)

Status: proposed. Not started. Sits above `game-world-system.md` (the systems) and `graphics-plan.md` (the look): this file is the path from one sentence of prompt to a finished, playable, good-looking browser game, and what each step needs that we do not have yet.

Target: desktop browser, keyboard, mouse, gamepad. Everything procedural and reachable from tool calls and the node language. A game is an example, never part of the engine.

## 1. The pipeline (one prompt in, one shareable game out)
A model (or a person) does these stages through the existing tools; each stage has an artifact the next stage reads, so it can be checked and re-run alone.

| # | Stage | Artifact | Tools that exist | What is missing |
| --- | --- | --- | --- | --- |
| 1 | **Brief** | one page: genre kit, setting, camera mode, session length, win and lose, difficulty, tone | `get_guide` | a brief schema and a `plan` tool that fills it from the prompt and echoes it back |
| 2 | **Style bible** | palette (5 to 8 colours), shape language, material set, light and grade preset, camera lens, UI skin | grades and material library in the guide | a `look.*` preset system (saved parameter sets for sky, environment, grade, fog, outline) and a palette-driven material generator |
| 3 | **World** | terrain, city, interior or arena built by generators | terrain, river, scatter, flora, `city.*` | interior and arena generators (room graph, layout from flow), level flow graph |
| 4 | **Cast** | player, enemies, NPCs, props, pickups from kits | `body.human`, creature kits, plant kits, `make_character` | a clip library and retargeting, a prop and creature kit set per style, readable faces |
| 5 | **Logic** | variables, kinds, rules, goals, flow | `game.*` nodes, JS scripts | combat and health, controller and camera modes, flow states and restart, inventory and quests, physics queries in rules |
| 6 | **Feel** | audio, particles, camera shake, UI, menus, hit feedback | procedural audio, particles, HUD text | soft particles and trails, real text, menus, screen effects |
| 7 | **Verify** | budget, completability, softlocks, look | `check`, `playtest` (idle, random, seek), `snapshot` | `goal_reachable`, `no_softlock`, `pacing`, `check look`, `simulate`, scripted persona bots |
| 8 | **Ship** | one URL, one start screen, one controls hint | export glb, static bundle | start screen and pause, load progress, quality auto-select, a share bundle |

## 2. The variables a brief fixes (so every game is a point in a small space)
- **Genre kit** (a recipe, not code): collect and escape, key and door, arena survive, shooter, platformer, racer, puzzle, social hub, exploration, rhythm-free sports. Each kit lists the systems it turns on.
- **Camera mode:** first person, third person follow, top down, side scroll, fixed.
- **Setting:** forest, canyon, city, interior, space station, village (the briefs in `game-world-system.md` section 10).
- **Scale and session:** world size, 2 to 10 minutes of play, number of enemies and goals.
- **Art style:** one of the presets below.
- **Time and weather:** dawn, noon, dusk, night; clear, fog, rain.
- **Difficulty:** enemy speed, health, spawn rate, time limit.
- **Feel:** sound set, hit pause, camera shake, particle intensity.
Every variable is a parameter of a node or a preset, never a code branch, so the model can change it by one tool call and the verifier can sweep it.

## 3. Art style (the owner decides; recommendation first)
A procedural pipeline is judged on consistency more than on realism, and a restrained style hides what generators cannot do yet (faces, hair, foliage, facade depth). Three presets, all built from the same nodes and the `look.*` system:
1. **Clean stylised (recommended default).** Simplified forms, a limited palette per game, flat or two-step shading with a thin outline (the toon and outline shaders already exist), strong sun and soft shadows, bloom, a colour grade. Characters read at a distance; procedural flaws vanish. Works today with small additions.
2. **Painterly low-poly.** Faceted shapes, vertex or gradient colour instead of textures, soft fog, warm light. Cheap, very consistent, low memory.
3. **Realistic PBR.** The path of the Paris and NYC districts: reflections, volumetric light, decals, real facade depth. Highest ceiling, most work (all of `graphics-plan.md`), and the ceiling is where generators look worst when they fall short.
Proposal: ship 1 first as the house style and the default for generated games, keep 3 as the showcase look for worlds, add 2 only if a game asks for it. Decision needed from the owner.

## 4. What we lack, in build order
Order is by what unlocks a complete, losable, good-looking game soonest.

**P0. Make one game complete (3 to 4 weeks of work, each item test first).**
- Combat and health: entity health, damage and death events, melee overlap and projectile or hitscan hits, effects on hit (approval pending from the owner).
- Desktop controller and camera modes: grounded character with gravity, jump, collision against game solids and terrain, key and gamepad bindings, first and third person cameras.
- Flow: title, pause, game over, restart, checkpoints, save and load (extends `ve-game` snapshot/restore).
- Demo: the zombie game becomes a game: it can be lost, restarted and won, third person, keyboard and mouse.

**P1. Style and feel.**
- `look.*` presets and the clean stylised preset (palette generator, outline and two-step shading as material options, grade).
- Soft particles, trails, hit flashes, screen shake presets, real text and menus (graphics plan G6).
- Demo: the same game in two presets, one prompt change.

**P2. Cast and levels.**
- Clip library and retargeting; fix the skinning tears; one stylised character and creature kit set.
- Interior and arena generators from a flow graph (rooms, doors, keys); genre kits as recipes (arena survive, key and door, collect and escape).
- Demo: three different games from three prompts, each built from kits.

**P3. Verification loop.**
- `goal_reachable`, `no_softlock`, `pacing`, `check look`, scripted persona bots, `simulate`; one `verify` report a model can act on.
- Demo: a model builds a game, runs `verify`, fixes what fails, and the report passes.

**P4. Evaluation and shipping.**
- Run the briefs through the pipeline with models across tool-call variants (M7), score by checks, bot outcomes and visual review, and fix the language and tools where models fail.
- Start screen, controls hint, load progress, auto quality, share bundle.
- Demo: ten prompts, ten games, a table of pass rates.

**Alongside.** The graphics plan phases G1 and G2 (reflections, atmosphere) raise the look of every preset; NPC minds (`ve-npc` wiring) and the physics queries for rules slot after P2.

## 5. Evidence we already have
- Models have built games through the tools: Blackout 2 (119 calls), minigolf (62 calls), the outpost (139 calls, 28 friction items fixed). None was built from a one-line prompt by a fresh model, and none was evaluated across several models; that is P4.
- Bots can already play wins on those games; they cannot yet prove completability or find softlocks.

## 6. Risks and open decisions
- **Owner decisions:** the house art style (section 3); combat module approval; whether `look.*` lives as nodes or as a tool-level preset.
- Style consistency across generators (a city and a creature made by different nodes must share a palette): the style bible must drive all of them through shared parameters, which means every generator takes a palette and a roughness range.
- Build time of large worlds (minutes) fights an iterate-and-verify loop: P3 needs a fast preview quality for checks.
- One prompt may need dozens of tool calls; long loops are flaky on some providers (`agentrouter-access`), so the pipeline needs resumable stages.

## 7. Log
(empty)
