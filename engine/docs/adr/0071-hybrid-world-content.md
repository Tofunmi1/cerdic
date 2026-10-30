# ADR 0071: Hybrid procedural and captured world content

- Status: accepted (owner direction, 2026-10-28)
- Date: 2026-10-28

## Context

The city reference supplied on 2026-10-28 is a dense, textured aerial reality model. The existing `ve_world::city` and `ve_world::building` path creates deterministic streets, lots, and editable procedural buildings, but it cannot create the unique observed roof and facade detail in a photogrammetric capture. Replacing that path with more procedural primitives will not reach the reference.

The engine currently has two important constraints:

- `ve-modeling` evaluates deterministic graph geometry and currently returns mesh-oriented `Evaluation` values. A large reality model should not be expanded into one graph mesh or baked into every AI edit.
- ADR 0003 and ADR 0070 say no imported assets. This decision changes that product policy for world content. It does not authorize any particular third-party dataset or waive attribution, license, privacy, or redistribution terms.

OGC 3D Tiles is designed to stream large photogrammetry, building, point-cloud, and other geospatial datasets. Its hierarchy uses spatial bounds, geometric error, and replacement/additive refinement; glTF is its primary renderable tile format. This provides the correct outer representation for dense captured cities.

## Decision

1. **Worlds may combine three representations.** (a) captured visual tiles for dense unique detail; (b) semantic features such as roads, parcels, building footprints, class, and stable source IDs; and (c) editable procedural or authored gameplay geometry. The captured layer is visual context; it does not become authoritative collision, navigation, or game logic by default.
2. **Use OGC 3D Tiles 1.1 as the first streamed capture format.** Preserve tileset hierarchy, tile/content bounds, geometric error, refinement mode, and metadata. Tile payloads use glTF/GLB where supported. Do not flatten the tileset to one GLB. The viewer requests and releases tiles according to camera-space error and an explicit memory budget.
3. **Keep runtime asset references outside deterministic geometry evaluation.** A project/world asset manifest owns stable asset IDs, source URIs, content hashes, coordinate transforms, license/provenance, and semantic links. Graph nodes may reference an asset ID through a typed world-asset input, but evaluation does not fetch network data or turn a whole tileset into a `Mesh`. The runtime resolves and streams the asset. Graph output remains deterministic given graph inputs and the resolved asset revision.
4. **Use a shared local coordinate frame.** Imported geospatial coordinates are converted to a local tangent frame near the project origin. Keep geospatial transforms in f64; tile vertex data remains local f32. Use origin rebasing/camera-relative transforms before large coordinates reach the renderer.
5. **Maintain a semantic editing layer.** Each semantic feature has a stable ID and optional links to one or more capture tiles and graph objects. AI tools address stable IDs, not mesh triangle indices. Roads, lots, building volumes, entrances, collision, navigation, and gameplay tags are editable independently of the captured visual mesh.
6. **Admit content only with a provenance record.** Record creator/source, exact license or service terms, attribution, permitted modifications, redistribution status, capture date where known, and a hash. Unknown or incompatible content is not bundled. The Epic City Sample marketplace assets and Google Photorealistic 3D Tiles are not default inputs: City Sample listing terms exclude AI use, while Google Maps Tile API policy restricts extraction, indexing, offline use, and non-visualization use.
7. **Keep procedural generation as a first-class authoring route.** Existing city, terrain, scatter, materials, and graph tools remain available to build semantic replacements, gameplay content, and areas without scans. The target is a hybrid world pipeline, not a replacement of the typed graph with a static model viewer.
8. **Use the City of Melbourne 2018 photomesh as the first candidate capture.** Its public listing describes citywide textured mesh tiles at L13-L20 and lists CC BY 4.0. Its service is ArcGIS I3S, so convert a small, bounded piece into 3D Tiles for the first local pilot. The 9.7 GB bulk ZIP URL listed in the catalog returned 404 during initial research; the service endpoint currently responds, but cache/extraction terms and the source-data path still need verification. The project catalog example has caching disabled until that is resolved.

## Consequences

- This supersedes only the no-imported-world-assets policy in ADR 0003 and ADR 0070. Those records remain historical; update their status and cross-reference this ADR when implementing the asset path. Existing Blender code, geometry operators, and material generation policies are otherwise unchanged.
- The implementation requires a project asset catalog, tile-set parser/resolver, streamed renderer integration, cache/memory policy, attribution display, stable semantic feature IDs, and import/validation tooling. None exists as a complete path today.
- Native/WASM deterministic parity applies to authored graph geometry and semantic generation. Network fetch order and GPU tile residency are runtime concerns and are not part of graph evaluation hashes.
- Initial capture experiments should use a small user-owned dataset or assets with explicit redistribution rights. OpenDroneMap can generate textured meshes from image sets; it does not supply source imagery or grant rights to the images. Conversion into tiled glTF/3D Tiles remains a separate step.

## Alternatives considered

- **Procedural-only:** retains strong editability and determinism, but cannot reproduce unique real-world roofs/facades in the reference.
- **One giant GLB:** easy to preview, but lacks hierarchical streaming and becomes an impractical unit for editing, caching, and memory control.
- **Google Photorealistic 3D Tiles:** visually relevant and already tiled, but its terms constrain extraction and offline/non-visual use; it is not a source for editable/redistributed game geometry.
- **3D Gaussian splats:** useful as a captured visual background, but not a substitute for mesh collision, semantic selection, or ordinary game-world geometry. Evaluate later as an additional visual layer, not the first format.

## Sources

- [OGC 3D Tiles standard](https://www.ogc.org/standards/3DTiles/) and [3D Tiles 1.1 specification](https://docs.ogc.org/cs/22-025r4/22-025r4.html).
- [OpenDroneMap textured model outputs](https://docs.opendronemap.org/outputs/) and [GLB output option](https://docs.opendronemap.org/arguments/gltf/).
- [City of Melbourne photomesh listing](https://data.melbourne.vic.gov.au/explore/dataset/city-of-melbourne-3d-textured-mesh-photomesh-2018/), [DataVic license metadata](https://discover.data.vic.gov.au/dataset/city-of-melbourne-3d-textured-mesh-photomesh-2018), [OGC I3S standard](https://www.ogc.org/standards/i3s/), and [loaders.gl I3S/3D Tiles converter](https://loaders.gl/docs/modules/tile-converter/cli-reference/tile-converter).
- [Google Map Tiles API policies](https://developers.google.com/maps/documentation/tile/policies).
- [Overture attribution and licensing](https://docs.overturemaps.org/attribution/); licenses differ by theme and source, and must be preserved per imported dataset.
- [Epic City Sample Buildings listing](https://www.fab.com/listings/008fe959-5511-428e-93bd-f99b1179f6d5).
