# Task for Codex: transparent materials (water, glass)

Owner: work in parallel with Claude. Claude is doing physics joints and collide triggers at the same time, so stay inside
the files named under "Boundaries". Read `engine/AGENTS.md` first and follow it, plus the owner rules at the end.

## Goal
`solid.material` has no opacity, so scenes cannot have water, glass, windows, bottles or aquariums. Add an `opacity`
parameter (0 clear to 1 solid, default 1) that the model sets in the typed graph, the viewer draws with real alpha
blending, and the glTF exporter writes as an alpha-blended material. Everything stays deterministic and runs in the browser.

## What is already there (verify, do not trust)
- `crates/ve-modeling/src/result.rs`: `pub struct Material` (color, metallic, roughness, emissive, emits_light, recipe,
  texture_scale, texture_program). Add `opacity: f32` here, serde default 1.
- `crates/ve-modeling/src/appearance.rs`: builds a `Material` from the `solid.material` node params.
- `crates/ve-graph/src/builtin/modeling.rs`: the `solid.material@1` spec (look for `"emits_light"`); add the param there with a
  doc line a model can act on ("0.3 is tinted glass, 0.6 murky water, 1 solid").
- `crates/ve-render/src/material.rs` line ~108 already carries `base.alpha`; `crates/ve-render/src/pipeline.rs` has the
  main pipeline (look at `alpha_to_coverage_enabled`, blend state, depth write). The viewer is a trimmed Bevy fork
  under `third_party/bevy`: find how `VeMaterial` becomes a draw and whether a transparent pass exists. If none, add one:
  draw opaque parts first, then transparent parts sorted back to front by part centre (deterministic tie-break by node
  id), with depth test on and depth write off.
- `crates/ve-runtime/src/projection/` stages materials into the scene by authored node (`eval_projection_material_region` in
  `src/engine/mesh-upload.ts` sends the material JSON). Make sure `opacity` flows through that path.
- glTF export: `src/engine/gltf.ts` writes the materials; set `alphaMode: "BLEND"` and `baseColorFactor[3] = opacity`
  when opacity < 1 (keep `OPAQUE` otherwise). The Khronos validator (`gltf-validator`, used in `engine/web/tests/g10-export.mjs`
  and `tests/skin.test.ts`) must report 0 errors and 0 warnings.

## Acceptance (all of these)
1. Rust tests: the node accepts `opacity` 0 to 1, refuses outside, defaults to 1; the evaluation result carries it; a part
   with opacity 1 produces byte-identical output to before (existing corpus lines unchanged).
2. Parity corpus: add a transparent material case to `crates/ve-modeling/examples/corpus.rs`; native and `wasm32-wasip1`
   outputs identical (`cargo run -q -p ve-modeling --example corpus` against `node tools/run-wasi.mjs
   target/wasm32-wasip1/debug/examples/corpus.wasm`).
3. Browser test `engine/web/tests/g10-opacity.mjs` modelled on `g10-physics.mjs` or `g10-terrain.mjs` (own port, `?eval=1`):
   a red box seen through a half-transparent blue pane shows a blended colour (measure pixels: neither pure red nor pure
   blue); the same pane at opacity 1 hides the box. Look at the screenshot yourself.
4. glTF: a unit test in `tests/` for the exporter with a transparent part, validated with the Khronos validator.
5. MCP guide (`tools/mcp/server.mjs`, search `solid.material`): a short paragraph with an example (a water slab, a window
   pane). A prompt set `tools/mcp/prompts/opacity-tools.json` with three prompts (aquarium, window in a wall, bottle)
   and a check in `tools/mcp/eval-prompts.mjs` that the material has `opacity < 1` on the right part; run
   `bun tools/mcp/eval-prompts.mjs --set opacity-tools --model sonnet --parallel 1` and report the result.
6. Add a short ADR `docs/adr/0036-material-opacity.md` (context, decision, consequences, status proposed) and one line in
   `docs/plan-animation-and-blender.md` under W16 ("transparency for water") marking it done with the evidence.

## Boundaries (to avoid colliding with Claude's work)
Do edit: `ve-modeling/src/result.rs` (the `Material` struct only), `ve-modeling/src/appearance.rs`,
`ve-graph/src/builtin/modeling.rs` (the `solid.material` spec only), `ve-render`, `ve-runtime/src/projection/` and
`projection/surfaces.rs`, `src/engine/gltf.ts`, `src/engine/mesh-upload.ts` (material lines only), the MCP files above, new tests.
Do not edit: `ve-physics`, `ve-runtime/src/physics.rs`, `ve-runtime/src/behavior*`, `ve-modeling/src/physics.rs`,
`ve-graph/src/builtin/physics.rs`, `ve-runtime/src/interact.rs`. If you must touch `ve-runtime/src/projection.rs`, keep the
change to a few lines and say so in your report.

## How to run things (shared machine, shared `engine/target`)
- Cargo uses one `target` directory: if a build waits on a lock, wait; do not delete `target`. Disk is tight (about 20 GB
  free): check `df -h /System/Volumes/Data` before a wasm build; `rm -rf engine/target/release` is safe.
- Rebuild the browser bundle with the scratch script, not xtask: the owner said xtask wastes time. The script lives at
  `/private/tmp/claude-501/-Users-mac-Work-victoria-vr-hack/9d098cda-d740-48c5-ac1e-a0a723c575c1/scratchpad/web_build.sh`
  (it runs `cargo build -p ve-runtime -p ve-evaluator --target wasm32-unknown-unknown`, wasm-bindgen, and copies the page
  files into `web/dist`). Read it and reproduce it if the path is gone.
- Run only your own browser test while Claude may be running another: use a port nobody uses (5230 to 5249). Never run the
  whole browser sweep. Kill only the processes you started (by pid), never `pkill` by name: that broke the last run.
- Tests: `cargo test -p ve-modeling -p ve-graph -p ve-render`, `bun test` only on named files under `tests/`
  (a root `bun test` also picks up `spikes/` and fails).

## Owner rules
- Edit files with your edit/write tool only; no heredocs, `sed`, `perl` or `python` to change repo files.
- No `Co-Authored-By`, "Generated with" or any tool attribution in commits or PRs. Do not commit unless asked.
- Everything runs in the browser. No pulled models, textures or assets. Fix lints, never silence them (`#[expect(..., reason)]`
  on the smallest item is the only allowed form).
- Report in chat what you ran and what passed, with the numbers; say plainly what you did not finish.
