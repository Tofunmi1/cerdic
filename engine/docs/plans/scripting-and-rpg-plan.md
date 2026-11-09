# Scripting layer and the flagship action RPG (v1, 2026-11-07)

Status: partially implemented; see `game-extensibility-plan.md` for the active cross-genre sequence. Decisions by the owner on 2026-11-07: **JavaScript only** (easier for models, runs in the browser) and **a mix**: declarative `game.*` rules, typed nodes and JavaScript behaviours working on one simulation. The script host, `rpg` and `combat` libraries, game UI first cut, and Wukong boss-fight example now exist. Items/inventory, quests/dialogue integration, full campaign progression, and the AAA acceptance bar remain open. The flagship is an example built on the engine, never part of it (memory: games are examples).

## 1. Why this exists

The check on 2026-11-07 found the scripting pathways incomplete. The root cause is that two worlds do not share a clock or an event stream:

- `game.*` rules run in `ve-game`'s deterministic 60 Hz step.
- `script.js` nodes run in Web Workers on the page's frame clock, with world calls answered one frame later, a seeded `Math.random`, and only pointer, clip, ragdoll and page-sent key events.

So scripts cannot be replayed, cannot be played by the bots, cannot react to touch, collision, spawn or destroy, and cannot read game variables. Everything else in the gap list follows from that.

## 2. Evidence (searched 2026-11-07; secondary sources are marked, weak spots stated)

