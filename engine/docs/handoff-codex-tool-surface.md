# Handoff: a smaller tool surface and the `edit` / `diff` tools (ADR 0069, ADR 0061), plus cleanup

Read `docs/adr/0061-model-tool-surface.md` (the `edit` and `diff` rows) and `docs/adr/0069-compact-text-and-batched-tools.md`.

## Test (already written, do not weaken)
`tools/batch-run-test.mjs` must keep passing, and `tools/tool-surface-test.mjs` (new, run `bun tools/tool-surface-test.mjs` from `engine/`) must pass. If a check looks wrong, say so in Status.

## Work
1. **Shrink every tool definition a model carries each turn.** Today all 26 definitions are about 17,000 characters; `script` (2,100) and `snapshot` (2,300) are the largest. Move their detail into guide topics `script_api` and `snapshot_modes` (the full text now in the descriptions, nothing lost, including every function and parameter), keep each description to what the tool is for and which topic has the rest. The same for any other definition over about 900 characters when the detail is reference material (`list_operators`, `playtest`, `make_character`, `apply_commands`, `measure`, `find`). Parameter descriptions in schemas shrink the same way. Targets in the test: each definition ≤ 1,300 characters, total ≤ 13,000. Update the core guide's topic menu. Generated tool schemas and descriptions come from the single manifest in `tools/mcp/` (ADR 0061 rule 6); keep it that way.
2. **`edit` tool** (ADR 0061): `edit {ops: [{op: "set", node, params: {...}} | {op: "rename", node, to} | {op: "delete", node} | {op: "replace", node, text}], label?, dry_run?}` applied atomically by node label (labels are unique; an ambiguous or unknown label is an error that lists the candidates), returning the same `delta` as `apply_text`. It is implemented over the page's existing command path (`apply_commands` already takes labels as node names; translate ops to typed commands in the page-side handler, where `apply_text`/`apply_commands` live). Unknown op names are errors that list `set, rename, delete, replace`. The page handler is JS in `web/`; add unit tests next to the existing bun tests (`web/tests/*.test.ts`) with the document mocked as the other tests do.
3. **`diff` tool**: `diff {since?: revision number}` returns the text difference (compact form) between the document at revision `since` (default: the previous revision) and now, as added/changed/removed node lines, capped at 80 lines with a count of the rest. Use the revision history the page already keeps (`History` in ve-graph; check what the page exposes). Argument validation (non-number `since` is an error) must work with no page connected (the test checks it).
4. **Cleanup:** fix the existing TypeScript errors reported by `bunx tsc -b --pretty false` in `src/engine/mesh-upload.ts:67`, `src/engine/snapshot.ts:14`, `tests/shader-expr.test.ts:84`, `tests/surface-signals.test.ts:30,34,37` (find out what each is; do not silence with `any` or `@ts-ignore`; if one needs a design decision, say so in Status and leave it).

## Rules
- Edit with edit tools; no sed/heredoc edits. No commits or staging. No `cargo xtask`. You cannot run a browser, and the sandbox may refuse the test server's port (`EADDRINUSE`); say so, Claude runs those.
- Run: `bun tools/tool-surface-test.mjs`, `bun tools/batch-run-test.mjs`, `cd web && bun test`, `bunx tsc -b --pretty false`.

## Status

Implemented the compact manifest, guide topics, `edit`, `diff`, and all listed TypeScript fixes. The single manifest still lives in `tools/mcp/server.mjs`: 28 definitions total 10,113 serialized characters; the largest is 636. Original reference descriptions and parameter documentation are preserved in guide topics, including `script_api` and `snapshot_modes`, and the core menu lists them.

The JS page handler is `web/tool-edits.js`, wired into the existing `src/engine/mcp-bridge.ts` command/evaluation path. Edits resolve labels in operation order, validate the whole request before sending one transaction, preserve replacement users/outputs/flags, and use the existing delta and dry-run path. Argument validation is shared with the server, so malformed edit/diff calls fail without a page. Diff reads bounded revision snapshots from the engine session, including undo/redo/branches, without moving history; compact node lines include group members and are capped at 80 with an omitted count. Revision snapshots are retained for up to 200 prior revisions and 16 MiB; unavailable revisions report the retained range.

Passed:
- `cd engine/web && bun test`: 10 page-handler tests.
- `bunx tsc -b --pretty false`: all listed errors fixed without `any` or suppression.
- Focused Bun tests for shader expressions, surface signals, guide topics, measure, and bake: 29 tests.
- `cargo test -p ve-graph --test history_list --test g72_tool_lines`: 8 tests.
- `cargo check -p ve-runtime --target wasm32-unknown-unknown` (existing unrelated warnings).
- Read-only manifest size/reference checks and actual `--guide` CLI checks for both new topics and the core topic menu.
- Rust formatting checks for changed Rust code and `git diff --check`.

Could not verify `bun tools/tool-surface-test.mjs` or `bun tools/batch-run-test.mjs`: both were run unchanged, but their child servers failed to bind ports 8792/8791 with `EADDRINUSE` in the sandbox. Their waiting parent processes were stopped. Claude must run these and the browser integration checks outside this sandbox; no browser was launched here. An additional `cargo clippy -p ve-graph --lib -- -D warnings` was blocked by 18 pre-existing errors in other graph code (mostly oversized builtin functions, plus identical match arms in `groups.rs`); none were silenced. No check was judged incorrect.

All Cargo commands used `CARGO_TARGET_DIR=/tmp/codex-target-surface`. No `cargo xtask`, commits, or staging; the strict gate remains pending as instructed.
