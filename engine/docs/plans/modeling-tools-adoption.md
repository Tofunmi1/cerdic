# Modeling tools: build, adopt, and extend

**Status:** Proposed roadmap, 2026-10-15. This document organizes tool work after the current G9 package; it does not mark any milestone complete or authorize a license-policy change. The owner’s standing constraints still apply: all authored geometry and materials come from Spatial Lab’s own system, no GPL/AGPL code enters the engine, and geometry-producing workflows must be editable and reproducible.

Read with [track2-modeling.md](track2-modeling.md), [handoff-after-g9.md](../handoff-after-g9.md), [the G9 continuation handoff](../handoff-g9-remaining.md), [ADR 0002](../adr/0002-node-graph-geometry-language.md), [ADR 0003](../adr/0003-licensing-and-no-imported-assets.md), and [ADR 0005](../adr/0005-licence-policy.md). Where this document and an accepted ADR disagree, the ADR wins until the owner changes it.

## 1. Goal

Build a broad, composable modeling system for authored scenes, props, environments, and VR games. It should support procedural construction, direct editing, materials, animation, and interaction through one formal document and command flow. “Anything” is an extensible direction, not a promise that one kernel or one release can implement every feature in Blender, ZBrush, CAD, and Houdini.

The product should combine native operators, selected libraries behind narrow adapters, and reusable procedural kits. It should not become a bundle of unrelated modeling applications. Every feature must have a defined document representation, operator contract, preview behavior, cost limit, undo/reload behavior, and path to the viewport and AI tools.

## 2. Current baseline and order

G0–G8 and V0–V4 are recorded as complete in the current plan. G9 is still in progress. The latest G9 report records authored `scene.environment@1` work in B3; captures and owner review remain open. B4 post effects, remaining B6 edge work, B7 composition, B8 visual scoring, and G9c’s frozen evaluation also remain open. Confirm the live reports before starting a dependent package.

Do the work in this order:

1. **Finish G9** using `handoff-g9-remaining.md`; capture and score the real Engine Viewer scenes, run the provider-backed G9c evaluation when credentials are configured, and close with the strict gate.
2. **Build the modeling productivity batch (G10b proposal):** placements/instances, curve and profile generators, decimation/LOD, selected solid wrappers, and reusable kits. These unlock complex scenes from combinations of manageable operations.
3. **Build procedural appearance (G10):** finish material recipes, then add texture graph and baking/render paths with measured budgets.
4. **Build VR gameplay foundations (L0–L3):** deterministic logic runtime, Play/Edit separation, interactions, then clips and simple procedural motion. This enables usable VR experiences, not only static models.
5. **Add advanced representation backends only where a concrete feature needs them:** first a bounded Rust CAD-kernel spike, then decide whether B-rep/NURBS belongs in the core.
6. **Expand to sculpting, rigs, physics, and authoring polish after headset profiling and core interaction acceptance.**
7. **Finish G13** with compatibility, security, performance, limits, migration, and integrated-demo evidence.

G9 closeout and any already independent L0/L1 work may overlap. Do not start broad library integrations while a current package is changing shared geometry or serialization contracts.

## 3. One formal flow for every tool

The graph is the source of truth. A mouse gesture, XR controller action, text program, AI proposal, and reusable kit must all produce the same typed command batch against the same versioned document. No AI-only mutation endpoint and no UI-only geometry shortcuts.

### 3.1 Keep geometry types explicit

Do not coerce every representation into triangles at creation time. Use typed values and explicit conversion operators for:

- `Profile2`: closed planar loops with validated winding and units.
- `Curve3`: control points, knots/segments, frame and sampling policy.
- `Surface`: supported analytic, patch, or subdivision surfaces.
- `Solid`: current Manifold-backed triangle solid with stable part and region metadata.
- `PolyMesh`: editable indexed topology and domain attributes.
- `Field3`: bounded scalar/vector expressions and sampled volumes.
- `Instances`: prototype references plus transforms, stable IDs, and per-instance attributes.
- `Material`/`Texture`: typed, bounded look graph and generated/baked data.
- Later: `Rig`, `Clip`, and `Behavior` as document types connected to authored parts and instances.

An operator declares its exact input/output types. A conversion is visible in the graph and has an error policy, a resource estimate, and documented attribute loss. Keep lineage/provenance from output regions back to source nodes and parameters wherever the backend allows it.

### 3.2 Operator manifest and generated tooling

Each registered operator specifies: stable name and version; typed ports; units, defaults and parameter ranges; purpose and examples; preconditions and postconditions; topology/attribute effects; determinism and seed policy; estimate and hard caps; cancellation points; preview/final policy; stable refusal codes and repair suggestions; and native/WASI test fixtures.

