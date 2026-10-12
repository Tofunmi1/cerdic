# Testing with a model through MCP (no API key)

A way to try the engine with a real model before any hosted provider exists. The engine runs an MCP
server; an MCP client (Claude Code, or the Claude CLI in the helper below) calls its tools, and the
client's own model is the provider. It is for local testing: the server binds to 127.0.0.1, takes
one editor page at a time and has no authentication.

## Pieces
- `engine/tools/mcp/server.mjs`: MCP over stdio, plus a WebSocket on `127.0.0.1:8765`
  (`VE_MCP_PORT` changes it) that the editor page connects to.
- `src/engine/mcp-bridge.ts`: the page side, enabled with `?mcp=1` (or `?mcp=<port>`). It runs each
  tool through the same document commands as the editor, waits for the worker to evaluate, and
  reports the outcome. A model gets no capability a person lacks.

## Tools
`get_guide` (the language, with an example), `list_operators` (ready operators with parameters),
`get_text`, `get_nodes` (IDs, labels, operators, parameters), `get_status`, `apply_text` (adds a
program's nodes; diagnostics carry line and column), `apply_commands` (typed edits by node ID),
`undo`, `redo`, `set_view`. A refused edit (for example `constraint.min_wall`, with the measured
thickness) leaves the document unchanged and says so.

## Use it
1. Register the server once: `claude mcp add spatial-engine -- bun engine/tools/mcp/server.mjs`.
2. Start the editor (`bun run dev`) and open `http://127.0.0.1:5173/?mcp=1`.
3. In Claude Code, ask for something ("use the spatial-engine tools to make a hollow vase, then widen
   its opening"). The page updates as the model works.

Or run the helper, which starts everything and a model for you (add `--headed` to watch):
`bun engine/tools/mcp/try-with-claude.mjs "make a hollow vase, then widen its opening"`.

## What is tested
`web/tests/mcp.mjs` (part of the strict gate) plays the MCP client with a script: protocol basics, a
clear message when no page is connected, building the vase, a diagnostic with a span, a wall
violation refused with the document unchanged, an edit by ID that keeps every ID, undo, and view.
`try-with-claude.mjs` was run once with Sonnet: it read the guide and operators, built the vase and
widened its opening in two calls; the wall check held.

## Not done
No screenshot tool (a WebGL canvas cannot be read back reliably from the page), no measurement tool
beyond evaluation stats, no authentication, one page at a time. A hosted provider (API key on a
server, `server/adapters.ts`) is separate and still to be built; it would sit behind the same
`Patch` boundary (ADR 0011).
