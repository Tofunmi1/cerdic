# Handoff: fewer calls to get oriented and to look (get_guide topics + operators, snapshot cameras, server instructions)

Measured on real runs (2026-10-26, `gpt-6.1-sol`): a cottage took 15 calls and 7 of them were orientation (`get_guide` x4, `list_operators` x2, `get_text`); the mini-golf rebuild took 153 calls: `get_guide` 15, `list_operators` 6, `get_nodes` 12, and `snapshot` + `set_view` 57. The models also never discover `batch`, `run`, `start_from` or `recipes` on their own.

## Test (already written, do not weaken)
`tools/orientation-test.mjs` (`bun tools/orientation-test.mjs` from `engine/`); keep `tools/tool-surface-test.mjs`, `tools/batch-run-test.mjs`, `tools/recipes-test.mjs` passing (total tool definitions stay under 13,000 characters). The sandbox may refuse their ports (`EADDRINUSE`); say so, Claude runs them. If a check looks wrong, say so in Status.

## What to build
1. **MCP server `instructions`**: the `initialize` result carries `instructions` (about 1,200 characters, under 2,500): what the server is, in five lines in the order a model should work: call `get_guide` once with `topics` and `operators` (see below) instead of many calls; begin with `start_from` (a working skeleton to edit) when the request resembles a template; write programs with pipes and loops (topic `compact`); batch any known sequence with `batch` and loops or conditions with `run`; look with ONE `snapshot` of several `cameras`, not one `set_view` + `snapshot` per angle; `edit` by label for small changes; `check` before you finish. Mention each of: batch, run, start_from, edit, snapshot, get_guide, cameras.
2. **`get_guide {topics?: string[], operators?: {query?, family?, ids?, detail?}}`** (keep `topic` as before): `topics` returns every named topic in one answer, each headed `## <topic>` (the core guide counts as topic `core`); an unknown name is an error that lists the valid topics. `operators` returns the same operator index text `list_operators` would give for those arguments appended under `## operators`; the operator list lives on the page, so without a page the guide part still comes back and the operators section says it needs the editor page (not an error for the whole call). Update the schema and description (short).
3. **`snapshot {cameras?: [{position:[x,y,z], target:[x,y,z], label?: string}], camera?: {position, target}}`**: `cameras` (1 to 6) renders one labelled contact sheet from those cameras (the same layout as `views`), `camera` one picture from that camera; the page handler sets each camera, captures, and restores the previous view at the end (exactly as `views` does). `camera`/`cameras` cannot be combined with `views`, `times` or `states` (error naming the conflict); position and target must be three finite numbers each (checked in the server before the page is asked, error says "position must be [x, y, z]"); more than 6 cameras is an error mentioning 6. The page side is `src/engine/mcp-bridge.ts` and the viewer code it calls (search for how `views` is implemented); add bun unit tests next to the existing ones in `web/tests/*.test.ts` or the repo's `tests/`. Move any detail text into the `snapshot_modes` guide topic to keep the definition short.
4. **Make orientation cheaper by default**: the core guide (`get_guide` with no arguments) ends with a four-line "fast start" that repeats the instructions' order; keep it under 1,500 characters of additions.

## Rules
- Edit with edit tools; no sed/heredoc. No commits or staging. You cannot run a browser; Claude runs the live check. Do NOT rebuild any wasm, but if you change anything under `src/engine/` rebuild nothing: Claude rebuilds `viewer-bridge.js` with `bun build src/engine/viewer-bridge.ts --outfile engine/web/dist-bevy-gpu/viewer-bridge.js --target browser --format esm` from the repo root.
- Run: `bun tools/orientation-test.mjs`, `bun tools/tool-surface-test.mjs`, `bun tools/batch-run-test.mjs`, `bun tools/recipes-test.mjs`, `cd engine/web && bun test`, `bunx tsc -b --pretty false` at the repo root.

## Status
Implemented:
- `initialize` now includes five workflow lines (about 1,100 characters) covering combined orientation, templates, compact programs, batch/run, camera snapshots, label edits and the final check.
- `get_guide` supports `topics` (including `core`), headings for each requested topic, unknown-topic errors listing the valid topics, and `operators` using the same selection/formatting path as `list_operators`. Without a page, the guide remains available and the operators section explains the missing editor. Legacy `topic` calls still work. The core guide ends with the four-line fast start.
- `snapshot` supports one explicit `camera` or 1–6 labelled `cameras` using the existing contact-sheet layout. Shared validation rejects invalid finite vectors, counts and conflicting modes before contacting the page, and also validates page-side calls. The viewer captures each camera and restores the previous pose in `finally`, including failed captures. Single-camera snapshots retain baseline comparison. `mcp-bridge.ts` already forwards the parameters and provides the required camera hooks, so its existing handler needs no change.
- Added browser-free Bun tests for camera capture, labels/layout, restoration on success/failure, validation, named-view compatibility, and the actual stdio server with an in-memory page transport. Updated the guide tests for the named core topic and fast-start order. `tools/orientation-test.mjs` is unchanged.

Passed:
- `cd engine/web && bun test`: 21 tests, 209 assertions.
- `bun test tests/guide-topics.test.ts tests/snapshot-diff.test.ts` from the repo root: 13 tests, 51 assertions.
- `bunx tsc -b --pretty false` from the repo root.
- The in-memory server tests confirm tool definitions total 11,163 characters, with every definition under 1,300 characters; they also verify combined operator answers match direct `list_operators` calls for filters, IDs, kits and full details.
- Whitespace checks on the changed tracked implementation files.

Could not verify / pending:
- Attempted all four requested MCP scripts with Bun from `engine/`: `orientation-test.mjs`, `tool-surface-test.mjs`, `batch-run-test.mjs`, and `recipes-test.mjs`. Each server failed to bind with `EADDRINUSE` (ports 8794, 8792, 8791 and 8793 respectively), before any assertions could run. Claude needs to run the original scripts outside this restriction; the new in-memory tests do not replace them.
- `cargo xtask quick` failed at its initial formatting check on existing Rust/vendored files. `cargo xtask rules` failed because 21 existing Rust files exceed 400 lines (including `ve-animstate/src/text.rs` and `ve-graph/src/builtin/world.rs`). No Rust files were edited for this task. The strict gate remains pending.
- No browser was run, and no wasm or viewer bundle was rebuilt. Claude needs to rebuild `viewer-bridge.js` from the repo root with `bun build src/engine/viewer-bridge.ts --outfile engine/web/dist-bevy-gpu/viewer-bridge.js --target browser --format esm`, then verify live camera pictures and restoration.

Nothing was committed or staged; existing workspace changes were preserved.
