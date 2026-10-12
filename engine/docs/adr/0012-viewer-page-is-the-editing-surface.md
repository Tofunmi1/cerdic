# 0012: The engine's viewer page is the editing surface; the React shell is not extended

- Status: accepted (owner, 2026-10-13); supersedes the first decision in ADR 0007 (the editor is a React shell)
- Date: 2026-10-13

## Context
ADR 0007 put the visual editor in a React shell in the root app, with the engine canvas embedded.
Since then the engine page (`engine/web`, served with `?viewer=1`) has become the place the work
happens: it has the viewport navigation (wheel zoom, orbit, frame), the studio stage, the document
panel, evaluation of the document into geometry, and the MCP test bridge a model uses. The owner asked
for that page's look and feel to be the editor, and for the MCP session to run in it, not in the shell.
The shell has stayed a thin debug host (`src/engine/Editor.tsx`: buttons, JSON boxes). Building V1
(outliner, inspector, history) in both would mean two UIs drifting apart.

## Decision
- The viewer page is the editing surface. Outliner, inspector, history and problems (V1), and later the
  node graph (V2), are plain JavaScript modules under `engine/web/panels/` with no framework and no
  build step beyond copying files. They own no document state: every edit is a typed `CommandBatch`
  through the page's document API, and the engine's evaluation and projection run as before.
- Selection is view state, held by the engine runtime (`selection_set`, `selection_get`,
  `selection_clear`) so the viewport can light up the selected parts and a click on a part comes back
  to the page. It is never saved and never a command. The page decides which parts a node builds
  (it holds the graph); the engine highlights them.
- The React shell stays as a host for the same engine (the typed bridge in `src/engine/` is still how
  it and the MCP bridge talk to the runtime) but is not extended with editing panels. The rest of
  ADR 0007 stands: Bun everywhere, generated types, the canvas created once outside React.
- The node canvas for V2 cannot start on `@xyflow/react` (that is a React library). Experiment X7's
  question becomes whether a custom canvas meets the V2 criteria; V2 starts from that.

## Consequences
- Panel logic that is pure (the outline, which parts a node builds, diagnostics by node) lives in
  `panels/graph.js` and is unit tested without a browser (`web/tests/panels-graph.mjs`); the rest is
  tested through the real UI (`web/tests/panels.mjs`).
- Users and tests reach the editor at `?viewer=1&eval=1`; the older document panel features stay.
- If a React front end is wanted later (a product shell, accounts, galleries), it embeds the viewer
  page or reuses the engine bridge; it does not re-implement the editing panels.