Generate the text grammar help, AI tool schema, MCP descriptions, inspector fields, docs index, and operator snapshot from the same registry. Keep the graph as a DAG with stable node IDs, named outputs, groups/kits, content hashes, and cache invalidation by dependencies. Editor layout stays outside semantic evaluation hashes. The typed-field model in Blender is a useful design reference: field functions are evaluated in the context/domain where geometry-flow nodes consume them; implement our own compatible concept rather than importing Blender code. [Blender Geometry Nodes fields](https://docs.blender.org/manual/en/latest/modeling/geometry_nodes/fields.html)

## 4. Tool families to build

This is the capability inventory. “Later” means retain a place in the type/operator design, not promise it in the next milestone.

| Family | Tool set | Priority |
|---|---|---|
| Construction | Primitives, regular/rounded profiles, dimensions, text only after a procedural font policy, assemblies, named parts and sockets | Existing base; extend through kits |
| Transforms and editing | Move/rotate/scale, pivots, snapping, align, mirror, duplicate, arrays, parent/group, instance editing, transform constraints | Core interaction; improve selection and edit modes |
| Polygon modeling | Vertex/edge/face selection; extrude, inset, bevel, bridge, fill, knife, loop cut, merge/weld, dissolve, separate/join, normals, material regions | Extend G6/V5 incrementally with explicit topology IDs |
| Curves and profiles | Arc, ellipse, regular/star/rounded rectangle, spline, helix/spiral, resample, offset, fillet, sweep, loft, revolve, pipe, profile extrude | First modeling batch; feeds existing surface/solid pipeline |
| Solid/CAD operations | Boolean, hull, slice/split/trim, projection, shell/offset, Minkowski for bounded convex cases, later sketch constraints, dimensions, revolve/extrude, B-rep fillet/chamfer, STEP import/export | Existing mesh solid first; B-rep after spike |
| Surface modeling | Loft/revolve/patch, subdivision, trims, continuity checks, thickness, surface bridge, controlled tessellation | Continue existing G5/G7 contracts; advanced NURBS later |
| Procedural graph and fields | Fields over point/edge/face/instance domains; noise, proximity, masks, attribute transfer, field-to-mesh, terrain/heightfield, reusable groups | Build on G8; field context and attribute-domain typing required |
| Placement and world building | Scatter on surface/volume, minimum spacing, slope/height/mask filters, along curve, grid-on-surface, seeded variation, collision spacing, instances, LOD, architecture/furniture/prop/environment kits | Highest scene-building payoff after G9 |
| Mesh quality and production | Decimation, LODs, weld/repair, normal tools, manifold diagnostics, UVs, remesh/retopology experiments, export validation | Add decimation/LOD early; retopology is later research |
| Sculpting | Brush-based displacement/smoothing, masks, symmetry, face sets, dynamic tessellation, voxel/SDF remesh, multiresolution sculpting | Later; requires dedicated interactive mesh/field representation and headset profiling |
| Appearance | PBR recipe graphs, noise/Voronoi/patterns, ramps/blends, domain warp, triplanar, UV and texture bake, normal/roughness/height/AO maps, decals, glass/water | G9 B5 then G10; tier and memory budgets |
| Character and animation | Hierarchies, pivots/joints, clips/keyframes, procedural motion, IK, rigs/weights, morph targets, particles, constraints, timeline | Logic + simple animation first; rigging later |
| Game authoring | Grab/throw, sockets, triggers, doors, switches, teleport/nav, interaction rules, collision shapes, Play mode, replay/debugger, scene/game kits | L0–L7; target the VR game vertical slice |

## 5. Build, integrate, fork, or study

### 5.1 Build natively (default)

Implement product-specific graph operators in `ve-graph`/`ve-modeling`, renderer/material features in `ve-render`, and direct manipulation in the existing editor/viewport. Keep reusable math or kernel code behind engine-owned contracts. This is the default for transforms, profiles, curves, placements, kits, fields, materials, commands, diagnostics, and AI-facing schemas.

### 5.2 Reuse current dependencies before adding new engines

| Existing component | Expose as product capability |
|---|---|
| Manifold, already in the C++ kernel | Continue robust mesh-solid booleans and add bounded wrappers such as hull/slice/split where supported. Preserve source and output attributes explicitly. |
| meshoptimizer, already in the kernel | Add `solid.decimate` and an instance/render LOD path; define whether the result is destructive or a derived preview. |
| OpenSubdiv, already pinned | Continue subdivision, UV and float-channel behavior through the current adapter. Do not add a second subdivision system. |
| Clipper2, already vendored | Use for suitable planar profile offsets/fillets after validating topology and numeric limits. |
| Bevy render crates, already vendored | Keep rendering integration aligned with the project’s current engine architecture; do not introduce a second scene/document owner. |

### 5.3 Candidate external projects to evaluate

| Candidate | Use | Recommended decision |
|---|---|---|
| [Truck](https://github.com/ricosjp/truck) | Rust B-rep/NURBS and CAD interoperability experiment | First exact-CAD spike candidate. It is modular Rust and Apache-2.0 upstream; test only the crates needed, WASM viability, operation coverage, tolerance behavior, tessellation, file size and deterministic output. Keep optional behind `CadKernel` adapter until evidence supports promotion. |
| [Open CASCADE Technology](https://github.com/Open-Cascade-SAS/OCCT) | Mature C++ CAD, surfaces and exchange formats | Fallback if Truck lacks required operations or STEP quality. It is a large C++ system under LGPL-2.1 plus exception; prototype isolated behind C ABI and measure build/runtime/WASM cost. |
| [MaterialX](https://materialx.org/) | Material graph interoperability and standard-node reference | Map a deliberately supported subset to Spatial Lab `Material`/`Texture`; don’t make the renderer depend on all MaterialX nodes. The library is Apache-2.0. |
| [MuTracer](https://github.com/pissang/mutracer) | Optional high-quality desktop still render / visual-review backend | Prototype only as a separate presentation renderer. It is MIT, Three.js-integrated, progressive, WebGPU-first with WebGL2 fallback. Do not assume it is a Quest/WebXR renderer; require device evidence. It is 0.x, so isolate behind a replaceable adapter. |
| [OpenVDB](https://www.openvdb.org/) / NanoVDB | Sparse volumetric fields and possible sculpt remesh research | Research only until a specific need and license/version audit are documented; footprint and C++/WASM work may be high. Prefer current bounded G8 fields for browser-native needs. |
| [libigl](https://libigl.github.io/) | Geometry-processing algorithms and research | Select individual algorithms only after package/dependency license and WASM audit; prototype outside core first. |
| [CGAL](https://www.cgal.org/license.html) | Robust computational geometry algorithms | Evaluate per package; it mixes GPL and LGPL packages and has commercial licensing. Do not add it as a blanket dependency. |
| Blender | UX reference, fixture authoring/reference, possible separate-process tool if owner explicitly changes asset policy | Blender source is GPL; ADR 0024 (2026-10-16) removed the exclusion, so porting selected modules is allowed with a feasibility note and `VENDOR.md`, and publishing a build that includes them triggers GPL source obligations. Geometry Nodes fields and workflow concepts can inform independently implemented features. |
| Houdini, ZBrush, commercial CAD/sculpt tools | Workflow and interaction references | Do not fork their products. Use public behavior/docs as design inspiration; all generated output still follows the owner’s procedural-content policy. |

License is not the only decision criterion, but the accepted policy excludes GPL/AGPL from the engine. Record exact upstream commit, license files, patches, build flags, and notices in `lib/<name>/VENDOR.md` for vendored code; update `deny.toml` for Rust crates as required by project policy. Upstream library licenses and product asset licenses are separate questions: model assets, brushes, textures, HDRIs, and fonts are not automatically covered by an application’s source license.

### 5.4 Forking policy

Fork only when an upstream library is demonstrably the right backend and an adapter/upstream contribution cannot meet a required fix. Before a fork:

1. Write a short spike report with required operators, test cases, upstream alternatives, license, target platforms and measured baseline.
2. Confirm the owner’s existing GPL/AGPL exclusion and no-imported-assets policy; identify licenses of the exact source files and transitive dependencies.
3. Pin upstream to a commit/tag and retain the upstream license/notices.
4. Keep patches small, documented, and isolated under `lib`; avoid product logic in the fork.
5. Add differential tests against upstream behavior, engine invariants, native/WASI parity where applicable, and a reproducible update procedure.
6. Re-evaluate whether to upstream the patch or replace the fork at each upgrade.

Never copy ZBrush, Houdini, or other proprietary implementation source into this engine (GPL source such as Blender's is allowed per ADR 0024). Reimplementing a documented behavior independently is different from copying implementation code; use original algorithms, citations, and clean-room fixtures where needed.

## 6. External-library spike gate

Use a bounded spike before promoting a library into a product dependency. The spike must answer:

- **Capability:** exact feature coverage, input representations, topology/attribute preservation, numerical tolerances, failure modes, and unsupported cases.
- **Integration:** Rust/C++/WASM toolchain fit, browser worker operation, C ABI or Rust boundary, cancellation, memory ownership, threading, and interaction with current evaluator/cache.
- **Size and speed:** added native and WASM bytes; cold build and runtime; representative low/medium/high geometry; peak memory; headset or target-browser measurement where relevant.
- **Determinism:** repeated-output stability and native/WASI parity for any serialized/evaluated result; explicitly classify visual-only nondeterminism.
- **Maintenance:** pinned version, release cadence, open issues affecting required features, security/advisory process, and cost of carrying local patches.
- **Licensing/provenance:** exact source license, dependency licenses, notices, and generated data/assets reviewed independently.

The spike ends with one decision: reject; keep as research reference; use through an optional adapter; or vendor/integrate. Do not let a successful demo silently become a core dependency.

## 7. Delivery packages and acceptance

### Package A — G9 closeout

Use the current G9 handoff as source of truth. Include final B4/B6/B7 implementation, B8 owner-scored captures, and provider-backed G9c evaluation. Record the scene, browser/device, render settings, visual score, performance, known gaps, and strict-gate output. MuTracer may be compared as a desktop capture backend but must not substitute for Engine Viewer acceptance.

### Package B — Scene-making operators

Implement in a high-utility order:

1. `inst.scatter`, `inst.along_curve`, `inst.jitter`, `inst.on_grid_of`, sharing prototype geometry and stable seeded IDs.
2. `profile.arc`, `profile.ellipse`, `profile.regular`, `profile.star`, `profile.rounded_rect`; `curve.arc`, `curve.helix`, `curve.spiral`, `curve.spline`, `curve.resample`, `curve.offset`, `curve.fillet`.
3. `solid.decimate` over meshoptimizer plus draw-time `inst.lod` with tested distance/threshold behavior.
4. Bounded Manifold wrappers: hull, slice/split/trim, shell or Minkowski only where invariants and cost limits are proven.
5. Reusable architecture, furniture, prop and mechanical kits as graph groups, with typed parameters and attachment sockets.

Each package adds cases, stable diagnostics, estimates, docs, generated AI schema, save/reload/undo/group/instance tests, determinism, and native/WASI parity. Scatter must test seed repeatability, minimum spacing, mask effects, and stable identity across parameter edits.

### Package C — Procedural appearance

Complete G9’s authored material path, then grow one typed texture recipe language. Start with noise, Voronoi, gradients, patterns, mix/ramp/levels/domain warp, normal-from-height, and triplanar sampling. Add both shader-side preview and bounded baked textures only after measuring a small representative recipe set. Set memory and resolution caps from browser and headset measurements, not guesses. Keep glass/water and expensive ray/path tracing in explicitly optional display paths.

### Package D — VR game authoring

Build L0–L3 from the existing logic design, with fixed-step deterministic rules, edit/play isolation, controller and pointer parity, collision/interaction primitives, and replayable tests. Add door, switch, grab/throw, socket, lamp and fan examples. Follow with clip/timeline authoring and only then joints/IK/rigging. Measure controller-to-action latency and headset frame time.

### Package E — Advanced surface/CAD experiment

Prototype Truck behind a feature adapter for a narrowly chosen set: exact profile extrusion/revolve, curve/surface representation, B-rep Boolean, tessellation, and STEP round trip. Compare against existing Manifold operations on common fixtures. Promotion requires a real user-visible benefit and acceptable WASM size/build/runtime; if no, keep mesh solids and continue with procedural profiles/surfaces.

### Package F — Sculpt and remesh research

Do not start brush features as a set of ad hoc mesh edits. First choose a representation (dynamic topology, multiresolution mesh, or bounded sparse SDF), then test stroke latency, local remesh quality, attribute preservation, undo storage, memory, parity needs, and XR input. Consider OpenVDB/NanoVDB only if a small self-contained spike beats current G8 fields for an actual sculpting case.

## 8. Shared acceptance checklist

Every operator/tool package must:

- enter through the typed command/document flow and preserve stable IDs where meaningful;
- validate before mutation, apply atomically, and support undo/redo/save/reload;
- declare domains, units, defaults, limits, estimates, deterministic seed behavior, and typed diagnostics;
- handle cancellation and stale worker results without corrupting the document;
- test success cases as well as refusals, with analytic/reference expectations;
- add corpus examples and native/WASI parity for evaluator-visible output;
- report preview/final quality, triangle/draw/instance counts, wall time, peak memory, and relevant target hardware;
- expose short generated help/examples to the UI, text language, and AI tool schema;
- update cases, diagnostics, limits, plan status, and a dated report entry;
- pass `cargo xtask quick` while in development and the required strict gate at the end of the package.

For image-only renderer features, exact pixel parity may not be suitable across GPUs. Define toleranced image metrics and deterministic CPU-side recipe outputs separately, and record hardware/browser/settings with each capture.

## 9. First concrete next handoff

After G9 closes, hand off **Package B, steps 1–3** as the first modeling-tool implementation. They reuse the current `Instances` type and mesh simplifier, produce visible scene complexity quickly, and help later terrain, architecture, and game-world kits. Keep B-rep/CAD and sculpting as separate evidence-driven spikes. Run the M9 Quest profile alongside the package so instance, LOD, material, and geometry caps are based on the actual device.

