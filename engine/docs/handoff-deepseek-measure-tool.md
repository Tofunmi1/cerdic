# Handoff to DeepSeek: make `measure` callable by a model (browser export + MCP tool), and fix two over-length files

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public
items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition
2024 --config-path rustfmt.toml <file>` from `engine`.** Use the edit tools for files. Do not build the browser bundle (the owner's other engineer does); use `cargo
check --target wasm32-unknown-unknown`, `bunx tsc --noEmit` from the repo root, and `node --check`.

## Context
You wrote `crates/ve-measure` (verified: tests pass). Nothing calls it. The point is that a model verifies what it built: "is it closed, how big, how thick".
The editor page exposes evaluated parts (positions, indices, normals, names) to JS (`window.__ve.evaluation.parts`), `crates/ve-runtime` exports wasm functions
(read `src/document_exports.rs`, `src/instance_exports.rs`), `src/engine/bridge.ts` and `src/engine/mcp-bridge.ts` (repo root) connect the page to the test MCP
server `engine/tools/mcp/server.mjs` (read how an existing tool such as `make_character` is wired through all four layers and copy that path exactly).

## You may edit
`crates/ve-measure/**`, a new `crates/ve-runtime/src/measure_exports.rs` (+ one `mod` line in `lib.rs`), `crates/ve-runtime/Cargo.toml` (add the dependency),
`src/engine/bridge.ts`, `src/engine/mcp-bridge.ts`, `engine/tools/mcp/server.mjs`, new tests next to the existing TS tests, and the Status below. **Do not touch**
`crates/ve-render`, `crates/ve-runtime/src/fx.rs`, `crates/ve-runtime/src/animation/**`, `crates/ve-modeling/**`, `crates/ve-graph/**`, `cpp/`, `web/`.

## Tasks
1. **Wasm export** in `measure_exports.rs`: `measure_mesh(positions: &[f32], indices: &[u32], uvs: &[f32], detail: &str) -> String` returning JSON (never
   panics, never traps: errors come back as `{ "error": "..." }` with the suggestion). `detail` is `"summary"` (the 12-line text from `Report::summary()` plus
   thickness as one extra line) or `"full"` (every field of `Report`, thickness, symmetry error for X/Y/Z, uv report when uvs are given, bounding shapes).
2. **MCP tool `measure`** (one consolidated tool, per the research on writing tools for agents: few tools, concise by default, detail on request): inputs
   `part` (optional output/part name; default every part), `detail` (`summary` default | `full`), `checks` (optional list of `thickness`, `symmetry`, `uv`,
   `collision`). Output: concise text for `summary`, JSON for `full`. It reads the current evaluation's parts through the same bridge the other tools use. A
   part that is not a closed solid says what to do (e.g. "3 open edges near (0.1, 0.0, 0.2): close the hole or use poly.fill"). Add the tool to the MCP guide text
   in `server.mjs` in one short line.
3. **Tests:** a Rust unit test for `measure_mesh` (cube summary has volume 1.0 and "watertight: yes"; a cube with one triangle removed reports 3 open edges and
   the suggestion; malformed input like indices out of range returns `{"error"...}` rather than panicking), a bun/TS test for the MCP tool's argument handling and
   formatting with a fake bridge, and `node --check` on the server.
4. **Two over-length files** the rules gate flags: `crates/ve-runtime/src/lib.rs` (411 lines; move a cohesive group of items into a new module file, keep every
   public item reachable at the same path so nothing else changes) and `crates/ve-graph/src/builtin/poly_network.rs` (407 lines; move one family of specs into a new
   sibling file). These two files were edited by others earlier; make the smallest mechanical move, change no behaviour, and run `cargo test -p ve-graph` and
   `cargo check -p ve-runtime --target wasm32-unknown-unknown` after each. Tell me exactly what moved.

## Acceptance
`cargo test -p ve-measure -p ve-graph`, `cargo clippy -p ve-measure --all-targets -- -D warnings`, `cargo check -p ve-runtime --target wasm32-unknown-unknown`,
`bunx tsc --noEmit`, the new bun test passes, `cargo xtask rules` shows neither of the two files over 400 lines.

## Status (DeepSeek: fill in, then stop)
- State: done; all four tasks implemented and the acceptance commands pass. Nothing committed. The full strict gate (`cargo xtask check --strict`) was **not** run (owner-level gate).
- What was built:
  - **Wasm export.** `crates/ve-runtime/src/measure_exports.rs` is a thin `#[wasm_bindgen]` wrapper: `measure_mesh(positions, indices, uvs, detail) -> String` calls `ve_measure::measure_mesh`. The root `lib.rs` declares `mod measure_exports; pub use measure_exports::measure_mesh;`, so the export path is unchanged. `ve-measure` was added to `crates/ve-runtime/Cargo.toml`.
  - **Testable logic in `ve-measure`.** `crates/ve-measure/src/tool.rs` holds `measure_mesh`: it turns the flat arrays into a `Mesh` (refusing odd lengths), then returns `Report::summary()` plus one thickness line for any `detail` other than `"full"`, or the complete JSON (every report field, thickness, symmetry X/Y/Z, UVs when given, bounding shapes) for `"full"`. Malformed input returns `{"error": ..., "suggestion": ...}` and never panics. Exported via `mod tool; pub use tool::measure_mesh;`. JSON is hand-written to keep the crate's dependency set (`glam` + `thiserror`) intact.
  - **Bug fix found on the way.** `crates/ve-measure/src/uv.rs` sized its union-find by `welded.len()` (UV count) but indexed it by triangle id, which panics on a mesh whose UVs differ from its triangle count. Changed to `Dsu::new(triangles)`; existing UV tests still pass.
  - **MCP tool `measure`.** `src/engine/bridge.ts` gains `measure_mesh` on the `Runtime` type. `src/engine/mcp-bridge.ts` replaces the old `measure` case with an exported `measureParts(host, params)`: it reads `host.evaluation.parts` and calls `runtime.measure_mesh(part.positions, part.indices, uvs, detail)`. `part` filters by output name or node; `detail` is `summary` (default) or `full`; `checks` (`thickness`, `symmetry`, `uv`, `collision`) limits which optional sections the full JSON keeps (`collision` = `bounding_shapes`). Because a part carries per-triangle-corner UVs and the engine wants per-vertex UVs, corner UVs are folded to per-vertex UVs; if a vertex's corners disagree (a seam) UVs are omitted rather than passed wrong. The default output is per-part plain text from the engine (the summary already ends with `problem: 3 open edges near (…) : close the hole or use poly.fill`, the requested "says what to do"). `engine/tools/mcp/server.mjs` gets the new `measure` inputSchema (`part`, `detail` enum, `checks` array) and one short GUIDE line; the stale `total_volume` wording was removed. `host.evaluation.measures` is left in place: `engine/tools/mcp/eval-prompts.mjs` and `engine/web/tests/g10-export.mjs` still read it.
  - **Tests.** `crates/ve-measure/tests/tool.rs` (6 tests: cube volume 1.0 + watertight, removed triangle → 3 open edges + fix, out-of-range and odd arrays → error JSON, full JSON sections, UV section when given). `tests/measure-tool.test.ts` (5 tests with a fake runtime/host covering per-part summary, `part` filtering, empty evaluation, full JSON + `checks` filtering, and corner→per-vertex UV folding/dropping).
  - **Two over-length files (task 4).**
    - `crates/ve-runtime/src/lib.rs`: 411 → 316 lines. Moved the whole gizmo/selection export group (`viewer_gizmo`, `viewer_gizmo_at`, `gizmo_begin`, `gizmo_begin_profile`, `gizmo_begin_camera_plane`, `gizmo_update`, `gizmo_cancel`) into a new `gizmo_exports.rs`, re-exported at the crate root (same public paths, matching the existing `instance_exports`/`tangent_exports` pattern).
    - `crates/ve-graph/src/builtin/poly_network.rs`: 407 → 259 lines. Moved the whole-mesh family (`poly.unwrap_uv`, dissolve, decimate, unsubdivide, symmetrize, triangulate, remove doubles) into a new sibling `builtin/poly_network_whole.rs` as `pub(super) fn whole_mesh_tools()`; `all()` now calls `super::poly_network_whole::whole_mesh_tools()`. The shared helpers (`POLY`, `optional_selection`, `number`, `length`, `flag`, `choice`) became `pub(super)`. One `mod poly_network_whole;` line was added to `builtin.rs`. No behaviour change.
- Commands run and results:
  - `cargo check -p ve-graph` — ok.
  - `cargo test -p ve-measure -p ve-graph` — all pass (6 new `measure` tests, 3 UV tests, and the full `ve-graph` suites).
  - `cargo clippy -p ve-measure --all-targets -- -D warnings` — clean (two test casts and a single-char pattern were fixed).
  - `cargo check -p ve-runtime --target wasm32-unknown-unknown` — ok. One pre-existing `dead_code` warning for `ViewerRig::is_walking` in `src/viewer/part_1.rs` (untouched here).
  - `bunx tsc --noEmit` (repo root) — clean.
  - `bun test tests/measure-tool.test.ts` — 5 pass, 0 fail.
  - `node --check engine/tools/mcp/server.mjs` — ok.
  - `cargo xtask rules` — exit 0, no file over 400 lines.
  - `rustfmt --edition 2024 --config-path rustfmt.toml` on the edited Rust files only (never `cargo fmt`).
  - Strict gate `cargo xtask check --strict` — pending (not run; owner/milestone gate).
- Open questions or things you chose not to do:
  - **Rule conflict.** The "Do not touch" list names `crates/ve-graph/**`, but task 4 requires moving specs out of `crates/ve-graph/src/builtin/poly_network.rs` (and the acceptance requires it under 400 lines). I treated task 4 as the explicit exception, touched only that file plus the new sibling and the one `mod` line in `builtin.rs`, and changed no behaviour. `builtin.rs` is likewise not on the may-edit list but needs the `mod` line. Please confirm this is the intended reading.
  - **Placement of `measure_mesh`.** The testable logic lives in `ve-measure`, not in the `ve-runtime` wrapper, because `ve-runtime` is `#![cfg(target_arch = "wasm32")]` and a unit test there would never run under the native acceptance command (`cargo test -p ve-measure -p ve-graph`). `measure_exports.rs` stays a one-line delegate.
  - **Summary vs `checks`.** `checks` filters the optional sections of the `full` JSON only; the plain-text summary is fixed (it always ends with the topology verdict and a thickness line), so `checks` has no effect in `summary` mode. Say the word if you want the thickness line suppressed in the summary too.
  - **Seam UVs.** A part whose corner UVs disagree per vertex (a seam) is measured without UVs (the `uv` section is `null`), since the engine's `Mesh` only takes one UV per vertex. A UV-aware measurement of seamed meshes would need corner UVs in `ve-measure`; not done here.
  - **`piece`/multipart totals.** The old tool summed `total_volume`/`total_triangles`; the new one reports per part and does not add them up, because the point of the tool is per-mesh topology. Multi-colour part volumes are all shown individually.
