# ADR 0061: The model tool surface (families, compact answers, state deltas, fixes)

- Status: accepted (2026-10-22, owner accepted keeping the existing tool names)
- Date: 2026-10-22
- Amends: ADR 0011 (text form and provider boundary), the MCP test provider (`docs/modeling/mcp-test-provider.md`)
- Part of: `plans/game-world-system.md` section 7.4

## Context

The MCP server (`tools/mcp/server.mjs`, 21 tools) was shaped by single-object and creature work. For worlds a model cannot: see a world from several angles in one call, price a program before applying it, judge a whole scene (draws, memory, colliders, budget), check quality with measured verdicts, ask the world a question (ground height, ray hit, path), test that it plays (physics, bots), read performance, edit by name instead of id, or apply a suggested fix as a command. Research (2026): Unity MCP servers auto-check the console after each tool call and expose screenshot and profiler tools; Anthropic's guidance on writing tools for agents stresses selective, namespaced tools, token-efficient high-signal answers, actionable errors and evaluation-driven refinement; GameCraft-Bench and PlaytestArena show that closing a play-and-repair loop is what separates mechanics from complete games.

## Decision

### Rules for every tool
1. **Intent-level, few, namespaced.** A tool does one job a model would name in a sentence; related tools share a family prefix. Existing tools keep their names (the eval harness and prompts depend on them); new tools use `family_name` where a family exists.
2. **Compact by default, detail on request.** Default answers are short text a model can read in one glance; `detail: "full"` returns the JSON. Pictures are small (contact sheets, not separate images). Lists are capped with a count of what was left out and how to get it.
3. **Every mutating call returns a state delta**: the new revision, new or cleared diagnostics, the budget change (triangles, entities, memory, estimated frame cost against the Quest budget) and a visual-change summary (share of the view that changed and where), plus one suggested next step. A model never needs a second call to learn whether an edit worked.
4. **Every error names the fix.** Diagnostics carry `code`, `severity`, `node` (label), `line` and `column`, `measured` and `limit` where numeric, `suggestion` in words, and `fix` as a ready typed command when one is unambiguous. Each diagnostic has a stable `id` for the call that applies it.
5. **Deterministic and cancellable.** Same document, same arguments, same answer. Anything over an interactive budget reports progress and takes `cancel`.
6. **One source of truth.** Tool descriptions and schemas are generated from the operator registry and a single tool manifest (extending ADR 0009's generated bridge), so the guide, the schema and the inspector never disagree.

### The families
| Family | Tools |
| --- | --- |
| build | `apply_text` (adds `dry_run: true`: validate, estimate and report without changing the document), `apply_commands` (a `node` may be an id **or a label**), `edit` (set parameters, rename, delete or replace by label), `diff` (text diff since a revision), `apply_fix` (by diagnostic id), `undo`, `redo`, `get_text`, `get_nodes`, `get_status` |
| find | `find` (by intent, over operators, kits and recipes, ranked), `describe` (one operator or recipe: parameters, one worked example, common mistakes, related nodes), `recipes` (list, show, save a group as a recipe), `list_operators`, `get_guide`, `texture_nodes` |
| view | `snapshot` (adds `views` for a contact sheet from named cameras: front, side, top, iso, player eye; `focus` a part; `overlays`: colliders, navmesh, bounds; keeps the change comparison), `set_view`, `frame_view` |
| inspect | `scene_overview`, `measure` (parts, as now), `probe` (a point: ground height, surface, part, material, nearby parts; a ray: first hit; a pair of points: path on the navmesh), `perf` (frame median and p90, entities, draws, triangles per band, memory, with a `profile` of CPU throttle such as `quest`) |
| check | `check` (runs the document's check nodes plus named ad hoc checks in one report) |
| sim | `simulate` (physics for N seconds with stimuli), `playtest` (bots play the game spec) |
| game | `game_describe`, `game_validate`, `game_state` (ADR 0062) |
| export | `export_glb` |

### Built-in checks (each also a `check.*` node so a world carries its acceptance)
`budget_quest`, `colliders_present`, `floating_parts` (props not touching the ground), `scale_sanity` (doorways, steps, reach), `walkable_path` (from, to), `crown_balance` and `cone_shape` (from the `ve-world` quality tests), and the game checks `goal_reachable`, `no_softlock`, `pacing` (ADR 0062). A verdict is `{ check, status: pass|warn|fail, measured, limit, where, suggestion, fix? }`.

### Implementation split
The server stays a thin JSON-RPC bridge; report-producing code lives behind the page and the engine (Rust where it must be deterministic and shared with the headless playtest: budget estimates, checks, probes, simulation), and a TypeScript layer that formats compact answers. Checks and probes use the evaluated geometry and `ve-physics` queries; `walkable_path` and `playtest` need `ve-nav` and `ve-game` (milestones M3 and M4); `perf` needs the live runtime counters.

## Consequences
- A model can see, price, judge, probe, test and fix without leaving the tool calls, and every step answers in a few lines. The same checks gate the evaluation harness.
- New tool work is split by dependency: `dry_run`, `scene_overview`, contact-sheet `snapshot`, label edits, diagnostic ids and `fix`, `find`/`describe` land in M1 without new engine systems; `check`, `probe`, `recipes` in M2; `simulate`, `perf` in M3; `playtest` and the game checks in M4.
- A test per tool: a golden transcript (arguments in, compact text out) and a size guard (default answers stay under a token budget).
- Cost: the diagnostics model gains `id` and `fix` across all operators (additive); `apply_commands` label resolution needs unique labels, so duplicate labels are refused with the ids of the duplicates.
- Not in this ADR: networked or multi-agent tool use, authentication, tool-call rate limits.
