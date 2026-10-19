# Vendored Material Maker (node library and shader-model reference)
- Upstream: https://github.com/RodZill4/material-maker
- Commit: 14e3caa1368926b6be4108bd09e8cdc864b3a887 (fetched 2026-10-17)
- Licence: MIT (`LICENSE.md`, Copyright (c) 2018-present Rodolphe Suescun and contributors). MIT is compatible with the engine's GPL policy (ADR 0024).
- What is here (fetched with git sparse checkout, nested `.git` removed, nothing modified; 499 files, 4.4 MB):
  - `addons/material_maker/nodes/*.mmg`: 398 node definitions. Each is JSON holding the node's parameters (with ranges and defaults), its inputs and
    outputs, and the GLSL that implements it (`shader_model`: `code`, `global`, `inputs`, `outputs`, `instance`). They cover noises (fbm, Voronoi,
    wavelet, clouds), patterns (bricks, weave, tiles, truchet, Cairo, beehive), shapes, gradients, warps, filters (blur, dilate, Kuwahara, emboss),
    colour and tone operations, blend modes, normal-map and curvature generators, 2D and 3D signed-distance shapes, and 3D (object-space) textures.
  - `addons/material_maker/shader_functions.tres`, `nodes/glsl_defs.tmpl`: the shared GLSL helper functions the nodes call.
  - `addons/material_maker/engine/nodes/*.gd`, `engine/io_types.gd`, `engine/preprocessor.gd`, `parser/`, `types/`: the GDScript that turns a graph of
    nodes into one shader. Kept only as the reference for the shader-model format; the engine does not run GDScript. The compiler is our own.
  - `material_maker/library/base.json`, `aliases.json`: the node library's menu structure and the names nodes go by.
- Not pulled, on purpose: Material Maker's example materials, brushes, environments, images, fonts and the Godot application. ADR 0003 still forbids imported
  assets; this is source code (algorithms as GLSL text) only.
- Used by: the texture generator (`src/engine/texture/`): a compiler from our typed texture graph to GLSL ES 3.0, baked in the browser with WebGL2.
