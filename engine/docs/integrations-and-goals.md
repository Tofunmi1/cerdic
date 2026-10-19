# What we are building, and what we have integrated so far

## Goal
An AI-native 3D/VR authoring engine that runs in the browser (Rust to WASM), built for the Victoria VR AI Builder Hackathon 2026. A person types plain language; a model turns it into a 3D scene and working logic. The same typed commands serve the model and the person, so anything the model can do, the editor can do, and every edit is undoable and checkable.

Principles that shape every choice:
- **Everything procedural.** All geometry, materials and textures come from our own language. No imported models, textures, fonts or external 3D generators (ADR 0003). Source code (including shader text) from open projects is fine; assets are not.
- **Everything in the browser.** Tools run as WASM or WebGL2. A server is allowed only for selected heavy jobs, never by default.
- **Typed graph language.** One line per node (`name = operator@1(inputs, params)`), validated, with errors that say how to fix them. Specs, guide, tool schemas and inspector all come from one registry.
- **Missing pieces are ours to build.** GPL code is allowed (ADR 0024), with a `VENDOR.md` per vendored tree. Nothing is published without owner approval.
- **Deterministic.** Native and `wasm32-wasip1` runs of the same document give the same geometry, checked by a parity corpus.

## Architecture in one picture
Document (typed node graph) -> evaluator (Rust/WASM, in a worker) -> parts (triangle meshes, materials, lights) -> viewer (trimmed Bevy 0.20 fork on WebGPU/WebGL, WebXR) and exporters (glTF). A C++ kernel behind a C ABI does the heavy geometry. An MCP server lets a model (Claude Code) drive the editor page.

## Integrations so far

| Integration | What it gives us | Where |
|---|---|---|
| **Bevy 0.20 (trimmed fork)** | Renderer, ECS, window and input on the web; WebXR layer for headsets | `third_party/`, `crates/ve-render`, `ve-runtime`, `ve-xr-web` |
| **Manifold** | Robust solid booleans | `cpp/`, `ve-kernel` |
| **OpenSubdiv** | Subdivision surfaces | `cpp/` |
| **meshoptimizer** | Mesh simplification and optimisation | `cpp/` |
| **Clipper2** | 2D profile offsetting and booleans | `third_party/` |
| **MikkTSpace** | Standard tangent generation for normal maps | `third_party/mikktspace` |
| **Blender source (GPL)** | Polygon editing in the language: bevel, inset, extrude, solidify, poke, wireframe, subdivide, dissolve, symmetrize, decimate (LOD), unsubdivide, UV unwrap (LSCM/ABF, island packing). About 640 Blender files compiled to native and WASM, with deterministic maths, trap stubs for unreachable symbols, and element provenance (`role` attributes) so a model can select "the cap" or "the ring" without indices | `cpp/blender/`, `third_party/blender`, `ve-kernel`, `ve-geometry`, `ve-graph` (`poly.*` nodes) |
| **Eigen, fmt** | Dependencies Blender code needs | `third_party/eigen`, `third_party/fmt` |
| **glTF export** | Our own exporter: welded meshes, crease-angle normals, PBR materials, instancing; validated by the Khronos validator. Button in the Document panel and an MCP tool | `src/engine/gltf.ts` |
| **MCP test server** | A model drives the editor with typed tools: write program, evaluate, measure (volume, size, triangles), snapshot, export `.glb`. A guide teaches the language and the Blender-backed tools | `tools/mcp/server.mjs`, prompt eval harness `tools/mcp/eval-prompts.mjs` |
| **Material Maker (MIT)** | A library of 398 procedural texture nodes (noises, patterns, filters, colour ops, 3D object-space textures). Our own compiler turns a node graph into one GLSL ES 3.0 shader, baked in the browser with WebGL2 | `third_party/material-maker`, `src/engine/texture/` |

### Blender integration, in short
We pull Blender's algorithms, not its application. They are compiled with our flags, run through the same C ABI, and exposed as graph nodes with bounded parameters, caps and cancellation checks before and after the call. Measured: bevel of a 540k-face mesh takes 232 ms native and 478 ms in WASM. Native and WASM outputs match bit for bit on the corpus. Blender's bundled assets (brushes, HDRIs, fonts, models) are not used.

