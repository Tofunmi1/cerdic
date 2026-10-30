# Procedural building assets

`building.asset@1` is the close-view asset path; `city.buildings@1` remains the low-cost district massing path. They serve different scales and should compose in one graph rather than make every city lot pay for hero-level detail.

## Current asset grammar

The current asset is a deterministic rectangle-based facade grammar. It splits each facade into storeys and bays, makes wall panels around window openings, sets glass behind projecting frames, adds mullions, gives the street-facing facade a framed entrance, repeats belt courses and pilasters, applies style-specific setbacks and residential balconies, and adds a parapet and roof equipment. The `part` parameter emits masonry, glass, trim, roof or doors separately, so ordinary material and texture nodes can style each region. Width, depth, storeys, bays, floor height, style, detail and seed are explicit graph parameters. Twelve starting families cover Art Deco, classical, tenement, brownstone, precast, curtainwall, industrial, brutalist, modernist, Mediterranean, Victorian and warehouse proportions. See `web/demos/building_asset.txt` and `crates/ve-modeling/tests/building_asset.rs`.

`web/demos/architecture_gallery.txt` is a side-by-side composition of twelve building families with distinct footprints, heights and materials, connected by a road segment with lane paint, crosswalks, kerbs and pavements. The frontage includes reusable lights, a traffic signal, utility mast, sign, hydrant, bench, shelter, bollard and bin. Open it in the engine viewer with `?viewer=1&eval=1&scene=open&model=architecture_gallery&frame=1&docname=architecture-gallery`. It is a procedural architecture study, not a complete generated city: buildings and street objects are currently arranged by hand, and the grammar does not yet include curved intersections, arches, fire escapes, interiors or a rich roof kit.

`web/demos/church_sanctuary.txt` is a hand-composed companion scene for the same close-view scale: a Gothic parish church with a rose window, lancets, bell towers, buttresses, slate roofs, a processional path and engine-grown garden trees. It uses ordinary geometry, texture and light nodes rather than the rectangle facade grammar, so it is a content demo and not a thirteenth style family. Open it with `?viewer=1&eval=1&scene=open&model=church_sanctuary&frame=1`.

`web/demos/main_street.txt` is a street-level block example: classical, tenement, Art Deco, brownstone and warehouse fronts share one frontage, with a road, green verge, pocket park, sidewalk trees, four lit streetlights, awnings, benches, bins, a hydrant, a bus shelter and a parked car. It is the composition demo for the asset family, hand-arranged like the gallery rather than a generated city. Open it with `?viewer=1&eval=1&scene=open&model=main_street&frame=1`.

`street.road@1` makes metre-scaled straight asphalt, lane markings, crosswalks, kerbs and pavement modules; `street.prop@1` makes ten placeable street objects with separate body/accent/lens groups. Both are ordinary typed graph operators and can be repeated and materialed like other geometry. Join road segments with matching dimensions and axes for straight streets and right-angle intersections; curved road surfaces and road-aware automatic prop placement remain future work. Validation lives in `crates/ve-modeling/tests/streetscape.rs` and the native/WASI modeling parity corpus.

This follows the useful part of shape-grammar research: a high-level shape is repeatedly split into smaller semantic regions, then rules turn those regions into facade elements. It is a new implementation using Spatial Lab operators and mesh routines; it does not copy CityEngine or Blender source. The rule hierarchy is more useful to prompts than a long sequence of opaque primitive placements, while remaining a normal typed graph node that can be saved, evaluated and composed.

## Unreal City Sample comparison

The local `UnrealEngine-5.8.4-release` checkout contains the Unreal PCG engine/plugin source, but not the separate City Sample PCG project or its building grammar assets. Epic's City Sample PCG documentation describes 28 named Chicago, New York, and San Francisco building styles. A building primitive consumes a footprint, extrudes it to a height, and applies a selected style's shape grammar; the full city is staged across dependent PCG graphs. The older City Sample instead uses Houdini for city layout and building grammar, with buildings assembled from many instanced modular pieces. These are two useful references: modular, style-specific facade grammars for hero buildings, and staged graph dependencies for districts.

