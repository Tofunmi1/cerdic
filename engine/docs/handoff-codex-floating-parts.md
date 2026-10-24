# Handoff: detect parts that float off the body they belong to (ve-measure only)

Owner: Claude manages; Codex implements. Crate you may edit: `engine/crates/ve-measure` only (src + tests). Rules: never run `git checkout`, `git clean`, `git stash` or
`git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use your own `CARGO_TARGET_DIR`
(for example `/tmp/codex-target-measure`); do not commit or stage; edit with your edit tools, not scripts. Pure Rust on plain arrays, runs on native and wasm32, no new dependencies.

## Why

The owner noticed that parts built separately (a dragon's horns, a horse's mane, a scar on bark) did not stay on the body: each was placed by typed coordinates, and where the
guess was a little off the part floated in the air. We fixed the dragon by hand. We want a check any author (a person or a model) can run: "which parts of this scene touch nothing?"

## What to build

In `crates/ve-measure/src/attach.rs` (new module, exported from `lib.rs`; keep `lib.rs` module docs in step):

```rust
/// One part of a scene: a name and its mesh (world positions, triangle indices).
pub struct NamedMesh<'a> { pub name: &'a str, pub mesh: Mesh<'a> }

/// How one part relates to the others.
pub struct Attachment {
    pub name: String,
    /// Smallest distance in metres from this part's surface to the surface of any other part (0 when they touch or overlap).
    pub gap: f32,
    /// Name of the nearest other part (empty when there is only one part).
    pub nearest: String,
}

/// `floating`: the parts whose `gap` is above `tolerance` (default suggestion 0.01 m), nearest-first or largest-gap-first (your choice, document it).
pub fn attachments(parts: &[NamedMesh<'_>], tolerance: f32) -> Result<Vec<Attachment>, MeasureError>
```

- `gap` is the **true minimum** surface-to-surface distance (not the Hausdorff max or mean that `distance_between` returns): the smallest closest-point distance over the vertices of each part to the other
  part's surface, in both directions, plus 0 if any triangles intersect or one part lies inside another (a vertex of A inside the volume of a closed B counts as touching; use a ray-parity or
  the existing `Bvh::raycast` for that; an open mesh has no inside). Use `Bvh` (`bvh.rs`) for the closest-point queries. A part is **attached** if its gap to some other part is at most `tolerance`.
- Budgets: refuse (typed `MeasureError`, see `error.rs` for how other budgets refuse) with more than 64 parts or more than 2,000,000 triangles in total. Empty meshes are an error as elsewhere in the crate.
- Also add `pub fn attachments_json(...)`-style model-facing text like `bake_maps_json`/`measure_mesh` do, if a pattern exists in `tool.rs` (follow it: one function returning a JSON string with a
  `floating` list, each entry `{name, gap, nearest}` and a `suggestion` string such as "move the root of `horn` 0.12 m toward `body`, or sink it inside the body"). If the pattern does not fit,
  skip this and say so in the Status.

## Tests (fixed, written by Claude; never weaken; add more if you like) in `crates/ve-measure/tests/attach.rs`

Use small boxes built by a helper in the test file (8 vertices, 12 triangles; look at `tests/hull.rs` or `tests/thickness.rs` for an existing box helper to copy).

1. `touching_boxes_are_attached`: unit box A at the origin, box B (unit) shifted by 1.0 in x (faces touch): both gaps are 0.
2. `a_box_inside_another_is_attached`: small box fully inside a big closed box: gap 0 for both.
3. `a_floating_box_reports_its_gap`: unit box A at the origin and unit box B shifted 2.5 in x (faces 1.5 m apart): both gaps are `1.5` within 1e-4 and `nearest` names the other.
4. `three_parts_the_far_one_floats`: A and B touch, C is 0.4 m from B: with tolerance 0.01 only C is floating (gap 0.4), A and B are not.
5. `tolerance_decides`: the same C with tolerance 0.5 is not floating.
6. `too_many_parts_are_refused`: 65 tiny boxes give a `MeasureError`.
7. `a_single_part_has_no_neighbour`: one part: gap is 0, nearest empty, nothing floating (a lone part cannot float).

## Check

`cargo test -p ve-measure` all pass (none ignored); `cargo clippy -p ve-measure --all-targets -- -D warnings` clean for your files (report unrelated pre-existing errors as such).

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done / [ ] blocked. Added bounded named-mesh attachment checks with true minimum vertex-to-surface gaps, closed-mesh containment, crossing-edge detection and a JSON adapter.
- Tests run and their output: `CARGO_TARGET_DIR=/tmp/codex-target-measure cargo test -p ve-measure` — passed (62 tests total including 7 new attachment tests; no ignored tests). `CARGO_TARGET_DIR=/tmp/codex-target-measure cargo clippy -p ve-measure --all-targets -- -D warnings` — passed.
- Files changed: `engine/crates/ve-measure/src/attach.rs` (new), `engine/crates/ve-measure/src/error.rs`, `engine/crates/ve-measure/src/lib.rs`, `engine/crates/ve-measure/src/tool.rs`, `engine/crates/ve-measure/tests/attach.rs` (new). JSON adapter implemented as `attachments_json`. This handoff Status section updated as requested.
