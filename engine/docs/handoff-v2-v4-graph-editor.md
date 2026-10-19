# Handoff: V2 node graph editor, then V4 group authoring

Read `handoff-common.md` first. V2 and V4 are one piece of work in two gates: V4 needs the V2 canvas
(plan table: V4 needs G4 and V2). Build V2 first. Plan text: `docs/plans/track2-modeling.md` section 5
"V2" and "V4", section 4B (the visual editor), experiment X7 in section 6, ADR 0012.

## 1. What must be delivered
**V2** (plan): "Canvas, typed sockets, link validation while dragging, search-to-add from the registry,
parameter widgets on nodes, frames and reroutes, per-node status and diagnostics, the 'view this node's
output' pin, mute and bypass, copy and paste with ID remap, minimap, and the side-by-side text view."
*Accept:* building the vase graph by UI gives the same document as the mock commands; a type-mismatched
or cyclic link is refused with the reason shown; text edits and graph edits stay in sync; moving nodes
causes no re-evaluation (instrumented counts); a 200-node graph pans and zooms without dropped frames on
the named desktop; golden screenshots reviewed.

**V4** (plan): "Create a group from selected nodes with exposed ports, enter and exit with a breadcrumb,
edit definition versus edit instance, explicit upgrade prompts, and a library panel of our own groups and
templates." *Accept:* making the wheel group by UI equals the G4 fixture; editing the definition updates
all instances and editing one instance does not; upgrade conflicts are shown before they are applied.

`cases.md` has a full V2 table (V2-01 to V2-15, include `V2-12` 200 nodes at no frame over 33 ms and the
experiment row `V2-X`). V4 has only a two-line draft at the end of `cases.md`: expand it into a table first.

## 2. The ground has moved since the plan was written (read this)
- **ADR 0012 (owner decision): the editing surface is the viewer page, plain JavaScript, no React.** The plan's
  X7 asks about `@xyflow/react`; React is out for the editor, so **X7 as written is moot**. Replace it
  with: build a small custom canvas (SVG or 2D canvas, your call, measured) and run the 200 and 500 node
  tests against it. Record that as the X7 result in `docs/modeling/experiments.md` and say so to the owner.
  Do not add a UI framework without a new ADR. The root React app (`src/engine/Editor.tsx`) still exists
  for tests and as the V0 shell; do not extend it.
- Panels today: `web/panels/{dock,outliner,inspector,history,groups,selection,widgets,graph}.js`,
  mounted by `web/docpanel.js`. They are stacked inside one fixed dock. There is no room for a canvas:
  decide a layout (resizable bottom or side panel, a toggle that fills the window, or a second tab),
  keeping the 3D view live. Panels own no document state: every edit is a typed command sent through
  `window.__ve.doc` (the page document API in `web/document-api.js`), and `graph.js` holds the pure
  helpers (`partsFor`, `sinks`, `instanceInfo`, `exposedParams`, `outlineRows`, `diagnosticsByNode`).
  Put new pure logic (layout, hit-testing, link validity messages, paste remap) in testable modules and
  test them with Node, like `web/tests/panels-graph.mjs`.
- The **editor layer** already exists: `Document.editor { positions: NodeId -> [x,y], collapsed }`, commands
  `editor_set_position` and `editor_set_collapsed`, saved and undoable, outside the evaluation hash
  (G1a-14). **Frames and reroutes are not in it yet** (V2-08 says they are saved in the editor layer): add
  them to `EditorLayer` with `#[serde(default)]`, commands, undo, persistence tests, and the TypeScript
  export (`typescript.rs`, `bun run schema:generate`). This is a schema change inside schema 1 only if
  additive and defaulted; check `store/migrate.rs` and the G1b cases. If you coordinate with L0 (which
  moves the schema to 2), do both in one migration step.
- **Positions for new nodes**: nodes created by text or MCP have no position. You need a deterministic
  auto-layout (layered, left to right by dependency) that writes positions through commands only on
  demand, never on every load, so loading does not create history. `L5` later needs auto layout of
  flow graphs; design it to take edges of two kinds.
