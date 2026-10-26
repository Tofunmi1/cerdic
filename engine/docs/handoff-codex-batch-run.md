# Handoff: `batch` and `run` MCP tools, shorter tool descriptions, ADR 0069

Read `docs/adr/0069-compact-text-and-batched-tools.md` section 4 first. Files: `tools/mcp/server.mjs` (Bun, MCP over stdio) and new modules next to it (`tools/mcp/batch.mjs`, `tools/mcp/run-sandbox.mjs`, guide text in `tools/mcp/guide.mjs`); keep `server.mjs` growth small.

## Test (already written, do not weaken)
`tools/batch-run-test.mjs`, run `bun tools/batch-run-test.mjs` from `engine/`. It spawns the server with no editor page and an env `VE_RUN_TIMEOUT_MS=1500`.
If a check looks wrong, say so under Status; do not edit the test silently.

## What to build
1. `batch {steps: [{tool, args}], stop_on_error?: boolean (default true), max_chars?: number (default 3000)}`: up to 32 steps in order through the same dispatcher as a direct call (`callTool`); the answer is JSON text `{steps: [{tool, ok, text?, error?, truncated?}], stopped_at?}`; a step's text is cut to `max_chars` with `truncated: true`; image blocks are dropped except from the last step (the answer then carries that image block too); more than 32 steps, `batch` or `run` inside a batch, and an unknown tool name are errors for that step (the 33-step refusal is a whole-call error mentioning 32).
2. `run {code}`: async function body with `tools.<name>(args)` (every tool except `batch` and `run`; returns the tool's text as a string, unchanged, so `typeof g === "string"`; a tool error throws an `Error` whose message is the tool's error text), `log(...)`, `return` value. Answer JSON text `{result, logs, truncated?}`; `result` JSON cut to 8,000 characters (then `truncated: true`). Limits: wall time `VE_RUN_TIMEOUT_MS` (default 120000), 200 tool calls per run (error mentioning 200), no `process`, `require`, `fetch`, `Bun`, `globalThis.process` visible to the code, an endless loop is stopped and reported with "time"/"seconds" in the message, a syntax error is reported. Run the code in a `Worker` (terminable, so an endless loop can be killed) that talks to the server for each tool call; the code cannot import anything.
3. `get_guide(topic="world_api")`: move the runtime-method list out of the `world` tool description into this topic (all methods currently in the description, with their parameters); the `world` description keeps two sentences and points at the topic. Descriptions of `batch` (< 900 chars) and `run` (< 1500) stay short; `world` description < 1200 chars.
4. Add a short `batch`/`run` paragraph to the core guide (`coreWithMenu`) that says when to use each (batch: a known sequence; run: loops and conditions, only what you return enters the conversation).

## Rules
- Do not run builds of the engine; this is JS only. Run `bun tools/batch-run-test.mjs` and `bun tools/mcp/server.mjs --guide` (prints the guide; must still work) and the other existing JS tests that do not need a browser page if any apply.
- Edit with your edit tools; no commits or staging; keep files under 400 lines where practical.

## Status
Implemented `batch` and worker-backed `run` through the server's shared `callTool` dispatcher. Added the `world_api` guide topic, shortened the `world` description, and added the batch/run usage paragraph to the core guide. The existing test was left unchanged.

Checks: `bun tools/mcp/server.mjs --guide` completed and printed the guide, and `bun tools/mcp/server.mjs --guide world_api` printed the new topic. `node --check` passed on all changed JavaScript modules. A worker smoke check confirmed `typeof process`, `require`, `fetch`, `Bun`, `globalThis.process` and `globalThis.Bun` all return `undefined`. `bun tools/batch-run-test.mjs` could not start its server because the test's fixed port 8791 returned `EADDRINUSE`; unmodified-content temporary copies with alternate ports 8792 and 18891 hit the same error. The environment prevented process/socket inspection, so the listener could not be identified. No test assertions ran. No handoff check appears incorrect based on the available evidence.
