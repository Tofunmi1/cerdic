# Handoff: hands in the game (grab, carry, use)

Owner: Claude. Helper: Codex. Crate you may edit: `crates/ve-game` only. Pure Rust, no engine dependency.

## Why
2026's best VR games (Fixer Undercover, Iron Rebellion, Into the Radius 2) are praised for hands-on interaction: you pick up tools and
parts, carry them, put them in sockets, pull levers. Our game rules only know a hero that follows the viewer and `touch`. This adds
hands. Sockets, levers and doors then need no new machinery: they are ordinary rules over `touch`, `grab`, `release` and `use`.

## The tests (fixed, do not weaken, do not edit)
`crates/ve-game/tests/hands.rs` (11 tests). Run: `CARGO_TARGET_DIR=target-codex-hands cargo test -p ve-game`. All existing ve-game tests must keep passing.

## What to build
1. **Input type** (public, exported from the crate root next to `Inputs`): `HandSide { Left, Right }` and
   `HandInput { side: HandSide, position: [f64; 3], grip: bool, trigger: bool }` (both `Clone, Copy/Debug, PartialEq`).
   `Inputs` gets `pub hands: Vec<HandInput>` (default empty; `Default` already derives). Non-finite hand positions are
   `GameError::InvalidInput("hand position")` like the viewer.
2. **Text format**: `input NAME follow=left_hand target=SELECTOR` and `follow=right_hand` (today only `follow=viewer`; unknown
   sources stay refused with a located error that mentions `follow`). `kind NAME ... grab=1` marks a grabbable kind (`KindDef` gets
   a `grab: bool`; printed as `grab=1` only when true; round-trips through `GameDef::to_text`).
3. **Hand follow**: each step, an entity whose kind is targeted by an input that follows `left_hand`/`right_hand` is moved to that
   hand's position when that hand is in `Inputs.hands`; an untracked hand leaves its entity where it was. Do it right after
   `apply_viewer_follow` (see `world/state/follow.rs`).
4. **Holding** (new file `world/state/hands.rs`, state in `World`, included in `Snapshot`, `restore` and the state hash in
   `world/state/hash.rs` so snapshots and determinism keep working). Per side, in order Left then Right, after hand follow and
   before `resolve_viewer_solids` and `collect_touches` (so a carried item touches things):
   - The hand's entity is the first entity targeted by that side's follow input. No such entity: nothing for that side.
   - Remember the previous `grip` and `trigger` per side (a hand missing from the input counts as grip=false, trigger=false and
     releases whatever it holds).
   - **Grab on the grip's rising edge** (previous false, now true), when the side is holding nothing: pick the nearest entity whose
     kind has `grab` and is not held by the other hand, with `distance(hand, entity) <= hand_kind.size + entity_kind.size`. Ties by
     lower id. Remember the offset `entity.position - hand.position`. Queue event `grab` with `a = hand id`, `b = item id`.
   - **Carry while the grip stays closed**: `item.position = hand.position + offset`. (Overwrites any velocity-driven move.)
   - **Release when the grip opens (or the hand goes untracked)**: queue event `release` with `a = hand id`, `b = item id`; the item
     stays where it is.
   - If a held item no longer exists (a rule destroyed it), drop the hold with no event.
   - **Use on the trigger's rising edge**: queue event `use` with `a = hand id` and `b = nearest other entity` (not the hand itself)
     within `hand_kind.size + other_kind.size + 0.05`, ties by lower id, `b = None` when nothing is in reach. A held item counts.
   Event names are exactly `grab`, `release`, `use`, handled by rules `on=grab` and so on, with `a=`/`b=` selectors like `touch`.
5. Document the three events and the new tokens in the doc comments and in `docs/` where game events are listed (grep for
   `host:` or `touch` in `engine/docs`).

## Rules
- Edit only `crates/ve-game` (and its docs comments, plus the one doc line in `engine/docs`). I (Claude) am editing `ve-graph`,
  `ve-modeling` and `ve-runtime` at the same time: do not touch them. Never run `git checkout`, `git clean`, `git stash`,
  `git restore`. No commits.
- Do not run `cargo fmt`; use `rustfmt --edition 2024 --config-path rustfmt.toml <file>` on your own files.
- `CARGO_TARGET_DIR=target-codex-hands`, no `cargo xtask`. No `unwrap`/`expect`/`panic!` in library code, no `#[allow]`,
  files under 400 lines (split `state.rs` pieces by responsibility; it is already 389 lines, so the new code goes in the new file).
- Write the final Status below yourself (your final message is not saved). Say honestly what you did not do.

## Status
Complete. Added public hand input types, left/right hand follow, grab/release/use processing, grab-kind parsing and printing, and deterministic snapshot/hash support. Documented the tokens and events in crate docs and updated the game events listing. All `ve-game` tests pass with `CARGO_TARGET_DIR=target-codex-hands cargo test -p ve-game`. Formatted touched Rust files with the specified `rustfmt` command; `world/state.rs` is 398 lines. Did not run `cargo xtask` or clippy, and made no changes to the fixed hand tests.
