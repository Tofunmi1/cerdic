# Handoff to Codex: `ve-gi`, a diffuse irradiance probe baker (global illumination for the viewer)

Read `engine/AGENTS.md` first and follow it (no commits, no `Co-Authored-By` or tool attribution; Rust rules: no `unwrap`/`expect`/`panic!`/`println!` in library code, no `#[allow]` (use `#[expect(lint, reason = "..")]` on the smallest item), files under 400 lines, public items documented, deterministic output). From `engine/`:
`cargo test -q -p ve-gi`, `cargo clippy -q -p ve-gi --tests`, `cargo xtask rules`. **Create only the new crate `crates/ve-gi/`** (the workspace picks up `crates/*` by itself) and write the Status into **this file**. Do not touch any other crate.

## Shared tree: hard rules
- **NEVER run `git checkout`, `git clean`, `git stash`, `git restore`, `git reset`.** Commit nothing, create no branches (the manager commits on `main`).
- The manager is editing `crates/ve-render` and `crates/ve-runtime` right now; you do not touch them.

## What it is for
The viewer's lighting has a constant sky ambient and no bounce light. This crate computes, on the CPU and in the browser (pure Rust, wasm-safe, no threads, no `std::time`, no randomness from the OS), a **3D grid of light probes**: at each grid point the diffuse irradiance arriving from every direction, stored as first-order spherical
harmonics, including sky light, sunlight bounced off surfaces, emissive surfaces and a few bounces between surfaces. The renderer (the manager's job) uploads the grid as 3D textures and the forward shader looks it up. It uses the BVH ray caster in `ve-measure` (`ve_measure::{Mesh, Bvh}`; read `crates/ve-measure/src/bvh.rs` and `geom.rs`: `Bvh::build(&Mesh)`,
`bvh.raycast(&mesh, origin, dir) -> Option<RayHit { distance, triangle, point, normal }>`, the normal is the triangle's, not normalised and not flipped).

## The API (exactly these names; add what you need privately)
```rust
/// A surface's response, in linear colour. `emissive` is radiance leaving the surface (same units as `Sky` and `Sun`).
pub struct Material { pub albedo: [f32; 3], pub emissive: [f32; 3] }
/// The sun: `direction` is a unit vector from a surface TOWARD the sun; `irradiance` is what a surface facing the sun receives (a diffuse white surface facing it sends back `albedo / PI * irradiance`).
pub struct Sun { pub direction: [f32; 3], pub irradiance: [f32; 3] }
/// The sky seen from anywhere: radiance looking straight up, at the horizon and straight down; in between, linear in the direction's y: `mix(horizon, zenith, y)` for y >= 0 and `mix(horizon, ground, -y)` below.
pub struct Sky { pub zenith: [f32; 3], pub horizon: [f32; 3], pub ground: [f32; 3] }
/// The geometry: one triangle soup (world space) and a material index per triangle. Two-sided.
pub struct Scene { pub positions: Vec<[f32; 3]>, pub indices: Vec<u32>, pub triangle_material: Vec<u32>, pub materials: Vec<Material>, pub sun: Option<Sun>, pub sky: Sky }
/// An axis-aligned grid of probes: probe (i, j, k) is at `origin + spacing * (i, j, k)`; index = i + dims[0] * (j + dims[1] * k).
pub struct GridSpec { pub origin: [f32; 3], pub spacing: f32, pub dims: [u32; 3] }
impl GridSpec { pub fn probe_count(&self) -> usize; pub fn position(&self, index: usize) -> [f32; 3];
    /// A grid covering the box `min..max` with about `target_spacing` between probes, padded by one cell on every side, at most `max_per_axis` (4 to 64) probes along an axis and 65 536 in all (the spacing grows until it fits; one spacing for all axes).
    pub fn fit(min: [f32; 3], max: [f32; 3], target_spacing: f32, max_per_axis: u32) -> Result<GridSpec, GiError>; }
/// Irradiance over pi at a probe, as the exact first-order reconstruction: for a unit normal n, E(n)/pi = max(dc + x*n.x + y*n.y + z*n.z, 0) per colour channel.
/// (A uniform sky of radiance L gives dc = L and x = y = z = 0. Dividing by pi means a diffuse surface's outgoing radiance is just albedo * E/pi.)
pub struct ProbeSh { pub dc: [f32; 3], pub x: [f32; 3], pub y: [f32; 3], pub z: [f32; 3] }
pub struct ProbeGrid { pub spec: GridSpec, pub probes: Vec<ProbeSh>, pub valid: Vec<bool> }
impl ProbeGrid { /// Trilinear lookup at `point` for the surface normal `normal`: weight the 8 surrounding probes by the usual trilinear weights times 0 for invalid probes, renormalise; if all 8 are invalid return the sky's unoccluded irradiance... (store it in the grid: add a private field) .
    pub fn sample(&self, point: [f32; 3], normal: [f32; 3]) -> [f32; 3]; }
pub struct Settings { pub rays_per_probe: u32 /* default 128 */, pub bounces: u32 /* default 2 */ }
pub enum GiError { /* thiserror: BadGrid, BadScene(String), ... */ }
/// Incremental bake so the browser stays responsive: build once, call `step` with a budget until it returns true, then `finish`.
pub struct Baker; impl Baker { pub fn new(scene: Scene, spec: GridSpec, settings: Settings) -> Result<Baker, GiError>;
    pub fn step(&mut self, max_probes: usize) -> bool /* true when finished */; pub fn progress(&self) -> f32 /* 0..1 */; pub fn finish(self) -> Result<ProbeGrid, GiError>; }
pub fn bake(scene: Scene, spec: GridSpec, settings: Settings) -> Result<ProbeGrid, GiError> /* new + step until done + finish */;
```

## The algorithm (follow this; it is chosen to be deterministic and to have closed-form answers in the tests)
- **Directions:** a spherical Fibonacci set of `rays_per_probe` unit vectors (equal-area, deterministic), the same set for every probe. Project the radiance along each direction onto L1 SH with weight `4*PI / n` each, then convert to the `ProbeSh` convention (the cosine-lobe convolution: with radiance SH `L00, L1m1(y), L10(z), L11(x)`, `E/pi(n) = L00 * Y0 * 1 + (2/3) * (L1 * Y1) . n` where `Y0 = 0.282095` and `Y1 = 0.488603`; the result must satisfy: uniform radiance L gives `dc = L`).
- **Radiance along a ray from a probe:** a miss gives the sky. A hit at point P on a triangle with unit normal N flipped to face the ray (`dot(N, dir) < 0`): the surface sends back `emissive + albedo / PI * (sun direct + indirect)` toward the probe, where *sun direct* = `sun.irradiance * dot(N, sun_dir)` if `dot > 0` and a shadow ray from `P + N * 1e-3` toward the sun is unobstructed (else 0), and *indirect* = the previous pass's grid sampled at `P + N * 0.5 * spacing` for normal N (`ProbeGrid::sample`). Count a hit whose raw triangle normal faces away from the ray (a back face) as "inside".
- **Passes:** pass 0's "previous grid" is the sky seen unoccluded (every probe: the sky's own L1 irradiance, no geometry). Run `bounces` passes (at least 1), each tracing every probe against the previous pass's result; the last pass is the output. With no triangles every pass returns the sky's irradiance.
- **Validity:** a probe is invalid if more than 25% of its rays hit back faces within 2 * spacing (it is inside a solid). An invalid probe's SH is replaced, after the pass, by the average of its valid face neighbours (6-neighbourhood, repeat up to 4 times); if it still has none it keeps the unoccluded sky value. `valid` records what was true before the fix-up.
- **No randomness, no hash maps with random order, no parallel sums**: the same input gives bit-identical output.
- Large scenes: build the `Bvh` once; `step` processes at most `max_probes` probes of the current pass.

