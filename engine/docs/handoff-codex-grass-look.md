# Handoff: grass look (ADR 0070 port 4, the shading of RedGPU's grass)

Reference (read first, read-only, MIT): `engine/third_party/redgpu/src/landscape/core/shader/library/common/grassVertexCommon.wgsl`, `grassFragmentCommon.wgsl`, and `landscape/grass/core/pipeline/grassVertex.wgsl`, `grassFragmentNear.wgsl`, `grassFragmentFar.wgsl`, `grassShadowVertex.wgsl`. RedGPU's grass is a blade mesh drawn thousands of times, each copy with its own position, size, yaw and fade; the shader darkens the roots, blends them into the ground colour, pushes the normals up, lets thin blades glow where light passes through (subsurface), and shrinks them to nothing with distance. We have the instancing (`instance_sets.rs`, retained sets drawn by chunk with frustum culling), a grass tuft kit (`grass_tuft`), scatter (`scatter.place`), and wind (`wind.rs`, `FoliageStyle` sway and flutter). What we lack is the grass LOOK. Write our own version; keep the upstream notice in a comment where the shader adapts a formula.

## Test (already written, do not weaken)
`crates/ve-render/tests/grass.rs` (9 tests). The formulas in it come straight from the reference. If one looks wrong, say so in Status with numbers.

## Part 1: CPU reference (`crates/ve-render/src/grass.rs`)
`height_ratio(y, min_y, inv_mesh_height)`, `ground_blend(base_rgb, ground_rgb, height_ratio, strength, exposure_boost)`, `sky_occlusion(ratio)`, `contact_ao(ratio)`, `upward_normal(normal, ratio)`, `sink_correction(local_y, ratio)` (the amount added to y), `fade_scale(distance, near, far)` (1 up to `near`, linear to 0 at `far`; finite for any input), exactly as the test and the reference shaders define them. Documented, no unwrap/expect/panic.

## Part 2: the shader (`crates/ve-render/src/shaders/grass.wgsl`, included into the forward shader like `wind.wgsl` and `water_waves.wgsl`)
Functions `grass_height_ratio`, `grass_ground_blend`, `grass_sky_occlusion`, `grass_contact_ao`, `grass_upward_normal`, `grass_sink_correction`, `grass_fade_scale` mirroring part 1. Use them in the forward shader for surfaces that opt in (extend `FoliageStyle` in `crates/ve-render/src/material.rs`; reflection and serde defaults so old documents load; carry the new values the way `alpha_cutoff`, `sway` and `flutter` travel today, avoiding new vertex attributes beyond WebGL2's limit of 16):
- `grass` (bool): turn the grass look on;
- `ground_color` (rgb) and `ground_blend` (0..1): the colour the roots blend into (the host's soil colour), strength 0 turns blending off;
- `fade_near`, `fade_far` (metres): distance fade. In the vertex shader scale the instance about its own base by `fade_scale(distance to camera)`, so far blades shrink out instead of popping (for instances with the grass look only; every other surface unchanged);
- `translucency` (0..1): light through the blade: extra diffuse on the side of the blade facing away from the sun, scaled by the height ratio (a simple wrap term; no new textures);
- `exposure_boost` default 1.
In the fragment: height ratio from the object-space y of the vertex (use the mesh's own min and height, passed as two floats or derived from the instance set's shape bounds), albedo through `grass_ground_blend`, multiply the ambient/diffuse by `grass_sky_occlusion * grass_contact_ao`, normal through `grass_upward_normal`. Shadows: receive as normal surfaces do.
Add the shader to the validation test (`shader_tests.rs`: parse, validate, GLSL ES 3.00) and keep it passing. No compute; WebGL2 keeps working. The lowest quality tier ignores `translucency` and the occlusion terms (single cheap path).

## Part 3: the language
- Find the node that builds `FoliageStyle` for a model (search `FoliageStyle` in `crates/ve-modeling`; flora nodes take `sway`; there may be a `solid.foliage` node). Add the new parameters there with ranges, defaults and guide text (a short paragraph in the `plants` guide topic of `tools/mcp/guide.mjs`), and follow the existing node tests.
- Make the shipped `grass_tuft` kit (find it: `crates/ve-modeling/kits/grass_tuft.txt` or similar) use the grass look by default (`grass=true`, a sensible `ground_blend`, `fade_near=25`, `fade_far=70`, `translucency=0.5`) with the tuft's `color` still exposed; keep its triangle count (about 42) and existing kit tests passing. Update `web/dist-bevy-gpu/worlds/showcase.txt` to use the kit's new parameters only if the pasted group there is replaced by the kit call (check how kits are made available to text; if the page resolves kits by name, drop the pasted `group grass_tuft` block from the showcase and call `grass_tuft(...)`; if not, update the pasted group instead).
- Do not rebuild wasm; Claude does after review and looks at it in the showcase.

## Rules
- AGENTS.md rules (files under 400 lines, `#[expect]` not `#[allow]`, docs on public items, tests for behaviour). No `cargo xtask`, no `cargo fmt` (`rustfmt --edition 2024 --config-path rustfmt.toml <file>`). Own `CARGO_TARGET_DIR`. No commits or staging.
- Run `cargo test -p ve-render` (all, including shader validation), the `ve-modeling` tests you touch (`--test` selectors; `cargo test -p ve-modeling` as a whole does not compile for existing reasons), `cargo check -p ve-runtime --target wasm32-unknown-unknown`.

## Status
(Codex: write what you did, which checks passed, what you could not verify.)
