# Task for Codex: renderer quality (ambient occlusion, bloom, tone mapping, better specular)

Owner: work in parallel with Claude. Claude is building the modelling and material pipeline (baked detail maps and a PBR material
kit) at the same time, so stay inside the files named under "Boundaries". Read `engine/AGENTS.md` and your finished briefs
(`task-codex-material-opacity.md`, `task-codex-water-and-sky.md`) first: the working rules, "How to run things" and "Owner rules"
sections apply unchanged.

## Goal
Our scenes look flat and "low poly" partly because the renderer only has sun shadows, baked per-vertex occlusion and sky ambient.
Add the screen-space and post-processing layer that makes every existing scene look richer without touching any asset:
1. Screen-space ambient occlusion (SSAO or GTAO) that darkens creases, contact points and under-eaves.
2. Filmic tone mapping with exposure, and bloom for emissive parts, lights and the sun glint.
3. Better specular: geometric specular anti-aliasing (reduces shimmer on normal-mapped, rough surfaces) and sky-based reflections
   for metals and gloss (the sky node already gives a colour/gradient; use it as the reflection source).
All of it must be cheap enough for a Quest-class GPU in stereo: every effect has a quality level (the viewer already has a
`Quality` setting: see `crates/ve-render/src/quality.rs`) and can be turned off, and `low`/XR defaults keep the cost small.

## What to look at first
- `crates/ve-render` (`pipeline.rs`, `pass.rs`, `prepare/`, `shaders/`, `grade.rs` is the existing colour grade, `quality.rs`,
  `schedule.rs`): find where the main pass, the depth/reverse-z buffers and the grade run, and where a screen-space pass would slot
  in per eye (stereo layout: two viewports in one target; each eye must be processed on its own viewport).
- The golden images in `web/tests/golden/` must NOT change for existing scenes when the new effects are at their "off" setting, and the
  default for the golden scenes must stay as-is; if you want the effects on by default, ask the owner in your report first and do not
  regenerate goldens yourself.
- Depth is reverse-z (see `ve-xr-math`); normals: reconstruct from depth or add a thin normal target, whichever is cheaper.

## Acceptance
1. Each effect is a separate setting reachable from the page (`set_quality` style, plus a line in the viewer panel next to Sun shadows),
   with `off` meaning byte-identical output to before.
2. Browser tests `engine/web/tests/g10-render-ao.mjs` and `g10-render-post.mjs` (ports 5250 to 5269): a corner of a room is darker
   with AO on than off by a measured margin and a flat open floor is not; a glowing sphere shows a halo with bloom on and none with it off;
   a rough metal sphere's specular does not alias (compare against a supersampled reference, or measure frame-to-frame flicker while
   the camera moves a sub-pixel). Look at the screenshots yourself and say what you saw.
3. Stereo: both eyes get the effects with no seam at the eye boundary (test with the mock headset split screen and with the IWER Quest 3
   test `engine/web/tests/xr.mjs` untouched and passing).
4. Cost: record frame time (software GL is fine for a relative number) with each effect on and off in your report, and the resolution scale you use
   for AO (half resolution plus a bilateral upsample is the usual trade).
5. Rust tests for any pure maths (kernels, sample patterns, tone curves); clippy clean on the render crate; no `#[allow]`.
6. ADR `docs/adr/0040-render-quality-passes.md` (proposed) and a line in `docs/plan-animation-and-blender.md`.

## Boundaries
Do edit: `crates/ve-render/**`, the quality plumbing in `ve-runtime/src/quality.rs` and the viewer panel lines in `web/viewer.js`, new tests, the ADR.
Do not edit: `ve-physics`, `ve-modeling`, `ve-graph`, `ve-runtime/src/physics.rs`, `behavior*`, `projection/*` (Claude is adding map-related plumbing there),
`src/engine/texture/**`, `src/engine/gltf.ts`, `web/demos/*`. If you must touch `web/index.html`, make it a few lines and re-read first.

## Owner rules (unchanged)
Edit with your write/edit tool only (no heredoc, sed, perl, python on repo files, not even "just to check"). No attribution lines in commits or PRs;
do not commit unless asked. Everything in the browser; nothing pulled. Fix lints, do not silence them. Kill only processes you started, by pid. Report in chat what
you ran and what passed with numbers, and what you did not finish.
