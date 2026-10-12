# Testing with a model through MCP (no API key)

A way to try the engine with a real model before any hosted provider exists. The engine runs an MCP
server; an MCP client (Claude Code, or the Claude CLI in the helper below) calls its tools, and the
client's own model is the provider. It is for local testing: the server binds to 127.0.0.1, takes
one page at a time and has no authentication.

It works on **the engine's own viewer page** (the Viewer panel with field of view, lenses and quality,
and the Document panel), not on the editor shell: add `eval=1` or `mcp=1` to `?viewer=1`.

## Pieces
- `engine/tools/mcp/server.mjs`: MCP over stdio, plus a WebSocket on `127.0.0.1:8765`
  (`VE_MCP_PORT` changes it) that the page connects to.
- `src/engine/viewer-bridge.ts`, bundled to `web/dist/viewer-bridge.js` by `cargo xtask web`: gives the
  viewer page the same evaluation pipeline the editor uses (worker, staged edits, scene upload) and the
  MCP bridge (`src/engine/mcp-bridge.ts`). Every tool runs through the same document commands as the
  page, so a model gets no capability a person lacks.
- `?viewer=1&eval=1` evaluates documents into geometry without MCP; `?viewer=1&mcp=1` also connects the
  bridge (`mcp=<port>` for another port). The stage starts empty ("Authored document" scene).

## Tools
`get_guide` (the language, with an example), `list_operators` (ready operators with parameters),
`get_text`, `get_nodes` (IDs, labels, operators, parameters), `get_status`, `apply_text` (adds a
program's nodes; diagnostics carry line and column), `apply_commands` (typed edits by node ID),
`undo`, `redo`, `frame_view` (the whole model fills the view), `set_view`. A refused edit (for example
`constraint.min_wall`, with the measured thickness) leaves the document unchanged and says so.

## Looking at the model
The first model that appears is framed automatically. With the mouse not held: the wheel (or a
trackpad scroll or pinch) zooms toward the model, Alt + drag (or the "Drag to orbit" box, or O, then
drag; middle-drag too) orbits it, F or the "Frame model" button frames it again. The existing
controls are unchanged: click to hold the mouse and look around, WASD to move.

## Use it
1. Build once and serve the page: `cargo xtask web dev`, then
   `node engine/tools/serve-lan.mjs --http --host 127.0.0.1 --port 8123`.
2. Register the server once: `claude mcp add spatial-engine -- bun engine/tools/mcp/server.mjs`.
3. Open `http://127.0.0.1:8123/?viewer=1&mcp=1`.
4. In Claude Code, ask for something ("use the spatial-engine tools to make a hollow vase, then widen
   its opening"). The page updates as the model works.

Or run the helper, which serves the page, opens it and runs a model for you (add `--headed` to watch
and then zoom and orbit yourself):
`bun engine/tools/mcp/try-with-claude.mjs "make a hollow vase, then widen its opening"`.

## What is tested
`web/tests/mcp.mjs` (part of the strict gate) plays the MCP client with a script against the viewer
page: protocol basics, a clear message when no page is connected, building the vase, a diagnostic with
a span, a wall violation refused with the document unchanged, an edit by ID that keeps every ID, undo,
then frame, wheel zoom in and out, and Alt + drag orbit with the distance kept.
`try-with-claude.mjs` was run with Sonnet: it built the vase and widened its opening, and built a
twisted tower from a prose description (it worked out the star's ten points itself); the wall check
held and the model was framed.

## Not done
No screenshot tool (a WebGL canvas cannot be read back reliably from the page), no measurement tool
beyond evaluation stats, no authentication, one page at a time. The React editor shell has the same
bridge (`?mcp=1` on `bun run dev`) but its viewport is the older one. A hosted provider (API key on a
server, `server/adapters.ts`) is separate and still to be built; it would sit behind the same `Patch`
boundary (ADR 0011).
