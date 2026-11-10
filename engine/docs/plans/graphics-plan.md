# Graphics plan (2026-11-03)

Status: proposed. Not started. Owner reads this first; nothing below is built until they agree.

Scope: everything that decides how a frame looks and how fast it appears, for the browser on desktop (WebGPU first; WebGL2 only where the cost is small). Everything stays procedural and reachable from the node language and tool calls (`everything-via-language`), so a model can ask for the look, not hand-place it. Take as much of the vendored Bevy 0.20 as possible and optimise last (`bevy-renderer-spike`).

## 1. Where we are (verified 2026-11-02/03)
- **Have:** physical sky and atmosphere (`ve-sky`, Bevy atmosphere), cascaded sun shadows, ambient occlusion (GTAO/SSAO), bloom, tone mapping with colour grades, SMAA/MSAA, distance haze, `EnvironmentMapLight`, traced light probes (`ve-gi`, `IrradianceVolume`), `VisibilityRange` bands, frustum culling, normal and height maps (tangents fixed 2026-11-02), emissive channel, texture programs, CC0 photo textures.
- **Present in vendored Bevy but unused by us** (found by listing `lib/bevy/crates`): screen-space reflections (`bevy_pbr/src/ssr`, unsupported on WebGL2), volumetric fog and light (`volumetric_fog`), forward and clustered decals (`decal`), reflection probes and environment-map generation (`light_probe/generate.rs`, `environment_filter`), contact shadows, transmission, order-independent transparency (`bevy_core_pipeline/src/oit`), auto exposure, depth of field, motion blur (`bevy_post_process`), parallax mapping, meshlets.
- **Weak, seen in renders:** glass, wet roads, water and metal show only sky colour; shaded sides read flat blue-grey; windows and balconies are painted relief; foliage is cards; particles are flat discs; skinned characters tear (head holes, misplaced eyes); the Paris road and pavement showed as flat brown (cause not found); a full city takes about 7 minutes to build plus about 4 for light probes and draws about 4.7M triangles.

