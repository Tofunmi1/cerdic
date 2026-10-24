# Handoff: a viewer panel that lists parts floating off their neighbours (`ve-measure::attachments` in the viewer) — second attempt

Owner: Claude manages; Codex implements. The first attempt stopped because it tried to read live meshes from the runtime (`ve-runtime/projection`), which was off limits. This time the check runs where the evaluated parts already are: in the **evaluator wasm** (`ve-evaluator`), which holds the
full evaluation result (`HOST.result.parts`, each `Part` with `positions`, `indices`, `transforms`, `name`), so no runtime or GPU data is needed.

**Files you may edit:** `engine/crates/ve-evaluator/src/wasm.rs` (+ its `Cargo.toml` to depend on `ve-measure`), `engine/web/eval-worker.js`, `src/engine/{bridge.ts,worker-types.ts,mesh-upload.ts}` and whatever TypeScript file receives the worker's `result` message and updates the viewer panels (find it),
`engine/web/viewer.js` (and `engine/web/panels/*`). Do NOT edit `ve-bevy`, `ve-runtime` or `ve-modeling`. Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`;
use exactly `CARGO_TARGET_DIR=/tmp/codex-target-attach2` (other helpers use other directories; if a build waits on a lock, someone is using your directory, wait and retry); do not commit or stage; edit with your edit tools, not scripts. The sandbox cannot run the browser or bind a port; Claude does the browser check.

## Why

Parts built separately (a dragon's horns, a horse's mane) were placed by typed coordinates and floated off the body. `ve-measure` has `attachments(parts, tolerance)` and `attachments_json(...)` (`crates/ve-measure/src/attach.rs`, `tool.rs`, tests `tests/attach.rs`, done): the true smallest surface gap of each part to every other. We want it visible in the viewer.

## What to build

1. **ve-evaluator `wasm.rs`**: `#[wasm_bindgen] pub fn eval_attachments(tolerance: f32) -> String`: from `HOST.result` (the last finished evaluation), build one `NamedMesh` per part and return `attachments_json` text (floating list `{name, gap, nearest}` plus suggestion). Part selection: skip parts that are placed sets with more than one copy
   (`transforms` has more than one matrix), parts with fewer than 4 triangles, and parts with no `transforms` problem (a part with exactly one transform: apply it to the positions; column-major `[f32;16]`). Parts of one output that are material regions (same `name`, different `region`) must be merged into ONE named mesh (concatenate vertices, offset indices) so a body with two colours is one part.
   Return `{"floating":[],"parts":N}` for fewer than two parts, `{"error":"<text>"}` when the `ve-measure` budget (64 parts, 2,000,000 triangles) is exceeded. Add the function type to `src/engine/bridge.ts` if the evaluator API is typed there.
2. **eval-worker.js**: after a `final` quality result is posted, call `api.eval_attachments(0.02)` (only for final, never preview) and include the parsed JSON as `attachments` in the `result` message (or post a second `attachments` message, your choice; keep the existing message shape for old readers) and thread it through the TypeScript worker types to wherever the viewer panels get their data.
3. **Viewer**: in `viewer.js` where the Diagnostics checkbox and the Problems list are built, add a **"Floating parts"** row visible when Diagnostics is on: "all parts attached" or one line per floating part `name  0.12 m from  nearest`; clicking a line selects that part the way clicking it in the outliner does (find the existing select function). Style like the surrounding file; no new dependency.

## Tests

- Rust: `ve-evaluator` has a native build? If its `wasm.rs` logic can be tested natively, put the part collection and merging in a plain function and test: two touching boxes -> empty `floating`; a box 1.5 m away -> one entry with `gap` about 1.5; a two-region part counts once. If the crate only builds for wasm, put that pure function in a testable place that you are allowed to edit and say so.
- Browser contract (write it, Claude runs it): `engine/web/tests/attach-panel.mjs` in the style of the other `web/tests/*.mjs` scripts: load the viewer with a world of two boxes floating 1 m apart, open Diagnostics, expect a "Floating parts" entry naming a box; then a world with touching boxes and expect "all parts attached".
- `cargo check -p ve-evaluator --target wasm32-unknown-unknown` passes and `cargo build -p ve-evaluator --target wasm32-unknown-unknown --release` builds.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked. Browser verification remains with Claude as specified above.
- Tests run and their output (browser test: unverified is expected): `CARGO_TARGET_DIR=/tmp/codex-target-attach2 cargo test -p ve-evaluator attachments::tests` passed (2 tests); `CARGO_TARGET_DIR=/tmp/codex-target-attach2 cargo check -p ve-evaluator --target wasm32-unknown-unknown` passed; `CARGO_TARGET_DIR=/tmp/codex-target-attach2 cargo build -p ve-evaluator --target wasm32-unknown-unknown --release` passed. Existing unrelated warnings appeared in `ve-geometry` and `ve-modeling`. Browser contract `engine/web/tests/attach-panel.mjs`: written, unverified; Claude runs it.
- Files changed: `engine/crates/ve-evaluator/Cargo.toml`, `engine/crates/ve-evaluator/src/lib.rs`, `engine/crates/ve-evaluator/src/wasm.rs`, `engine/crates/ve-evaluator/src/attachments.rs` (new), `engine/web/eval-worker.js`, `engine/web/viewer.js`, `src/engine/evaluation.ts`, `src/engine/worker-types.ts`, `engine/web/tests/attach-panel.mjs` (new), and this handoff status.
