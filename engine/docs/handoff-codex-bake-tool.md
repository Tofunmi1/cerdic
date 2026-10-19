# Handoff to Codex: make the baker callable by a model (wasm export + MCP `bake` tool that returns the maps as images)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool
attribution; TypeScript follows the style of `src/engine/mcp-bridge.ts`; no network). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml
<file>` from `engine`.** Do not build the browser bundle. Run TypeScript checks from the **repo root**: `bunx tsc --noEmit`, `bun test tests/*.test.ts`, `node --check engine/tools/mcp/server.mjs`. Write the Status into **this
file** at the end.

## Shared tree: hard rules (another engineer works in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails, say so in the Status; do not revert it.
- **Files you own this round**: `engine/crates/ve-measure/src/bake_tool.rs` (new) plus one `mod`/`pub use` line in `engine/crates/ve-measure/src/lib.rs`, `engine/crates/ve-measure/tests/bake_tool.rs` (new),
  `engine/crates/ve-runtime/src/bake_exports.rs` (new) plus one `mod` line in `engine/crates/ve-runtime/src/lib.rs`, `src/engine/bridge.ts` (the `Runtime` type), `src/engine/mcp-bridge.ts`, a new
  `src/engine/bake.ts` and `src/engine/png.ts` if you need them, `engine/tools/mcp/server.mjs`, `tests/bake-tool.test.ts` (new).
- **Others are editing**: `engine/crates/ve-graph/**`, `engine/crates/ve-modeling/**`, `engine/cpp/**`, `src/engine/gltf*.ts`. Do not touch them.

## Context
`ve_measure::bake_normals` and `bake_ao` exist and are tested (read `engine/crates/ve-measure/src/bake.rs`; the public types are `LowMesh { positions, indices, corner_uvs }`, `Bake { size, rgba, covered }`, and a high
mesh is a `ve_measure::Mesh`). Nothing calls them from the browser. The pattern to copy end to end is the `measure` tool: `engine/crates/ve-measure/src/tool.rs` (`measure_mesh` returns JSON text and never panics),
`engine/crates/ve-runtime/src/measure_exports.rs` (the thin wasm wrapper), `src/engine/bridge.ts` and `src/engine/mcp-bridge.ts` (`measureParts`), and `engine/tools/mcp/server.mjs`. The `snapshot` tool in
`src/engine/snapshot.ts` and `server.mjs` shows how a tool returns an **image** to the model (an `image` content block) with a text summary beside it.

## What to build
1. `ve-measure/src/bake_tool.rs`: `bake_maps_json(low_positions: &[f32], low_indices: &[u32], low_corner_uvs: &[f32], high_positions: &[f32], high_indices: &[u32], size: u32, maps: &str, cage: f32,
   max_distance: f32, rays: u32, bias: f32) -> String`. `maps` is `"normal"`, `"ao"` or `"both"`. It validates the flat arrays (lengths multiple of 3 and 2), builds `LowMesh` and `Mesh`, runs the requested bakes and returns
   JSON: `{"ok":true,"size":N,"normal":{"covered":C,"rgbaBase64":"..."}|null,"ao":{...}|null}`; every failure is `{"ok":false,"error":"...","suggestion":"..."}` using the same suggestion text as `tool.rs`
   (extend its `suggestion` match only if you must; a missing UV list says to run `poly.unwrap_uv`). No panics, no new dependency (write the small base64 encoder yourself).
2. `ve-runtime/src/bake_exports.rs`: a `#[wasm_bindgen]` function `bake_maps(...)` with the same arguments, calling `bake_maps_json`; export it like `measure_mesh` is exported.
3. `src/engine/bridge.ts`: add `bake_maps` to the `Runtime` type. `src/engine/mcp-bridge.ts`: a new method `bake` (export a function `bakeParts(host, params)` like `measureParts`). Parameters: `low` (output or part name of
   the low-poly part, required), `high` (the high-poly part, required), `size` (64 to 2048, default 512), `maps` (array of `"normal"` and/or `"ao"`, default both), `cage`, `max_distance`, `rays` (1 to 256, default 64), `bias`.
   Defaults scale with the low part's bounding-box diagonal D: cage = 0.05 * D, max_distance = 0.1 * D, bias = 0.001 * D. The low part must have UVs (`part.uvs`, one per triangle corner: refuse otherwise with a suggestion
   that names `poly.unwrap_uv`). A part name that does not exist is refused with the list of part names on screen. Convert each baked RGBA image to a PNG in the page (a small PNG encoder in `src/engine/png.ts` using the
   browser's `CompressionStream('deflate')` or a stored-block (uncompressed) zlib stream if you prefer no async: either is fine, but the PNG must be valid) and return `{ ok, size, covered, maps: { normal?: <base64 png>,
   ao?: <base64 png> } }`.
4. `engine/tools/mcp/server.mjs`: a `bake` tool (inputs as above, short description: bake a high-poly part's normals and ambient occlusion onto a low-poly part's UVs; the model must have unwrapped the low part with
   `poly.unwrap_uv`), returned as an `image` content block per map (`image/png`) plus one text block with the summary. Add one short line about it to the guide text.

## Tests (fixed; do not weaken any assertion; if one fails, say which and why)
Rust, `engine/crates/ve-measure/tests/bake_tool.rs` (build the flat quad `[0,0,0],[1,0,0],[1,1,0],[0,1,0]`, indices `[0,1,2,0,2,3]`, corner UVs equal to xy; a tilted high quad has z = 0.5 x; decode base64 in the test):
1. Low = flat quad, high = tilted quad, size 32, cage 0.6, max_distance 1.0, `maps="normal"`: `ok` is true, `ao` is null, the decoded image has 32*32*4 bytes, `covered` is at least 960, and the centre pixel is (70, 128, 242)
   within 2 per channel.
2. `maps="both"` against the flat quad itself: both entries present; the AO centre pixel is 255 within 3.
3. No corner UVs (an empty list) gives `ok:false` and a suggestion containing `poly.unwrap_uv`.
4. `size=4`, `rays=0` (with `maps="ao"`), and an empty high mesh each give `ok:false` with a non-empty suggestion.
5. The base64 decodes to exactly `size * size * 4` bytes for sizes 8, 33 and 64.
TypeScript, `tests/bake-tool.test.ts` (a fake `runtime.bake_maps` that records its arguments and returns canned JSON with a valid base64 image; fake parts with `positions`, `indices`, `uvs`, `name`):
6. Defaults: with a low part whose bounding-box diagonal is D, the call receives cage 0.05 D, max_distance 0.1 D, bias 0.001 D, rays 64 and size 512 (within 1e-6).
7. A missing part name is refused and the message lists the names that exist.
8. A low part with no `uvs` is refused with a suggestion that contains `poly.unwrap_uv`.
9. The returned PNGs start with the PNG signature (137 80 78 71 13 10 26 10) and their IHDR width and height equal `size`; `maps: ["ao"]` returns only the ao image.
10. Out-of-range arguments (`size` 8, `rays` 500) are refused with a message that names the allowed range.

## Acceptance
`cargo test -p ve-measure`, `cargo clippy -p ve-measure --all-targets -- -D warnings`, `cargo check -p ve-runtime --target wasm32-unknown-unknown`, `bunx tsc --noEmit`, `bun test tests/*.test.ts`, `node --check
engine/tools/mcp/server.mjs`, `cargo xtask rules` (from `engine/`; all your files under 400 lines).

## Status (Codex: fill in with real numbers for each of the ten tests, then stop)
- State: complete
- Per test: pass and the measured value:
  1. Rust normal bake: covered 1024/1024 texels; decoded RGBA length 4096; centre RGB `[70, 128, 242]`; AO was `null`.
  2. Rust both maps: normal and AO entries present; AO centre RGBA `[255, 255, 255, 255]`.
  3. Rust empty corner UVs: `ok:false`; suggestion contains `poly.unwrap_uv`.
  4. Rust invalid inputs: size 4, rays 0, and empty high mesh each returned `ok:false` with a non-empty suggestion.
  5. Rust base64 lengths: size 8 = 256 bytes, size 33 = 4356 bytes, size 64 = 16384 bytes.
  6. TypeScript defaults: low bounds diagonal D = 5; cage 0.25, max_distance 0.5, bias 0.005, rays 64, size 512.
  7. TypeScript missing part: refused and listed on-screen names `low, high`.
  8. TypeScript missing UVs: refused with a suggestion containing `poly.unwrap_uv`.
  9. TypeScript PNG: signature `[137, 80, 78, 71, 13, 10, 26, 10]`; IHDR 64 x 64; `maps:["ao"]` returned only `ao`.
  10. TypeScript range checks: size 8 refused with `64 to 2048`; rays 500 refused with `1 to 256`.
- Failing tests anywhere: none in the requested acceptance commands. `cargo xtask quick ve-measure ve-runtime` failed its initial formatting check on pre-existing, unowned `ve-measure/tests/query.rs`, `ve-measure/tests/symmetry.rs`, `ve-measure/tests/uv.rs`, and `ve-modeling/src/lib.rs` diffs. The targeted rustfmt command passed for the Rust files owned here; no unowned files were edited.
- What you tried when something blocked you: investigated the quick-gate formatting diff and left the other engineer's files untouched per the shared-tree rules. The first focused bake test run exposed a base64 test-decoder bug; fixed its bit accumulator, reran it, and all five Rust bake tests passed. Did not run a browser bundle build or commit.