Spatial Lab's twelve grammar families are a first authored catalog, not replicas of the 28 City Sample styles. The next useful change is to split family profiles into independently composable rules for base/storefront, shaft, bay/window module, floor band, crown/roof, palette, and optional accents. Add footprint classes (row/terrace, slab, courtyard, point tower, podium tower), facade-specific details (arches, lintels, sills, balconies, fire escapes, cornices, parapets, roof machinery), and explicit street frontage. Build fixed-view comparisons and tests for silhouette, repeated bay rhythm, entrances and UV scale before growing toward a much larger style catalog. The City Sample PCG project and its assets would need to be obtained separately before inspecting those exact rules; they are not present in the local engine checkout.

## Next increments

1. Give window bays real reveal depth and add doors, shopfronts, balconies, cornices, fire escapes and facade-mounted utilities as typed grammar rules.
2. Let the asset consume a polygon footprint/profile, with a clear rule for corner bays and non-rectangular elevations.
3. Separate reusable facade modules from placement. Instance repeated windows and trim where supported; realize only when downstream mesh editing requires it.
4. Add named architectural styles as parameter presets over the same rules, then let the LLM choose a preset and edit stable parameters instead of inventing hundreds of placements.
5. Bake tileable masonry, glass and roof materials through the existing UV/texture pipeline; keep geometry detail and image texture resolution independently adjustable.
6. Compose several asset families into blocks with setbacks, zoning, street frontage and deterministic seeds. Keep this district scale under a triangle/instance budget and use the close-view asset only near the camera.
7. Add facade silhouette, window-count, UV-scale, deterministic-seed, budget and native/WASI parity evidence for every grammar expansion.

## Verification

Native `eval_text` on `architecture_gallery.txt` ran 180 nodes and 89 outputs in 13.0 s and drew 189,754 triangles (63% of the provisional Quest budget), with one light. The same evaluator on `building_asset.txt` produced 23,494 triangles across the six outputs. `church_sanctuary.txt` ran 136 nodes and 29 outputs in 27.4 s, drew 126,572 triangles (42%) with five lights and eleven texture programs. `main_street.txt` ran 181 nodes and 82 outputs in 28.0 s, drew 194,904 triangles (64%) with five lights and three texture programs. The Engine Viewer loaded the gallery at 180 nodes / 89 outputs, the church at 136 nodes / 29 outputs and the main street at 194,904 triangles / 294 draws with no page or JavaScript errors; renders are in `target/reports/demos/architecture-gallery-final.png`, `building-asset-final.png`, `church-sanctuary-final.png` and `main-street-final.png`. The main-street final projection is slow under desktop SwiftShader, so the saved capture may show the committed preview while the final upload finishes. These are desktop SwiftShader runs, not headset frame-time evidence. The focused facade test suite passes, including an outward-facing wall/glass winding check on all twelve styles; the full strict gate remains with the owner.

## Known limits

The current asset has rectangular footprints, twelve broad facade families, one primary surface layer for window openings, and no interiors. The panes sit behind projecting frames and the wall panels are open surfaces, but this is not architectural BIM, a watertight building solid, or a complete building kit. Procedural geometry also cannot recover the photographic detail in a captured city model; that needs licensed source assets, measured references, or a separate reconstruction pipeline.

## References

- Müller et al., [Procedural Modeling of Buildings](https://doi.org/10.1145/1179352.1141931), ACM SIGGRAPH 2006: hierarchical, context-sensitive shape grammars for detailed buildings.
- Esri, [CityEngine CGA introduction](https://doc.arcgis.com/en/cityengine/latest/cga/cityengine-cga-introduction.htm): documented split, component, envelope, roof, setback, scatter, UV and material operations.
- Blender Manual, [Geometry Nodes instances](https://docs.blender.org/manual/en/latest/modeling/geometry_nodes/instances.html): instances reference shared geometry until it is explicitly realized, which is a useful memory model for repeated facade modules.
- Epic Games, [City Sample PCG for Unreal Engine](https://dev.epicgames.com/documentation/unreal-engine/city-sample-pcg-for-unreal-engine?lang=en-US): PCG city stages, building shape-grammar data assets, and the documented building-style catalog.
- Epic Games, [City Sample PCG and MCP Server Interaction](https://dev.epicgames.com/documentation/unreal-engine/city-sample-pcg-and-mcp-server-interaction-in-unreal-engine?lang=en-US): examples of LLM-directed graph edits that target named styles and bounded city parameters.
- Epic Games, [City Sample project overview](https://dev.epicgames.com/documentation/unreal-engine/city-sample-project-unreal-engine-demonstration?lang=en-US): the original Houdini-based city pipeline, modular building assembly and instancing context.
