# Handoff: model-facing tooling (MCP and provider interface)

Date: 2026-10-16. A model can only build what the interface lets it see and check. Today `engine/tools/mcp/server.mjs` offers `get_guide`,
`list_operators`, `get_text`, `get_nodes`, `get_status`, `apply_text`, `apply_commands`, `undo`, `redo`, `frame_view`, `set_view`. This handoff adds
what was missing in the visual-fidelity tests (a model could not see a result, measure it, or learn why an edit failed). It comes before G9c's
evaluation. Read [`handoff-common.md`](handoff-common.md) and [`docs/modeling/mcp-test-provider.md`](modeling/mcp-test-provider.md). Do not commit.

## Tasks

1. **`snapshot`**: render the current view (or a named fixed view) and return a bounded PNG (size cap, one image per call) plus the rendered
   triangle count and quality tier. Implemented through the page bridge (`src/engine/mcp-bridge.ts`, `viewer-bridge.ts`); no second renderer.
2. **`measure`**: return typed numbers for the evaluated result: per-output bounding box, volume, triangle count, instance count, and the distance
   between two named nodes or sockets. Deterministic, read-only, same numbers the tests assert.
3. **`get_operator_help`**: on demand, one operator's full contract (parameters with units and ranges, ports, one working example, common
   refusals) so the base guide can shrink. Generated from the registry, not hand-written, so it cannot drift; test that every Ready operator has help.
4. **Diagnostics with suggestions**: add a `suggestion` field to the typed diagnostics that models hit most, with a concrete fix the model can
   apply: `budget.commands` (split at `#--split--`), `budget.instances`, `budget.triangles` (lower resolution; planner estimate), `scatter.unsatisfiable`,
   `poly.subdivide.concave_face_triangulated`, `poly.bevel_network_unsupported`, `constraint.*`, parse errors (nearest operator name or parameter).
   Update `diagnostics.md`; test that each listed code carries a suggestion.
5. **Limits visibility**: `get_status` (or a new `get_limits`) returns the live caps: batch size, instance and triangle budgets per quality, current
   usage, last evaluation time. Models should stop discovering limits by failing.
6. **Trimmed guide and prompt caching**: split the guide into a short stable core plus on-demand help; keep the stable part first so provider
   prompt caching applies. Measure guide tokens before and after and record both. List the kits (`ve_modeling::KITS`) in the guide.
7. **Program loading for people**: a `?program=<gallery name>` loader or a small "load gallery model" menu on the viewer page, replacing the
   console call `window.__ve.doc.sendText(...)` described in [`handoff-running-the-web.md`](handoff-running-the-web.md).
8. **Eval harness hook**: a headless runner that takes a task file and a provider (mock or CLI route), drives these tools, and writes structured
   results. G9c builds on it; keep it in `tools/` with no keys in files.

## Rules

Tool inputs and outputs are bounded and schema-validated; MCP stays a test provider on `127.0.0.1` only. Every tool has a test through
`web/tests/mcp.mjs` or a unit test. Keep Rust files under 400 lines and functions under 80. Done means `cargo xtask quick`, `cargo xtask rules` and the
focused browser scripts pass; the strict gate stays pending (see the policy in `handoff-common.md`).
