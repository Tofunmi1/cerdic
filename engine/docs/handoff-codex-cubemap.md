# Handoff to Codex: procedural environment cubemaps in `ve-sky`, one job

Read `engine/AGENTS.md` first and follow it: no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason = "...")]` on the
smallest item if a lint is truly wrong), Rust files under 400 lines, docs on public items, no commits, no `Co-Authored-By` or tool attribution.
**Never run `cargo fmt` on the workspace; format only your files with `rustfmt --edition 2024 --config-path rustfmt.toml <file>` from `engine`.**
**Never run `git checkout`, `git clean`, `git stash` or `git restore`. Edit only `engine/crates/ve-sky/src/cubemap.rs` and this file's Status section.** Other
people are editing other crates at the same time. Do not build the browser bundle. No network.

## The job
`engine/crates/ve-sky/src/cubemap.rs` has five functions whose bodies are stubs (`texel_direction`, `texel_solid_angle`, `sky_radiance`, `gradient_cubemap`, `irradiance_cubemap`,
`prefiltered_mips`; the signatures and doc comments, including the cube face table, are the contract). They build the sky environment that Bevy's environment light reads on
WebGL2, where the atmosphere compute passes cannot run. Implement them in `f32`.
The tests in `engine/crates/ve-sky/tests/cubemap.rs` are fixed: **do not edit them to make them pass** (a premise you think is wrong goes in the Status).

Notes:
- `irradiance_cubemap` and `prefiltered_mips` sum over the source texels weighted by `texel_solid_angle` (and by the cosine, or by the cone membership). Brute force is fine for
  source sizes up to 64 and outputs up to 32. For the cone: a source texel is inside when `dot(direction, texel_direction) >= cos(half_angle)`; normalise by the summed solid angle
  of the texels that were inside, so a uniform source stays uniform. If no source texel falls inside (a tiny cone on a coarse source), use the source texel nearest the direction.
- Mip `i` of `prefiltered_mips` is computed from the source cube (not from the previous mip). Mip 0 is a clone of the source.
- All output alpha is 1.

## Acceptance
`cargo test -p ve-sky` passes (the 11 new tests and the older ones), `cargo clippy -p ve-sky --lib -- -D warnings` is clean, and no file of yours is over 400 lines.
Tests are slow if you build the sums wastefully: they should finish in a few seconds in debug.

## Status (Codex: fill in, then stop)
Implemented the five cubemap functions in `crates/ve-sky/src/cubemap.rs`: cube directions and solid angles, gradient sky and sun sampling, cosine-weighted irradiance, and source-based cone-filtered mips with nearest-texel fallback. Mip 0 clones the source; generated texels have alpha 1.

Verification: `cargo test -p ve-sky` passed (22 tests); `cargo clippy -p ve-sky --lib -- -D warnings` passed. `cubemap.rs` is 227 lines. Formatted only that file with the prescribed `rustfmt` command. No browser bundle build or workspace gate was run.
