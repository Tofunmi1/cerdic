# Handoff to Codex: make the tree species look like the trees they are named for

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`: use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, **no commits, no `git checkout`/`clean`/`stash`/`restore`**, no `Co-Authored-By` or tool attribution, file changes through the edit tools only). **Never run `cargo fmt`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`.** Use `CARGO_TARGET_DIR=target-codex-floraq`. You are the only agent in `crates/ve-world` now. Edit only `crates/ve-world/src/flora/` (the growers `tree.rs`, `colonize.rs`, `tuft.rs`, `preset.rs` and their helpers); do not edit any file under `crates/ve-world/tests/`.

## Why
The finished `flora` module grows valid skeletons, but when the six species were meshed and rendered (a single 10 m oak, and a forest of them) they looked like saplings: a thin trunk, a few branches, a few leaves. Measured on a 10 m plant (seed 4) with the new test `crates/ve-world/tests/flora_quality.rs`:

| species | leaf points now | wanted | other defects now |
| --- | --- | --- | --- |
| oak | 315 | at least 900 | leaves reach 12.4 m on a 10 m tree (wanted 7.5 to 11.5), crown radius 6.9 m |
| pine | 48 | at least 600 | leaves top out at 7.0 m (wanted 8.5 or more), radius 3.6 m (wanted 1.4 to 3.8) |
| birch, willow, shrub | not measured, the same test asks for 500, 700 and 120 leaf points | | |

(A leaf point stands for a small cluster of leaves; the viewer draws several cards per point. Today leaves are placed only at twig tips, which is why so few exist.)

## What to do
1. Run `cargo test -p ve-world --test flora_quality` and make all four tests pass (the numbers in the test are the targets; read them, they are the contract). Keep `tests/flora.rs` and `tests/lod.rs` passing.
2. How to get there (a guide, not a prescription): place leaf points along the outer twigs, not only at tips (every node of depth at least `leaf_depth_from`, spaced by about the leaf size, with a few per node), add more branches per level and a third level for the broadleaf species, give the oak a short trunk with its first branches starting at about 25 percent of the height and a rounded crown whose width is about 0.6 to 0.8 of the tree height, give the pine a straight tall trunk with whorls of short drooping branches carrying many needle clusters, a birch slender with thin drooping twigs, the willow long gravity-bent branches, the shrub many thin stems from the ground with leaves from low down, the palm a curved trunk with a crown of long fronds. Keep the plant's own height honest: the highest leaf must be within about 15 percent of the requested height (limit branch lengths and upward angles, or normalise at the end).
3. Keep the budgets: a 10 m oak at most 20,000 skeleton nodes and growth under 20 ms in release (`cargo test -p ve-world --release -- --ignored`, which includes the existing timing test), leaf points under 6,000.
4. Wind data stays as it is (per node `wind`, `phase`, `stiffness`, `branch`); new nodes get the same treatment.
5. Keep determinism (same seed, same plant) and platform independence (no std trig in the growers; the module already has its own helpers).
6. After the tests pass, run `cargo clippy -p ve-world --all-targets -- -D warnings` (no `#[allow]`) and `cargo check -p ve-world --target wasm32-unknown-unknown`, and write the Status honestly: the measured table above for all six species with your numbers, release timings, and anything you could not meet.