- Link validation lives in `ve-graph/src/links.rs` (types, cycles, arity). Do **not** reimplement it in
  JS. For "validation while dragging", ask the engine: expose a pure query like `doc_check_link(from,to,port)`
  returning ok or the diagnostic (the commands already return typed diagnostics; the query can apply
  the batch to a candidate copy and discard it). Show the reason text next to the cursor.
- Operators and their specs are available to the page (`doc_operators`), including `readiness`,
  `inputs[].accepts`, arity and output type. Search-to-add lists only `ready` operators with cost and
  readiness (V2-04), fuzzy match.
- "View this node's output" pin (V2-11): evaluation produces results for **outputs** only today
  (`doc.outputs`). A pin needs either a temporary preview output (not saved, not in history, not in the
  hash) or an evaluation target parameter. Prefer a view-state target passed with the evaluation request
  (like selection, never a command). Check `ve-modeling/src/evaluate.rs` and the worker protocol for how
  the target set is formed, and keep the cache key unchanged.
- Mute and bypass are node flags already (`set_flags`; the outliner shows muted rows). V2-09: bypass
  passes the first input when types allow; the evaluator already implements bypass and muted
  placeholders for group instances (ADR 0013); confirm for plain nodes with a test.
- Text view: `web/docpanel.js` shows canonical text (`doc_text`) read-only. V2-05 needs **editable** text
  that re-parses into the document (`doc_parse_text` exists; `doc_stage` and the text provider
  boundary, ADR 0011). Decide how a text edit turns into commands (parse to a replacement document and
  diff into commands is the cleanest; a whole-document replace loses identity: IDs must be preserved by
  aliases or by matching labels, see ADR 0011 and `text/build.rs`). Test that undo after a text edit is
  one step and keeps unrelated node IDs.
- Mistakes the earlier panels work caught: duplicate problem rows, `<details>` collapsed on a new problem,
  reload tests across separate Playwright contexts. Read `web/tests/panels.mjs` for the pattern.

## 3. V2 design (proposal; deviate with a note to the owner)
- Rendering: SVG for nodes and wires until the 200-node case fails, then canvas. Keep node DOM minimal;
  a virtualised render (cull nodes outside the viewport) is cheaper than optimising per node.
- Interaction state (pan, zoom, selection box, drag, hover port) is view state: never a command until a
  drag ends. A node move is one `editor_set_position` (batch for a multi-select) on release, **zero
  evaluations** (instrument via the evaluation counters the status panel already reads).
- Sockets are drawn from `OperatorSpec` ports, coloured by `PortType`; variadic ports grow a socket.
  Compatible targets highlight while dragging; incompatible ones dim with a reason on hover.
- Parameter widgets on nodes reuse `panels/widgets.js` (generated from specs), so a new operator needs no
  canvas code.
- Selection: one selection model shared with the viewport (`createSelection`). Selecting a node in the
  graph already lights up its parts through `partsFor`; the graph view must also follow a viewport click.
- Copy and paste: serialise a subgraph (selected nodes, internal links, params, labels, positions), paste
  as one `CommandBatch` with new IDs (aliases) and **external links dropped** (V2-10). Remember the batch
  limits (200 commands). Pasting across documents is allowed; across groups needs the V4 rule (members
  cannot link outside their group, `group_check.rs`).
- Per-node status and diagnostics: `diagnosticsByNode` already merges problems; evaluation state per node
  (queued, running, cached) needs a small worker report (check `evaluation.ts` state and the G2 status
  panel for what exists before adding anything).
- Keyboard-only add and connect (V2-14) and focus visibility are real cases: design the keyboard model
  early (arrow keys between nodes, Enter to connect, search palette on a key).

