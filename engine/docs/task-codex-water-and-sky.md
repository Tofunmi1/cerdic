# Task for Codex: water and a procedural sky

Owner: work in parallel with Claude. Claude is doing scene gravity and a walking character controller (physics) at the same
time, so stay inside the files named under "Boundaries". Read `engine/AGENTS.md` and the finished brief
`engine/docs/task-codex-material-opacity.md` first (same working rules; its "How to run things" and "Owner rules" sections apply
unchanged here). Your opacity work is in: build on it.

## Goal
Scenes need lakes, seas and a sky. Add two things to the typed graph, both deterministic, both running in the browser:
1. `water.surface@1`: a water body with a gently animated surface and transparency.
2. `scene.sky@1`: a procedural sky (gradient dome, sun disc and glow, optional clouds) that is the viewer's background
   and also feeds the ambient light, so a sunset scene is lit orange.
No pulled assets: no skybox images, no normal-map textures, everything procedural from our own code. Blender's `intern/sky`
(Nishita, Apache-2.0, already allowed by ADR 0024) may be ported as the reference for the sun and atmosphere colours, but only if a
small numeric port is enough; do not vendor a large closure. A cheaper analytic model (Preetham or Hosek style gradient)
is fine if you document it.

## Water (do this first)
- Node `water.surface@1(size=[x,z], level=0, depth=1, color, opacity=0.6, wave_height, wave_length, speed, seed, resolution)`.
  Output type: a Solid (so booleans, material and export work like any solid): a thin slab whose top is a regular grid with
  gentle waves (reuse `crates/ve-modeling/src/noise.rs` `fractal` for the shape, as `terrain.rs` does) and whose body goes
  down `depth` metres so you see a volume at the shore. Put it in `crates/ve-modeling/src/water.rs`, spec in
  `crates/ve-graph/src/builtin/water.rs`, register like `terrain` (see `builtin.rs`, `ready.rs`, `operations.rs`,
  `plan/estimate.rs`). Cap resolution (192 like terrain) and triangle count.
- Material: it should carry a water look by default (colour, low roughness, opacity from your opacity work) so
  `water.surface` alone looks like water; `solid.material` after it can still override.
- Animation: a vertex-shader ripple driven by the shared animation clock (viewer only, the glTF export stays static, say so in the
  node doc). The CPU shape stays the deterministic rest shape. Add the ripple to the existing pipeline in `crates/ve-render`
  with a per-material flag; opaque and non-water materials must render byte-identically to before.
- Interaction: water is not a physics body; do not touch physics.
- Test: lake in a terrain basin (use `terrain.heightfield` `flat_below` for a lake bed and put the water slab at that level)
  seen from the side shows terrain through the water; screenshot by eye.

## Sky
- Node `scene.sky@1(sun_direction=[x,y,z] or sun_elevation/sun_azimuth, sun_intensity, turbidity, ground_color, clouds, cloud_cover,
  seed)`. Output type `Environment`-like: coordinate with the existing `scene.environment` (ambient colours, ADR 0023-ish; read
  `crates/ve-modeling/src/environment.rs`, `ve-graph/src/builtin/environment.rs`,
  `ve-runtime/src/projection/environment.rs`): a document may have at most one sky and one environment; a sky replaces the
  ambient sky colour and drives the sun light direction and colour (find how `light.sun` and the shadow pass take direction), so
  `scene.sky` alone gives a lit, shadowed outdoor scene. If both are authored, `scene.environment` wins for ambient and `scene.sky`
  only paints the background, and say so in the docs.
- Rendering: a background pass drawn behind everything (full-screen triangle, reverse-z far plane), per eye in stereo, no
  extra textures. Keep it cheap (a Quest-class GPU): analytic gradient plus sun, cloud layer by a few octaves of value noise
  with the shared noise function ported to WGSL with the same integer hash (CPU/GPU parity for the hash is a test).
- The mock headset and the XR path both must show it, and the golden images in `web/tests/golden/` must not change for scenes
  without a sky (do not regenerate goldens; if one changes, that is a bug in your change).

## Acceptance (all of these)
1. Rust tests for both nodes: parameter ranges, determinism (same seed, same bits), triangle cap, documents with two skies
   refused with a reason.
2. Parity corpus lines for `water.surface` and `scene.sky` in `crates/ve-modeling/examples/corpus.rs`; native and
   `wasm32-wasip1` identical.
3. Browser tests `engine/web/tests/g10-water.mjs` and `g10-sky.mjs`: a lake you can see through to the bed; a noon sky is
   bluer overhead than at the horizon and a sunset scene's lit side is warmer than its shadow side (measure pixels, look at
   the screenshots). Port range 5230 to 5249 only.
4. MCP guide paragraphs and prompt sets `water-tools.json`, `sky-tools.json` (three prompts each: a lake with a jetty,
   a sea with a lighthouse, a sunset beach; a clear noon field, a stormy dusk, a night sky with a moon) and the checks in
   `tools/mcp/eval-prompts.mjs`; run Sonnet with `--parallel 1` and report. Mind that the guide is a JS template literal: do not put
   backticks inside it (a backtick broke the MCP server once; run `node --check tools/mcp/server.mjs`).
5. Demo scene: add `web/demos/scenery.txt` (terrain with a lake and a sky and sunlight, a few trees via `inst.drop`) to the model
   dropdown the way `physics_joints` was added (`web/viewer.js` SCENES and the change handler, `web/index.html` list,
   `xtask/src/web.rs` demo copy list; copy the text file into `web/dist/models/` when you rebuild). Test it in `g10-demos.mjs`
   (add a case; do not remove existing ones).
6. ADR `docs/adr/0038-water-and-sky.md` (proposed), and a line each in `docs/plan-animation-and-blender.md` under W16.

## Boundaries (to avoid colliding with Claude's work)
Do edit: new `ve-modeling/src/water.rs` and `sky.rs`, `ve-modeling/src/environment.rs`, `ve-graph/src/builtin/water.rs`,
`sky.rs`, `environment.rs`, `ve-render` (pipeline, a new sky pass, shaders), `ve-runtime/src/projection/environment.rs`
and `projection/surfaces.rs`, the MCP files, new tests, `web/demos/scenery.txt`, and the demo wiring lines named above.
Do not edit: `ve-physics`, `ve-runtime/src/physics.rs`, `ve-runtime/src/behavior*`, `ve-modeling/src/physics.rs`,
`ve-graph/src/builtin/physics.rs`, `ve-graph/src/spec.rs` (Claude is adding a port type there; if you need a port type, ask in
your report and reuse `Environment` meanwhile). `ve-modeling/src/lib.rs`, `operations.rs`, `plan/estimate.rs`,
`ve-graph/src/builtin.rs` and `ready.rs` are shared registries: make small, additive edits (a line or two) and re-read the file
just before editing, because Claude edits them too.

## Owner rules (unchanged)
Edit with your write/edit tool only (no heredoc, sed, perl, python on repo files, and none "just to check"). No attribution lines in
commits or PRs; do not commit unless asked. Everything in the browser, nothing pulled. Fix lints, do not silence them. Kill only
processes you started, by pid. Report in chat what you ran and what passed with numbers, and what you did not finish.