## 2. Principles
1. **Look is a parameter, not a fix.** Each effect is a node or a parameter on `scene.environment`, `solid.material` or `scene.sky`, with a default that is on and cheap, so a model gets good frames without asking.
2. **Spike before building.** Each phase starts with a one-day spike in the vendored Bevy on our materials (our `VeMaterial` and the toon, detail, terrain and water shaders may not support Bevy's deferred or prepass features). A spike ends in a yes/no and a measured frame cost.
3. **Every visual feature has a checkable property and a reviewed render.** Property tests run in the live browser scripts (pixel statistics, e.g. mirrored-sky correlation on a glass pane); an eye-reviewed render goes in the report. No claim without the render.
4. **Measured cost.** Frame time and drawn triangles are recorded per phase on the desktop GPU in Chromium (median and p90); a feature that costs more than 2 ms at 1080p needs a quality switch.
5. **Judges' machines vary.** Every effect has a `quality` level (off, low, high) chosen automatically from a measured frame time, with the options visible in the status. Default is the lowest level that still looks right.

## 3. Phases (each ends in a demonstration; order is by how much it changes a screenshot)

### G1. Reflections (largest visible gap)
- Spike: reflection probes from the sky (`GeneratedEnvironmentMapLight`/`LightProbe`) on our materials; SSR on a glass pane and a wet road (needs deferred prepass: check whether `VeMaterial` can join it, else write the ray-march in our forward shader, using the depth prepass).
- Build: `scene.environment` gains `reflections` (off, probe, probe_ssr); `solid.material` already has metallic and roughness, so no new material parameter. Water gets the sky and nearby parts reflected (planar or SSR). Wet-surface response: a `wetness` texture channel that lowers roughness and darkens albedo.
- Tests: a glass pane facing a bright sky and a red wall shows the sky colour; a wet road mirrors a lit window; probe and SSR agree within a tolerance on a flat mirror. 
- Demo: the Paris district street view with glass, wet road and a river that mirror their surroundings.

### G2. Air and light (atmosphere in the frame)
- Spike: Bevy `VolumetricFog` and `VolumetricLight` with our sun, on WebGPU; cost at 1080p.
- Build: `scene.environment` parameters `fog_density`, `fog_height`, `light_shafts`; contact shadows on; auto exposure so interiors and exteriors both read; night sky (stars, moon) as part of `scene.sky`; weather: `scene.weather` (rain, snow, mist) with surface wetness feeding G1.
- Tests: shaft brightness behind an occluder; exposure keeps mean luminance of a test scene inside a band at noon and at night.
- Demo: a dawn street with shafts between buildings, then the same street in rain at night.

### G3. Surface depth
- Decals: `fx.decal` node (forward decals): dirt, cracks, markings, bullet holes, with a texture program; placement by position and normal, or scattered by a pattern.
- Layered material: `solid.layers` (a list of layers, each a texture program and a driving field: slope, height, exposure, noise) for moss, snow, rust, wetness; extends `ADR 0060` fieldable parameters.
- Parallax or real displacement for near surfaces (Bevy parallax mapping first, tessellated geometry only if parallax fails by eye).
- Subsurface-style translucency for leaves, skin and wax (a cheap wrap-lighting term in our shaders; spike transmission first).
- Real facade depth in the city generator: recessed windows with reveals, balconies, shop fronts, signage as mesh on near buildings (`city.buildings` detail levels by distance band).
- Demo: a street where walls stain by exposure, cracks and posters sit on walls, windows and balconies have shadows from their own depth.

### G4. Foliage and ground at scale
- GPU grass and undergrowth from geometry with density maps, wind and clumping (the approach in the plan's M9); imposters for distant trees; ground detail that does not tile visibly.
- Terrain clipmaps or tiled streaming with a splat plan; carved roads and paths.
- First task: find why the Paris road and pavement rendered flat brown (texture program, UV scale, or lighting) and fix it with a test.
- Demo: a meadow to the horizon with grass blades, trees, a path and a stream, holding the frame budget.

### G5. Characters
- Fix skinned tearing: reproduce on the zombie, write a test that measures mesh holes after skinning (mesh topology and per-triangle area change against rest), then fix the cause (dual-quaternion path, eye attachments, bone limits).
- Face and hair (deferred earlier by the owner; pick up here): eyes, brows, ribbon or card hair, hands.
- Animation without hand-keyframing: clip library (idle, walk, run, attack, hurt, die) and retargeting between our skeletons; foot-to-ground IK.
- Demo: three different characters walk, run, attack and die with the same clips, no tearing, readable faces.

### G6. Effects
- Soft particle sprites, trails and ribbons, fire, smoke, sparks, splashes as `fx.*` nodes with presets; GPU particles where the spike shows they are needed.
- Camera look as parameters: depth of field, motion blur, lens effects, behind a `scene.camera` node and off by default.
- UI and text: a real text node (outline font path or SDF generated from our own glyph outlines; no font assets pulled), in-world and screen panels, menus.
- Demo: a muzzle flash, an explosion with smoke and sparks, a campfire, and a title menu.

### G7. Scale and speed
- Streamed tile generation: evaluate world tiles on demand around the camera instead of the whole document up front (M8 in the game plan). Target: first frame under 20 s for any size, full detail arrives progressively.
- Generic LOD for any mesh (mesh simplification from `ve-geometry`, bands as already done for placed copies), occlusion culling (spike Bevy's, fall back to a CPU depth pyramid), cached static shadow maps, texture compression, pipeline warm-up during load.
- Light probes: bake in the worker and stream; bake time for the NYC world from about 4 minutes to under 1.
- Demo: the full 760 x 640 NYC district reaches interactive in under 30 s and stays above 60 fps on the desktop GPU.

### G8. Look checks (the model can see)
- `check look`: pixel-statistic checks a model can run on its own render: flat-shading detector (low local contrast), blue-cast detector, missing-reflection detector (glass/water correlation with sky), exposure band, banding, empty horizon.
- Look presets: `look.paris`, `look.noir`, `look.overcast`, ... as saved parameter sets (sky, environment, grade, fog), each with a reference render in the docs.
- Demo: a model builds a street from a one-line prompt, runs `check look`, fixes what it reports, and the result passes.

## 4. Order and dependencies
G1 and G2 first (they share the environment node and the deferred/prepass question; spike both before building either). G4's road fix can happen any time and is small. G3 and G5 are independent of G1/G2 and can run in parallel with them. G6 follows G2. G7 is long and starts with the tile-generation design; it can run beside G3. G8 comes last because it measures what the others add.

## 5. Risks and open decisions
- Bevy's SSR and volumetric fog may need the deferred or prepass path that our custom materials skip: the spike decides whether we adapt the materials or write our own forward-shader versions.
- WebGPU only for G1's SSR and volumetric effects (WebGL2 falls back to probes and haze); confirm the fallback is acceptable.
- A real text path needs a glyph source: proposal is to generate glyph outlines procedurally (no font files), which is a design decision for the owner.
- Frame cost on weak GPUs is unmeasured beyond the desktop; the auto-quality rule needs a calibration pass.

## 6. Process
Every task follows the task template in `game-world-system.md` section 9: tests first, the five-part slice (node, tool visibility, diagnostics, guide example, tests), report in chat with what was not measured, and a dated row in section 12 of that file. Each phase's spike result is written here (section 7) before building.

## 7. Spike log
(empty)
