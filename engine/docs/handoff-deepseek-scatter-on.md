# Handoff to DeepSeek: `inst.scatter_on`, instances scattered over the surface of a solid

Read `engine/AGENTS.md` first and follow it (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`, Rust files under 400 lines, docs on public items, tests, no commits, no `Co-Authored-By` or tool
attribution). **Never run `cargo fmt`/`cargo fmt --all`; format only your Rust files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.** Use the edit tools. Do not build the browser bundle.
Write the Status into **this file** at the end.

## Shared tree: hard rules (two other engineers work in the same working tree right now)
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`, or write a file from `git show` over a working file.** If a file you did not change fails to compile, say so in the Status; do not "fix" it by
  reverting it. Another engineer's half-finished edit can break the build for a minute: wait and retry.
- **Files you own this round** (edit only these): `crates/ve-modeling/src/instances/**` (new files such as `instances/scatter_on.rs`), `crates/ve-graph/src/builtin/instances.rs` (the spec), one dispatch line where
  `inst.scatter` is dispatched (find it with `grep -rn "inst.scatter" crates/ve-modeling/src`), `crates/ve-modeling/src/plan/estimate.rs` (only the estimate line for your family: another engineer edits other lines of this
  file, so make a minimal edit), `crates/ve-graph/src/builtin/ready.rs` (one line), the new test `crates/ve-modeling/tests/g11_scatter_on.rs`, and this file's Status.
- **Others are editing**: `cpp/**`, `crates/ve-kernel*`, `crates/ve-geometry/**`, `poly_ops/**` (Codex), and `crates/ve-graph/src/spec.rs`, `value.rs`, `links.rs`, `crates/ve-modeling/src/cache.rs`, `evaluate.rs` (the manager). Do not touch them.
  Keep the tree compiling after every edit you make.

## Resume note (the first run was stopped by a provider rate limit, not by you)
Already in the tree: `crates/ve-modeling/src/instances/scatter_on.rs` (the sampler, 192 lines), `mod scatter_on` and the `"inst.scatter_on"` arm in `instances.rs`, and an import in `instances/preflight.rs`
that is not used yet (it produces an unused-import warning until you use `MAX_SCATTER_ON` there). **Missing**: the operator spec in `crates/ve-graph/src/builtin/instances.rs` (`inst.scatter_on@1` with the ports
and parameters below), the `ready.rs` line, the preflight/estimate use of `MAX_SCATTER_ON` and an estimate for the node, the corpus line, and the eight tests. Read what exists first and continue; do not rewrite
the sampler unless a test proves it wrong. Number sockets exist now (a number parameter can be filled by linking a `value.*` node): you do not need to do anything for them.

## Context
`inst.scatter@1` places instances over a flat rectangle; `inst.drop@1(in, surface)` drops instances straight down onto a solid. Neither puts things **on the surface of a solid in any orientation** (pebbles on a hill's
sides, bolts on a tank, moss on a wall, hair or studs on a shape). Read `crates/ve-modeling/src/instances/scatter.rs` and `drop.rs`, the specs in `crates/ve-graph/src/builtin/instances.rs`, and how an instance set
is represented (a list of 4 x 4 matrices, see `output.rs`) before you design anything.

## The node
`inst.scatter_on@1(in, surface)`: `in` is the instance set to copy (as for `inst.scatter`: the thing being placed), `surface` is a solid. Parameters: `count` (int 1..5000: the number of placements wanted), `min_distance`
(metres, 0 = none: no two placements closer than this; refused rather than reduced when `count` cannot be met, as `inst.scatter` does, with a suggestion that names a smaller `count` or `min_distance`), `align`
(bool, default true: rotate each copy so its +Y axis is the surface normal; false keeps the original orientation), `spin` (bool, default true: a deterministic random turn about the normal), `tilt` (angle, 0..45 degrees,
default 0: deterministic random lean away from the normal), `scale_jitter` (0..1, default 0: scale each copy by 1 +/- this fraction), `seed` (int; use the document seed convention `inst.scatter` uses), `region` (choice
`all` | `up` | `side` | `down`, default `all`: only where the surface normal points up (y > 0.5), sideways (|y| <= 0.5) or down). Points are chosen **uniformly by area** over the solid's triangles (area-weighted
triangle choice, then uniform barycentric coordinates), deterministically from the seed (use the same hash or generator `scatter.rs` uses, no new dependency), and `min_distance` is enforced by simple rejection
(dart throwing with a bounded number of attempts: refuse with a typed error if it cannot place `count`). Output: the instance set the other `inst.*` nodes accept.

## Tests (`g11_scatter_on.rs`; model them on `crates/ve-modeling/tests/g11_round_edges.rs` for the session/evaluate helpers and on an existing instance test for reading the placed matrices; find one with
`grep -rln "inst.scatter" crates/ve-modeling/tests`). Fixed; do not weaken any assertion; if one fails, report it.
Fixture: a unit cube `solid.box@1(size=[1,1,1])` as the surface, a small `solid.box@1(size=[0.02,0.02,0.02])` as the prototype.
1. `count=200`: exactly 200 placements.
2. Every placement's translation lies on the surface: the distance to the cube's surface is at most 1e-4 (the max over x, y, z of |coordinate| is 0.5 within 1e-4, and the point is inside [-0.5,0.5] on the other axes).
3. Area-uniform: over 2000 placements on the unit cube, each of the six faces gets between 14% and 19% of them (the expected share is 1/6 = 16.7%).
4. `align=true`: each placement's local +Y axis (second column of its matrix) equals the unit normal of the face it sits on within 1e-4 (the face is the axis where the point's |coordinate| is 0.5; the normal is its sign).
5. `min_distance=0.2` with `count=60`: every pair of placements is at least 0.2 apart; and `count=100000` refuses with a typed code starting `inst.` and a suggestion mentioning `count` or `min_distance`.
6. `region=up` puts every placement on the top face (y = 0.5 within 1e-4); `region=down` on the bottom; `region=side` on none of those.
7. `seed` is honoured: the same seed gives bit-identical matrices on two evaluations, a different seed gives different ones.
8. `scale_jitter=0.5` gives scales within [0.5, 1.5] and not all equal; `tilt=30` keeps the +Y axis within 30 degrees of the normal (and some beyond 5).
Add one corpus fixture line like the existing ones (find the corpus example `crates/ve-modeling/examples/corpus*` and add a line for your node; another engineer is also adding one, so make a minimal edit).

## Acceptance
`cargo test -p ve-modeling --test g11_scatter_on`, `cargo test -p ve-modeling -p ve-graph` (report any failing test by name), `cargo clippy -p ve-modeling --tests` shows nothing new from your files, `cargo xtask rules`.

## Status (DeepSeek: fill in with real numbers for each of the eight tests, then stop)
- State: not started
- Per test: pass/fail and the measured value:
- Commands run and results:
- Open questions or things you chose not to do:
