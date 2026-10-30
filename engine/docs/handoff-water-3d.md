# Handoff: true 3D liquid simulation

## Goal

Integrate bounded volumetric liquid simulation into the existing `water.fill` workflow. Do not add another water operator or user-facing mode. `water.fill(..., sim=true)` selects the 3D solver internally and keeps the current visible surface and `water.height` API. The same disturbance drives the 3D particles and the visible surface ripple. Do not describe the existing `WaterSim` height field itself as volumetric: it evolves displacement on an XZ grid.

## Existing workflow, geometry-aware solver

- Keep `water.fill` as the authored entry point for terrain basins, with its existing parameters and simulation setting.
- A filled basin carries its basin mask and depth data into runtime; when its existing `sim=true` setting is enabled, runtime selects the bounded 3D solver and routes the projected surface through the same water shader and `water.height` API.
- Rivers and open water keep the height-field/flow-map path because they are authored surfaces rather than filled basins. No extra LLM command or user-facing solver toggle is introduced.
- Scene documents store only configuration and initial conditions. Particle positions, grid velocities, pressure, and solver scratch are runtime state. Any future checkpoint format must be versioned and separate from ordinary scene graph commands.

## Solver recommendation

Use a particle/grid hybrid for the main 3D mode:

1. Liquid marker particles carry volume and move with the flow.
2. A staggered 3D MAC grid stores velocity and pressure. Transfer particle velocity to the grid, apply gravity and solid boundaries, project velocity to near-zero divergence, then transfer the corrected velocity back and advect particles.
3. Use PIC/FLIP blending (and evaluate APIC if grid transfer loses too much angular momentum) to balance stability and retained motion.
4. Reconstruct the visible liquid with a screen-space fluid pass: particle depth, bilateral smoothing, thickness, normals, and shaded composite. Do not CPU-mesh every frame. Support a lower-cost particle renderer as a diagnostic/fallback, and generate a polygon mesh only for explicit bake/export.
5. Spawn bounded spray/foam/bubble particles from high curvature, impact, and fast motion as a later visual pass; they do not participate in bulk liquid volume.