## 4. V4 design (after V2)
- **Group from selection**: the command exists. `create_group { name, members, output, alias }` turns
  **existing** nodes into a group definition, but refuses unless the members read only each other and
  nothing outside reads them (`command.rs`, `group_check.rs`). So the UI's job is a pure builder (put it in
  `ve-graph` so the MCP can use it too) that, from a set of node IDs, produces one batch that first makes
  the selection legal and then groups it: for each link entering the selection from outside, add a
  `group.input@1` member and reconnect; pick the result member (the selected node nothing else in the
  selection reads, or ask when several); `create_group`; `add_instance`; reconnect every outside
  consumer of the result to the instance; move outputs that named it. Define the naming and
  ambiguity rules (inputs named after the source node's label; refuse when two candidates tie) and test
  them. Then suggest exposed parameters (changed-from-default ones pre-ticked; the user can tick any)
  with `expose_param`. Equality target: make the wheel by UI, compare with
  `ve-graph/tests/fixtures/wheel_group.json`. Check whether `group.input@1` members can be added to a
  group after `create_group` (they can via `add_node` with `group`, but verify the interface-version
  bump so the first instance is not instantly outdated).
- **Enter and exit with a breadcrumb**: the canvas shows either the top level (groups collapsed to
  instance nodes) or one group's members (the outliner already lists them). Entering is view state only.
- **Edit definition versus edit instance**: the inspector already says "Part of the group wheel" and
  edits the instance overrides for exposed parameters (`panels/groups.js`). The canvas must make the
  mode obvious (colour, banner) and never silently edit the definition when the user meant the instance.
- **Upgrade prompts and conflicts shown before applying**: `upgrade_preview` returns what an upgrade would
  drop (see `docs/modeling/g4b-report.md`); the page shows it for **one** instance today. V4 needs it for
  **all instances at once** before an interface edit is applied, in one dialog, with the list of dropped
  overrides per instance; then send the existing combined batch (command plus `upgrade_group` per
  instance, one undo step, as `panels/groups.js` does).
- **Library panel**: "our own groups and templates". Groups live in the document. Cross-document reuse
  needs a library store: decide (an ADR) between copying a group definition into another document with
  a new `GroupId` and a versioned library file in the saved-document store (`store/`). Keep it local and
  private; no sharing service. Templates are documents or groups saved by us and procedural; no imports.
- Gaps from the G4b report to close: make group from selection by UI, breadcrumb, library, upgrade
  preview for all instances.

## 5. Order of work
1. ADR 0016/0017 (canvas technology and layout; library storage) and the V4 case table.
2. Pure modules and tests: layout, link-check query, paste remap, group-from-selection builder.
3. Engine/doc additions: frames and reroutes in the editor layer; `doc_check_link`; the group-from-selection
   builder; view-pin evaluation target.
4. Canvas v1: pan, zoom, nodes, sockets, wires, selection, moves (V2-07, V2-13), then link dragging with
   reasons (V2-02, V2-03), search (V2-04), widgets on nodes.
5. Text view editing (V2-05, V2-06 highlight both sides), copy/paste (V2-10), pin (V2-11), mute/bypass.
6. Frames, reroutes, minimap (V2-08), keyboard model (V2-14).
7. Performance: 200-node and 500-node cases measured on this machine; name it in the report (V2-12).
8. V4 features in the order of 4. Browser tests `web/tests/graph-editor.mjs` and `groups-editor.mjs` driven
   by real pointer and keyboard events; goldens; reports `v2-report.md`, `v4-report.md`.

## 6. Traps
- The 33 ms case needs a real machine; SwiftShader is slow. Measure DOM/script cost separately from GPU
  frame time and say exactly what was measured (Playwright `performance` entries, long-task counts).
- Do not make the canvas the owner of the document or a second copy of the graph: it renders
  `doc.state().document` and sends commands. Concurrent edits (MCP, undo) must re-render cleanly, and a
  drag in progress must be cancelled or rebased when the revision moves.
- IDs in the DOM: node IDs are opaque strings; labels may be duplicated; never key by label.
- Group members are nodes with a `group` field and are hidden from the top level by `topLevel(doc)`. Keep
  that rule in one place.
- The batch cap is 200 commands; pasting or grouping a large subgraph must chunk or refuse with
  `budget.commands` in a message a person understands.
- `src/engine/Editor.tsx` constructs commands by hand and fails typecheck when serde types change.
- Accessibility: focus visible, keyboard reachable (V1-09 already requires it of panels).

## 7. Done means
V2 and V4 case tables pass through real UI events, strict gate passes, reports with measured numbers and the
machine named, goldens reviewed by eye, X7 recorded as "custom canvas, measured", ADRs written, plan status
paragraphs, MCP guide unchanged unless commands were added (then update `tools/mcp/server.mjs` and
`commands.md`), no `#[allow]`, no attribution lines.
