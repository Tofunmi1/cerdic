# Handoff: draw statistics for the Bevy renderer path (`status().render`)

Owner: Claude manages; Codex implements. **Files you may edit: a new `engine/crates/ve-bevy/src/stats.rs`, one `mod`/plugin registration line in `ve-bevy/src/lib.rs`, `engine/crates/ve-runtime/src/status.rs`, and the Cargo.toml of those two crates if a dependency is needed.**
Two other helpers work at the same time in `ve-bevy/src/{toon,terrain,water}.rs` and `ve-bevy/src/shaders` (materials) and in `ve-runtime/src/measure_exports.rs`, `ve-runtime/src/lib.rs` exports and `engine/web/viewer.js` (a viewer panel): do not touch those; if the build breaks because
of THEIR half-written files, wait a minute and retry. Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; own `CARGO_TARGET_DIR`
(for example `/tmp/codex-target-stats`); do not commit or stage; edit with your edit tools, not scripts. The sandbox cannot run the browser; Claude does that.

## Why

`status().render` (in `ve-runtime/src/status.rs`, `render_counts()`) reads `ve_render::last_frame_counts()` and `last_set_stats()`, which only the engine's old renderer fills. With Bevy's renderer drawing the scene they are all 0 (`"main":{"draws":0,"triangles":0}`),
so we cannot measure what a scene really draws, e.g. whether camera-distance bands (`VisibilityRange`) cut the triangles the way we estimate. We need the Bevy path to report, every frame, how many entities and triangles are **actually visible to the main camera**.

## What to build

1. `ve-bevy/src/stats.rs`: a plugin `VeBevyStatsPlugin` (registered where the other ve-bevy plugins are, one line in `lib.rs`) with a `Resource` `BevyDrawStats { entities: u32, triangles: u64, sets_entities: u32, frame: u64 }` updated in a system that runs after Bevy's visibility
   systems (`VisibilitySystems::CheckVisibility` / the `ViewVisibility` update in `PostUpdate`): count every entity with `Mesh3d` whose `ViewVisibility::get()` is true for the main camera, and add the triangle count of its mesh (`Assets<Mesh>`, `mesh.indices()` length / 3, or vertex count / 3 for non-indexed meshes). `sets_entities` counts those whose parent has `VeInstanceSet` (the copies made by `sets.rs`).
   Cache the per-mesh triangle count by `AssetId<Mesh>` in a `HashMap` that is cleared when assets change, so a forest of 10,000 copies does not recount indices every frame. Cheap enough to run every frame; if it is not, update every 10th frame.
   `ViewVisibility` includes the shadow and other cameras? Check, and if it cannot distinguish cameras, count only entities visible in the first camera's `VisibleEntities` instead (`bevy_camera::visibility::VisibleEntities` component, `get::<Mesh3d>()`); explain your choice in the Status.
2. A plain accessor the runtime can read without depending on Bevy ECS types in `ve-runtime`'s status code: expose `pub fn latest() -> BevyDrawStats` backed by a `static` atomic or `Mutex` that the system writes (the status function is called from JS, outside the ECS).
3. `ve-runtime/src/status.rs`: when the Bevy path is active, `render` gets an extra object `"bevy": { "entities": .., "triangles": .., "set_entities": .., "frame": .. }` (keep the old `main`/`shadow`/`depth`/`sets` objects untouched so nothing that reads them breaks). Find how `status.rs` knows which renderer is active (or always include `bevy`
   when `ve-bevy` is linked).

## Tests (written by Claude; never weaken; add more if you like), in `ve-bevy/src/stats.rs` `#[cfg(test)] mod tests`, no GPU needed

Build a tiny `App` with `MinimalPlugins`-style setup as the sibling modules' tests do (look at `sets.rs` tests), insert `Assets<Mesh>`, spawn entities by hand with `Mesh3d`, `ViewVisibility` set to visible/hidden (use the public `set_visible` or the way Bevy's own tests set it), and run only your counting system:
1. `counts_visible_meshes_only`: three entities with a 12-triangle cuboid mesh, two visible and one hidden -> `entities == 2`, `triangles == 24`.
2. `set_copies_are_counted_separately`: one visible entity whose `ChildOf` parent has a `VeInstanceSet` -> `sets_entities == 1` and it is also in `entities`.
3. `a_mesh_is_counted_once_per_copy`: two visible entities sharing one 12-triangle mesh handle -> `triangles == 24`.
4. `latest_returns_what_the_system_wrote`.

## Check

`cargo test -p ve-bevy` and `cargo test -p ve-runtime` pass (none ignored); `cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` builds.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [ ] done / [x] blocked: the wasm build could not get the Cargo build-directory lock; it remained at `Blocking waiting for file lock on build directory` for over a minute on two attempts.
- How the main camera is distinguished: `ViewVisibility::get()` includes visibility from any view, so the stats system uses the active `Camera3d` with the lowest `Camera::order` and its `VisibleEntities` `Mesh3d` list. Shadow views do not get counted independently.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target- cargo test -p ve-bevy` passed (24 passed, 0 failed, 0 ignored; doc tests 0). `CARGO_TARGET_DIR=/tmp/codex-target- cargo test -p ve-runtime` passed (0 tests; doc tests 0). `CARGO_TARGET_DIR=/tmp/codex-target- cargo build -p ve-runtime --target wasm32-unknown-unknown --profile web-dev --features bevy-atmosphere` did not start compiling because Cargo waited on the build-directory lock; interrupted and retried once with the same result.
- Files changed: `engine/crates/ve-bevy/src/stats.rs` (new), `engine/crates/ve-bevy/src/lib.rs`, `engine/crates/ve-runtime/src/status.rs`, and this handoff Status section.
