# Numeric limits and budget profile (v1, initial)

These are the caps enforced from G1 on. They are starting values chosen to be safe, not tuned: G2
measures them and revises this file with the evidence. A limit that is hit produces a `budget.*`
diagnostic naming the limit and the observed value; the document stays unchanged.

## Hard limits (checked before work begins)
| Limit | Value |
|---|---|
| Source text | 256 KiB |
| Document JSON | 2 MiB |
| Nodes per document (authored) | 5,000 |
| Expanded nodes after groups | 20,000 (the existing enforced expansion cap) |
| Named frames on each evaluated solid/prototype | 128 (frame-pattern copy products preflighted before geometry) |
| Instance placements per set / result | 16,384 / 16,384 (G4c measured entity representation) |
| `ve_kernel::tangents` triangles per call | 2,000,000 (the Final triangle budget); vertex count at most `4 * indices + 4`. About 16 KB of wasm code (`-Os` object, unlinked) |
| `inst.scatter` points per node | 16,384; 32 draws per requested point (plus 64) before `scatter.unsatisfiable`. 16,384 points with 0.5 m spacing took 0.12 s in a debug test |
| Group nesting depth | 8 |
| Inputs on a variadic port | 64 |
| List parameter length | 4,096 values |
| String parameter | 256 bytes |
| Commands per batch / batch size | 1,000 / 1 MiB (G9a-4; the study-room transaction is 386 commands / 43,465 serialized bytes) |
| Explicit and emissive-derived scene-light outputs | 128 combined; exceeds with `budget.lights` |
| Staged scene-light JSON | 64 KiB per evaluation candidate; the light count remains 128 |
| Forward shader lights | One shadowed sun; point caps Low/Medium/High = 2/4/16; unshadowed spot caps = 2/4/8 |
| WebGL2 light uniform | 1,088 bytes: 192-byte shared block (flat ambient, L0/L1 sky-ground band, ground bounce, sun, shadow) + 16 × 32-byte point + 8 × 48-byte spot records; fixed arrays stay within WebGL2's 16 KiB minimum uniform-block limit (`crates/ve-render/src/light.rs`) |
| Scene environment | At most one authored `scene.environment@1` output per document; `intensity` 0–8, `reflection` and `bounce` 0–1, colours 0–1 per channel; a second output refuses as `scene.environment.duplicate` |
| Ambient model | A two-colour sky/ground hemisphere packed as a first-order (L1) spherical-harmonic band plus an optional warm ground bounce. A bounded approximation of an environment: no inter-surface light transport unless `gi=true` turns on light probes (below) |
| Texture memory (viewer) | 192 MiB of baked maps, mip chains counted, in three arrays by size (1024, 512, 256 pixels square): 36 maps at 1024, 144 at 512 or 576 at 256. `solid.texture size` picks the size (auto is 1024); over budget the viewer shrinks the biggest maps first, then drops channels. Not a limit on the exported glTF |
| Light probes (`gi`) | Opt-in on `scene.environment`/`scene.sky`: L1 irradiance probes traced on the CPU a few ms a frame, at most 64 per axis and 65 536 in all, 500 000 triangles traced (instances thinned past that), 1 to 4 bounces, 32 to 512 rays per probe. Static geometry only; the lights uniform is 1 296 bytes |
| Emissive-derived point light | `emits_light=true`; intensity is `20 × surface_area_m² × max(emissive)` capped at 20; range is 6 × AABB diagonal clamped to 1–1,000 m. These are starting visual heuristics, not physically calibrated photometry |
| Viewer grading | Renderer-only, not authored and never part of a document: exposure multiplier 0.25–4, contrast and saturation 0–2, each tint channel 0.5–1.5. Controls clamp and a non-finite value becomes neutral. Applied in the forward shader after tonemapping, so windows and XR view the same grade (`crates/ve-render/src/grade.rs`) |
| Viewer grading presets | Five named presets (neutral, studio, warm, cool, mono). The active values ride in the view uniform; the shader adds no pass, so there is no extra target or draw. Bloom and depth of field are not implemented |
| History | 200 transactions or 16 MiB, whichever first |
| Profile loops / points per loop | 64 / 16,384 |
| Initial G5 point list / loft sections | 4,096 / 64 (reserved targets: 65,536 / 256) |
| Revolve segments / extrude divisions | 512 / 256 |
| Refine factor | 16, and the predicted triangle count must pass the geometry limit |
| Field grid samples | 2,000,000; additionally limited to 2,097,152 padded Manifold voxel slots; checked during graph preflight |
| Field tape instructions / sample-instruction work | 4,096 / 64,000,000 |
| Triangles per result (stored) | 2,000,000 final, 200,000 preview. A shape placed many times (`inst.*` sets) is stored once and counts once |
| Triangles per result (drawn at full detail) | 200,000,000 final, 20,000,000 preview: every placement counted, a sanity bound (`budget.drawn_triangles`, refused in preflight before any node runs). Above 4,000,000 an evaluation succeeds with the warning `budget.drawn_heavy` on the heaviest output |
| Subdivision output vertices / faces / corners | 2,000,000 / 2,000,000 / 8,000,000 (kernel cap; performance-based retuning pending) |
| Subdivision levels | 1–4; graph preflight bounds output at 6 × input triangles × 4^(levels−1), using at most three input corners per triangle |
| Subdivision attributes | Face attributes copy by parent face; integer point attributes are omitted with a typed evaluation warning; float point, corner UV/FVar and edge attributes refuse with typed diagnostics. Float point primvars need an interleaved channel ABI and channel-aware growth/memory preflight; UV FVar additionally needs per-corner value indices and output corner remapping. F64 point/UV values are refused rather than rounded through the kernel's f32 API |
| Subdivision attributes | Paired F32 corner `u`/`v` values interpolate as OpenSubdiv face-varying data with linear boundaries; F32 point attributes interpolate in up to 8 channels and at most 32 million refined floats. F64 is refused as `poly.subdivide.f64_unsupported`; arbitrary corner and edge channels remain unsupported. |
| Subdivision holes | A non-zero face `hole` U32 attribute is recognized and refused as `poly.subdivide.holes_unsupported`. The pinned OpenSubdiv 3.7.0 source marks base-level holes, but exposes no child-face inheritance contract in the refiner path used here. `PolyMesh` also lacks an explicit hole-face output representation, so child tagging and filtered-topology provenance are not claimed; open boundary cages remain supported. |
| Polygon edits after subdivision | Treat `poly.subdivide` as the final editing step in v1. Tested planar-patch extrusion refuses with `poly.patch_nonplanar`; beveling a selected curved edge network refuses with `poly.bevel_network_unsupported`, at levels 1 and 2. Other polygon operations retain their own contracts; no general post-subdivision edit compatibility is claimed |
| Solid edge treatment | `solid.round@1` and `solid.chamfer@1` are whole-solid Minkowski erosion/dilation (a sphere and an octahedron). They cut convex edges and vertices only; concave edges stay sharp, so this is not arbitrary CAD filleting. Input is capped at 20,000 triangles and the result at 250,000, and the shape size must be less than half the thinnest bounding dimension or the eroded core is empty. `round` takes 6–24 sphere segments; `chamfer` has a fixed 45-degree flat facet and no segments |
| Poly edge-network contract | `poly.bevel_edges@1` (v1) accepts only disjoint manifold interior edges whose end vertices have valence three. A connected network or vertex fan refuses as `poly.bevel_network_unsupported` naming the vertex; boundary, non-manifold and winding-conflict edges refuse with their own codes. Seams must be welded first (coincident unwelded vertices read as boundary edges). A selection mixing valid and invalid edges refuses atomically and leaves the input unchanged. Edge networks are not enabled |
| Predicted realization | 4,000,000 triangles and 256 MiB |
| Kernel extent (exists) | `MAX_EXTENT` 1e6 m; 2^28 elements |