| Question | Finding | Consequence |
|---|---|---|
| Code or fixed tool calls? | CodeAct (ICML 2024): equal on single calls, 74% vs 52% success on multi-tool tasks, about 30% fewer turns ([overview](https://www.alphaxiv.org/overview/2402.01030v4), secondary). | Keep a general script pathway next to typed nodes. |
| Declarative rules enough? | Ludii covers 1,000+ board and card games; VGDL only 2D arcade; GGP-GDL the most restricted ([Ludii](https://arxiv.org/pdf/2205.00451)). Grammar-constrained generation works for CFG languages ([paper](https://arxiv.org/pdf/2407.17404)). | `game.*` stays the high-level path; scripts are the escape hatch. |
| Language | Python 91 to 97% vs Lua 76 to 84% on matching benchmarks; Lua called low-resource ([paper](https://arxiv.org/pdf/2606.16827)). No data for Luau or GDScript; JS varies by benchmark. | JS (owner's choice) is defensible. A measurement on our own tasks is still needed (P4). |
| Determinism | A rollback RTS shipped ten years on plain floats by controlling the compiler ([post](https://dev.to/chromealex/ten-years-of-rollback-netcode-in-a-cross-platform-rts-without-fixed-point-2c71)); Rune pinned clock and RNG and rounded math results ([post](https://developers.rune.ai/blog/making-js-deterministic-for-fun-and-glory)); QuickJS-in-WASM can snapshot VM state ([quickjs-wasi](https://github.com/vercel-labs/quickjs-wasi)). | Run QuickJS **inside our own wasm**: the same binary runs everywhere, and WebAssembly arithmetic is deterministic apart from NaN payloads (reasoned, to be tested). The browser's own JS engine is never used for simulation scripts. |
| Containment | Luau docs: isolate untrusted code in its own VM; Roblox sandboxed assets get no capabilities until granted ([Luau](https://luau.org/sandbox/), [Roblox](https://devforum.roblox.com/t/protect-your-games-with-script-capabilities-sandboxing/4634642?page=3)). QuickJS has an interrupt handler, `JS_SetMemoryLimit` and `JS_SetMaxStackSize`; the interrupt is uncatchable, but some built-ins loop in native code without polling it ([PR notes](https://github.com/kwallaschek/surimono/pull/5)). | Capability grants per script; interrupt budget per call; memory and stack caps; a counting allocator; no ambient capabilities at all (the embedded VM has none unless we add them). |
| Fixed step | Fixed timestep with render interpolation gives run-to-run reproducibility ([Gaffer](https://gafferongames.com/post/fix_your_timestep/)). Input should be sampled once per step, not inside handlers (one blog, verify). | Input is recorded per step as part of the replay; scripts see the step's input snapshot. |
| Engine lifecycles | Unity/Unreal init, begin-play, tick, destroy; Godot signals avoid manual unsubscribing; big Blueprint graphs are hard to review and diff ([map](https://godotlearning.com/equivalents), [Blueprint vs C#](https://scriptsforunity.com/migration/scripting-blueprint-vs-csharp/), blogs). | Per-entity handlers with scope-bound subscriptions; scripts stay text that diffs well. |
| Combat | Hitboxes and hurtboxes move with the animation and are live for active frames; fast ones need swept checks; hitstop on contact; cancel windows and input buffering; poise or stagger meters; animation timing must be decided up front ([frames](https://sites.google.com/site/scrubfighter/understanding-the-mechanics-frames-hitboxes-and-hurtboxes), [ARPG design](https://howtomakeanrpg.com/r/a/realtime-combat-design.html)). | Combat is data on animation timelines (windows, events), resolved in the step. |
| Stats and abilities | Unreal's GAS: attributes with base and current value, modifiers aggregated by one owner, effects as the only way to change attributes, hierarchical tags gating activation and state, damage through a transient meta attribute ([Epic docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/understanding-the-unreal-engine-gameplay-ability-system), [practitioner guide](https://vorixo.github.io/devtricks/gas/)). | Same shape: attributes, effects, tags, abilities as data; scripts compute, never poke numbers directly. |
| Enemy AI | Layered: state machine for modes, utility scoring for choice, small state machines or trees for scripted attack sequences; telegraphs make bosses readable ([overview](https://www.socratopia.app/library/game-code-anatomy-en/chapter-12), [boss guide](https://artificial-intelligence-wiki.com/industry-ai/ai-in-gaming/ai-boss-fight-design/); forum-level sources). | `ve-npc` already has FSM, trees, utility, HTN: wire it to the bus and make attacks timeline assets. |

**Not established:** per-call marshalling cost of QuickJS in wasm with hundreds of entities (no public benchmark; we measure in step 1); Luau or GDScript against JS for models; nodes against code on our tasks; cross-browser identical results of our build (test). Sources on combat and AI are practitioner-level, not studio postmortems.

## 3. Design

### 3.1 One simulation, one bus, one clock
- The 60 Hz step in `ve-game` is the only clock. Physics contacts, input, animation events, spawn, destroy, variable changes and timers all publish **typed events** into one ordered queue per step (order: input, script timers, physics, animation events, rules, scripts reacting, cleanup). Order is documented and tested.
- `game.*` rules are handlers on the bus. Scripts are handlers on the same bus. The two mix freely: a rule can `emit` a custom event a script hears and the reverse.
- The page's frame loop only renders (interpolated). Web Workers stay for the editor console and tools (`script` tool), never for simulation.

### 3.2 The script host (module `ve-game::script`, no new crate)
- QuickJS (`rquickjs`, already used by `poly.script` in `ve-modeling`) running in the runtime's wasm. One VM per game, one realm per script (isolation without one VM per script); a snapshot is the heap plus the script state, so **save, load, undo and replay work for scripts too**.
- **Deterministic by construction:** `Math.random` is the simulation's seeded generator; `Date` and `performance` do not exist, `time()` is the step count; math results from our own libm build; no `Intl`, no async scheduling outside the step (`await` on step timers only); object key order is insertion order, so iteration is stable.
- **Limits:** instruction budget per step through the interrupt handler (default 2 ms equivalent, counted in instructions, not wall time, so it is deterministic too), memory cap with a counting allocator, stack cap. A script that exceeds its budget is stopped with a typed diagnostic naming the script, the handler and the last event, never a freeze.
- **Capabilities** granted per script in its node (`reads: ["game", "physics"]`, `writes: ["game.vars", "spawn"]`, `ui`, `audio`, `camera`, `storage`); everything not granted is simply absent. Packs get their manifest's list.

### 3.3 The API (small, regular, typed)
One namespace per concern; all synchronous inside the step; every function returns plain data; a generated `game-api.d.ts` ships with the guide, and the typed node `script.js` is checked against it (type errors become diagnostics before the game runs).

```
on(event, handler) / on(event, selector, handler)   // scoped to the script or entity; removed automatically with it
every(seconds, fn) / after(seconds, fn)             // step-based timers, saved in snapshots
entity(id|name) / find(tag|kind|query) / spawn(kind, props) / destroy(e)
e.pos, e.vel, e.tags, e.get(attr), e.has(tag), e.send(event, data)
attr: base, current, modifiers          // read; changes only through effects
fx.apply(target, effectId, source) / fx.remove(...)   // the only way attributes change
ability: grant, can(), start(), cancel()              // data from ability assets
physics: raycast, sweep, overlap, impulse
anim: play(clip, opts), on timeline events 'hit_on' 'hit_off' 'cancel_open' 'footstep' ...
nav: path, move_to, crowd
npc: say, mind(brain), perceive
ui: widgets (see game-maker-plan section 5), audio: sfx, music layers, camera: mode, shake, fov, lock_on
game: vars (read, write with rules), flow (pause, checkpoint, save, load, level), rng
input: action map snapshot per step (pressed, held, released, axes, buffered)
```
Per-entity behaviours: `behaviour("name", { on_start, on_tick, on_hit, on_touch, on_damaged, on_death, on_destroy })` attached with `game.behaviour@1(kind=…, source=…)` and run by the step only for entities that have it (a list, not a global scan).

### 3.4 The mix: three authoring levels, one runtime
1. **Typed nodes** (`game.*`, `rpg.*` below): the default for common things; cheapest, validated, diffable, previewable.
2. **JS behaviours**: for anything a node set does not say; the model writes a few lines, calling the API above.
3. **Packs**: reusable bundles (nodes + scripts + assets + manifest). The RPG pack is the first big one.
Selection rule written into the guide: use a node if one exists, a script if not, and when a script is written twice, propose a node. P4 measures this on real prompts (nodes vs code vs mixed).

### 3.5 Verification
- **Replay:** inputs per step plus the seed reproduce the run; a hash of the world state every 60 steps is compared; a mismatch names the first diverging step.
- **Bots** run the whole simulation including scripts (idle, random, seek, and per-genre personas); assertions in the pack (`game.expect`: "the boss can be beaten by seek-bot within N minutes", "no entity leaves bounds", "no script exceeds budget").
- **Script unit tests:** `test("name", ctx => …)` run headless in the step with a scripted input timeline; part of `check`.
- **Cross-browser test:** the same replay in Chromium, Firefox and WebKit must end with the same hash.

## 4. The flagship: a proper action RPG fight game (AAA bar)

**Brief:** a third-person melee RPG in a ruined arena-and-ravine hub. The player (the `body.human` character with armour and a weapon) fights packs of enemies in encounters and a multi-phase boss, with dodge, block, parry, light and heavy attacks, two weapon types, three abilities, a stamina and poise economy, loot and a skill tree, a lock-on camera and full UI. It is built **only through the language, nodes and JS scripts** (no hard-coded look or generators in the engine). Never night unless asked; studio default lighting; no unrequested extras.

### 4.1 What "AAA level" has to mean (acceptance, measured)
| Area | Bar |
|---|---|
| **Feel** | Input latency one step; light attack startup 8 to 14 frames, active 3 to 6, recovery 12 to 20 with a cancel window; hitstop 3 to 8 frames scaled by weapon weight; camera shake and screen punch on heavy hits; input buffer 6 to 10 frames; dodge i-frames 12 frames; parry window 8 frames. All values data, tunable live. |
| **Combat depth** | Combos of 3 light plus finishers, heavy charge, dodge-attack, parry riposte, stagger and poise (break leads to a punish window), status effects (bleed, burn, stun), damage types and resistances, weapon-weight differences, enemy attack priority rules (clash rules written down). |
| **Enemies** | 5 enemy types (grunt, shield, archer, brute, caster) with perception (sight cone, hearing), group roles (attacker, flanker, waiter so only N attack at once), telegraphed attacks with readable windup, retreat and heal behaviours. |
| **Boss** | 3 phases, 8 or more attacks as timeline assets, utility choice between attacks with a state machine for sequences, arena hazards, a stagger window, a phase-change cinematic, a death sequence. Beatable and fair: telegraph length scales with player deaths (adaptive, off by default setting). |
| **Animation** | Locomotion blend (idle, walk, run, strafe, turn), attack set, dodge, block, hit reactions by direction, death, procedural foot IK on slopes, root motion where it helps, ragdoll on death. All from our own clip and state-machine nodes (`ve-animstate`, `ve-gait`). |
| **Camera and controls** | Third-person follow with collision, lock-on with target switching, soft lock, gamepad and keyboard-mouse maps with remap, dead zones. |
| **Progression** | Attributes (vigour, strength, dexterity, intelligence), levels, a skill tree of 20 or more nodes, loot with rarity and affixes from tables, equipment slots with visible gear changes (`body.human` clothes), consumables, a stash. |
| **UI** | Health, stamina, poise, boss bar, target frame, damage numbers, buff icons, inventory grid, equipment screen, skill tree, map or compass, pause, settings, death screen with retry from checkpoint, dialogue box for two NPCs. All through the UI widgets of the game-maker plan. |
| **Audio** | Layered music (explore, combat, boss with phase stems), hit sounds by material and weapon, footsteps by surface, voice barks as text-to-sound intents, procedural from our audio nodes. |
| **Look** | Full renderer quality: GI, contact shadows, volumetrics at the ravine, hit sparks and blood or dust particles, weapon trails, screen effects, a stylised but coherent set built from the language. |
| **Performance** | 60 fps on desktop Chromium at the default quality with 12 active enemies; script budget under 2 ms a step in the encounter; frame hitches reported by the profiler. |
| **Robustness** | Replay of a full boss run reproduces; save in the middle of a fight and load restores it exactly; bots finish the game; no script errors in 30 minutes of random-bot play. |

### 4.2 Systems the RPG needs (what is new, by workstream)
- **S1 Script host and bus** (section 3): the foundation; nothing else starts before it passes the spike gates.
- **S2 Attributes, effects, tags, abilities** (module `ve-game::rpg`): attribute sets with base and current value, modifier stacking rules (flat, percent, multiplier order), effects with duration, stacking and tags, ability assets (cost, cooldown, tags, timeline), meta-attribute damage pipeline (raw, resist, armour, poise, final). Nodes: `rpg.attributes`, `rpg.effect`, `rpg.ability`, `rpg.damage_rules`. This is the GAS shape (section 2) in data.
- **S3 Combat timelines**: an animation clip gets **named events and windows** (`active`, `cancel_open`, `i_frames`, `parry`, `footstep`, `spawn`), authored in the language beside the clip (`anim.window`, `anim.event`); hit shapes (box, capsule, swept arc) attached to bones for active windows; resolved in the step with swept checks; hit stop as a per-entity time scale; input buffer and cancel resolution in one `combat` module; hit reactions by direction.
- **S4 Controllers and cameras (F1)**: grounded third-person character (slopes, step, jump optional), lock-on camera, camera modes by rules, camera collision, gamepad action map.
- **S5 Enemy brains**: `ve-npc` FSM, utility and tree wired to the bus; perception, group coordination (attack tokens), boss pattern assets; nav crowds with combat spacing.
- **S6 Items and progression**: items, stacks, equipment slots tied to visible `body.human` gear, loot tables with affixes, skill tree data, save slots.
- **S7 UI and flow (F3, F4)**: widgets and screens from the game-maker plan, focus navigation, accessibility defaults.
- **S8 Feel and audio (F5)**: hit stop, shake presets, trails, damage numbers, layered music, juice settings with reduced motion.
- **S9 Content**: arena and ravine from terrain, river, flora, scatter nodes; five enemy models and the boss through `body.*` and fields; two weapons and armour sets; props. Built in the same series style as the dragon, horse and tractor: procedural, no assets.
- **S10 Verification**: replay, bots with combat personas, expectations, script unit tests, cross-browser hash (section 3.5).

## 5. Order (no dates; each step has a gate)

1. **Script host (built 2026-11-07, no measuring phase: the owner chose to build, not benchmark).** QuickJS inside `ve-game` (`world/state/script.rs`, `script/bridge.rs`, `script/prelude.js`), run inside the fixed step; `game.script@1` node; API v1 (`on`, `every`, `after`, `find`, `entity`, `spawn`, `game.*`, `state`, `log`); work budget, memory and stack caps; `state` saved in snapshots and the hash; logs and errors reach the page as `game.script_log` events. Tests: `ve-game/tests/scripts.rs`, `ve-modeling/tests/game_script_node.rs`. Speed is watched through the game's own profiler later, not through a gate.
1b. **`rpg` library (built 2026-11-07, S2 first cut):** `game.script@1(lib="rpg")` loads `lib_rpg.js`: attributes with base and modifiers, hierarchical tags, effects (duration, period, stacking), abilities (cost, cooldown, tag gates), damage pipeline (resist, armour, block, poise and stagger, death), all in `state.rpg`. Tests: `ve-game/tests/rpg.rs`. Demo: `web/tests/scenes/duel/duel.txt` (two fighters trading blows in the studio, verified running in the browser).
1c. **Combat library (built 2026-11-07, S3 first cut):** `lib="combat"`: attacks as timelines (startup, active, recovery, cancel window, combos by `next`), hit arc against hostile actors, input buffer (8 frames), hitstop on both fighters, knockback, lunge, dodges with i-frames, block, 8-frame parry window. Tests: `ve-game/tests/combat.rs`. Not yet: bone-attached hit shapes, swept checks, animation-driven windows (the sim drives animation through `anim` effects), hit reactions by direction.
1d. **Game UI and fonts v1 (built 2026-11-07, the game-maker plan's section 5, first cut):** `ve-game/src/ui.rs` and `ui/font.rs`: procedural skeleton font (95 printable ASCII glyphs as strokes; presets sans, rounded, techno, pixel, hand, bold, italic; axes weight, width, slant, spacing, wobble; no font files), elements `text`, `bar` (with damage trail and label), `panel`, `ui.stack` rows and columns, anchors with a 5% safe margin, units in screen heights; scripts set them by id with `ui.*` in the script host; the engine lays them out (`World::ui_frame`, wasm `game_ui_frame`) and the studio's `GameUi` canvas draws the rectangles and strokes. Tests: `ve-game/tests/ui.rs`. Not yet: icons, buttons and focus menus, lists and grids, minimap and world markers, in-world (diegetic) placement, `ui.dialogue` with `ve-npc`, screens (title, pause, settings), engine-side rendering for exports, damage numbers (needs a world-to-screen call), non-ASCII scripts.
1e. **Flagship demo: "The Monkey King and the Bull Demon" (built 2026-11-07, `web/tests/scenes/wukong/wukong.txt`, served as `worlds/wukong.txt`):** a third-person boss fight, playable with keyboard and mouse. New engine pieces it forced: script input (`input.axis/held/pressed`, page `GameInput` sets `mx mz lx ly block` and presses `light heavy dodge spell`), script camera (`camera.look`, drives the viewer; the studio room is hidden while a game holds the camera), `game.reset()` (restart from the initial snapshot), `e.size`/`e.forward`, hyper-armour tag, stagger cancels the attack in progress. Content: a 25-part animated hero (fur, gold armour, phoenix cap, staff, tail, scarf; machine with idle/run/slash/slash2/heavy/roll/hit/block/cast/death clips), a 6 m lava-cracked bull demon (horns, nose ring, mace, cape; slam/sweep/stomp/roar/charge/breath/death clips), a courtyard on a mountain peak (trigram floor, pillars with lanterns, gate, five-tier pagoda, karst peaks, cloud sea, drifting mist and petals), three scripts (hero controller and camera, boss AI with three phases, telegraphs, fireballs and flame patches, UI and flow). Not verified by eye: the animation clips in motion, real mouse look, gamepad, balance; hit reactions and the boss's attack animations are triggered but only the state changes were checked.
2. **Bus and API v1 (rest):** typed events from physics (contacts, sensors), animation windows and input actions onto the same queue; per-entity `behaviour` handlers; generated `.d.ts`; guide section; capability grants; existing `script.js` nodes (worker host) stay for tools and the editor console.
3. **S2 attributes, effects, tags** with a tiny arena demo (a dummy that takes damage numbers) and script tests.
4. **S3 combat timelines** on a rigged `body.human`: one weapon, light combo, dodge, block; tune the numbers against the feel table with the owner looking at it.
5. **S4 controller and lock-on camera.**
6. **S5 first enemy and an encounter,** then the other four; **S8 feel pass** (hit stop, shake, sparks, sounds).
7. **S6 items, loot, skill tree;** **S7 UI and flow.**
8. **Boss** (phases, attacks as assets, cinematic), **S9 content pass,** **S10 verification suite;** then the whole-game replay, save-in-fight and bot-finish checks.
9. **P4 measurement:** run the same RPG briefs across models with nodes only, JS only and mixed, and record where each fails; feed the failures back into the language and API.

Everything is reachable from the typed graph and the MCP tools (memory: everything via language); the studio can inspect events, attributes, timelines and script logs.

## 6. Risks and open decisions
- **QuickJS speed in wasm** is the unknown that decides the split between per-entity scripts and batched list handlers (step 1 measures it).
- **QuickJS native built-ins can bypass the interrupt** (sort, join, etc. on huge array-likes): cap sizes in our globals or replace those built-ins; tested in the spike.
- **Animation quality for AAA** depends on the clip authoring through the language (poses, blends, IK); the plan assumes `ve-animstate` and `ve-gait` can express attack animations. The first attack animation in step 4 tests that honestly; if weak, an animation-authoring workstream goes ahead of combat.
- **Human body fidelity** (face, hair, hands are deferred by the owner, memory) limits close-up cinematics; the camera and staging in the RPG avoid face close-ups until that returns.
- **Scope:** very large. The gates in section 5 are the control; each step ends with something playable.
- **Open for the owner:** weapon set (sword and shield plus greatsword proposed), art direction (stylised dark fantasy proposed), keyboard-mouse and gamepad both required, whether the boss adaptive-difficulty option ships on.
