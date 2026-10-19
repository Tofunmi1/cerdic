# Handoff: more modeling tools now (parallel packages)

Date: 2026-10-16. The owner needs more modeling tools immediately. This splits the "G10b" batch in [`handoff-after-g9.md`](handoff-after-g9.md) section 3
into packages that can run as separate agents at the same time without editing the same files. Read [`handoff-common.md`](handoff-common.md) (gate
policy: `quick`, `rules`, `parity`/`cpp`, focused browser scripts; no agent strict gate) and follow section 3.1 of the roadmap for the operator pattern
(spec, evaluator, plan estimate, tests, corpus line, guide block, gallery fixture, limits/cases rows). Do not commit. Everything is deterministic and
procedural; no imported assets.

## Already present (do not rebuild)

`solid.box/sphere/cylinder/cone/torus/capsule`, `union/difference/intersect`, `extrude/revolve/sweep/loft`, `round`, `chamfer`; `profile.rect/circle/polygon/
offset/difference/smooth/fillet`; `curve.polyline/bezier/circle`; `xform.move/rotate/scale/mirror/array/radial/repeat/align/attach/twist/taper/bend`;
`inst.array/repeat/radial/mirror/scatter/jitter/realize`; 19 `field.*` operators; 18 `poly.*` operators; kits `desk`, `shelf`, `window`.

## Packages (each owns its own new files; the only shared edits are one line each in `builtin/ready.rs`, `plan/estimate.rs`, `operations.rs`, `corpus.rs`)

Merge order does not matter; resolve conflicts in those shared lines by keeping both sides.

### P1: curves and profiles (feeds sweep, loft, revolve)
New `ve-graph/src/builtin/curves2.rs` and `ve-modeling/src/curves/`: `curve.arc`, `curve.helix`, `curve.spiral`, `curve.spline` (Catmull-Rom through points),
`curve.resample` (by count or spacing), `curve.offset`, `curve.fillet`; `profile.arc`, `profile.ellipse`, `profile.star`, `profile.regular`,
`profile.rounded_rect`. Caps on point counts; refuse self-intersecting offsets with a typed code. Cases: a spring from helix plus sweep, a rope, a gear
outline, an arched window profile.

### P2: placement along and over things
New `ve-modeling/src/instances/along.rs`: `inst.along_curve` (count or spacing, align to tangent, up vector, seeded jitter reuse), `inst.on_surface`
(seeded scatter over a mesh surface with slope and height filters), `inst.on_grid_of`. Reuse `scatter.rs` random stream; same 16,384 cap; refuse before
allocation. Cases: fence posts along a curve, books on a shelf, pebbles on a slope.

### P3: Manifold wrappers
New `ve-kernel/src/solid/ops2.rs` plus `cpp/src/kernel_ops2.cpp` (keep `kernel.cpp` from growing): `solid.hull`, `solid.split` and `solid.trim` (by plane),
`solid.slice` (cross-section to a profile), `solid.project` (silhouette), `solid.shell` (hollow by thickness), `solid.minkowski` (convex inputs first;
source vendored under `third_party/manifold`). Size and cost limits, refusal for non-manifold input, `cargo xtask cpp` clean, parity corpus lines.

### P4: LOD and decimation (Quest budgets)
`solid.decimate` over the existing `ve_simplify` (meshoptimizer) with ratio and error targets, preserving boundaries and material regions where it can,
and `inst.lod` (prototypes chosen by distance at draw time, not by the kernel; needs a small render-side change in `ve-render`, so coordinate with
whoever edits `extract.rs`). Report triangle counts and error for gear, creature and the aqueduct.

### P5: deformers and expressions
New `ve-modeling/src/deform2.rs`: `xform.shear`, `xform.wave`, `xform.noise_displace` (G8's deterministic noise), `xform.lattice` (bounded free-form
deformation). Separate work item: pure bounded parameter expressions (numbers, vec3, small function set, no loops, no user code) and `measure.bounds`,
`measure.volume`, `measure.count` nodes, so models can say "table height = leg height + top thickness". Needs an ADR first (it changes the language's data
flow); check whether any `math.*` node already exists before adding.

### P6: terrain
`solid.heightfield` from a noise or field expression (bounded grid mesh with normals), erosion-free first; scatter masks by slope and height (uses P2).
Showcase: terrain plus rocks and trees from kits.

### P7: kit library (no Rust; text groups under `crates/ve-modeling/kits/`, exported through `KITS`)
Furniture (table, chair, bed, sofa, lamp), architecture (wall with openings, door, stairs, railing, roof, column, arch, dome), props (books, mug, plant,
fence, crates), mechanical (gear, spring, bolt). Each exposes only deliberate parameters (colour, seed, counts). Add each to the review gallery and to
`g9_kits.rs` (footprint, seed stability, save/reload). Until P5 lands, sizes cannot be parameters, so offer a few fixed-size variants instead.

### P8: edge networks for `poly.bevel_edges`
Extend the pinned v1 contract (`ve-geometry/tests/g9_bevel_edges_contract.rs`) to adjacent edges and vertex fans so a box can be fully rounded
selectively. Requires an ADR for the new contract and keeping every current refusal test passing for the cases still unsupported.

## Priority if only some run now

P7 (fastest visible payoff, no Rust), then P1, P2, P3, then P5 expressions, P4, P6, P8. P1 and P2 together unlock splines, ropes, fences and organic scenes;
P3 unlocks real hollow and cut shapes.

## Rules for every package

Rust files under 400 lines, functions under 80, no `#[allow]`; typed diagnostics added to `diagnostics.md`; limits in `limits.md`; cases in `cases.md`;
`cargo xtask parity` for evaluator output; MCP guide block and one test; a gallery program with a rendered check through the Engine Viewer. Report which
checks ran and that the strict gate is pending.
