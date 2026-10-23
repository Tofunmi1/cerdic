# 0056: Clothes and hair from the CC0 MakeHuman library
- Status: accepted by the owner on 2026-10-23 (amends 0055, which said "hair, clothing and skin appearance stay procedural", and 0003)
- Date: 2026-10-23

## Context
Procedural clothes (the body's own surface lifted a few millimetres, `body.garment`) and procedural hair (ribbon wisps, `body.hair`) were judged by the owner to look like trash: skin-tight shells with ragged
edges, no folds or thickness, flat wide ribbons for hair. The owner asked whether real ones could be taken from Blender or elsewhere. Blender ships no clothes or hair models (its hair is a node-group system,
its cloth a simulation tied to its dependency graph); the ready-made assets that exist are the MakeHuman system assets, CC0, from the same project as our body.

## Decision
- We import a second dataset: selected clothes and hair from MakeHuman's system assets, CC0 (the licence text sits in every file's header; the converter refuses any file whose header does not say CC0 or names another
  licence). Today: `female_casualsuit01` (T-shirt and jeans), `shoes02`, hair `long01`, `bob02`, `short02`.
- MakeHuman's base mesh, in decimetres times 0.1, is our body template (Anny is built on it: measured mean distance 0.005 mm), so the assets fit without a mapping. `tools/human-pack/outfits.py` binds every
  asset vertex to the nearest template triangle (three vertex ids, barycentric weights, offset in the triangle's smooth-normal frame) and stores that in `human.pack`. The pictures (diffuse, normal) are shrunk to
  1024 by `outfit_textures.py` and shipped next to the page in `outfits/`.
- The node `body.outfit@1(body, asset, offset, standoff, bones)` evaluates `sum(weights * body vertices) + offset` on the CURRENT body, so the garment follows every phenotype slider, never goes closer to the skin
  than `standoff`, and is skinned with the body's own weights when `bones` is given.
- A texture program written `image(albedo="name", normal="name")` takes its maps from those pictures (`src/engine/texture/outfit-images.ts`); hair is cut out with `solid.foliage` on the picture's alpha.
- Nothing else is imported. Skin looks, eyes and everything else stay procedural. `body.garment` and `body.hair` stay as the procedural path.

## Consequences
The pack grows by about 0.4 MB and the pictures add about 5.6 MB for five assets. A person dressed this way draws about 12,000 triangles instead of 100,000, and evaluates in a fraction of the time.
Known limits: the garments are fitted by binding, not simulation (no folds from motion); a garment made for MakeHuman's neutral body can sink into a much heavier one, which `standoff` hides;
the hair's crown and fringe sit in a low-density part of the artist's atlas and show blocky at close range; bindings are made at the neutral shape, so very loose garments follow the body less faithfully than
close ones.

## Amendment 2026-10-23: photographic materials
The owner extended the exception to CC0 photographic PBR materials from ambientCG (https://ambientcg.com, CC0 1.0): ten materials today (concrete, brick and stone, plaster, tiles, corrugated steel, asphalt, paving), shipped as 1K JPEG maps in `outfits/` by `tools/human-pack/cc0_materials.py`, which only renames the downloaded files. They are used as `image(name=..., repeat_x=, repeat_y=)` nodes of texture programs (`src/engine/texture/image-node.ts`), combined with procedural windows, grime and wear (`blend`, `expr`): a photograph carries the realism, our nodes add the variation.