## Runtime caps
| Cap | Value |
|---|---|
| Pending worker queue | 8 jobs; a newer job supersedes older independent ones |
| Preview job | 2 s soft, terminated at 10 s |
| Final job | 30 s soft, terminated at 60 s |
| Result cache | 128 MiB, least recently used eviction |
| GPU upload per frame | 8 MiB and 2 ms of main-thread time |
| Worker restarts | 3 in 60 s, then the editor reports a persistent failure |
| Editor graph | 5,000 nodes loaded; interaction goal measured at 200 and 500 |

## Logic limits (draft for L2; X8 measures them)
| Limit | Value |
|---|---|
| Tick | fixed 60 Hz, integer ticks, no wall clock |
| Node steps per tick | 10,000 (excess chains resume next tick) |
| Cascade depth (`emit` chains) | 16 |
| Event queue | 1,024, newest dropped on overflow |
| Timers | 256 |
| State variables | 1,024 |
| `repeat` iterations | 64 |
| Queued re-triggers per rule | 8 |
| Rebuild requests per node | 4 per second, coalesced |
| One tick of the door fixture | under 1 ms (target, window thread) |
| Rules in a document | 1,000 (counts toward the node limits) |

## Performance goals (targets until measured, on the baseline machine)
| Goal | Value |
|---|---|
| Warm preview p95, fixtures F1 and F2 | under 150 ms |
| Editor pan, zoom, drag, F3 (200 nodes) | display refresh, no frame over 33 ms |
| Moving a node | zero evaluations (instrumented) |
| Regeneration soak | 100 regenerations, bounded memory after warm-up |
| Long soak | 30 minutes of alternating edit, cancel and reload, editor open |
| Release size | recorded against 3.24 MB brotli (stripped) for every gate |

## Fixture manifest (names used by tests and reports)
| ID | Fixture | Size and workload |
|---|---|---|
| F1 | Vase (see `commands.md`) | 6 nodes, 64-segment revolve |
| F2 | Wheel assembly | 4 instances of one group, about 60 nodes; G4 |
| F3 | Synthetic 200-node graph | mixed chains and fans of cheap booleans and transforms |
| F4 | Synthetic 500-node graph | same generator, larger |
| F5 | Creature field | `field_creature.txt`: 32 tape instructions for the body field; bounds and 0.026 m Final spacing produce a 70 × 74 × 58 (300,440) point lattice. Preview doubles spacing to 0.052 m and produces 35 × 37 × 29 (37,555) points. Three release runs on Apple M1 / Node 26.7.0 measured native median 286.9 ms, WASI median 349.1 ms and WASI linear memory 22.1 MiB; it produced 29,112 Final triangles. Preview test enforces 200,000 triangles and compares against Final within 12% volume and 0.06 m bounds. |
| F6 | Facade | G4 and G6: 40 instances, profile details |

Each performance report states: hardware, browser and version, build mode, warm or cold, fixture,
quality (preview or final), nodes, output triangles, cache hits and wall time (p50, p95, max).