## Round 2 (after looking at renders of round 1; do this now)
Round 1 passed its numbers, but rendered one species at a time (a single 10 m tree, seed 4) the shapes are poor, because the Weber-and-Penn presets grow only a handful of main limbs: the **oak** is a leafy "Y" of two limbs, the **pine** is a bare pole with five leafy side branches (not a conifer), the **willow** is one big sideways limb, the **shrub** is a pole with three limbs; the birch and the palm are acceptable. The tests now measure balance and conical shape too (`crowns_are_balanced_around_the_trunk_not_thrown_to_one_side`, `a_pine_is_a_cone_with_branches_all_the_way_up`); the numbers on the current presets: an oak sector holds 0.3 percent of the leaf points (target at least 5 percent per sector), and the pine's mean branch reach by height band is 2.3, 2.0, 1.9, 2.1 m (target: the lowest band at least 1.5 times the top band, and at least 10 percent of the points in every band).
The remedy is already in the module: **`colonize` (space colonization) fills a crown volume organically with branches reaching every side**, which is what real crowns look like. Make the species recipes for **oak, birch, willow, pine and shrub** produce `Preset::Colonize` setups (`preset` returns the enum already) with crown volumes and parameters that give the right silhouette: oak a broad ellipsoid (width about 1.1 to 1.4 times the crown height) above a clear trunk of about 30 percent of the height; birch a narrow ellipsoid with a slender trunk and fine twigs; willow a wide, low ellipsoid whose branch tips droop (add a gravity or droop term to the grower if needed, so tips bend down as they extend, giving a weeping outline reaching toward the ground); pine a `Crown::Cone` from about 20 percent of the height to the top with a straight trunk through it; shrub a low sphere or ellipsoid starting at the ground with trunk height near zero and several stems (seed multiple stems near the base). Keep the palm on the parametric grower. Tune `points`, `attraction_radius`, `kill_distance` and `step` so each plant has enough nodes to read as dense (aim for 3,000 to 12,000 skeleton nodes and 1,500 to 6,000 leaf points per 10 m tree; growth under 20 ms in release; `max_iterations` large enough that the crown fills). Leaves go along the outer twigs as before. Keep every test in `flora.rs`, `flora_quality.rs` and `lod.rs` passing (the older oak-height test uses its own hand-built tree, not the preset). The wind data stays per node.
How to judge it beyond the tests: render the single tree of each species (`flora.tree@1(kind=..., height=10, seed=4)`) and look at its silhouette from the side; the owner-side session will do that review after you finish and send corrections.

## Status (Codex: fill in when you finish, then stop)
- State: Blocked on the protected LOD fixture. All six `flora_quality` tests and all non-timing `flora.rs` tests pass. `lod.rs` cannot pass with the required oak recipe: its `oak()` helper matches `Preset::Colonize(_)` to `unreachable!("the oak is a parametric tree")`. I did not edit anything under `tests/`, as instructed.
- What was built: oak, pine, birch, willow and shrub now return colonization recipes. The colonizer seeds balanced radial stems, supports low multi-stem shrubs, bends wide ellipsoid growth down for the willow, constrains ellipsoid branches to the actual crown radii, and distributes foliage along growth nodes. Pine receives denser needle clusters near the cone top. Palm remains parametric. Wind fields continue to be populated per node.

  Measured leaf crown extents at 10 m, seed 4:

  | Species | Skeleton nodes | Leaf points | Lowest leaf (m) | Radius (m) | Highest leaf (m) |
  | --- | ---: | ---: | ---: | ---: | ---: |
  | Oak | 3,835 | 3,788 | 3.02 | 5.50 | 8.95 |
  | Pine | 3,268 | 3,680 | 2.12 | 2.89 | 9.89 |
  | Birch | 3,657 | 3,610 | 2.62 | 2.69 | 9.61 |
  | Willow | 4,007 | 3,963 | 2.33 | 6.86 | 8.62 |
  | Shrub | 2,355 | 2,340 | 0.64 | 4.41 | 11.29 |
  | Palm | 61 | 48 | 2.91 | 7.21 | 10.00 |

  The five colonized 10 m recipes are within the 20,000-node and 6,000-leaf budgets; shrub has fewer than the suggested 3,000 nodes. The shrub quality test at 2 m passes. Release oak colonization averaged 16.15 ms over 20 runs. No render review was performed.
- Commands run and results (all with `CARGO_TARGET_DIR=target-codex-floraq`, from `engine/`):
  - `cargo test -p ve-world --test flora_quality` — 6 passed.
  - `cargo test -p ve-world --test flora` — 13 passed, 1 timing test ignored in debug.
  - `cargo test -p ve-world --test flora --test lod` — `flora.rs` passed (13 passed, 1 ignored); `lod.rs` failed 7 tests at its protected `Preset::Colonize(_) => unreachable!` arm. The LOD timing test also failed there in the release ignored run.
  - `cargo test -p ve-world --release -- --ignored` — erosion quality passed and the flora growth timing test passed; the command stopped at the LOD timing fixture failure above.
  - `cargo clippy -p ve-world --lib -- -D warnings` — passed; flora library is clippy-clean.
  - `cargo clippy -p ve-world --all-targets -- -D warnings` — blocked by `clippy::cast_sign_loss` in protected `tests/flora_quality.rs:85`.
  - `cargo check -p ve-world --target wasm32-unknown-unknown` — passed.
- Could not complete: making `lod.rs` green requires its fixture to accept `Preset::Colonize` (and mesh the resulting skeleton), while the instructions prohibit editing tests and Round 2 explicitly requires the oak preset to be `Preset::Colonize`. All-target clippy also needs the protected quality-test cast fixed. The strict workspace gate was not run; strict gate pending.