## Tests (fixed; do not weaken any assertion; if one fails say which and why). Put them in `crates/ve-gi/tests/probes.rs` (and small unit tests beside the code if you like)
1. **White furnace:** no triangles, sky zenith = horizon = ground = `[0.7, 0.4, 0.2]`, 256 rays, 2 bounces: every probe has `dc` equal to the sky within 0.5% per channel and `x`, `y`, `z` below 0.5% of the sky's largest channel in magnitude. `sample` at any point and any normal returns the sky colour within 0.5%.
2. **Linear sky gradient:** no triangles, zenith = `[1, 1, 1]`, horizon = `[0.5, 0.5, 0.5]`, ground = `[0, 0, 0]`, 512 rays: the sky is exactly `0.5 + 0.5 * y`, so each probe has `dc = 0.5` and `y = 1/3` (`= 0.5 * 2/3`) within 1% and `x`, `z` below 0.01.
3. **A black ground plane hides the lower half:** a 400 m by 400 m plane at y = 0 (two triangles, albedo 0, emissive 0), sky uniform `[1, 1, 1]` (zenith = horizon = ground), a grid of one probe at (0, 1, 0) (use `GridSpec { origin: [0,1,0], spacing: 1.0, dims: [1,1,1] }`), 512 rays: `dc` within 2% of 0.5 and `y` within 2% of 0.5 (the upper hemisphere is open, the lower is black, so irradiance over pi is 1 facing up and 0 facing down). `sample` at the probe with normal up returns 1.0 within 3% and with normal down returns below 0.03.
4. **Bounce from a sunlit white plane:** the same plane with albedo `[0.8, 0.8, 0.8]`, sky black, sun direction `[0, 1, 0]` and irradiance `[3, 3, 3]`, 2 bounces, one probe at (0, 1, 0), 512 rays: the plane radiates `0.8 / PI * 3 = 0.7639` toward the probe over the whole lower hemisphere and the sky is black, so `dc` is within 3% of 0.382 and `y` within 3% of -0.382; `sample` with normal down is within 4% of 0.7639 and with normal up is below 0.03.
5. **An emissive plane:** the plane with albedo 0 and emissive `[2, 1, 0.5]`, sky black, no sun, one probe at (0, 1, 0), 512 rays: `dc` is within 3% of `[1.0, 0.5, 0.25]` and `y` within 3% of `[-1.0, -0.5, -0.25]`.
6. **Shadowed floor:** the white plane (albedo 0.8) lit by the straight-down sun (irradiance 3, sky black), plus a closed 2 m cube of albedo 0 centred at (0, 3, 0) (12 triangles, outward windings; two-sided anyway): a probe at (0, 1.0, 0) just under the cube sees the cube above (black) and the plane below: its `y` is more negative than the same probe at (10, 1.0, 0) away from the cube by at least 0.05 absolute, because the plane under the cube receives no sun and so radiates less. (Use two 1-probe grids.)
7. **A probe inside a solid is invalid:** the closed 2 m cube of albedo 0.5 centred at the origin, sky `[1, 1, 1]` uniform: a 3 by 3 by 3 grid with spacing 1.5 centred at the origin has its centre probe invalid and at least one corner probe (3 m from the origin, outside the cube) valid; the centre probe's SH after fix-up is finite and each of its `dc` channels lies between 0 and 1.5 (the unoccluded sky is 1).
8. **Determinism:** baking the test 6 scene twice gives bit-identical `probes` (compare with `to_bits`), and baking it in `step(7)` slices gives exactly the same result as `bake`.
9. **`GridSpec::fit`:** `fit([-5,0,-5], [5,4,5], 1.0, 64)` has `spacing` 1.0, `origin` at or below `[-6, -1, -6]` and a grid whose far corner (`origin + spacing * (dims - 1)`) is at or above `[6, 5, 6]`; `fit` of a box 1000 m wide at spacing 0.5 with `max_per_axis` 16 gives at most 16 probes along each axis and a larger spacing; `fit` with a negative or NaN input is `Err`.
10. **`sample`:** on a 2 by 1 by 1 grid whose two probes have `dc` `[1, 0, 0]` and `[3, 0, 0]` (set them through a test-only constructor or a public field), a lookup at the midpoint with any normal gives red 2.0 within 1e-5; at a probe's own position it gives that probe's value; outside the grid it clamps to the border probe (no panic, finite result).

