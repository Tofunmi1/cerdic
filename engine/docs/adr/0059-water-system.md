# ADR 0059: Water system (rivers, lakes, ocean, interaction)

- Status: proposed (design from the code survey and research of 2026-10-21; nothing here is measured on a headset yet)
- Date: 2026-10-21

## Context

What exists: `water.surface` (ADR 0038) is a bounded slab with a clock-driven vertex ripple and a translucent colour; the Bevy path (`ve-bevy`) maps it to a plain material and does not read the water recipe. The erosion module's hydraulic pass is a real shallow-water simulation, but it shapes terrain, not pictures. The river module (ADR 0056 family) gives a static flow field, water height and masks, with nothing drawing them.

What the vendored trees offer:
- **Bevy** (`lib/bevy`): `ExtendedMaterial` (our pattern), environment-map specular, atmosphere, distance fog, volumetric fog, depth prepass, `ssr` (screen-space reflections), `transmission` (a screen copy), SSAO, decals. Several are compute-based or need a screen copy: none of that runs on the **WebGL2** build, which is the WebXR (Quest) path, where we already replaced SSAO and auto-exposure with fragment passes (`ve-bevy/src/ao.rs`, `exposure.rs`).
- **Blender** (`lib/bl`, sparse): `intern/` holds only guardedalloc, clog, eigen, atomic, utfconv and iksolver; **`intern/ocean` is not vendored**. Blender's ocean modifier is an FFT simulation (Phillips, Pierson-Moskowitz, JONSWAP, Texel-Marsen-Arsloe spectra; displacement, normals and an "extra data" grid that drives foam; parameters such as choppiness, wind speed, fetch, depth, damping) ([manual](https://docs.blender.org/manual/en/2.83/modeling/modifiers/simulate/ocean.html), [API](https://docs.blender.org/api/4.2/bpy.types.OceanModifier.html)). Its source needs an FFT library we would have to port, so we take its **parameters and spectra as the reference** and write our own FFT (ADR 0003, one pure module).

