# Handoff: `detail.panels@1`, panel lines and greebles on one face of a solid

Owner: Claude manages; Codex implements. **Crates you may edit, in this order, running that crate's tests before moving on: `ve-graph` (operator spec, ready list), `ve-modeling` (evaluation: a new `src/detail_panels.rs`, registration in `operations.rs`, estimate in `plan/estimate.rs`).** Do not touch `ve-world`, `ve-render`, `ve-bevy`, `ve-runtime`, `ve-evaluator`, `src/material_presets.rs` (Claude edits that file at the same time; do not touch it even if a build message mentions it), the tests of other features.
Rules: never run `git checkout`, `git clean`, `git stash` or `git restore`; never `cargo fmt` (use `rustfmt --edition 2024 --config-path rustfmt.toml <file>`); never `cargo xtask`; use exactly `CARGO_TARGET_DIR=/tmp/codex-target-panels`; do not commit or stage; edit with your edit tools, not scripts; leave `[lints]` alone. `ve-modeling`'s lib test target may not compile for reasons that are not yours: test with `--test detail_panels` and the other integration targets. If another file of `ve-modeling` fails to compile because someone else is mid-edit, wait a minute and retry; do not "fix" it.

## Why

A sci-fi wall, hull or machine reads as built when its flat faces are broken into panels: seams between plates, some plates thicker than others, vents with slats. In VR the player stands a metre from these surfaces, so the detail has to be real geometry, not a picture. A model should get this from one node, not by placing hundreds of boxes. The research (2026-10-24, secondary sources) agrees on the method: subdivide a face into a grid, extrude random cells by random heights, keep every plate attached to the surface.

## What to build

`detail.panels@1(in, face="+z", cols=4, rows=3, gap=0.02, depth=0.04, raised=0.3, vents=0.1, seed=1)`:
- Input `in`: a solid (the family `solid.box@1` and `xform.move@1` give). Output: a **new solid** containing only the plates (not the input), so the model can join it, texture it or keep it apart. Output type a solid like `solid.box`.
- `face`: choice `+x -x +y -y +z -z`: which side of the input's axis-aligned bounding box to cover. The plates sit on that face's plane, spanning the whole face rectangle of the bounding box.
- `cols`, `rows`: integers 1 to 32 (defaults 4 and 3): the grid along the face's two axes (for ±z: x and y; for ±x: z and y; for ±y: x and z). `gap`: metres 0 to 1 (default 0.02): the seam between neighbouring plates; each plate is inset by gap/2 on every side, so no vertex lies within gap/2 of a seam line (the outer edge of the face is not a seam: plates also stay inside the face rectangle). A gap that leaves no plate (gap at or over the smaller plate side) is refused.
- `depth`: metres 0.001 to 1 (default 0.04): the thickness of a flat plate. `raised`: 0 to 1 (default 0.3): the share of plates that stand out, each 1.5 to 3 times `depth` thick (chosen per plate by the seed); `vents`: 0 to 1 (default 0.1): the share of plates drawn as a vent: a flat plate with a row of 4 to 6 thin slats standing on it (slat heights at most `depth` higher than the plate, still within the 3 times depth bound). A plate is never both raised and vented. Parameters refused outside their ranges with a typed `param.range` diagnostic, the face name refused otherwise; read numbers with `Param::as_f64` (a whole number such as `raised=1` arrives as an integer in a saved document, and it once failed in the browser because a reader accepted only floats: the tests include that case).
- Every plate's base lies exactly on the face plane (the plates touch the input, so `check.attached` passes and a `solid.union` with the input joins them); nothing goes below the plane.
- Deterministic: element choices depend only on `seed` and the plate index, not on global state. Different seeds give different plates (when `raised` or `vents` is above 0).
- Bounded: under 60,000 triangles at `cols=12 rows=8 raised=0.5 vents=0.5`; refuse inputs and parameters that would exceed a sensible limit with a typed diagnostic instead of hanging.
- Operator doc (one paragraph the model reads): say what it does, which faces, that the output is separate plates that touch the face, that `raised` and `vents` are shares, and give a one-line example.
- In `ve-graph`: the spec (see how `solid.texture@1` and `check.attached@1` are declared in `builtin/modeling/appearance.rs` and `builtin/modeling.rs`), the ready list in `builtin/ready.rs`; in `ve-modeling`: register in `operations.rs`, and `plan/estimate.rs` (`Ok(0)` like other cheap nodes unless the estimator needs the triangle count).
- Build the geometry in the simplest way that gives exact, closed, outward-wound boxes (each plate and slat a closed box mesh; the parts can overlap where a slat stands on a plate, or union them if there is a helper that is fast; look at how `solid.box@1`/`solid.union@1` produce their value).

## Tests (written by Claude; never weaken; add more if you like): `crates/ve-modeling/tests/detail_panels.rs`

Eleven tests: plates cover the face and sit on it (bounds); the gap is empty; flat plates are `depth` thick and raised ones stand out 1.5 to 3 times; vents add slats; seeds; all six faces; triangle budget; bad parameters refused; integers accepted; it joins with the wall and takes `material.preset`; it passes `check.attached`. Also run `cargo test -q -p ve-graph` (update only entries for the new operator).

## Check

`cargo test -q -p ve-graph`, `cargo test -q -p ve-modeling --test detail_panels --test material_preset --test check_attached`, `cargo build -q -p ve-evaluator --target wasm32-unknown-unknown --release`.

## Status (Codex: write this section yourself when you stop; your final chat message is not saved)

- [x] done (2026-11-07, implemented by Claude; the Codex run never delivered)
- Algorithm: every plate and slat is a closed box (`Solid::cube` + `translate`), joined by the kernel's batch union (64 at a time, then the results); slats start inside their plate so they fuse. A plate is vented when a seeded draw is below `vents`, raised when a second draw is below `raised` (never both); thickness, slat count and slat height come from further draws keyed on the seed and plate number.
- Tests run: `cargo test --release -p ve-modeling --test detail_panels` 11 passed; `ve-graph` 247 passed; `material_preset`, `check_attached` pass.
- Files changed: `ve-graph/src/builtin/detail.rs` (new; also holds `sign.text@1`), `builtin.rs`, `builtin/ready.rs`; `ve-modeling/src/detail_panels.rs` (new), `lib.rs`, `operations.rs` (one arm in `solid`), `plan/estimate.rs`; `src/engine/operators.d.ts` regenerated.
