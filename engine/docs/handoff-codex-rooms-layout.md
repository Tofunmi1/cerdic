# Handoff: `ve_world::rooms::layout`, a floor plan from a room list and the links between rooms

Owner: Claude manages; Codex implements. **Crate you may edit: `ve-world` only** (a new `src/rooms.rs` and one `pub mod rooms;` line in `src/lib.rs`). Do not touch other crates (the graph node comes in a later job, after this one passes).
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-rooms`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone (`workspace = true`; fix clippy findings in your own file, do not allow them away). `ve-world` is pure arithmetic: std only plus `thiserror`, no randomness crate: use a small seeded generator (splitmix64 like `random.rs` in `ve-flora` or the one in `ve-world`, look for it; write your own 10 lines if none is public), no `f64::powf`-style platform-sensitive maths in anything that decides the layout (integer cell maths decides it).

## Why

A VR sci-fi set (a station, a ship, a bunker, a lab) is rooms joined by corridors with doors. Today a model has to place every wall by hand. A layout generator takes "an airlock, a hub, a bridge, an engine room, quarters, a lab, linked like this" and returns a valid floor plan: sizes within each room's range, corridors wide enough to walk, doors wide enough for a body, every room reachable, nothing overlapping, the same seed the same plan. The graph node that builds walls from it (`scifi.station` or similar) is the next job.

Research notes (2026-10-24, secondary sources): good practice is circulation first (a corridor skeleton), rooms placed around it, doors only on shared walls, then validate connectivity and regenerate with the next seed on failure; very narrow leftover spaces are the common bug, so keep every corridor at least `corridor_width` and every room at least its `min`. Use any algorithm you like (BSP, growth along a spanning tree of the link graph, corridor-first): the tests fix the rules, not the method.

## API (the tests in `crates/ve-world/tests/rooms_layout.rs` use exactly this)

In `ve_world::rooms`, all `pub`, with docs:
- `RoomReq { name: String, min: [f64; 2], max: [f64; 2] }` (width and depth in metres; derive `Clone, Debug, PartialEq`).
- `RoomSpec { rooms: Vec<RoomReq>, links: Vec<(usize, usize)>, seed: u64, cell: f64, corridor_width: f64, door_width: f64 }` (derive `Clone, Debug, PartialEq`) and `RoomSpec::new(rooms, links, seed)` with defaults `cell` 0.5, `corridor_width` 1.6, `door_width` 1.0.
- `Rect { x: i32, z: i32, w: i32, d: i32 }` (cell units; `Clone, Copy, Debug, PartialEq, Eq`); `Kind::Room(usize)` | `Kind::Corridor` (`Clone, Copy, Debug, PartialEq, Eq`); `Region { kind: Kind, rect: Rect }`; `Axis::X` | `Axis::Z` (`Clone, Copy, Debug, PartialEq, Eq`).
- `Door { a: usize, b: usize, axis: Axis, line: i32, from: i32, to: i32 }`: `a` and `b` index `Layout::regions`; the door is a segment on the wall the two regions share: with `Axis::X` it runs along x at the grid line z = `line` from x = `from` to `to`; with `Axis::Z` along z at x = `line`.
- `Layout { regions: Vec<Region>, doors: Vec<Door>, cell: f64 }` (derive `Clone, Debug, PartialEq`).
- `RoomError` (a `thiserror` enum with `Display`) and `pub fn layout(spec: &RoomSpec) -> Result<Layout, RoomError>`.

## Rules the layout must satisfy (all tested)

1. Exactly one `Kind::Room(i)` region per room `i`, its width and depth in whole cells, `min <= size <= max` after rounding up to the cell (the test allows up to one cell over `max`).
2. No two regions overlap. Every corridor is at least `corridor_width` wide (narrow side). No empty region.
3. Every door lies on the wall two regions share, within the stretch both cover, and is at least `door_width` long.
4. All regions are connected through doors, and every link `(a, b)` is a short walk: a direct door, or doors through corridors only (never through a third room).
5. The plan's minimum x and z are 0 (normalised), and it stays compact: the bounding box area is at most 8 times the sum of the rooms' maximum areas.
6. Deterministic: the same spec gives an equal `Layout`; across eight seeds the plans are not all equal.
7. Errors (not panics): a link to a missing room, a room linked to itself, `min` larger than `max` on either axis, a non-positive or non-finite `cell`, `corridor_width` or `door_width`, an empty room list, more than 64 rooms, a room that no link reaches when there is more than one room (cannot be connected), a link graph that is not connected. One room and no links is a valid plan with no doors. Also bound the work: refuse or fail cleanly rather than loop forever; if a seed cannot be satisfied, retry with derived seeds a bounded number of times, then return an error.
8. Works for a chain of 24 rooms, a ring of 6 and a tree (tests).

## Tests (written by Claude; never weaken; add more in your own file or `src/rooms.rs` unit tests)

`crates/ve-world/tests/rooms_layout.rs`: 9 tests.

## Check

`CARGO_TARGET_DIR=/tmp/codex-target-rooms cargo test -q -p ve-world --test rooms_layout`; `cargo clippy -q -p ve-world --all-targets -- -D warnings`; `cargo check -q -p ve-world --target wasm32-unknown-unknown`. The other test files of `ve-world` must still pass (`cargo test -q -p ve-world`).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done (2026-11-07, implemented by Claude)
- Algorithm: a spine corridor with rooms on both sides, each with a door onto it, so every link is door, corridor, door; sizes, door positions and room order (a seeded walk over the link graph) come from the seed.
- Tests run: `cargo test --release -p ve-world --test rooms_layout` 8 passed, plus 3 unit tests in `src/rooms.rs`; whole `ve-world` crate 375 passed.
- Files changed: `ve-world/src/rooms.rs` (new), `ve-world/src/lib.rs`.
