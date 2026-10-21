# Handoff to Codex: measuring and fitting a human (`ve-human::measure` and `ve-human::fit`)

Read `engine/AGENTS.md` and `engine/docs/handoff-common.md` section 2 first and follow them (no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]`:
use `#[expect(lint, reason)]` on the smallest item only, files under 400 lines, docs on public items, tests, **no commits, no `git checkout`/`clean`/`stash`/`restore`**,
no `Co-Authored-By` or tool attribution). **Never run `cargo fmt` or `cargo fmt --all`; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>`
from `engine`.** Edit only `crates/ve-human` (new files `measure.rs`, `fit.rs`, plus whatever you need in `lib.rs`). Another engineer is editing other crates; do not run cargo
in other crates. Read `docs/adr/0055-human-body-from-cc0-data.md` and `docs/handoff-codex-human.md` (the evaluator you build on, already done).

## Why
The human is editable by algorithm, not by sliders alone: a model or a person says "1.82 m, 84 cm waist, 78 kg" and gets a plausible person with those numbers. The body is linear in its
blend shapes for fixed phenotype weights, and the phenotype weights are smooth products of the sliders, so a bounded, regularised Levenberg-Marquardt fit over a few parameters with
finite-difference Jacobians converges in a few iterations (an evaluation is ~10 ms).

## What to build
**Measurements** (`measure.rs`): `Measure` (enum, `Clone, Debug, PartialEq`): `Stature` (max y minus min y of the body, m), `Mass` (kg: signed volume of the triangle surface
times 980 kg/m3, as the reference model does), `ShoulderWidth` (distance between the heads of `upperarm01.L` and `upperarm01.R`), `ArmLength` (`upperarm01.L` head to `wrist.L` head along the
chain `upperarm01, lowerarm01, wrist`, i.e. the sum of the segment lengths), `LegLength` (height of the `upperleg01.L` head above the lowest point), the circumferences
`NeckCircumference`, `ChestCircumference`, `WaistCircumference`, `HipCircumference` (the perimeter of the torso's cross-section at a landmark height: neck at `neck02`, chest at `spine03`,
waist at the narrowest of the torso between `spine02` and the pelvis, hip at the widest of the glutes below the pelvis: choose and document landmarks from the bone heads and check them
against the plausibility ranges in the tests; of the section's loops take the one around the body axis (the arms are separate loops in the A-pose; if the arms touch the torso at the
chest, take the convex hull of the section points within the torso's x range and say so), `HeadWidth` (x extent of the vertices dominated by the `head` bone), and
`BoneDistance(String, String)` (distance between two bone heads, any names in `bone_names()`). `Measure::name() -> String`, `Measure::parse(&str) -> Result<Measure, Error>`
(snake_case names: `stature`, `mass`, `shoulder_width`, `arm_length`, `leg_length`, `neck_circumference`, ..., and `bone_distance:<a>:<b>`; an unknown name names itself and lists the valid ones).
`measure(&Human, &Body, &Measure) -> Result<f64, Error>`. Everything in metres and kilograms, f64 results. The section routines may live on `Human` or be free functions: tests only use `measure`.

**Fit** (`fit.rs`): `Target::new(Measure, f64)` (plus public fields `measure`, `value`, and `weight: f64` default 1.0, a relative importance), `FitOptions { free_phenotype: Vec<String>,
free_local: Vec<String>, max_iterations: usize, regularisation: f64 }` with `Default`: free phenotype `height, weight, muscle, proportions` (age, gender and race are NOT free by default),
free local the 20 `measure-*` local changes (`measure-waist-circ-incr` and so on: the `Human::local_change_labels()` starting with `measure-`), 20 iterations, regularisation small (a
penalty on moving from the start, so a request is met by the least change). `fit(&Human, start: &Params, &[Target], &FitOptions) -> Result<FitResult, Error>` with
`FitResult { params: Params, achieved: Vec<Achieved>, converged: bool, iterations: usize, residual: f64 }` and `Achieved { measure, target, value }` (`Debug, Clone`). Parameters stay inside their
ranges (phenotype 0..1, local -1..1) by projection or a bounded step; an unreachable target gives the nearest body with `converged = false`, never an error. Normalise the residuals
so metres and kilograms are comparable (divide each by a tolerance: 1 cm for lengths and circumferences, 1 kg for mass, times the target's weight). Errors: no targets (message contains
"target"), a non-positive target value, an unknown free parameter name (names itself and lists the valid ones). Deterministic (no randomness, no threads). Do not call `evaluate` more
often than needed (cache the start; use forward differences; stop at a residual below 0.05 tolerance units or when a step no longer helps).

## Tests, already written
`crates/ve-human/tests/fit.rs`. Do not weaken them. If a premise is wrong (a plausibility range, the male/female stature, the 20-second budget), say so in the Status with the number
you measured and do not edit my tests quietly. Add your own unit tests for the section routine (a cylinder or box analogue with a known perimeter) and for the solver on a small linear problem.

## Acceptance
`cargo test -p ve-human` passes, `cargo clippy -p ve-human --all-targets -- -D warnings` clean, `cargo check -p ve-human --target wasm32-unknown-unknown` passes, no file over 400 lines,
docs on public items.

## Status (Codex: fill in when you finish, then stop)
- State: Measurement refactor is in progress; two confirmed measurement failures remain. The stature premise was corrected by the owner: 2.3828 m is acceptable and was not changed.
- What was built: Replaced unordered section-point hulls and the central 24 cm torso cutoff with triangle-plane intersection segments, segment chaining into closed loops, and a central-loop selector. Neck now samples between `neck01` and `head`; chest uses a plane below `spine03`. No changes were made to `tests/fit.rs`.
- Commands run (all Cargo commands used `CARGO_TARGET_DIR=/Users/mac/Work/victoria-vr-hack/engine/target-codex` and `-p ve-human`): `cargo test -p ve-human` — latest run has 4 unit tests pass and 6/9 integration tests pass. Remaining failures: weight extremes still do not produce the required >0.2 m waist increase; adult female neck is measured at 0.2073 m (expected 0.28–0.45 m); whole-set fit misses mass by 3.10 kg (118.80 requested, 115.70 achieved), downstream of the changed circumference measurements. Stature recovery and the 3 m nearest-body request pass. Earlier run before the latest loop filtering measured adult male neck at 0.0131 m, showing loop choice remains unstable. No clippy or wasm check was run after this measurement change.
- Could not verify / open questions: The closed-loop chaining/selection needs further correction to select the anatomical torso and neck loops reliably; the requested waist response and adult neck range are not yet met. `cargo xtask quick`, `cargo xtask rules`, and the strict gate were not run because the requested validation was restricted to Cargo commands scoped to `-p ve-human`; strict gate pending.