Research: shipped games render rivers with **flow maps** (Valve: two layers scrolled along a flow field, half a phase apart, to hide the restart and the tiling) ([Portal 2 water](https://www.slideshare.net/slideshow/siggraph-2010-water-flow-in-portal-2/4899875)); open water with Tessendorf-style FFT cascades, foam from the displacement Jacobian going negative ([luma.gl spectral ocean, WebGPU](https://luma.gl/docs/api-reference/experimental/spectral-ocean-simulation)); interaction with a small local heightfield wave simulation (128 by 128 ran over 30 fps even in software) ([Clemson](https://jtessen.people.clemson.edu/reports/papers_files/Interactive_Water_Surfaces.pdf)). From general knowledge, not verified here: on tile-based mobile GPUs a screen-copy refraction is expensive, so mobile water fakes refraction and takes depth from data instead of the scene.

## Decision

1. **One water material, three surfaces.** A Bevy `ExtendedMaterial` (`WaterExtension`, vertex and fragment WESL like the terrain material) draws every water surface; what differs is the mesh and the data it carries:
   - **River and stream:** a strip mesh along the carved channel (from `river::carve_river`) at the computed water height; per vertex it carries flow direction, depth and shore factor in the vertex colour channels (ADR 0056 vertex channels; Bevy's mesh colour attribute is the transport until those land).
   - **Lake:** a flat or gently waved sheet at a level; depth from the terrain below (`shore_masks`, baked per vertex or per tile into the same channels).
   - **Ocean:** a large camera-following grid with waves, below.
2. **Depth comes from data, not from the scene.** The shader never samples the scene depth or a screen copy: absorption colour, soft shore edge and foam use the per-vertex depth baked from the terrain. This works on WebGL2 and in VR, costs no framebuffer copy, and is deterministic. Refraction is faked by tinting and bending the bed colour by depth. On the WebGPU high tier an optional scene-depth term and Bevy SSR add local reflections (trees in the water); they are never required.
3. **Rivers: flow-map shading.** Two phases of procedural detail normals (gradient noise, no texture assets) scrolled along the flow direction, half a phase apart and cross-faded; speed from the baked flow speed; depth-based colour from clear shallows to dark deeps; foam from shore factor, speed and a noise threshold; fresnel reflection from the environment map (sky and probes), sun glint from Bevy's lighting.
4. **Open water: waves in two tiers.**
   - Low and medium tiers: sum of 4 to 8 Gerstner waves in the vertex shader (cheap, no textures; direction, wavelength, steepness from the wind parameters), foam from crest steepness.
   - High tier: **FFT cascades computed on the CPU** in WASM (module `ocean` of `ve-world`: Phillips, JONSWAP and TMA spectra after Blender's options, three inverse FFTs per cascade, displacement plus Jacobian foam), uploaded as small textures (128 or 256 squared, one to three cascades, updated at 30 Hz or less). No compute shader is needed, so it runs on WebGL2; a GPU compute path can follow on WebGPU with this module as the reference. Cascade tiling is hidden by differing wavelength bands and a large-scale noise.
5. **Interaction: a local ripple window.** Module `ripple` of `ve-world`: a 128 by 128 heightfield wave simulation (two buffers, damping) around the camera and moving objects, written as a normal-map texture the shader adds. CPU in WASM first, measured before widening.
6. **Underwater and edges.** When the camera is below a water level the viewer sets distance fog to the water's absorption colour; shallow terrain gets a cheap caustic term in the terrain shader driven by a water-level uniform; shores darken and gloss wet ground (`shore_masks` wet rock and beach masks).
7. **Physics reads the same data.** A `WaterField` query (surface height, flow vector at a world point) over river meshes, lake levels and the ocean's Gerstner sum (analytic, CPU), for buoyancy and current in `ve-physics` later.
8. **Language.** Everything is nodes an LLM can write: `river.carve` (terrain edit plus water surface), `water.lake`, `water.ocean`, `water.material` (colour, clarity, foam, roughness, wave parameters), with the quality tier a viewer setting, not authored data.
9. **Tiers and budgets.** Low (baked depth colour, one normal layer), medium (flow-map two phase, foam, Gerstner), high (FFT cascades, ripples, WebGPU extras). Budget target for the water fragment and vertex work on a Quest-class GPU is set by measurement in the first milestone; until then no claim is made.

## Consequences

- Rivers read as moving, deep and shallow water with foam and reflections at a cost bounded by fragment work only; no screen copies, no compute, so the WebXR WebGL2 path gets the same look as desktop minus the optional high-tier extras.
- Open water gets Blender-compatible controls without importing Blender code.
- The vertex-channel plumbing of ADR 0056 becomes a hard dependency of water (flow and depth per vertex); the colour attribute is the interim transport.
- Not covered: fluid-accurate dynamics (waterfalls, floods that change the terrain), volumetric spray, caustics on arbitrary objects, planar or screen-space reflections in VR.

## Alternatives considered

- Per-pixel scene-depth soft edges and Bevy `transmission` refraction on every tier: simplest, but needs a screen copy and a depth prepass for blended surfaces, costly on tile-based GPUs and unavailable on WebGL2.
- A GPU compute FFT first: fastest at scale, but WebGL2 (the VR path) has no compute; CPU FFT of a few small cascades is affordable and portable.
- A full shallow-water or SPH simulation for rivers: physically richer but far beyond a VR frame budget; flow maps give the look, the ripple window the interaction.
- Importing Blender's `intern/ocean`: brings a library we would have to port anyway; the parameters are what users need.

## Evidence needed (first milestone)

Reference scenes R5 (river through a valley with a bend, rapids and shore), R6 (lake with beach and cliff), R7 (open sea with swell); screenshots reviewed by eye; frame time of the water shader per tier in the browser harness; CPU time of a 256 squared cascade update in WASM (target under 2 ms); a native-versus-WASM hash of the ocean and ripple fields.

## Implementation follow-up: basin simulation

The `water.fill` workflow now has an integrated bounded 3D PBF reference path when its existing `sim=true` setting is enabled. The basin mask/depth channel seeds particles and supplies a sampled bed; `water.disturb` maps into a 3D column impulse and the existing visible surface ripple. `water.status` identifies the solver and particle count. River and open-water surfaces continue using the shallow-water height field. This is a CPU prototype with a capped particle budget; the visible surface still uses the height-field renderer, not particle surface reconstruction. See `handoff-water-3d.md` for remaining performance, rendering, and device-validation work.
