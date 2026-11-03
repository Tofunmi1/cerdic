# The LLM pipeline and the harness around it (design, 2026-11-03)

Status: proposed. Companion to `harness-speed.md` (how fast the engine answers) and `game-generation-pipeline.md` (what a game needs). Everything here runs in the browser page with the chat; the only network call is the model provider's.

## 1. What the research says (sources read 2026-11-03)

- **Start simple; add structure only where it measurably pays.** Anthropic separates *workflows* (fixed code paths around model calls) from *agents* (the model picks its steps) and advises the simplest thing that works. Evaluator-optimizer fits when there are clear criteria and refinement shows value; parallel calls fit speed or multiple attempts ([Building effective agents](https://www.anthropic.com/research/building-effective-agents)).
- **Few, high-impact, intent-level tools.** Consolidate multi-step operations into one call, namespace related tools, name parameters unambiguously, return high-signal fields instead of ids, offer a concise/detailed switch, paginate and truncate with sensible defaults, write errors that say what to fix with an example, describe tools like onboarding a new hire, and evaluate with many real tasks, each with a verifiable outcome, a held-out set, and metrics on tool calls, tokens, runtime and errors ([Writing tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents)).
- **Long runs need context management:** compaction near the window limit, clearing old tool results (the lightest, safest form), structured notes kept outside the window, sub-agents with clean windows that return 1,000 to 2,000 token summaries, and just-in-time retrieval of detail ([Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)).
- **LLM-to-3D systems use a planner, an actor and a visual critic, and the critic loop saturates.** SceneCraft pairs a render-and-critique inner loop with a growing skill library; LL3M uses a planner, a retrieval agent over API docs, a coder and a VLM critic on multi-view renders; a planner-actor-critic study found the actor does not always apply the critic's feedback and quality degrades or stalls after about three rounds; other work stresses that agents emitting tool calls without seeing results are blind ([SceneCraft](https://arxiv.org/pdf/2403.01248), [LL3M](https://www.emergentmind.com/papers/2508.08228), [planner-actor-critic](https://arxiv.org/html/2601.05016), [Cutscene Agent](https://arxiv.org/pdf/2604.25318)). VLM judgement misses subtle spatial faults, so cheap deterministic checks should come first.
- **Calling the model from a browser** needs the `anthropic-dangerous-direct-browser-access: true` header, and a key placed in client code can be stolen by anyone who loads the page ([Willison](https://simonwillison.net/2024/Aug/23/anthropic-dangerous-direct-browser-access)). I did not find how prompt caching, tool use and streaming behave in browser calls; they are standard Messages API features and must be tested.

## 2. The harness (what runs in the page)

```
chat panel  ->  agent loop (page worker)  ->  provider client (fetch, streaming)
                      |                              ^
                      v                              |
                tool layer (plain functions)    model (user's key)
                      |
   document + evaluator worker + runtime + checks + bots
```

1. **Agent loop in the page.** A small module owns the conversation, calls the provider with streaming, runs tool calls as direct function calls (the tool bodies already exist in `src/engine/mcp-tools.ts`; the MCP server stays for outside clients and developers), enforces budgets (tokens, tool calls, wall time per task), supports cancel and resume, and records every turn.
2. **Provider client.** One interface (`send(messages, tools, options)`) with an Anthropic implementation (Messages API, tool use, images, prompt caching, streaming) and room for others. The key is the user's own, kept in the browser (IndexedDB), never in our code. **Open decision:** a hosted demo with no user key needs a server-side proxy, which conflicts with "everything in the browser"; proposal: key entry by the user as the default, and an optional proxy URL setting for whoever wants to host one.
3. **Tool layer, consolidated.** About ten intent-level tools instead of 30 (aliases keep old names working, per ADR 0061): `build` (apply text or commands, edit by label, batch, dry run, undo), `find` (operators, recipes, kits, materials, textures), `inspect` (overview, measure, probe, status), `view` (snapshot, contact sheet, set camera), `check` (budget, attached, look, game validity, completability), `play` (playtest bots, simulate, game state), `script` and `world` (the runtime), `assets` (recipes, bake, export) and `note` (persistent memory). Every answer is concise by default, takes `detail`, ends with the state delta and one next suggestion, and every error names the fix.
4. **Fast check.** `check` runs the evaluator in evaluate-only mode (no upload, no light probes, no render): geometry, budget, diagnostics, game validation, per-node time, in about 3 s on a city. This is the model's main loop; `view` is for judging looks.
5. **State in the page.** The document, chat, notes and recipes persist in IndexedDB or OPFS, so a reload resumes the same project; every document revision is already stored and replayable.
6. **Progress and honesty.** The page shows its stage (evaluating, uploading, baking, lighting) and lets the model look at a draft frame before light probes finish.

## 3. The pipeline (stages, roles and models)

A workflow of five stages, each a model call or a short agent loop with its own clean context and its own artifact. Roles and defaults (configurable; model ids from the current family: `claude-fable-5-1`, `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-5-5`):

| # | Stage | Artifact | Model class | Notes |
| --- | --- | --- | --- | --- |
| 1 | **Brief** | one page: genre kit, camera, setting, art style (`look.*`), scope, budgets, acceptance checks | strongest | asks at most one clarifying question; writes the brief to notes |
| 2 | **Plan** | todo list of build steps, each with a check that proves it | strongest | the planner owns the list; items are small and verifiable |
| 3 | **Build** | document edits | fast and capable | one agent loop per todo item: batch edit, fast check, fix errors, repeat; saves what worked as a recipe (SceneCraft's skill library) |
| 4 | **Critique** | ranked list of concrete fixes, as tool-level edits | strongest with vision | clean context: brief, checks, a contact sheet of named views; at most 2 to 3 rounds, because critic loops stall |
| 5 | **Verify and ship** | report: checks, bot results, budgets, a one-line summary | fast | deterministic gates decide pass or fail, not a model's opinion |

Rules that follow from the research:
- **Deterministic first.** Budget, attached parts, look checks (flat shading, blue cast, missing reflections, exposure), game validity and bot completion run before any model critiques; a critic only sees what the checks cannot judge.
- **Critic output is edits, not prose.** The actor applies listed edits mechanically; unapplied items are reported, not silently dropped.
- **Parallel where independent.** Independent assets (a character, props, terrain) go to separate sub-agents with clean windows, each returning a short summary and a recipe; two or three variants of one parameter go to a worker pool and return one contact sheet.
- **Context discipline.** A stable prefix (system prompt, compact guide, tool schemas, byte-identical between turns) for prompt caching; old tool results and snapshots are cleared once acted on; the brief, plan, decisions and open issues live in notes that survive compaction; detail is fetched on demand (`describe`, `get_guide topic`).
- **Small first.** Build a coarse whole, check, then refine; never author a 2,000-line world before the first check.

## 4. Evaluation (so we know it works)

- A suite of 10 to 20 briefs across genres and settings, each with a verifiable outcome (checks pass, the bot completes the game, budget holds, a look check passes) and a held-out half.
- Metrics per run: success, tool calls, tokens, wall time, tool errors, critic rounds used, whether critic edits improved the checks.
- Variants to compare (the research is silent on our domain, so measure): with and without the critic; one model for everything versus the routed models; consolidated versus current tools; with and without notes and recipes.
- Runs replay headlessly through Playwright as a developer tool; nothing in the product depends on it.

## 5. Build order

1. Fast check (evaluate-only) and skip-unchanged uploads (from `harness-speed.md`), because every loop depends on them.
2. The agent loop, provider client and chat panel in the page, with the current tools behind it unchanged.
3. Tool consolidation with aliases; concise/detail and state delta everywhere.
4. Notes, tool-result clearing and compaction.
5. Brief and plan stages, then the critic with deterministic look checks.
6. The evaluation suite and the first measured comparison.

## 6. Open decisions for the owner

- Key handling for a hosted demo (user key only, or an optional proxy).
- Default model routing and whether a model chooser appears in the UI.
- Whether consolidation may rename tools (proposal: new names with old names as aliases).
- Where the agent loop lives: page worker (proposed: keeps the UI responsive) or main thread.

## 7. Landed (2026-11-03)

- `src/engine/agent/loop.ts` (+ `types.ts`): the agent loop. Tool calls run in order and their results are paired by id; a tool that throws or is unknown becomes an error result the model can read; results are cut at a limit with a note; the request is a trimmed view of the full history (old tool results replaced by a marker, only the newest pictures kept) while the whole history is kept for the log; budgets for turns, tool calls, tokens and wall time and a cancel signal stop the run with a named reason and leave every tool_use answered, so the conversation can continue; the system prompt and tools pass through untouched for caching; events feed a chat panel. `formatToolResult` splits snapshot and bake pictures from the text the way the MCP server does. `tests/agent-loop.test.ts`: 17 tests, scripted provider.
- `src/engine/agent/anthropic.ts`: the Messages API from the page with the person's key (browser-access header, cache markers on the system prompt and on the newest turn, retry on 429, 529 and 5xx with `retry-after`, errors that never contain the key). `tests/agent-anthropic.test.ts`: 10 tests, fake fetch. Not tried against the live API, and it is not streaming yet.
- **Not done, and larger than it looked:** the page already has a tool dispatcher (`run` in `mcp-bridge.ts`), but six tools run only in the Node MCP server: `get_guide` and `list_operators` (the guide text is inline in `server.mjs`), `start_from` (templates), `recipes` (saved to disk), and `batch` and `run` (a Node sandbox). To make the in-page agent complete they need in-page versions: the guide text moved into an importable module, recipes in IndexedDB, `batch` as a loop, `run` in a restricted worker. Then the tool schemas shared between the server and the page, a chat panel, and a key and model setting.
