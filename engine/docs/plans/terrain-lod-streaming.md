# Terrain LOD meshes and streaming

`ve_world::terrain_lod::select_tiles` supplies the balanced leaf set for one
`terrain.tile` world rectangle, using camera position and conservative terrain
height bounds. Keep selection on the CPU initially on both WebGPU and WebGL2;
keys `(level, x, z)` identify patches, while morph changes update draw data each
frame. Selection covers the world once; squares at unaligned world boundaries
overhang and need clipping to the terrain's actual bounds.

## Patch geometry and morph

Start with 32 by 32 quads (33 by 33 vertices) per selected patch at every level.
Use one shared index buffer of 6,144 indices, with the same +Y winding and
triangle diagonal as `terrain::to_mesh`. A reusable unit x/z grid is transformed
by each selected patch's `min` and `size`; larger levels increase world sample
spacing without increasing vertex count.

Heights come from the evaluated `terrain.tile` height grid, including authored
mask/erosion changes, rather than regenerating noise. Its stored `Tile.grid`
includes a 32-cell margin; derive the world rectangle from `Tile::origin()` and
the interior sample extents, excluding that margin. Sample in world coordinates
using the grid's origin and cell spacing. A coarse patch spanning multiple
source tiles gathers the corresponding grid windows. Preserve shared boundary
samples and use margin samples for normals, as the existing terrain mesh does.

Bake height and normal pages per LOD patch with filter gutters and consistent
height decoding across neighbours. Upload patch origin, size, page mapping,
parent mapping and `SelectedTile::morph` as draw data. Add a terrain vertex
helper to the forward shader composition in `ve-render/src/pipeline.rs`, following
the inclusion pattern of `water_waves.wgsl` and `wind.wgsl`. Blend fine heights
toward the parent's triangulated surface. Collapse odd grid vertices onto the
parent x/z lattice as morph reaches one; height blending alone cannot remove
T-junction cracks. Top-level roots have no parent, so draw them without collapse
and use their own height page as the parent fallback. Edge vertices need a
neighbour-aware collapse/morph rule so fine and coarse edges agree throughout
the transition. Keep identical deformation
in depth and shadow passes. WebGL2 can use vertex texture sampling with per-draw
uniforms, or baked vertex heights if format capabilities require it.

## Residency and streaming

Feed consecutive desired leaf sets into `diff_tiles`. Queue `load` entries for
baking/upload of height and normal pages, prioritising camera distance with key
order as the tie-breaker and bounded CPU/upload work per frame. Cache keys also
include terrain identity and evaluation revision: an edit can invalidate page
content without changing a selection key. Ignore obsolete asynchronous results.

Retain resident ancestors until every replacement child is ready, then switch
the covering draw set together. Keep parent pages pinned while children sample
them for morphing. Treat `unload` entries as eviction candidates; release them
only after replacement coverage, parent dependencies and in-flight GPU work no
longer need them. A memory budget and short retention period can absorb camera
oscillation without repeated baking. Morph-only changes need no residency diff.

## WebGPU visibility and the WebGL2 path

For WebGPU only, adapt the approach of RedGPU's
`lib/redgpu/src/landscape/core/shader/landscapeCull.wgsl`: test selected
patch AABBs against frustum planes in compute, compact visible patch records
with bounded atomic counters, and fill indexed indirect draw counts. Its
64-thread workgroups and optional HZB rejection are useful starting points.
Keep our CPU-selected levels and balancing; independently choosing GPU levels
would invalidate the selected coverage. Reset counters before dispatch, size
output buffers for the full selected set, and include both eyes conservatively
for XR. Add HZB only with bounds for morph displacement and our reverse-Z depth
convention verified. Preserve Redcamel's MIT notice wherever formulas are adapted.

WebGL2 keeps CPU selection and CPU frustum culling, followed by ordinary indexed
draws; it needs neither compute nor GPU readback. Visibility does not remove
tiles from the desired residency set on either backend.

## Open questions

- Measure whether 32 quads gives acceptable error for the finest grid spacing;
  choose range defaults and a page resolution/format from that error and memory.
- Specify and test edge collapse, neighbour morph synchronisation, clipping and
  parent transition timing; the selector's tile-wide factor alone is insufficient
  to guarantee continuous, crack-free rendering.
- Decide how to combine edited source grids across coarse patches and expose
  their bounds/revisions without losing erosion margins or shared normals.
- Choose cache limits, asynchronous bake cancellation and hysteresis, including
  parent fallback when the camera outruns streaming.
- Validate texture formats and vertex sampling on WebGL2, and measure CPU versus
  compute culling costs, XR visibility and HZB conservatism on target devices.

This is a renderer integration plan; this handoff adds no renderer code.
