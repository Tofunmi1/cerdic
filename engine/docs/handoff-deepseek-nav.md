# Handoff to DeepSeek: `ve-nav`, navigation and steering

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or
`cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Create a new crate
`crates/ve-nav` (pure Rust, depends on nothing but `glam` if needed; no Bevy, no I/O, compiles to wasm32). Do not edit other crates. Other engineers are
editing `ve-graph`, `ve-modeling`, `ve-runtime`, `ve-render`, `web/`.

## Why (research, short)
Recast/Detour is the standard: rasterise triangles into a voxel heightfield, filter out cells an agent cannot stand in (slope, step height, headroom, agent
radius), connect walkable cells, find a corridor with A*, then pull the string with the funnel algorithm; DetourCrowd adds local steering with reciprocal
velocity obstacles, which gets unstable in dense crowds. We need a small deterministic version so any generated level can have walking AI.

## What to build
1. **Build:** `NavMesh::build(triangles: &[[f32;3]], indices: &[u32], cfg: &NavConfig) -> Result<NavMesh, NavError>`. `NavConfig`: cell size, agent radius,
   agent height, max step, max slope (degrees), bounds. Steps: voxelise into a heightfield (span columns), mark walkable by slope, headroom and step,
   erode by agent radius, then build a connected graph of walkable cells (4 or 8 neighbours with step limit) and merge into convex-ish regions or keep the
   grid graph (your call; document it). Budget: max 2 million cells, typed error when exceeded; deterministic (no hash-map iteration order, no
   floating-point sums whose order varies).
2. **Query:** `find_path(start, end) -> Result<Path, NavError>` (A* with deterministic tie-breaking, then funnel/string-pull smoothing so paths cut corners
   but never leave walkable cells), `nearest_walkable(point)`, `raycast(a, b) -> Option<hit>` for line-of-sight over the grid, `is_walkable(point)`.
3. **Steering:** `Crowd` with up to 256 agents (position, velocity, radius, max speed, goal): per step, follow the path, apply separation and a simple
   reciprocal-avoidance rule (ORCA-lite), clamp to walkable cells, deterministic ordering by agent index. `crowd.step(dt)`; agents report `Arrived` or
   `Blocked`.
4. **Behaviours for games:** small pure functions on top: `wander(seed)`, `patrol(points)`, `chase(target)`, `flee(threat)` that return goals for the crowd;
   `line_of_sight`.
5. **Tests (real behaviours):** a flat floor is fully walkable; a wall with a gap routes through the gap and the path length matches the analytic one within
   a cell; a ramp steeper than max slope is excluded; a step higher than max step is excluded and a lower one is crossed; an agent radius shrinks the gap so
   a gap narrower than 2r is blocked and the error says so; funnel path never longer than the raw A* path; crowd of 50 agents crossing a corridor in opposite
   directions arrive without overlapping by more than 10% of the radius; determinism (same input, same result hash, run twice); budget errors; an empty mesh
   gives a typed error, never a panic.
6. Files small: `build/`, `astar.rs`, `funnel.rs`, `crowd.rs`, `behaviour.rs`, `error.rs`.

## Acceptance
`cargo test -p ve-nav`, `cargo clippy -p ve-nav -- -D warnings` clean, `cargo check -p ve-nav --target wasm32-unknown-unknown`, `cargo xtask rules` shows none
of your files over 400 lines. Add `examples/maze.rs` that builds a small maze from triangles, finds a path and prints it as ASCII art.

## Status (DeepSeek: fill in when you finish, then stop)
- State: complete; uncommitted, in `crates/ve-nav` only.
- What was built:
  - `NavMesh::build` rasterises triangles into a single-level heightfield: walkable-slope triangles become
    floor candidates per cell, steep triangles are step risers (height `<= max_step`, ignored) or walls
    (taller, block the cell); the lowest candidate with `agent_height` of clearance wins, then walkable
    cells are eroded by `agent_radius`. The grid is kept as an 8-neighbour graph (no region merging).
  - `find_path` runs deterministic A* (integer costs, lower-index tie-break, no corner cutting), then the
    string-pulling funnel over cell portals, then a greedy line-of-sight simplification. Also
    `nearest_walkable`, `raycast`, `line_of_sight`, `is_walkable`, `height_at`, `dimensions`.
  - `Crowd` (max 256) follows paths with reciprocal separation and ORCA-lite avoidance, clamps every move
    to walkable cells, relaxes overlaps, and reports `Moving`/`Arrived`/`Blocked`. Once a funnel path's
    waypoints are exhausted it steers to the exact goal, so a crowd shove cannot strand an agent short
    of the arrival radius.
  - `Wander` (seeded), `Patrol`, `Chase`, `Flee` goal pickers; `examples/maze.rs` prints the grid and path.
  - Typed `NavError` for every failure, `MAX_CELLS = 2_000_000`, `MAX_AGENTS = 256`.
- Commands run and results:
  - `cargo test -p ve-nav`: 17 passed (5 unit, 12 behaviour), 0 failed. Includes flat floor, wall gap,
    slopes, steps, radius-closed gap, raycast, behaviours, determinism hash, 2M-cell budget, empty mesh,
    and 50 agents crossing a 20 x 6 m corridor both ways (max overlap <= 10% of radius, all 50 arrive).
  - `cargo clippy -p ve-nav --all-targets -- -D warnings`: clean.
  - `cargo check -p ve-nav --target wasm32-unknown-unknown`: clean.
  - `cargo xtask rules`: no `ve-nav` file over 400 lines (largest is `mesh.rs`, 297). The command still
    reports failures in pre-existing files outside this task: `crates/ve-measure/src/collision.rs`,
    `crates/ve-graph/src/builtin/poly_network.rs`, `crates/ve-runtime/src/lib.rs`.
  - `cargo run -p ve-nav --example maze`: prints the ASCII grid, `9 corners, 13.37 m`.
- Open questions or things you chose not to do:
  - Multi-level geometry (bridges, stacked floors) is out of scope: the heightfield keeps one floor per
    cell and documents that spans/region merging are the next step.
  - Grid cells rather than convex regions: simpler and deterministic, at the cost of more waypoints before
    the funnel and no region-based crowd density control.
  - Arrival uses `goal` distance (`radius + 0.75 * cell`); agents are not given per-agent priority or
    path-replanning under congestion. `Blocked` is cleared automatically when an agent moves again.
  - Full strict workspace gate (`cargo xtask check --strict`) not run; it is the owner's milestone gate.

