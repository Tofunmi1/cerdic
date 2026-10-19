# Handoff to Codex: the atmosphere CPU reference (`ve-sky`), one job

Read `engine/AGENTS.md` first and follow it: no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason = "...")]` on the
smallest item if a lint is truly wrong), Rust files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution.
**Never run `cargo fmt` on the workspace; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.**
**Never run `git checkout`, `git clean`, `git stash` or `git restore`. Edit only files under `engine/crates/ve-sky/` and this file's Status section.** Other
people are editing other crates at the same time. Do not build the browser bundle. No network.

## The job
Implement the methods of `Atmosphere` in `engine/crates/ve-sky/src/lib.rs` (the signatures and their doc comments are the contract; every body is a stub that returns
zeros). It is the physically based sky model Bevy and Hillaire use (Rayleigh, Mie, ozone, on a spherical planet), computed in `f64`, as a CPU ground truth for a GPU
version. The tests in `engine/crates/ve-sky/tests/reference.rs` are fixed: **do not edit them to make them pass** (a premise you think is wrong goes in the Status).

Method:
- `extinction(height)`: Rayleigh `rayleigh_scattering * exp(-h / rayleigh_scale_height)`, plus Mie `(mie_scattering + mie_absorption) * exp(-h / mie_scale_height)` added to every
  channel, plus ozone `ozone_absorption * tent(h)` where `tent(h) = max(0, 1 - |h - 25000| / 15000)`.
- `distance_to_top(radius, mu)`: the positive root of `|p + s d| = top_radius` for a ray from distance `radius` from the centre with cosine `mu` to the local up:
  `-radius * mu + sqrt(radius^2 * (mu^2 - 1) + top_radius^2)`. `distance_to_ground`: the nearer root against `bottom_radius`, only when `mu < 0` and the discriminant is not
  negative, otherwise `None`.
- `optical_depth(radius, mu, distance)`: integrate `extinction(r(s) - bottom_radius)` for `s` in `0..distance`, with `r(s) = sqrt(radius^2 + 2 radius mu s + s^2)`, by the midpoint rule
  with at least 256 steps (more is fine; the tests allow 1% plus 0.001).
- `transmittance_to_top(radius, mu)` is `exp(-optical_depth(radius, mu, distance_to_top(radius, mu)))` per channel. `transmittance(radius, mu, distance)` is
  `exp(-optical_depth(radius, mu, distance))`.
- `single_scattering(radius, view, sun, sun_illuminance)`: the viewer is at `(0, radius, 0)` in the planet's frame (so local up is +y), the view ray runs to the top of the atmosphere
  or to the ground if `distance_to_ground` is `Some`. Integrate along it with at least 64 steps: at each sample point, the in-scattered light is
  `sun_illuminance * T_view(viewer to sample) * T_sun(sample to top along the sun direction, zero if that ray hits the ground sphere) * (rayleigh_scattering(h) * phase_r(cos) + mie_scattering(h) * phase_m(cos)) * ds`,
  with `cos` the cosine between `view` and `sun`, `phase_r = 3/(16 pi) (1 + cos^2)`, `phase_m` Henyey-Greenstein with `g = mie_asymmetry`
  (`(1 - g^2) / (4 pi (1 + g^2 - 2 g cos)^1.5)`). `T_view` and `T_sun` come from the transmittance functions above. No ground reflection, no multiple scattering.
  Scattering coefficients decay with height exactly as extinction does (Rayleigh and Mie only, no ozone: ozone absorbs, it does not scatter).

Keep it to one file if it fits under 400 lines, otherwise split into `src/lib.rs` plus a module. Add unit tests of your own where useful (in a `#[cfg(test)]` module), but the
fixed tests are the acceptance.

## Acceptance
`cargo test -p ve-sky` passes (all 11 tests), `cargo clippy -p ve-sky --tests -- -D warnings` is clean, `cargo xtask rules` shows no file of yours over 400 lines.

## Status (Codex: fill in, then stop)
- State: complete. Implemented the atmosphere CPU reference in `ve-sky/src/lib.rs`; `cargo test -p ve-sky` passes all 11 fixed tests.
- `cargo xtask rules` passes, and `cargo clippy -p ve-sky --lib -- -D warnings` is clean.
- `cargo clippy -p ve-sky --tests -- -D warnings` is blocked by two `clippy::float_cmp` diagnostics in the fixed `tests/reference.rs` (the exact constants assertion and zero-radiance assertion); tests were left unchanged.
- `cargo xtask quick ve-sky` is blocked at its workspace fmt stage by formatting differences in unrelated crate tests and the fixed `ve-sky/tests/reference.rs`; those files were left unchanged.
