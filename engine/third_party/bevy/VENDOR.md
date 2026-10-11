# Vendored Bevy

- Upstream: https://github.com/bevyengine/bevy
- Tag: v0.20.0
- Commit: b3bdd75becaffe9a16677cf5bae5bd6d198f052b
- Licence: MIT OR Apache-2.0 (see LICENSE-MIT, LICENSE-APACHE)
- Contents: the 38 crate directories in the path-dependency closure of bevy_render (+ ecs, app, asset,
  mesh, camera, transform), copied unmodified. bevy_scene and bevy_android are present only
  because cargo needs every path dependency's manifest (optional/dev deps); they are not compiled. Crates removed from upstream: core_pipeline, pbr,
  ui, winit, gltf, animation, light, text, audio, gilrs, sprite and the rest (see ADR 0001).
- Local changes: patch 2 in `bevy_render` (see ../PATCHES.md). Every change is a numbered patch described there.