This choice handles overturning surfaces and disconnected splashes, unlike a height field. Position Based Fluids is a useful stable prototype/reference when a bounded CPU/WASM experiment is needed, but it should not be mistaken for the final high-throughput solver. PBF's authors describe density constraints and stability for larger real-time steps ([project and paper](https://blog.mmacklin.com/project/pbf/)). Bridson's course notes cover the 3D incompressible flow, pressure projection, boundaries, and particle/grid methods ([notes](https://www.cs.ubc.ca/~rbridson/fluidsimulation/fluids_notes.pdf)).

## Runtime and language boundary

Keep the existing typed water commands rather than exposing solver internals or raw particle buffers:

- `water.fill@1`: terrain basin, waterline, and existing simulation controls.
- `water.disturb`: stable water part name, world position, radius, and amount; runtime maps this to a 3D impulse for filled basins and a height-field disturbance for surface water.
- Sources, drains, and arbitrary obstacle commands remain deferred until basin contacts are validated.

Validate bounds, cell counts, particle count, CFL/time-step limits, and total memory before allocating. Commands describe intent; the runtime owns solver state. LLM edits target stable node/volume IDs, and manual XR/controller interactions emit the same impulse command.

Use a backend boundary with an engine-independent configuration/state contract. Start with a small deterministic CPU reference for correctness tests. Implement the production solver as WebGPU compute integrated with the existing Bevy render device and WASM runtime. Capability detection must select the 3D mode only where its compute and render path is supported; retain the existing height-field mode elsewhere. Keep device-specific buffers and shaders out of serializable scene data.

## Rendering and collision milestones

### V0: contained volume slice

- Box of water in a box tank, gravity, floor/wall collision, stable 3D neighbor/grid lookup, fixed substeps and hard particle/grid budgets.
- WebGPU solver and screen-space surface; visible mesh/particle debug view; orbit camera and one controller/hand impulse.
- Status: particle count, active grid cells, substep count, divergence residual, dropped time, frame cost, and bounds.
- Save/reload the authored setup. Runtime state can restart from its declared initial fill.

### V1: useful scene interaction

- Static collision meshes from existing solid outputs; moving rigid-body obstacles; XR hand and object impulses; multiple volumes with isolated IDs.
- Transfer momentum between water and objects only after one-way obstacle coupling is stable.
- Add export/bake as a separate command producing a static mesh, with explicit resolution and triangle budget.

### V2: quality and scale

- Adaptive sparse grid or tiled active region, APIC evaluation, better curvature/surface tension, foam and spray, multiple fluid presets, and VR performance profiles.
- Profile GPU time and memory on representative desktop and headset-class hardware; lower resolution/budgets gracefully instead of silently freezing.

## Validation gates

- Unit tests for 3-axis gravity/advection, conservation within stated tolerances, divergence reduction, obstacle collision, deterministic stepping, stable IDs, bounds, invalid budgets, and finite outputs under small/large time steps.
- CPU reference and WebGPU backend agree on small fixtures within documented tolerances; don't require bit-identical floating-point GPU output.
- Browser tests show a falling blob, a tank impact, a surface that overturns/splashes, an obstacle deflection, and repeatable save/reload of initial conditions.
- Capture performance and memory as a function of particle count and grid resolution. Establish an explicit default suitable for desktop and a lower XR preset.
- Test WebGPU capability fallback. Emulator/headless evidence does not replace a headset performance check for release.

## Fork/reference audit

- [fluid-webgpu](https://github.com/dr-chunshui/fluid-webgpu) describes GPU SPH/PIC-FLIP and several 3D surface renderers, including screen-space fluid and marching cubes ([repository](https://github.com/dr-chunshui/fluid-webgpu)). Treat it as an implementation reference first: inspect the exact files, dependency licenses, history, and shader/data contracts before copying or vendoring. Its standalone Vite application is not an engine plugin.
- [GridFluidSim3D](https://github.com/rlguy/GridFluidSim3D) is a PIC/FLIP reference with a Zlib license and explicit 3D MAC-grid/pressure/surface-reconstruction pipeline. Its C++/OpenCL-oriented implementation is not a direct browser/WASM dependency; port algorithms selectively only after isolating the required code and checking each file's license/provenance.
- Prefer a clean integration with this engine's typed commands, Bevy render device, and scene lifecycle over importing another app shell. Record any reused code in `third_party/VENDOR.md` and run the repository license gates.

## First implementation order

1. Add this architecture decision to the engine docs and keep the current water fix as the 2.5D path.
2. Define bounded config and backend interfaces plus deterministic CPU fixtures.
3. Implement and validate the small 3D solver against those fixtures.
4. Wire `water.fill` to the bounded 3D runtime solver and report solver and particle telemetry through `water.status`.
5. Replace CPU reference stepping with a WebGPU solver and screen-space surface; compare it against CPU fixtures.
6. Add controller impulses, obstacle coupling, surface-quality work, and headset profiling.

Do not begin with arbitrary fluid/solid coupling, foam, an unconstrained particle count, or a mesh export that runs every frame.

## Current implementation status

`crates/ve-world/src/water_sim/volume.rs` contains a small CPU PBF reference kernel. `water.fill` marks its existing material for volumetric simulation; when the existing simulation setting is enabled, runtime samples particles from the basin depth mask, constrains them against a sampled terrain bed, and advances 3D density projection alongside the existing surface-wave solver. `water.disturb` applies both a column impulse to the 3D particles and a height-field disturbance for the visible propagated ripple. `water.status` reports the 3D solver, elapsed time and particle count. Rivers and other open surfaces remain on the height-field path.

This is an integrated prototype, not production fluid. The CPU reference is capped at 4,096 particles and is not a performance target. The visible free surface still uses the height-field renderer; particles do not yet reconstruct the rendered surface, show overturning liquid, or create spray. Terrain contact is sampled at the basin grid resolution. WebGPU acceleration, screen-space surface reconstruction, larger basins, and headset performance remain open.
