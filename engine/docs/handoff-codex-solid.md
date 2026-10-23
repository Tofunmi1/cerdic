# Handoff to Codex: solid kinds and ordered bot targets (`ve-game`)

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: `#[expect(lint, reason)]` on the
smallest item only, files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution). **Never run `cargo fmt`; format
only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Own `CARGO_TARGET_DIR=target-codex-solid`; delete it when done.
Context: building a key-and-door world showed that a "locked" door is only something that disappears: nothing stops the hero (which follows the viewer) from walking
through it, so a bot reaches the exit without the key. This task makes a kind able to be an obstacle for the hero, and lets a bot pursue targets in order.

## Part 1: solid kinds
- Kind text key `solid=w,h,d`: full extents in metres of an axis-aligned box centred on the entity (all three > 0, exactly three numbers). Bad values are a located
  diagnostic on the kind's line whose message contains `solid`. `KindDef.solid: Option<[f64; 3]>` (public); `to_text` prints it back (`solid=0.5,2.6,7.6`); equality after a round trip.
- During a step, after entities that follow the viewer are placed (the `follow=viewer` handling in `world/state/follow.rs`) and before touches and rules: every followed entity
  is pushed out of every *other* living entity whose kind has `solid`, treating the followed entity as a sphere of radius `size` of its kind: if the sphere centre is inside the box expanded by
  that radius on every side, move it out along the horizontal axis (x or z) of least penetration, to the face of the expanded box on the side it is nearer to. Ignore the vertical axis for
  the push but only block when the entity's y lies within the box's vertical extent expanded by the radius. Several solids: resolve in entity id order, one pass each. A destroyed entity no longer blocks.
- Nothing else changes: touches use the same spheres as before, an entity without `solid` is never an obstacle, determinism and snapshots hold (positions are already in snapshots).

## Part 2: ordered targets for the Seek bot (`ve_game::playtest`)
`Policy::Seek { target }` accepts a comma-separated list of tags in priority order (`"key,gate,exit"`): the walker (or the steered hero) goes for the nearest living entity of
the *first tag in the list that has any living entity*, so it takes the key, then the door, then the exit. A single tag behaves as before. Update the `playtest` module doc and the
`eval_text` example doc line if needed (`crates/ve-modeling/examples/eval_text.rs` mentions the policy words).

## Contract (written first, do not weaken)
`crates/ve-game/tests/solid.rs` (and every existing `ve-game` test stays green). Report a test that looks wrong in Status; do not edit it.

## Verify
```
cd engine
CARGO_TARGET_DIR=target-codex-solid cargo test -p ve-game -p ve-evaluator
CARGO_TARGET_DIR=target-codex-solid cargo clippy -p ve-game --all-targets -- -D warnings
CARGO_TARGET_DIR=target-codex-solid cargo check -p ve-game -p ve-evaluator -p ve-modeling --target wasm32-unknown-unknown
```

## Status

- Added `KindDef.solid`, parsing and canonical printing for positive finite `solid=w,h,d` extents, and located diagnostics for malformed extents.
- Added deterministic solid resolution after viewer-follow placement and before touches. Followed entities use their kind radius; each live solid is processed in entity-id order. The previous-to-followed segment is checked so sampled movement cannot tunnel through a solid. Touch detection is unchanged.
- Seek now accepts comma-separated target tags in priority order for both walker and axis steering. Updated API and `eval_text` policy docs.
- `CARGO_TARGET_DIR=target-codex-solid cargo test -p ve-game -p ve-evaluator` — passed (all package tests, including all 8 `solid.rs` tests).
- `CARGO_TARGET_DIR=target-codex-solid cargo check -p ve-game -p ve-evaluator -p ve-modeling --target wasm32-unknown-unknown` — passed; emitted existing warnings in `ve-geometry` and `ve-modeling`.
- `CARGO_TARGET_DIR=target-codex-solid cargo clippy -p ve-game --all-targets -- -D warnings` — blocked by `clippy::unwrap_used` and `clippy::float_cmp` in the handoff contract file. Left that test file unchanged as instructed.
- `CARGO_TARGET_DIR=target-codex-solid cargo xtask quick ve-game ve-evaluator ve-modeling` — stopped at workspace `fmt --all --check`, which reports formatting differences in unrelated files; it did not modify them.
- `CARGO_TARGET_DIR=target-codex-solid cargo xtask rules` — the changed game files pass the 400-line rule; the gate still reports four unrelated oversized files: `ve-graph/src/builtin/fields.rs`, `ve-graph/src/builtin/world.rs`, `ve-modeling/src/human_hair.rs`, and `ve-runtime/src/game.rs`.
- `CARGO_TARGET_DIR=target-codex-solid cargo xtask parity` — reported native/WASM corpus mismatches. This result is invalid under the requested custom target directory: the build uses `target-codex-solid`, while `xtask/src/parity.rs` reads the hard-coded `target/wasm32-wasip1/debug/examples/corpus.wasm`. Left the gate unchanged.
- `CARGO_TARGET_DIR=target-codex-solid cargo xtask web-test dev game-vault` — wasm bundles built, then the web gate stopped at pre-existing TypeScript errors in `src/engine/snapshot.ts` and `tests/shader-expr.test.ts`, before the browser script ran.
- No solid contract test appeared semantically wrong; all passed without edits. The strict workspace gate remains pending.
