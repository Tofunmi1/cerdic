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

Both apply tools use one atomic undoable transaction, up to 1,000 typed commands and 1 MiB of batch
JSON. `apply_text` also retains the 256 KiB source-text cap. The furnished study-room fixture applies
in one transaction (382 commands); larger work must be split into deliberate transactions.

`poly.grid` supplies normalized UV corner attributes. `poly.subdivide` carries those UV seams and F32
point attributes into its refined mesh. The `uv_subdivided_grid` gallery fixture exercises this path
through the browser evaluator and viewport; the gallery test checks the visible viewport from six views.

## Colour
`solid.material@1(solid, color=[r,g,b], metallic, roughness, emissive)` gives a solid a surface
(sRGB colour, metal 0 to 1, roughness 0 to 1, emissive glow); the shape is unchanged. It also takes a
placement set (`inst.*`) and colours every placement. Inputs of one colour keep it through transforms
and booleans. Inputs of two or more colours stay those colours: unions, differences, intersections,
moves, rotations, scales, mirrors and arrays label each input's faces in the kernel and the result is
one solid drawn as one part per colour (ADR 0018). A cutter without a colour of its own leaves the body's
colour on the faces it cuts; a coloured cutter inlays its colour. A material on top recolours the whole
solid, and twist, taper and bend keep only the first input's colour. A part's surface travels with its
mesh from the evaluator to the renderer (`eval_projection_material_region`, keyed by node and colour
region; `eval_projection_material` is region 0). Opacity
(glass) is not supported by the renderer yet. This is the first slice of the appearance work planned
for G10 (procedural materials), pulled forward because a request for a "red vase" has to be possible.

Text added to an existing document may read its nodes by label (`red = solid.material@1(vase, ...)`
then `output vase = red`), so a model can extend what is there instead of rewriting it.

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
then frame, wheel zoom in and out, and Alt + drag orbit with the distance kept, then a material added
to the existing vase turns the rendered canvas red (pixels counted before and after) and an
out-of-range colour is refused.
`try-with-claude.mjs` was run with Sonnet: it built the vase and widened its opening, and built a
twisted tower from a prose description (it worked out the star's ten points itself); the wall check
held and the model was framed.

## Not done
No screenshot tool (a WebGL canvas cannot be read back reliably from the page), no measurement tool
beyond evaluation stats, no authentication, one page at a time. The React editor shell has the same
bridge (`?mcp=1` on `bun run dev`) but its viewport is the older one. A hosted provider (API key on a
server, `server/adapters.ts`) is separate and still to be built; it would sit behind the same `Patch`
boundary (ADR 0011).
