# Handoff to Codex: spatial queries, collision layers, sensors, heightfield colliders and kinematic movers in `ve-physics`

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no `Co-Authored-By` or tool attribution, file changes through the edit tools only). **Never run `cargo fmt`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`.** Use `CARGO_TARGET_DIR=target-codex-physics`. Edit only `crates/ve-physics/` (and its `Cargo.toml` if a Rapier feature is needed; Rapier stays pinned at `=0.22.0` and nothing outside this crate may see Rapier types). Do not edit `crates/ve-physics/tests/queries.rs`: it is the contract.

## Why
A game world needs more from physics than bodies and contacts: aiming and sight (rays), picking up things near a hand (overlaps), sweeping a character through a doorway (shape casts), things that must not collide with each other (layers), trigger zones (sensors), terrain that is not a huge triangle mesh (heightfield), platforms and doors that move (kinematic movers), and more than 256 bodies. The crate has none of these (no tests at all today). Rapier's `QueryPipeline` is already inside `World` for the walker.

## The interface (exactly what `tests/queries.rs` calls)
```rust
pub struct Layers { pub member: u32, pub mask: u32 }   // Default: member 1, mask u32::MAX; Clone, Copy, Debug, PartialEq, Eq
// BodyDesc gains: pub layers: Layers, pub sensor: bool   (defaults: Layers::default(), false)
pub struct RayHit { pub body: BodyId, pub point: [f32; 3], pub normal: [f32; 3], pub distance: f32 }   // Clone, Copy, Debug, PartialEq
pub struct ShapeHit { pub body: BodyId, pub point: [f32; 3], pub normal: [f32; 3], pub distance: f32 }  // same derives
impl World {
    pub fn raycast(&mut self, origin: [f32; 3], direction: [f32; 3], max_distance: f32, mask: u32) -> Option<RayHit>;
    pub fn overlap_sphere(&mut self, centre: [f32; 3], radius: f32, mask: u32) -> Vec<BodyId>;           // ascending id order
    pub fn shape_cast(&mut self, shape: &Shape, from: [f32; 3], direction: [f32; 3], max_distance: f32, mask: u32) -> Option<ShapeHit>;
    pub fn touching(&mut self, sensor: BodyId) -> Result<Vec<BodyId>, Error>;                               // ascending ids; empty for a non-sensor
    pub fn move_kinematic(&mut self, id: BodyId, position: [f32; 3], rotation: [f32; 4]) -> Result<(), Error>;
}
// Shape gains: Heightfield { rows: usize, cols: usize, heights: Vec<f32>, size: [f32; 2] }
// MAX_BODIES rises to at least 4096.
```
Export the new types from `lib.rs`. Details:
- **Layers.** Two bodies collide only if `(a.member & b.mask) != 0 && (b.member & a.mask) != 0` (Rapier `InteractionGroups`). A query with `mask` sees a body only if `(body.member & mask) != 0`. A mask of `u32::MAX` sees every body.
- **Queries work straight after `add`**, with no step needed in between (refresh the query pipeline lazily: keep a dirty flag set by add, set_pose, move_kinematic and step). They take `&mut self` for that reason. Direction is normalised inside; a zero or non-finite direction, origin or length returns `None` or an empty list, never an error. `distance` is in metres along the ray. A ray starting inside a solid body reports distance 0 for that body (Rapier's `solid = true`).
- **Sensors.** `sensor: true` makes the collider a Rapier sensor: it blocks nothing, appears in no `contacts()` pair, and `touching(sensor)` lists the bodies whose colliders currently intersect it (from the narrow phase's intersection pairs, after a step). Sensors may be static, kinematic or dynamic. `touching` of a non-sensor returns an empty vector; of an unknown id `Error::NoSuchBody`.
- **Heightfield.** Static bodies only (like `Mesh`). `heights` has `rows * cols` samples, row by row, rows along +z and columns along +x, the field centred on the body and spanning `size[0]` metres in x and `size[1]` metres in z (so the sample at column `c` sits at `x = -size[0]/2 + size[0] * c / (cols - 1)`). Refuse with `Error::BadShape` when `rows` or `cols` is below 2, `heights.len()` differs from `rows * cols`, any number is not finite, `size` is not positive, or the body is not static. The shape cap `MAX_SHAPE_POINTS` applies to `rows * cols`. Mind Rapier's own convention for `HeightField` (its matrix is indexed `(row, col)` with scale along x and z); verify the orientation with the test, do not assume.
- **Kinematic movers.** `move_kinematic` sets the body's next pose as a position target (Rapier `set_next_kinematic_position`), so the solver derives its velocity and it carries or pushes dynamic bodies. Only kinematic bodies: any other kind returns `Error::BadValue("only kinematic bodies can be moved")`. `set_pose` on a kinematic body should keep working as before.
- **Capacity.** Raise `MAX_BODIES` to 4096 or more; check the per-step cost with 2,000 static boxes and 200 dynamic ones stays small (print the release time of 60 steps in your Status).

## Tests
`cargo test -p ve-physics` must pass, and `cargo clippy -p ve-physics --all-targets -- -D warnings`. The test file may need its `#![expect(...)]` list adjusted by the owner-side session if a listed lint is not triggered or another is: tell me in the Status instead of editing it. Also add unit tests of your own in `crates/ve-physics/tests/` in a new file for anything the contract does not cover (for example a rotated heightfield body is not needed; a mesh body ray hit, a capsule shape cast, a layered overlap are good). Keep determinism: no hash-map iteration order, results ordered by body id. `cargo check -p ve-physics --target wasm32-unknown-unknown` must pass.

