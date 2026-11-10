# ADR 0070: Code and ideas taken from RedGPU and Trompe

- Status: accepted (2026-10-26, owner: take everything usable from both)
- Date: 2026-10-26

## Context

Two public projects bear on the landscape, city and verification work.
- **RedGPU** (github.com/redcamel/RedGPU, MIT, JavaScript/TypeScript, WebGPU only, about 5,700 commits): an engine with a GPU-driven landscape (tile streaming, compute culling with a hierarchical depth buffer, grass and foliage scatter, octahedral impostors, stochastic texture sampling), a physically based lake with wave interaction, instanced meshes with GPU culling and LOD, plus post effects, lights, shadows and materials.
- **Trompe** (github.com/Patch-gg/trompe, Apache-2.0, TypeScript with three.js, 2 commits): textures a level from a gray blockout. It plans cameras where a player can stand, has an image model paint each view, projects the paintings back onto the UVs and bakes one texture.

At the time of this decision, worlds were generated from our own language, with no retrieved assets or external generators. [ADR 0071](0071-hybrid-world-content.md) now permits licensed captured world assets with provenance, while retaining graph-authored procedural content and Bevy as the renderer. The port decisions below still apply to the code integrations.

## Decision

1. **Vendor reference copies** under `engine/lib/redgpu/` and `engine/lib/trompe/`, pinned to the commits below, with their licences, a `VENDOR.md` each (upstream, commit, licence, what is kept, local changes: none) and an `INDEX.md` that names, per folder, what it does and whether we port it. They are read-only references; nothing links to them at build time. RedGPU commit `95aaf9a9ecb3c5ccfcfc824ccad1a6489392cf68` (2026-10-26, subset: `src/landscape`, `src/water`, `src/display/instancingMesh`, `src/display/mesh/core/LODManager.ts`, `src/systemCodeManager`, `src/postEffect`, `src/light`, `src/shadow`, `src/material`, `LICENCE.md`, `README.md`). Trompe commit `fd81fe467b1e29659597109f14bcf4e7054afa7f` (`src/core`, `test`, `scripts`, `LICENSE`, `README.md`).
2. **Port, do not link.** A port is our own code in our own crate under our own tests, written from the idea and the shader maths, and keeps the upstream licence notice in a comment where a formula or shader body is adapted (MIT and Apache-2.0 allow this; Apache-2.0 also needs a NOTICE entry, kept in `engine/lib/NOTICE`).
3. **Not taken:** Trompe's image-model painting (`providers/`, the fal.ai calls and the prompts that drive them): an external generator, against our rule that everything is generated from our own language. RedGPU's post effects (SSR, depth of field, TAA, bloom, tone mapping, anti-aliasing) are not ported: Bevy has them. Nothing from RedGPU's WebGPU-only code paths replaces a Bevy path; a compute-based path is added only behind a WebGPU feature gate, with the existing CPU path kept for WebGL2.
4. **Ports, in order** (each gets tests first, an ADR section or note, and a node or tool the model can reach):
   1. `ve_world::survey` and the `survey` tool (from Trompe `planner.ts`, `semantics.ts` roof/walkable ideas): choose cameras that see a whole level, so a model looks once instead of many times. Coverage is computed with ray casts on the evaluated geometry, not GPU probes.
   2. Stochastic (non-repeating) texture sampling for `terrain.material` and tiled procedural textures (RedGPU `stochasticSampling.wgsl`): removes visible tiling without extra texture layers.
   3. Water: the lake surface (RedGPU `src/water`: waves, reflection, refraction, foam, interaction ripples) as a Bevy material for `river.water` and `water.surface`.
   4. Grass and foliage rendering on landscapes (wind, blade shape, distance fade, octahedral impostors for far trees) beside our `ve_world::lod::impostor`.
   5. Instance LOD and culling: compute culling with indirect draw on WebGPU, CPU path kept (RedGPU `instanceCullingCompute.wgsl`, `LODManager.ts`, hierarchical depth culling).
   6. Landscape tile streaming and virtual height, normal and blend textures (RedGPU `src/landscape/core`): for very large landscapes.
   7. View-projection baking (Trompe `atlas.ts`, `composite.ts`): bake our own rendered detail or lighting into an atlas.

## Consequences
- A model gets a survey (where to look) before it gets better looks; the landscape gets non-repeating textures, better water and cheaper far vegetation.
- We carry licence notices for two more upstream projects, and the reference copies add about 5 MB to the repository.
- Each port is judged by its own tests and a look in the browser; none of them is accepted on the strength of the upstream code working in its own engine.
