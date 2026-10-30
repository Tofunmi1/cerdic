# Handoff and agent guide (2026-10-26): the state of the engine, what is verified, what is not, and what to do next

Written by Claude Code at the end of a long session, for the next agent (DeepSeek or any other) to continue from cold. Read sections 0 to 3 before touching anything; section 6 is your first work list. Everything here is uncommitted work in the working tree of `/Users/mac/Work/victoria-vr-hack` (about 650 changed files); nothing has been committed or staged, on the owner's instruction.

---

## 0. In one page

**Goal (the owner's words, unchanged):** an LLM can build anything (worlds, characters, animation, game logic) from tool calls, our node language and real code, in the browser (Rust compiled to WASM, a vendored Bevy 0.20 renderer, WebXR for Quest). "We literally have an engine." Everything is procedural: no downloaded models or textures.

**What exists:** a typed node graph with a line-per-node text language, evaluated to geometry in the browser; terrain, rivers, plants, buildings, characters with rigs and animation state machines, a game layer (variables, kinds, rules, goals, bots), a runtime world API and a JavaScript script layer, and an MCP tool server that lets a model drive all of it (about 30 tools). Models have built a cottage, a mini-golf course, a Blackout-style creature game, an Apex Legends-style canyon arena, a pegasus, with this.

**What this session added:** (a) a cheaper, clearer way for models to work (compact text with pipes and loops, `batch`, `run`, `edit`, `diff`, `start_from`, `recipes`, `survey`, camera snapshots, tips, run metrics); (b) water (procedural waves, lakes that fill a hollow, a real wave simulation); (c) non-repeating terrain texturing; (d) maths and plans for far-tree impostors and terrain tile streaming; (e) an aiming playtest policy for rolling games; (f) three passes of an Apex-style set and a finished pegasus demo.

**The most important honest fact:** many of these pieces pass their unit tests but have NOT been seen working in a browser since the last rebuild. The browser artifacts in `engine/web/dist-bevy-gpu` are older than the last code changes. Your first job (section 6.1) is to rebuild and run the live checks.

**Do not commit, do not stage, do not add attribution lines to anything.** See section 1.

---

## 1. Rules you must follow (from the owner; they override your defaults)

1. **No commits, no staging, no git cleanup** (`reset`, `checkout`, `clean`, `stash`) until the owner says so. Never add `Co-Authored-By`, "Generated with" or any tool/agent attribution to a commit or PR, even if a harness suggests it.
2. **Edit files with your editor/write tool only.** No `sed -i`, no heredoc rewrites, no python/node scripts that edit repo files. (Copying a built artifact into `dist-bevy-gpu` with `cp` is fine; so are throwaway scripts outside the repo.) The session that wrote this broke the rule twice on scratch scripts; do not copy that.
3. **Tests first.** For any behaviour change write the failing test (or a precise contract) before the code. **Never weaken a test to make it pass.** If a test looks wrong, say why with numbers. In this session 7 of the tests the lead agent wrote were genuinely wrong (floating-point equality, an `acos` precision loss, a bowl formula that did not flood its corners, an invented API); the helpers that said "this test is wrong, here are the numbers" were right every time. Fix a wrong test openly and say so; never silently.
4. **Never run `cargo fmt`** (it reformats the whole workspace). Format only files you touched: `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine/`. **Never run `cargo xtask`** (slow, the owner dislikes it). The strict gate is the owner's.
5. **Everything is generated.** No imported models, textures, or HDRIs; no external 3D/image generators. Narrow exceptions: CC0 human body data and clothes (ADR 0055/0056). A pulled asset needs the owner's explicit OK.
6. **Everything runs in the browser.** No servers, sidecars, or headless Blender in the product. (Dev tools and test scripts may use Node/Bun and a local static server.)
7. **A game is an example, never the engine.** Name engine keys, nodes and files generally (`roll`, `swing`, `pocket`, not `golf`). Game-specific numbers live in the game's own text.
8. **Research before building; report research in chat.** Do not write research notes into the repo unless asked. Building work does carry its ADR/tests/plan-status row (`engine/docs/plans/game-world-system.md` is the world-systems guide: read it before world or game tasks and update its status).
9. **Be honest about verification.** Say plainly what you ran, what you saw, and what you could not check. A model's own report ("verified X") is not evidence; look yourself (`tools/look.mjs`).
10. **No time pressure.** Never cut scope for schedule reasons. Prefer the right fix.
11. **Few crates.** World algorithms are modules of `ve-world`; do not add crates for one feature.
12. **Disk space is tight** (about 17 GB free). Delete finished `/tmp/codex-target-*` build folders. Failed wasm builds can leave stale `dist-*` files.
13. **Use they/them** for people whose pronouns you do not know.
14. Repo rules in `engine/AGENTS.md`: no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason = "...")]`), docs on public items, files under 400 lines, every behaviour change ships with a test, `unsafe` denied outside FFI crates.

The owner's style: wants direct answers, evidence not claims, things opened in the browser when finished (`open -a "Google Chrome" "<url>"`), and uses delegation to helper agents (Codex, `gpt-6.1-sol`) for big implementation jobs, at **medium** reasoning effort.

---

## 2. Repo and runtime orientation

```
engine/
  AGENTS.md                      repo rules (read it)
  crates/
    ve-graph      the node graph, the TEXT LANGUAGE (src/text/), operator specs (src/builtin/), commands, history
    ve-modeling   evaluates the graph to geometry (terrain, rivers, water, plants, characters, game nodes, ...)
    ve-world      world algorithms as modules: terrain, river (+ lake), erosion, scatter, flora, lod, survey, water_sim, terrain_lod, city
    ve-geometry / ve-kernel / ve-kernel-sys   meshes, field/solid kernels (C++ underneath)
    ve-render     OUR custom forward renderer (WGSL in src/shaders/): water_waves, stochastic, grass, wind, forward
    ve-bevy       the Bevy-based path (Bevy materials/extensions: terrain, water): the browser page in dist-bevy-gpu draws through THIS
    ve-runtime    the wasm entry point the page calls; world API (world_api*.rs), game runtime, water sim runtime, survey_plan export
    ve-evaluator  the evaluator wasm (parses text, evaluates the graph in a worker)
    ve-game       the deterministic game layer (rules, kinds, rolling, throwing, creatures, ground fields)
    ve-animstate  animation state machines;  ve-nav navmesh;  ve-physics;  ve-particles;  ve-sky;  ve-gi;  ve-npc ...
  web/
    dist-bevy-gpu/   what the viewer page SERVES (wasm + js + worlds/*.txt). Static server on 127.0.0.1:8801
    demos/           example documents shipped with the viewer (pegasus_studio.txt lives here)
    tests/           browser/Bun tests;   tool-edits.js, aim-policy.js  page-side JS used by the bridge
  tools/
    mcp/server.mjs   THE MCP SERVER (stdio) the models talk to; guide.mjs, hints.mjs, batch.mjs, run.mjs, try-animated.mjs (benchmark harness) ...
    look.mjs         look at a world in the real viewer and save pictures   <-- use this to verify
    *-test.mjs       tool and live tests (listed in section 8)
  docs/
    adr/             decisions (0066 game entities as animated models, 0068 rolling things, 0069 compact text and batched tools, 0070 RedGPU/Trompe)
    plans/           game-world-system.md (the guide to world work), terrain-lod-streaming.md, world-generators.md ...
    handoff-codex-*.md   one file per delegated job: the contract, the tests, and a STATUS section the helper filled in (read the Status of the relevant one)
  third_party/       vendored sources (Bevy, Manifold, Blender subset, ...); redgpu/ and trompe/ are READ-ONLY reference copies (nothing builds from them)
src/engine/          TypeScript bridge (mcp-bridge.ts, snapshot.ts ...), bundled to web/dist-bevy-gpu/viewer-bridge.js
```

**Two renderers.** `ve-render` is our custom forward renderer; the page served from `dist-bevy-gpu` draws through Bevy (`ve-bevy`). A shader feature (water, terrain texturing, grass) can exist in one and not the other. Check which path a feature must reach before calling it live. The water-simulation live bug (section 6.1) was exactly this: the simulation only had a binding in the custom forward path.

**Three browser artifacts must be rebuilt after code changes** (section 3): the runtime wasm, the evaluator wasm, and the page bridge bundle (`viewer-bridge.js`). The page rejects node parameters its evaluator wasm does not know (`parse.unknown_param`), so a new parameter needs the evaluator rebuilt before any world uses it.

---

## 3. Environment and recipes

Run commands from `engine/` unless stated. Use `export CARGO_TARGET_DIR=target` for your own builds.

**Static server (viewer):** `curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8801/` must print 200. If not, serve `engine/web/dist-bevy-gpu` on 8801 with any static server that sends `application/wasm` (for example `cd web/dist-bevy-gpu && python3 -m http.server 8801`). Viewer URL: `http://localhost:8801/?viewer=1&eval=1&scene=stage|open&quality=high&bloom=on&world=worlds/<file>.txt` (`stage` is the studio room for characters; `open` is for landscapes; a world file must be in `web/dist-bevy-gpu/worlds/`; copy demos there to serve them).

**Rebuild the browser artifacts** (3 to 8 minutes; do it after any change to `ve-*` crates or `src/engine/`):
```
cd engine; export CARGO_TARGET_DIR=target
# runtime
cargo build -q -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere
target/tools/bin/wasm-bindgen --target web --out-dir web/dist-bevy --no-typescript target/wasm32-unknown-unknown/web-dev/ve_runtime.wasm
cp web/dist-bevy/ve_runtime.js web/dist-bevy/ve_runtime_bg.wasm web/dist-bevy-gpu/
# evaluator (needed whenever node specs or the text parser change)
cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release
target/tools/bin/wasm-bindgen --target web --out-dir web/dist-bevy-gpu --no-typescript target/wasm32-unknown-unknown/release/ve_evaluator.wasm
# in BOTH web/dist-bevy-gpu/ve_runtime.js and ve_evaluator.js replace every  from "wasi_snapshot_preview1"  with  from "./wasi_snapshot_preview1.js"  (use your edit tool)
# page bridge (from the REPO ROOT, not engine/)
cd ..; bun build src/engine/viewer-bridge.ts --outfile engine/web/dist-bevy-gpu/viewer-bridge.js --target browser --format esm
```
Verify: no bare `from "wasi_snapshot_preview1"` left; file times are new; `grep -c survey_plan web/dist-bevy-gpu/ve_runtime.js` is above 0.

**Native tools (fast, no browser):**
```
cargo build -p ve-modeling --example eval_text   # evaluates a world text natively: ./target/debug/examples/eval_text web/dist-bevy-gpu/worlds/X.txt
cargo build -p ve-modeling --example ops         # operator reference: ./target/debug/examples/ops [family | op@1 | --search word]
```
`eval_text` prints each part's triangles and box, total triangles against the provisional Quest budget (300,000), and errors. If the binary is missing, rebuild it.

**Browser tests need Chromium for Playwright:** `cd engine/web && bunx playwright install chromium` if `~/Library/Caches/ms-playwright` is missing (it vanished once this session).

**Look at a world:** `bun tools/look.mjs worlds/<file>.txt <prefix> --cams '[{"position":[x,y,z],"target":[x,y,z],"label":"a"}]'` (up to 6 per call; or `--tour`, `--times`, `--states`); pictures go to `/tmp/ve-look`. A heavy world takes minutes to evaluate in the page; the tool waits for it. **Trap:** choose camera heights from the real ground (`world` tool `ground.sample`), never from a guess: a camera under the terrain shows a flat blue plane (this misled the lead agent about the Apex set).

**The benchmark harness (a model builds something through the MCP tools in a real viewer):**
```
bun tools/mcp/try-animated.mjs --agent codex --model gpt-6.1-sol --effort medium --port <unique 87xx-88xx> --name <world> [--from worlds/<seed>.txt] "<request>"
```
It saves `web/dist-bevy-gpu/worlds/<name>.txt/.json` (the world, the calls and tokens), prints `metrics:`, and `--repeat N` runs it N times. `bun tools/compare-runs.mjs <names...>` prints a table. A seed world loads through the page's `world=` URL (sending big text through the page API timed out).

**Helper agents:** Codex CLI `codex exec -m gpt-6.1-sol -c model_reasoning_effort=medium -s workspace-write --json -o <out> -C <repo> "<prompt>"` with its own `CARGO_TARGET_DIR=/tmp/codex-target-<name>`; it cannot bind ports or run a browser, so you run live tests yourself. It hit its usage limit once (14:42 to 17:57 on 10-26); a queued retry script worked. `gpt-6-luna` is the cheaper model for mechanical jobs (rebuilds). A job's contract lives in `engine/docs/handoff-codex-<name>.md` with a Status section the helper writes.

**AgentRouter and opencode:** `agentrouter.org` only accepts approved clients (Claude CLI, Codex CLI, opencode). `~/.config/opencode/opencode.json` has providers `agentrouter` (responses API) and `agentrouter-chat`, key in `~/.config/opencode/agentrouter.key`. Single requests work on `gpt-6.1-sol`, `gpt-6-astra`, `gpt-5.6-sol`; long tool loops fail randomly ("item was created under a different Azure OpenAI resource"), the sol models hit a token rate limit, `deepseek-v4-flash` had no channel (503), `gpt-6-astra` once returned 401. Do not rely on it for long agent loops.

---

## 4. The language and the tools (what a model building a world uses)

The text language is one statement per line: `name = family.op(inputs, key=value)`; outputs are `output name = node`. `@1` is optional (latest). New this session (ADR 0069), all expand to the same typed commands:
- **Pipes:** `post = solid.box(size=[0.1, 1.2, 0.1]) | xform.move(by=[0, 0.6, 0]) | xform.array(count_x=3, spacing=[0.75, 0, 0])`. Each stage needs parentheses even with no arguments (`| inst.realize()`). Stage names are `post_1`, `post_2`, the last is `post`. A taken name is `parse.name_taken`.
- **Loops:** `for i in 0..5` / `for i in [list]` / `for [x, z] in [[1,2],[3,4]]` ... `end`; `{expression}` in values and names (`stones_{i}`); `let size = 160` constants; nesting to depth 3; 2,000-statement limit; no `group` inside a loop; negative numbers in names print as `m1`.
- **Near-miss forgiveness:** `size=2` fills a 3-vector, enum case is forgiven, and misspelled operators, parameters, ports and node names get "Did you mean ...?".
- `bun tools/mcp/server.mjs --guide <topic>` prints guide topics (compact, world, plants, water, animation, ...). `get_text` answers in compact form by default (`format: "canonical"` for the stored form).

**MCP tools** (definitions total about 11.7k characters; keep under 13k: `bun tools/tool-surface-test.mjs`): `get_guide` (takes `topics: [...]` and an `operators` selection in one call), `list_operators`, `apply_text`, `apply_commands`, `edit` (set/rename/delete/replace by label), `diff`, `batch` (up to 32 steps in one call), `run` (JavaScript that calls the tools; only its return value comes back), `start_from` (working templates: `rolling-course`, `creature-game`, `chase-arena`, `district`), `recipes` (save/list/show/delete reusable groups), `survey` (+ `snapshot {tour}`) to pick cameras, `snapshot` (`camera`, `cameras` up to 6, `times`, `states`, `views`), `check`, `measure`, `scene_overview`, `find`, `playtest` (policies `idle`, `random`, `seek:`, and `aim:<thing>:<targets>` for rolling games), `world` (runtime world API: `world.bounds`, `ground.sample/profile`, `water.disturb/height/status`, `animation.*`, `fx.burst`, `world.batch` ...), `script`, `game_describe`, `make_character`, `bake`, `export_glb`. The server also sends `instructions` and appends one-time `Tip:` lines to plain answers when a model's call pattern shows a cheaper way.

**Traps that cost the lead agent time (put these in every prompt you write for a model):**
- `scene.sky` and `scene.environment` must be **outputs** (`output sky = sky`) or the scene is lit by a dim default. The outliner shows "not used by a part".
- A rigged creature plays its main clip forever unless an **`anim.machine`** on the ROOT bone chooses clips (see the guide's animation topic). The pegasus demo lacked it.
- Pipe stages need `()`.
- Heavy worlds (1,000+ nodes) take minutes to evaluate in the page; wait for `eval_projection_status().committed.quality === "final"`.
- `terrain.place` / `scatter.place` for repeated things; each species is its own output; textures share about 32 layers.
- A model's "check passes" can coexist with a broken game (mini-golf hole 2): run the aiming playtest, do not trust a teleport test.

---

## 5. What is done and verified (evidence you can re-run)

Counts are tests passing when last run in this session. Re-run before relying on them.

| Area | What | Evidence |
|---|---|---|
| Phase 1 core (earlier) | world API merge-patch, `world.spawn_entity`, `world.bounds`, animation params, `world.batch`, `fx.burst`, `ground.sample/profile`, script layer | `tools/world-api-test.mjs` (42 checks), `tools/script-test.mjs` |
| ADR 0066 | game entities as animated models (per-entity clones, `heading`) | `tools/model-instance-test.mjs` |
| ADR 0068 | rolling things, ground fields, hand-held tools; `game.terrain/surface` nodes | `ve-game` tests/rolling.rs (21), `ve-modeling` game_rolling_nodes (5), `tools/rolling-test.mjs` |
| Blackout 2 | creatures, throwing, navmesh | `tools/creature-test.mjs`, `tools/blackout2-verify.mjs`; `web/tests/game-blackout.mjs` |
| Mini-golf | `worlds/minigolf2.txt` wins with simulated swings (1, 1, 3 strokes) | `bun tools/minigolf-verify.mjs worlds/minigolf2.txt` |
| Compact text (ADR 0069) | pipes, loops, compact printer, near-miss | `cargo test -p ve-graph` (about 215): g70_compact_text 21, g71_compact_edges 16, g73_near_miss 10 (+6), g72_tool_lines, g70_guide_examples |
| Tools (ADR 0069/0061) | `batch`/`run`, `edit`/`diff`, `start_from`/`recipes`, orientation, hints, metrics, survey, snapshot cameras | `tools/batch-run-test.mjs` (23), `tool-surface-test.mjs`, `recipes-test.mjs`, `orientation-test.mjs`, `hints-test.mjs`, `run-metrics-test.mjs`; live: `mcp-edit-diff-live-test.mjs` (10), `snapshot-cameras-live-test.mjs`, `templates-live-test.mjs` (4 templates apply); `cd web && bun test` (about 40) |
| Survey | `ve_world::survey` + modes walk/orbit/auto, tool + `snapshot {tour}` | `cargo test -p ve-world --test survey --test survey_modes` (15 + 5); live tour worked |
| Procedural water waves | sum of dispersive sines, uniform table shared CPU/GPU | `cargo test -p ve-render --test water_waves` (12) |
| `water.fill` | lake fills the hollow, shoreline follows ground, depth/foam like rivers | `cargo test -p ve-world --test lake` (11), `-p ve-modeling --test water_fill` (10) |
| Stochastic (non-repeating) texturing | CPU reference + WGSL + `no_repeat` params (`terrain.material` default on) | `cargo test -p ve-render --test stochastic`; `-p ve-modeling --test stochastic_nodes` |
| Octahedral impostor maths | mapping, 3-frame selection, plan data | `cargo test -p ve-world --test octahedral --test octahedral_edges --test octahedral_plan` (22) |
| Terrain quadtree LOD | tile selection, balancing, morph, load/unload diff; renderer plan only | `cargo test -p ve-world --test terrain_lod` (10 + 5 unit); `docs/plans/terrain-lod-streaming.md` |
| Water simulation (CPU) | linear shallow-water solver, 14 physics tests | `cargo test -p ve-world --test water_sim` (14) |
| Renderer | all of `ve-render` | `cargo test -p ve-render` (about 198) |

**Benchmarks (one run each; treat as indicative):** cottage 15, 16 (via AgentRouter), 8 calls (with the new tools); mini-golf rebuild 131, 153, 95 calls; Apex-style set 137, 121, 114 calls (the roughness pass); a full pass 1 took 14 min 46 s. Orientation and looking dominate small tasks; `batch`/`run` are adopted only when the server instructions say so.

**Demos and scenes that exist:** `worlds/showcase.txt` (two terrain tiles, lake, river, trees, grass, stones, compact syntax; about 750k triangles), `worlds/sim_test.txt` (simulated lake), `worlds/apex_set.txt`, `apex_set2.txt`, `apex_set3.txt` (Cinder Basin passes 1 to 3: 195k, 126k, 143k triangles), `worlds/cottage_*.txt`, `worlds/minigolf*.txt`, `engine/web/demos/pegasus_studio.txt` (rigged winged horse with a hover/ground state machine; copy in `worlds/`).

---

## 6. Where we are right now: your work list, in order

### 6.1 Rebuild and verify (do this first)
The browser artifacts predate the last fixes. Rebuild (section 3), then run, and read the output yourself:
1. `bun tools/water-sim-live-test.mjs` (needs `worlds/sim_test.txt`; checks the simulation steps, a poke spreads ripples past 5 m, volume kept, damping calms). **The fix for the original failure (frozen water, no ripples) is unverified in a browser.** The fix found: the Bevy water material had no simulation binding; `water.fill` sheets did not get their `water_time`; motion sheets left the clock paused. If the test fails, read `docs/handoff-codex-water-sim-live.md` (Status) for what changed and where.
2. `bun tools/aim-playtest-live-test.mjs` (expects `minigolf2` won by the `aim:ball:cup1,cup2,cup3` policy and `minigolf6` NOT won). Never run live; unverified.
3. Re-run the other live tests (section 8) and `cd web && bun test`, `bunx tsc -b --pretty false` (repo root).
4. Look at `worlds/showcase.txt` (left tile repeats texture, right tile `no_repeat`; lake via `water.fill`) and `worlds/sim_test.txt` with `tools/look.mjs`.

### 6.2 Known bugs and open issues (ranked)
0. **(2026-10-27) Navy terrain, FIXED in source, needs your rebuild check.** All terrain rendered flat navy in the browser. Cause: the Bevy terrain shader did not compile (`cannot find declaration of TerrainParams`): `VeBevyTerrainPlugin` embeds `terrain.wesl` + `stochastic.wgsl` + `forward_terrain.wgsl`, and the struct lives in ve-render's `surface_types.wgsl`, which cannot be embedded there (it needs `GrassParams`). No test composed the terrain shader, only the water one. Fix: `crates/ve-bevy/src/shaders/terrain_params.wgsl` (a copy of the struct) included in `terrain.rs`; new `crates/ve-bevy/tests/terrain_shader.rs` (composes and validates the real shader, and fails if the copy drifts from ve-render's). Verified in the browser: terrain-only world, showcase and Bevy-side terrain are green/sand again (runtime rebuilt, `worlds/blue_a.txt`, `blue_b.txt` are the bisect worlds). NOT re-checked: `minigolf2` (the capture failed with "no editor page connected"; the owner's screenshot of it was navy too). **Process lesson:** any shader assembled from several files needs a compose-and-validate test like `terrain_shader.rs` / `water_shader.rs`.
0b. **(2026-10-27) The `water.fill` lake is not visible in the showcase** (the dish shows sand). It evaluates natively (lake part, 2,268 triangles, box 61..99 at y = 1.0), so the browser draw of volumetric water is the suspect: another agent is mid-way through "volumetric" water (`water.fill` now sets `volumetric`, a `water-fill-3d` demo, `water_volume.rs`). Check after their work settles; `tools/water-sim-live-test.mjs` is the test.
1. **Mini-golf rebuilds `minigolf3` and `minigolf6` cannot be won on hole 2:** the ball ends back at the tee with two strokes added after every shot (the out rule, or a bad tee/ground, fires). `minigolf2` works on the same engine. Unfound cause; diagnose with `ground.profile` and the out/respawn rule in the world text. The models' own "tests" teleported the ball.
2. **Apex set:** `apex_set2` said it could not make the native terrain shader render and used a procedural fallback, and `check` failed its memory budget; `apex_set3` passes `check` at 142,616 triangles and 30 texture maps. Pass 3 (roughness) has only been looked at from the air (more weathered cliffs, speckled sand); it was NOT inspected at eye height. A flat blue plane in an eye-level capture was a camera under the terrain, not a bug. The wing-like grey slab near Skull Hollow in pass 1 was not identified; check pass 3.
3. **Pegasus demo:** cream blotches on the body look like dirt (cause unknown); run vs idle poses look almost the same; in the hover sheet the wings stay raised, so the flap may be subtle.
4. **Cancelled grass-look job:** `ve-render` has half-wired grass code (`grass.rs`, `grass_gpu.rs`, `shaders/grass.wgsl`, `FoliageStyle` additions, the `grass_tuft` kit changed). It compiles and `cargo test -p ve-render` passes, but it was never finished or seen. Either finish it (contract: `docs/handoff-codex-grass-look.md`, tests `ve-render/tests/grass.rs`) or remove it by hand; the owner said "cancel" (they do not need the RedGPU ports), so do not extend it unasked.
5. **Repo-wide red:** `cargo test -p ve-modeling` as a whole does not compile (duplicate `mod tests` from `src/anim/drop.rs:286` and `src/anim/part_2.rs:154`); use `--test` selectors. `ve-world` `tests/rooms_layout.rs` imports a missing `ve_world::rooms` (unlaunched task `handoff-codex-rooms-layout.md`, also `-sign-text.md`, `-detail-panels.md`: `detail_panels` has 10 failing tests for an unregistered `detail.panels@1`); `g12_world` has a tree-LOD triangle-count failure; `cargo clippy -D warnings` has 18 existing errors in `ve-graph`; `cargo xtask rules` finds 21 files over 400 lines; `xtask quick` stops at formatting. None caused by this session's changes; the strict gate is the owner's.
6. **`survey` before modes** put cameras on roofs; fixed by `auto` mode (cottage now gets an aerial ring) but not yet re-checked live after the last rebuild.
7. **Quest budget:** 300,000 triangles is provisional. The showcase is 750k and the Apex passes are 126k to 195k; no headset run has happened. Hands in VR (grip, throw, swing) are unverified on hardware.

### 6.3 Where we are going (the owner's direction, in priority order as I read it)
1. **Make water behave:** finish verifying the simulation, then ripples from moving bodies and hands, buoyancy using `water.height`, river flow around obstacles (shallow-water on the river flow field), rain (the param exists). Later jobs, not started.
2. **Quality of cities and sets (the owner said our cities "are not even good yet"):** the real gap is detail generators, not layout. Order proposed: polygon offset and lot subdivision, road junction meshes, a facade grammar (split/repeat/component rules, a style database), piece generators (window with reveal and sill, door, balcony, awning, cornice, roof types), instancing/LOD design, then `level.dress` (assign procedural materials and wear by what a part is, using the `surface_convex/cavity/up/ao` texture nodes: the procedural stand-in for Trompe's image-model painting). Nothing is built beyond `city.buildings`/`city.ground` (a fixed modern district). Epic's City Sample PCG was studied: shape-grammar slots, 18 ordered graphs, an LLM driving primitives; the hard part is the module library, which must be generated.
3. **Cheaper and more reliable model builds:** adoption of `batch`/`start_from`, fewer orientation calls, numeric checks instead of pictures (cup visible, putter not a flat block, stream smooth), evaluation speed for big worlds, a guide that says a rigged creature needs `anim.machine`. Measure with `try-animated.mjs --repeat 3`; one run proves little.
4. **Benchmark DeepSeek 4.1** on the cottage, the mini-golf prompt and the Apex prompt with the same harness (calls, time, quality). It built the pegasus (via Rust examples that use the creature API, then pasted the text). The path to run it through the harness is not set up (AgentRouter had no channel); an opencode or Claude-CLI provider with its own key would do.
5. **Not wanted now:** more RedGPU ports (grass look, impostor rendering, GPU culling, tile streaming render, view-projection baking). The maths for octahedral impostors and terrain LOD exists and is tested; vendored copies sit in `third_party/redgpu` and `third_party/trompe` (the owner has not decided whether to delete them; ask).

### 6.4 What is left, as a checklist
- [ ] Rebuild browser artifacts; run every live test (6.1).
- [ ] Diagnose the mini-golf hole-2 failure (6.2.1); add a check or bot so models can catch it.
- [ ] Inspect Apex pass 3 at eye height at Skull Hollow, Dam Works, Crash Site, Artillery Ridge with real ground heights; decide the next pass (detail, silhouettes, the ship, buildings are still blocky boxes); ask the owner what "close look-alike" must mean in practice.
- [ ] Pegasus polish (6.2.3).
- [ ] Decide the fate of the half-built grass code and of `third_party/redgpu`, `third_party/trompe` (ask).
- [ ] Water: bodies/hands disturb, buoyancy, rivers, rain verified.
- [ ] City-quality program (6.3.2): write the plan with the owner before building.
- [ ] Update `docs/plans/game-world-system.md` status rows for this session's work (water, survey, compact text, tools); ADR text for 0067 (creatures) is still only referenced.
- [ ] Headset run, Quest budgets, hand-driven Blackout 2 win with creatures alive, soft particle sprite shader, creatures vs closed doors, navmesh timing on big levels, skinned meshes as game models.
- [ ] The owner will say when to commit; until then everything stays in the working tree.

---

## 7. How to work (the loop that worked)

1. **Read** the relevant ADR/plan and the `handoff-codex-*.md` Status of the last job in that area. Read the tests that already exist.
2. **Write the contract and tests first** (a Rust integration test with analytic expectations, or a Bun script for tools). Prefer physical or algebraic checks (ripple speed = sqrt(g h), weights sum to 1, volume conserved) over golden pictures.
3. **Delegate or implement.** For big jobs write `engine/docs/handoff-codex-<name>.md` (goal, reference files, tests, parts, rules, an empty Status) and give it to a helper at medium effort in its own target dir. Run jobs that touch the same crate one after another (concurrent edits break each other's builds).
4. **Verify yourself:** run the tests the helper could not (ports, browser); read its Status; if it says a test is wrong, check its numbers (it was right every time here).
5. **Rebuild the artifacts, look** (`tools/look.mjs`), and only then say it works. Say what you did not see.
6. **Report honestly,** including when your own earlier statement was wrong (this session had several: a "running" job that had crashed, an assumed `@1` requirement, test fixtures with bad arithmetic). Correct it in the next message.

Common helper failure modes: usage limit mid-job (rerun, the job resumes from the working tree), "model at capacity" (rerun), a half-finished job leaving the tree compiling but unwired (check with the crate tests before building on it), reformatting noise from `rustfmt` on touched files only.

---

## 8. Verification inventory (all commands, from `engine/` unless noted)

```
# Rust (own target dir)
export CARGO_TARGET_DIR=target
cargo test -p ve-graph                      # text language, specs (about 215)
cargo test -p ve-evaluator                  # 13+
cargo test -p ve-render                     # renderer, shaders validate/translate to GLSL ES 3.00 (about 198)
cargo test -p ve-game                       # 153
cargo test -p ve-world --test survey --test survey_modes --test lake --test water_sim --test terrain_lod --test octahedral --test octahedral_edges --test octahedral_plan
cargo test -p ve-modeling --test water --test water_fill --test river_level --test stochastic_nodes   # whole-crate test does not compile (6.2.5)
cargo check -p ve-runtime --target wasm32-unknown-unknown
# JS/TS
bun tools/tool-surface-test.mjs  bun tools/orientation-test.mjs  bun tools/batch-run-test.mjs  bun tools/recipes-test.mjs  bun tools/hints-test.mjs  bun tools/run-metrics-test.mjs
cd web && bun test                          # about 40
cd .. (repo root) && bunx tsc -b --pretty false
# live (need the rebuilt viewer + static server on 8801 + Playwright Chromium)
bun tools/mcp-edit-diff-live-test.mjs   bun tools/snapshot-cameras-live-test.mjs   bun tools/templates-live-test.mjs
bun tools/water-sim-live-test.mjs       bun tools/aim-playtest-live-test.mjs        # both UNVERIFIED so far
bun tools/minigolf-verify.mjs worlds/minigolf2.txt /tmp/mg2      # simulated swings win it
bun tools/blackout2-verify.mjs   bun tools/creature-test.mjs   bun tools/rolling-test.mjs   bun tools/world-api-test.mjs
```
Test tools use fixed ports (8791 to 8799, 8812 to 8817, 8830); run them one at a time. If a test server says `EADDRINUSE`, a previous run is still alive.

---

## 9. Where to look for anything else

- The decisions: `engine/docs/adr/` (read 0061 tool surface, 0065 runtime world API, 0066, 0068, 0069, 0070).
- The world and game systems guide: `engine/docs/plans/game-world-system.md` (status table per feature, with dates).
- What a helper did and what it could not verify: the Status section at the bottom of each `engine/docs/handoff-codex-*.md` (this session: `compact-text`, `compact-wiring`, `batch-run`, `tool-surface`, `templates-recipes`, `near-miss`, `orientation`, `run-tools`, `survey`, `survey-modes`, `stochastic`, `octahedral`, `terrain-lod`, `water-waves`, `water-fill`, `water-sim`, `water-sim-live`, `grass-look` (cancelled), `vendor-import`).
- The model-facing guide (what models read): `bun tools/mcp/server.mjs --guide` and `tools/mcp/guide.mjs`; the operator list: `./target/debug/examples/ops`.
- Research the owner asked for this session, summarised in chat only: Epic City Sample PCG (18 graphs, shape grammar slots, LLM+MCP), Trompe (camera planner idea), RedGPU (water, stochastic sampling, octahedral, CDLOD, grass formulas), Apex Legends Kings Canyon layout (zones as outdoor rooms, ziplines, three routes between areas).
- A fresh memory index for Claude Code sessions lives outside the repo (`~/.claude/projects/.../memory/`); the rules in section 1 are the same content.