## Out of scope
Joints, the walker and the runtime wiring (`ve-runtime`) are untouched except to compile against the new `BodyDesc` fields (give them the defaults `layers: Layers::default(), sensor: false` where `BodyDesc` is built by struct literal in other crates: search `BodyDesc {` across `crates/`). No graph nodes: the owner-side session adds those after.

## Status (Codex: fill in when you finish, then stop)
Implemented spatial queries, collision layers, sensors, static heightfields, kinematic movers and the 4,096-body capacity. Added mesh-ray, capsule-cast and release step-cost coverage. Updated the runtime's `BodyDesc` literal with default layers and `sensor: false`.

Commands and results (all Cargo commands used `CARGO_TARGET_DIR=target-codex-physics`):

- `cargo test -p ve-physics`: 30 tests passed; one contract test failed. `a_world_holds_thousands_of_bodies_and_refuses_more_than_its_cap` fills 2,000 bodies, then adds 2,000 more (the range `MAX_BODIES - 2000..MAX_BODIES` has 2,000 values), and expects the next add to fail. With the required `MAX_BODIES = 4096`, that next add is body 4,001 and correctly succeeds. I left the test unchanged and kept actual capacity consistent with the public constant.
- `cargo test -p ve-physics --test queries`: 12 passed, one failed on the same capacity contradiction.
- `cargo clippy -p ve-physics --all-targets -- -D warnings`: blocked by `clippy::assertions_on_constants` in the unchanged `tests/queries.rs` assertion `MAX_BODIES >= 4096`. The owner-side test lint list needs to account for this assertion as anticipated above.
- `cargo check -p ve-physics --target wasm32-unknown-unknown`: passed.
- `cargo check -p ve-runtime`: passed.
- `cargo test -p ve-physics --test additional_queries --release thousands_of_bodies_step_in_a_measured_release_window -- --nocapture`: passed; 2,000 static boxes and 200 dynamic spheres completed 60 release steps in 10.897333 ms on this machine.
- `cargo xtask rules`: failed on the unrelated `crates/ve-graph/src/builtin/fields.rs` file at 409 lines (400 maximum).
- `git diff --check` on the task files passed; all edited Rust files are under 400 lines.
- `rustfmt --edition 2024 --config-path rustfmt.toml` was run only on edited Rust files. `cargo fmt` and `cargo xtask quick` were not run as instructed. No commits were made. Strict gate pending.