## Acceptance
`cargo test -q -p ve-gi` passes (report counts), clippy clean for the crate, `cargo xtask rules` passes, every file under 400 lines, the crate has no dependency except `ve-measure`, `glam` (optional) and `thiserror`. Performance note for the Status (not a test): time `bake` of a 16 by 8 by 16 grid with 128 rays and 2 bounces on a plane plus a cube, in release (`cargo test --release`, print the elapsed time with `std::time::Instant` in a `#[ignore]`d test, which native tests may use).

## Status (Codex: fill in with real numbers for each of the ten tests, then stop)

Implemented `crates/ve-gi/` with deterministic sampling, incremental passes, BVH rays, interpolation,
validity fix-up and the ten acceptance cases. `cargo test -q -p ve-gi`: 9 passed, 1 failed, 1 ignored
(the ignored release timing case). Case results:

1. White furnace: passed.
2. Linear sky gradient: passed.
3. Black ground plane: passed.
4. Sunlit white plane: passed.
5. Emissive plane: passed.
6. Shadowed floor: failed its specified direction comparison. Actual `y` was `-0.1816004` under
   the cube and `-0.38196683` away. With the specified SH convention, reducing radiance from the
   lower hemisphere moves `y` toward zero; the assertion asks for it to become more negative. The
   fixture includes the closed black cube and the assertion was left unchanged.
7. Interior probe invalidity: passed.
8. Determinism and sliced stepping: passed.
9. Grid fitting: passed.
10. Sampling and clamping: passed.

`cargo clippy -q -p ve-gi --tests` exits 0 but emits Clippy warnings (35 library warnings and 18
test warnings); clippy cleanliness is not yet met. `cargo xtask rules` passes. Release timing for
the implemented ignored timing case (the current fixture is a plane, without the requested cube):
510.059583 ms for 16x8x16 probes, 128 rays, 2 passes. Only the new crate and this Status section
were changed. The strict workspace gate remains pending per `engine/AGENTS.md`.
