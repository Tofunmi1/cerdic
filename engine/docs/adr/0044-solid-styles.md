# ADR 0044: Named solid styles
- Status: proposed
- Date: 2026-10-18

## Context

Setting cel shading, outlines, and the finish applied to baked texture maps independently makes it difficult to apply a consistent look to generated assets. A named style should also be reversible without losing the material's authored colour or roughness.

## Decision

`solid.style@1` is implemented in `crates/ve-modeling/src/style.rs`. Its six names are `anime`, `realistic`, `painterly`, `flat`, `retro`, and `ink`. It applies toon settings and/or a `Material.texture_style`; `appearance.rs::restyle` updates the surface, regions, and placement-set prototype while retaining existing material properties and texture programs. `realistic` removes earlier toon and texture-finish settings. `outline_scale` scales the selected outline.

The texture finish runs on the baked colour map through `Baker.finish` in `src/engine/texture/bake.ts`: painterly uses a radius-four Kuwahara filter, retro samples blocks at 1/128 of map width and quantizes each RGB channel to four levels, and ink maps luminance to three greys. These finishing passes affect albedo; the material's other baked maps are not finished this way.

## Consequences

- A style changes rendering and texture finishing without changing geometry, authored colour, roughness, or the texture program.
- `painterly` has feathered toon bands and no outline; `ink` has a thick outline; `realistic` clears a prior style.
- The confirmed retro artefact is block-grid alignment: `RETRO` anchors sampling blocks to map texel coordinates, not to individual UV charts. Consequently separate charts in one map can show independently positioned pixel blocks or visible alignment discontinuities at chart boundaries. This is inherent in the current shader; no separate mitigation is present in `bake.ts`.

## Alternatives considered

- Keeping shading and map finish as separate authored controls would not provide the one-node bundled look.
- A style-specific geometry/material representation is unnecessary because `restyle` composes ordinary material fields.

## Evidence

`crates/ve-modeling/tests/g11_style.rs` checks style parameters, retained colour and roughness, realistic reset, outline scaling, and invalid names. The finishing shaders are in `src/engine/texture/bake.ts`. Tests were inspected, not run for this documentation task.
