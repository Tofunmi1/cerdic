# ADR 0034: Curve tools, more deformers, terrain and surface placement

Status: proposed (owner confirmation pending). Built and tested 2026-10-17.

## Context
The plan (plan-animation-and-blender.md, batch 3) lists the Blender modelling pieces still missing: curve tools,
deformers, geometry queries and terrain. Blender's own code for these (curves geometry, simple deform, lattice, the
noise library, BVH trees) pulls large dependency closures for what are, here, a few hundred lines of arithmetic. The
rule from ADR 0026 applies: port small numeric kernels with Blender as the reference rather than vendoring a closure.

## Decision
1. **Curves.** `curve.spline` (centripetal Catmull-Rom, Barry-Goldman form), `curve.helix`, `curve.fillet` (a quadratic
   curve replaces each corner, starting and ending `distance` away, shortened on short segments), `curve.resample`
   (even arc-length spacing), `curve.reverse`; `solid.sweep` gets `scale_end` and `twist` (open curves only). All use
   `+ - * /` and `sqrt` (`sin`/`cos` for the helix and twist), so native and WASM agree; at most 8,192 samples.
2. **Deformers.** `xform.shear`, `xform.wave`, `xform.noise`, `xform.lattice` join twist, taper and bend as one family
   of point maps (`deform/maps.rs`) behind the same adaptive refinement, orientation check, fold refusal and Manifold
   validation. The ones that can ripple at any scale get 7 refinement passes under a 100,000 triangle cap. The lattice is
   a Bernstein free-form deformation over the bounding box with sparse control-point offsets, at most 4 points per axis.
3. **Noise** is one shared module (`noise.rs`): integer hashing, smooth trilinear value noise, fractal sums. The noise
   deformer and the terrain use it.
4. **Terrain.** `terrain.heightfield` is a closed solid slab: a regular grid of fractal heights, vertical walls and a flat
   base. Being an ordinary solid, it takes materials, booleans and export with no special cases. Resolution is capped at
   192 cells per side (about 74,000 triangles). `edge_falloff` makes islands; `flat_below` makes flat lake beds.
5. **Placement.** `inst.drop` lowers each copy straight down onto a surface solid: triangles are bucketed on an XZ grid,
   the highest hit under the copy's origin wins, ties break by triangle index; `offset`, `align` (tilt toward the
   slope) and `outside` (remove or keep the copies that miss). All misses is an error that names the surface extent.
6. Not built here (still open in the plan): NURBS, trim, mesh to curve and `curve_fit_nd`; deformers on `poly` meshes;
   BVH nearest and shrinkwrap; ocean and sky.

## Consequences
- A model can build landscapes, winding pipes, springs, horns, flags and lumpy rocks from a handful of typed nodes, each
  with a diagnostic that says how to fix a refusal.
- Every output is covered by the native vs WASI corpus (92 lines) and a real-model prompt set per family.
