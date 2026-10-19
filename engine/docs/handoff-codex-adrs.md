# Handoff to Codex: decision records for the work done since ADR 0040

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution anywhere). The repo's rule is "add an ADR
before changing a decision"; several decisions were made in code without one. Write them down **accurately**.

## Scope: you may create and edit only
- new files `engine/docs/adr/0041-*.md` to `0046-*.md` (use `engine/docs/adr/0000-template.md` for the layout and look at 0038 and 0040 for tone)
- the Status section at the bottom of this file

Do NOT edit code, tests, other ADRs, or other docs. Read-only on everything else.

## Rule for accuracy
Each ADR must be written from the code and tests, not from this brief. Read the files named below, quote file paths and function names,
and state a limitation only if you saw it. If you cannot verify a claim, write "not verified" instead of guessing. Use `git diff` and
`git log` to see what changed (most of it is uncommitted). Keep each ADR short (about one page): context, decision, consequences,
alternatives considered, how it is tested.

## The six records
1. **0041 Cel shading and outlines.** `solid.toon@1`; inverted-hull outline pass (`PassKind::Outline` in `ve-render/src/pipeline.rs`, `pass.rs`,
   `shaders/forward.wgsl` `vertex_outline`); the smooth-normal packing into the occlusion float (`ve-runtime/src/projection/upload.rs`
   `pack_smooth_normals`, `unpack_occlusion` in the shader) that closes hull gaps at hard edges; instance attribute budget (15 of 16 used).
2. **0042 Texture layers, tiling and sharing.** `ve-render/src/textures.rs` (`LAYER_SIZE`, `MAX_LAYERS`, `allocated_layers` and the GL
   cube-array layer-count quirk), `solid.texture` `tile`, world-space UVs (`src/engine/texture/uv.ts`), layer sharing by content (Rust) and by
   reference (`src/engine/texture/budget.ts`, `mesh-upload.ts`).
3. **0043 Surface signals for textures.** `src/engine/texture/{signals,surface-nodes,bake}.ts`, the six `surface_*` nodes, per-texel curvature
   from normals (the `SURFACE` shader in `bake.ts`), the calibration (5 cm radius reads as 1), why such programs are never shared or tiled,
   and the browser test `web/tests/g10-surface-signals.mjs`.
4. **0044 Styles.** `solid.style@1` (`ve-modeling/src/style.rs`, `appearance.rs` `restyle`), `Material.texture_style`, the GPU finishing passes
   (`Baker.finish` in `bake.ts`: painterly Kuwahara, retro, ink), tests `crates/ve-modeling/tests/g11_style.rs`. State the known retro artefact
   if you can confirm it from the code.
5. **0045 Creature maker.** `ve-modeling/src/creature*` (spec, validation, templates, rig and clip generation), `field.tube` `end_radius` and the
   planner's area-based triangle estimate (`plan/fields.rs`), the `make_character` MCP tool (`tools/mcp/server.mjs`, `src/engine/mcp-bridge.ts`),
   the document-panel button (`web/docpanel.js`), tests `crates/ve-modeling/tests/g11_creature.rs` and `g11_tapered_tube.rs`. Limits: 32 rig bones,
   parent depth 8, node-name length 32.
6. **0046 Skinning of generated bodies is distance-based (open decision).** Record that weights are the existing distance falloff
   (`rig.skin`), what alternatives were researched (heat-diffusion weights from Blender's `meshlaplacian.cc`, geodesic voxel binding,
   bounded biharmonic weights) and that no choice has been made. Status: proposed.

## Acceptance
- Six files, correct numbering and the template's headings, no claim you could not tie to a file.
- A final list in the Status section of every claim you marked "not verified".
- Do not run builds or tests; this is documentation only.

## Status (Codex: fill in when you finish, then stop)
- State: complete
- Files written: `docs/adr/0041-cel-shading-and-outlines.md`, `docs/adr/0042-texture-layers-tiling-and-sharing.md`, `docs/adr/0043-surface-signals-for-textures.md`, `docs/adr/0044-solid-styles.md`, `docs/adr/0045-creature-maker.md`, `docs/adr/0046-distance-based-skinning-weights.md`.
- Claims marked "not verified": repository evidence for research/evaluation of geodesic voxel binding and bounded biharmonic weights is not present in the inspected files; they are recorded as alternatives from the handoff only.
- Checks: no builds or tests run, as requested for this documentation task.