### Procedural materials and UVs, in short
- **Compiler:** subgraphs, generic nodes, buffers and switches are flattened; parameters, gradients, curves and inputs become GLSL. 357 of 398 nodes compile and link on the GPU. The rest need meshes, materials or multi-pass buffers.
- **Text language:** `n = fbm(scale_x=6)`, `albedo = colorize(n, gradient=[0:#3b2210, 1:#c98a4b])`; channels are albedo, roughness, metallic, height (becomes a normal map), emissive. Mistakes name the line and the fix. A catalog generated from the library keeps the guide in step with what compiles.
- **Baker:** 2D textures draw in UV space. 3D textures read an object-space position map, so patterns stay continuous across UV seams (checked on a sphere).
- **UV generation:** `poly.unwrap_uv` exposes Blender's LSCM/ABF++ flattening and island packer. A model can select seam edges in the typed graph; UVs remain per-corner through polygon triangulation, the WASM worker, material preview and GLB export. This is a modeling capability used to make generated assets texture-ready, not a texture-generation algorithm by itself.
- **Working path:** `solid.texture` persists the material program; browser baking creates PBR maps; the engine document panel offers an on-demand Three.js preview; GLB export embeds the maps. MCP can search/describe texture nodes and export the result.
- **Viewport:** the Bevy viewport shows the baked maps (albedo, normal, metallic-roughness, emissive) through one shared texture array; each mesh carries a UV attribute and each surface its map layers. `poly.unwrap_uv` cuts seams automatically and reports stretch, flips and overlap. A real-model run built six textured objects (Sonnet, 6/6).
- **Still open:** checking the viewport's look against the Three.js overlay and the exported GLB (normal-map direction is by derivation, not yet against a reference), a graceful fallback past 16 fully textured surfaces, better cuts on smooth closed shapes, and coverage for unsupported Material Maker features. See `HANDOFF-texture-generator.md` at the repo root.

## Where the goals stand
Working: the typed language, evaluator, viewer with editing panels, Blender-backed modeling, LOD, measure and export, model-driven building through MCP (real-model runs built a raised panel, house, cottage, stool and LOD sphere), the procedural texture compiler and baker.
Texturing and keyframed animation now run end to end: baked maps show in the viewport, export as glTF, and a model can author both through MCP (real-model runs: 6 of 6 textured objects, 4 of 4 animated ones). `anim.track` (ADR 0026) moves a part with position, rotation or scale keys about a pivot; the viewer plays it from a shared clock and export writes a glTF animation.
First behaviours now work (ADR 0027, proposed): `behavior.on` makes an animated part react, when it is clicked in play mode or when the viewer comes near, by running its own clip (`play` once, or `toggle` forward and back). Behaviours advance in fixed 1/60 s steps, so the same triggers give the same poses everywhere; play mode is separate from editing (clicks select in edit mode, act in play mode, and Stop resets everything). The viewer shows a Play button when the scene has something that reacts. A real-model run built a clickable door, an automatic sliding door and a lever (3 of 3).
Next for behaviours: targets other than the part itself (a switch that moves a lamp), state and conditions, grab and socket triggers, per-instance behaviours. After that: joints, skinning and IK (bit-exactness of the IK solver is open); the physics choice (Rapier or Bullet) and a CAD kernel choice are owner decisions.
Evaluation still owed: the 60-task G9c comparison of typed nodes versus generated code. The browser suite was re-run after the texture and animation changes (all scripts pass; the long `g9-review` capture run completes in the background).

## Hackathon fit
Judged on originality, functionality, intelligent interaction and polished UX; must run in the browser; plain language to 3D scenes and working logic. A procedural, fully in-browser engine with a model-safe typed language, real CAD-style modeling operators and generated materials addresses all four. The terms and conditions and the submission cutoff time still need to be read and confirmed by the owner.

## Open owner items
Confirm `engine/xr` goldens and ADRs 0017 to 0027; decide the commit plan for the large uncommitted tree (new vendored trees: blender, eigen, mikktspace, fmt, material-maker); rotate any pasted keys; the superseded C++ texture VM has been removed.
