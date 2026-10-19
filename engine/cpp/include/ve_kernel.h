/* Geometry kernel C ABI.
 *
 * Thin boundary over Manifold (booleans, primitives, transforms) and meshoptimizer (simplify).
 * Rules: no exceptions cross this boundary; every fallible call reports through `ve_status`;
 * every handle is owned by the caller and released with `ve_solid_free`; inputs are validated
 * here as well as in the Rust wrapper (defence in depth). Units are metres, Y is up. */
#ifndef VE_KERNEL_H
#define VE_KERNEL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ve_solid ve_solid;
typedef struct ve_context ve_context;
typedef struct ve_profile ve_profile;
typedef struct ve_subdivision ve_subdivision;

typedef int32_t ve_status;
enum {
  VE_OK = 0,
  VE_ERR_INVALID_ARG = 1,
  VE_ERR_NOT_MANIFOLD = 2,
  VE_ERR_NON_FINITE = 3,
  VE_ERR_OUT_OF_BOUNDS = 4,
  VE_ERR_TOO_LARGE = 5,
  VE_ERR_BUFFER_TOO_SMALL = 6,
  VE_ERR_INTERNAL = 7,
  VE_ERR_CANCELLED = 8,
  VE_ERR_ATTRIBUTE_LIMIT = 9,
  VE_ERR_FIELD_CLIPPED = 10,
  VE_ERR_FIELD_EMPTY = 11,
  VE_ERR_FIELD_BUDGET = 12,
  /* A Blender operator ran and declined the input (for example a solidify of a non-manifold mesh).
   */
  VE_ERR_OP_REFUSED = 13
};

/* Upper bound on circular segments, so parameters cannot request unbounded work. */
#define VE_MAX_SEGMENTS 4096

/* Constructors. On failure they return NULL and set *status (status may be NULL). */
ve_solid* ve_solid_cube(double x, double y, double z, int32_t center, ve_status* status);
ve_solid* ve_solid_sphere(double radius, int32_t segments, ve_status* status);
ve_solid* ve_solid_cylinder(double height, double radius_low, double radius_high, int32_t segments,
                            int32_t center, ve_status* status);
/* `positions` holds 3*vertex_count floats; `indices` holds index_count (multiple of 3) entries. */
ve_solid* ve_solid_from_mesh(const float* positions, size_t vertex_count, const uint32_t* indices,
                             size_t index_count, ve_status* status);

/* Origin tracking. `ve_reserve_ids(n)` returns the first of n fresh, consecutive origin ids. A mesh
 * given in runs (triangle ranges starting at run_first_triangle[i], ascending, the first at 0)
 * labels each run with run_ids[i] (ascending); booleans and transforms keep those labels on the
 * surviving faces, new faces from a cutter take the cutter's label. */
uint32_t ve_reserve_ids(uint32_t count);
ve_solid* ve_solid_from_mesh_runs(const float* positions, size_t vertex_count,
                                  const uint32_t* indices, size_t index_count,
                                  const size_t* run_first_triangle, const uint32_t* run_ids,
                                  size_t run_count, ve_status* status);
/* The runs of the mesh `ve_solid_copy_mesh` copies, in the same order: first triangle and origin id
 * of each, ascending by id. */
size_t ve_solid_num_runs(const ve_solid* s);
ve_status ve_solid_copy_runs(const ve_solid* s, size_t* first_triangle, uint32_t* ids,
                             size_t capacity);

/* MikkTSpace tangents for a triangle mesh with per-vertex positions (3 floats), normals (3) and uvs
 * (2). `indices` holds index_count entries (a multiple of 3; at most 2,000,000 triangles). Writes
 * four floats per index, in index order: the tangent (xyz) and the bitangent sign (+1 or -1).
 * Refuses non-finite values and out-of-range indices. */
ve_status ve_tangents(const float* positions, const float* normals, const float* uvs,
                      size_t vertex_count, const uint32_t* indices, size_t index_count,
                      float* out_tangents);

/* Booleans and transforms return a new solid; inputs are not consumed. */
ve_solid* ve_solid_union(const ve_solid* a, const ve_solid* b, ve_status* status);
ve_solid* ve_solid_difference(const ve_solid* a, const ve_solid* b, ve_status* status);
ve_solid* ve_solid_intersect(const ve_solid* a, const ve_solid* b, ve_status* status);
/* Shrinks by a sphere then expands by the same sphere to round convex edges. */
ve_solid* ve_solid_round(const ve_solid* solid, double radius, int32_t segments, ve_status* status);
/* Shrinks by an octahedron then expands by it to cut convex edges with a flat 45-degree facet. */
ve_solid* ve_solid_chamfer(const ve_solid* solid, double width, ve_status* status);
ve_solid* ve_solid_translate(const ve_solid* s, double x, double y, double z, ve_status* status);
ve_solid* ve_solid_rotate(const ve_solid* s, double x_deg, double y_deg, double z_deg,
                          ve_status* status);
ve_solid* ve_solid_scale(const ve_solid* s, double x, double y, double z, ve_status* status);
/* Proper rigid transform, row-major 3x4 (translation in entries 3,7,11); count must be 12. */
ve_solid* ve_solid_transform(const ve_solid* s, const double* matrix, size_t count,
                             ve_status* status);

/* Queries (NULL handle yields 0). */
double ve_solid_volume(const ve_solid* s);
double ve_solid_surface_area(const ve_solid* s);
size_t ve_solid_num_verts(const ve_solid* s);
size_t ve_solid_num_tris(const ve_solid* s);
size_t ve_solid_num_components(const ve_solid* s);
int32_t ve_solid_is_empty(const ve_solid* s);
/* out = {min.x, min.y, min.z, max.x, max.y, max.z}. */
ve_status ve_solid_bounds(const ve_solid* s, double out[6]);

/* Copies the render mesh. Capacities are in elements (floats / indices); both buffers must hold
 * 3*num_verts floats and 3*num_tris indices. */
ve_status ve_solid_copy_mesh(const ve_solid* s, float* positions, size_t positions_capacity,
                             uint32_t* indices, size_t indices_capacity);

/* Register tape for deterministic, positive-inside spatial fields. Every input register must
 * refer to an earlier instruction; xyz/w are opcode-specific constants. */
typedef struct ve_field_instruction {
  uint32_t opcode;
  uint32_t input_a;
  uint32_t input_b;
  double x;
  double y;
  double z;
  double w;
} ve_field_instruction;
typedef struct ve_field_input {
  size_t struct_size;
  const ve_field_instruction* instructions;
  size_t instruction_count;
  double bounds[6];
  double edge_length;
  size_t max_cells;
  size_t max_operations;
  ve_context* context;
} ve_field_input;
enum {
  VE_FIELD_SPHERE = 1,
  VE_FIELD_BOX = 2,
  VE_FIELD_TORUS = 3,
  VE_FIELD_CAPSULE = 4,
  VE_FIELD_CYLINDER = 5,
  VE_FIELD_PLANE = 6,
  VE_FIELD_ELLIPSOID = 7,
  VE_FIELD_TUBE_POINT = 8,
  VE_FIELD_TUBE_SEGMENT = 9,
  VE_FIELD_UNION = 10,
  VE_FIELD_INTERSECTION = 11,
  VE_FIELD_DIFFERENCE = 12,
  VE_FIELD_SMOOTH_UNION = 13,
  VE_FIELD_MOVE = 20,
  VE_FIELD_SCALE = 21,
  VE_FIELD_MIRROR = 22,
  VE_FIELD_OFFSET = 23,
  VE_FIELD_SHELL = 24,
  VE_FIELD_REPEAT = 25
};
ve_solid* ve_solid_from_field(const ve_field_input* input, double clipped_point[3],
                              ve_status* status);
ve_status ve_field_sample(const ve_field_instruction* instructions, size_t instruction_count,
                          const double point[3], double* out_value);

/* Mesh simplification (meshoptimizer). Writes at most `out_capacity` indices, returns the count in
 * *out_count. `target_ratio` in (0,1], `target_error` is relative to the mesh extent. */
ve_status ve_simplify(const float* positions, size_t vertex_count, const uint32_t* indices,
                      size_t index_count, double target_ratio, double target_error,
                      uint32_t* out_indices, size_t out_capacity, size_t* out_count);

/* Versioned input for uniform CPU Catmull-Clark. Crease endpoints are pairs of cage vertex
 * indices. Point channels are interleaved vertex-major floats. The optional FVar channel carries
 * `fvar_width` floats for each unique value and one value index per cage corner. */
typedef struct ve_subdivide_input {
  size_t struct_size;
  const float* positions;
  size_t vertex_count;
  const uint32_t* face_sizes;
  size_t face_count;
  const uint32_t* corners;
  size_t corner_count;
  const uint32_t* crease_edges;
  const float* crease_weights;
  size_t crease_count;
  const uint32_t* corner_vertices;
  const float* corner_weights;
  size_t corner_count_values;
  const uint32_t* hole_faces;
  size_t hole_count;
  uint32_t levels;
  int32_t boundary_rule;
  int32_t crease_method;
  const float* point_values;
  size_t point_channels;
  const float* fvar_values;
  size_t fvar_value_count;
  uint32_t fvar_width;
  const uint32_t* fvar_corner_values;
  const ve_context* context;
} ve_subdivide_input;

/* Boundary rules are 0 none, 1 edge-only, 2 edge-and-corner; crease method is 0 uniform or 1
 * Chaikin. Returned vertex provenance identifies its parent in the immediately coarser level.
 * Face provenance resolves to the level-zero cage face. */
ve_subdivision* ve_subdivide_catmark(const ve_subdivide_input* input, ve_status* status);
size_t ve_subdivision_num_vertices(const ve_subdivision* subdivision);
size_t ve_subdivision_num_faces(const ve_subdivision* subdivision);
size_t ve_subdivision_num_corners(const ve_subdivision* subdivision);
size_t ve_subdivision_num_point_channels(const ve_subdivision* subdivision);
size_t ve_subdivision_num_fvar_values(const ve_subdivision* subdivision);
ve_status ve_subdivision_copy(const ve_subdivision* subdivision, float* positions,
                              size_t positions_capacity, uint32_t* face_sizes, size_t face_capacity,
                              uint32_t* corners, size_t corner_capacity,
                              uint32_t* vertex_origin_kind, uint32_t* vertex_origin_index,
                              size_t vertex_origin_capacity, uint32_t* face_parent,
                              size_t face_parent_capacity);
ve_status ve_subdivision_copy_channels(const ve_subdivision* subdivision, float* point_values,
                                       size_t point_values_capacity, float* fvar_values,
                                       size_t fvar_values_capacity, uint32_t* fvar_corner_values,
                                       size_t fvar_corner_capacity);
void ve_subdivision_free(ve_subdivision* subdivision);

/* Polygon meshes through Blender's bmesh (vendored Blender code, see third_party/blender). The
 * input is an indexed polygon mesh: `face_sizes[i]` corners per face, `corners` holds vertex
 * indices face after face. Faces need at least 3 distinct corners; at most 1,000,000 vertices,
 * 1,000,000 faces and 4,000,000 corners. The result keeps provenance: a vertex's origin is the
 * input vertex it descends from and a face's parent is the input face it inherits attributes from,
 * or 0xFFFFFFFF when new. */
typedef struct ve_polymesh ve_polymesh;
typedef struct ve_polymesh_input {
  size_t struct_size;
  const float* positions;
  size_t vertex_count;
  const uint32_t* face_sizes;
  size_t face_count;
  const uint32_t* corners;
  size_t corner_count;
} ve_polymesh_input;
size_t ve_polymesh_num_vertices(const ve_polymesh* mesh);
size_t ve_polymesh_num_faces(const ve_polymesh* mesh);
size_t ve_polymesh_num_corners(const ve_polymesh* mesh);
ve_status ve_polymesh_copy(const ve_polymesh* mesh, float* positions, size_t position_capacity,
                           uint32_t* face_sizes, size_t face_capacity, uint32_t* corners,
                           size_t corner_capacity, uint32_t* vertex_origin, size_t vertex_capacity,
                           uint32_t* face_parent, size_t parent_capacity);
/* What an operator made each face: 0 not made or changed by it, 1 the inner face of an inset or the
 * moved cap of an extrusion, 2 a ring face of an inset or a side wall of an extrusion. One value
 * per face, in the face order of ve_polymesh_copy. */
ve_status ve_polymesh_copy_roles(const ve_polymesh* mesh, uint32_t* roles, size_t capacity);
void ve_polymesh_free(ve_polymesh* mesh);

/* Edge bevel (Blender's bmesh bevel). `edges` holds vertex-index pairs, each an edge of the mesh
 * and manifold (two faces), else VE_ERR_NOT_MANIFOLD. `offset_type` is 0 offset, 1 width, 2 depth,
 * 3 percent; `width` > 0; `segments` 1..64; `profile` 0.1..1 (0.5 is a circular arc). The offset is
 * clamped so the bevel never overlaps itself. */
typedef struct ve_bevel_input {
  size_t struct_size;
  ve_polymesh_input mesh;
  const uint32_t* edges;
  size_t edge_count;
  float width;
  int32_t segments;
  int32_t offset_type;
  float profile;
} ve_bevel_input;
ve_polymesh* ve_bevel(const ve_bevel_input* input, ve_status* status);

/* Blender's bmesh operators on a polygon mesh, selected by faces or edges. Faces are indices into
 * the mesh's face list; edges are vertex-index pairs, each an edge of the mesh. An empty face
 * selection means "every face" where the operator allows it. `params` and `flags` by operator:
 *   INSET        faces; params[0] thickness > 0, params[1] depth; flags 1 even, 2 relative, 4
 * boundary, 8 outset, 16 edge rail. SOLIDIFY     faces (empty: all); params[0] thickness
 * (non-zero), moved along each face's normal. EXTRUDE      faces; params[0..2] offset vector the
 * new faces move by. DISSOLVE     the whole mesh; params[0] angle in radians, 0..pi (faces flatter
 * than this merge); flags 1 also dissolve boundaries. POKE         faces (empty: all); params[0]
 * offset; params[1] centre 0 median, 1 weighted, 2 bounds; flags 1 offset relative to the face
 * size. SUBDIVIDE    edges; params[0] cuts 1..8, params[1] smooth 0..1; flags 1 grid fill quads.
 *   WIREFRAME    faces (empty: all); params[0] thickness > 0, params[1] offset; flags 1 even,
 *                2 relative, 4 boundary, 8 replace the original faces.
 *   SYMMETRIZE   the whole mesh; params[0] direction 0..5 = -X -Y -Z +X +Y +Z (the side kept and
 *                mirrored onto the other), params[1] merge distance >= 0.
 *   DECIMATE     the whole mesh; params[0] ratio of faces to keep, 0 < ratio <= 1; params[1]
 * symmetry axis 0 x, 1 y, 2 z or -1 for none; flags 1 triangulate first (needed unless the mesh is
 * already triangles). Collapses edges that change the shape least: level of detail. UNSUBDIVIDE the
 * whole mesh; params[0] iterations 1..4. Reverses a subdivision, removing the vertices a regular
 * grid of quads could be rebuilt without. The result keeps provenance as for ve_bevel. */
enum {
  VE_MESH_OP_INSET = 0,
  VE_MESH_OP_SOLIDIFY = 1,
  VE_MESH_OP_EXTRUDE = 2,
  VE_MESH_OP_DISSOLVE = 3,
  VE_MESH_OP_POKE = 4,
  VE_MESH_OP_SUBDIVIDE = 5,
  VE_MESH_OP_WIREFRAME = 6,
  VE_MESH_OP_SYMMETRIZE = 7,
  VE_MESH_OP_DECIMATE = 8,
  VE_MESH_OP_UNSUBDIVIDE = 9,
  VE_MESH_OP_TRIANGULATE = 10,
  VE_MESH_OP_REMOVE_DOUBLES = 11,
  VE_MESH_OP_BISECT = 12,
  VE_MESH_OP_SPIN = 13,
  VE_MESH_OP_MIRROR = 14,
};
typedef struct ve_mesh_op_input {
  size_t struct_size;
  ve_polymesh_input mesh;
  int32_t op;
  const uint32_t* faces;
  size_t face_count;
  const uint32_t* edges;
  size_t edge_count;
  const uint32_t* vertices;
  size_t vertex_count;
  double params[12]; /* Operator numbers; unused entries are zero (most operators read the first
                        four). */
  int32_t flags;
} ve_mesh_op_input;
ve_polymesh* ve_mesh_op(const ve_mesh_op_input* input, ve_status* status);

/* UV unwrapping (Blender's LSCM / ABF++ parametrizer and island packer). `seams` holds vertex-index
 * pairs, each an edge of the mesh, that cut it into islands; every island is flattened on its own.
 * `method` is 0 for least-squares conformal maps or 1 for angle-based flattening (less angle
 * distortion, slower). With `pack` the islands are scaled into the unit square with `margin`
 * (0..0.5 of it) between them, optionally rotated; without it they are left overlapping near the
 * origin. Writes two floats per corner (u, v), corner after corner in the mesh's order, into `uvs`
 * (capacity in floats, 2 * corner_count needed). *failed_islands counts islands that could not be
 * flattened and keep their previous (zero) coordinates; it may be NULL. */
typedef struct ve_uv_input {
  size_t struct_size;
  ve_polymesh_input mesh;
  const uint32_t* seams;
  size_t seam_count;
  int32_t method;
  int32_t pack;
  int32_t rotate;
  float margin;
} ve_uv_input;
ve_status ve_uv_unwrap(const ve_uv_input* input, float* uvs, size_t uv_capacity,
                       uint32_t* failed_islands);
/* Packs an existing corner UV map. UV seams are inferred from discontinuities. `shape` is 0 for
 * axis-aligned bounds and 1 for convex hulls; `keep_density` preserves island scale where possible.
 */
typedef struct ve_uv_pack_input {
  size_t struct_size;
  ve_polymesh_input mesh;
  const float* uvs;
  int32_t rotate;
  int32_t keep_density;
  int32_t shape;
  float margin;
} ve_uv_pack_input;
ve_status ve_uv_pack(const ve_uv_pack_input* input, float* uvs, size_t uv_capacity);

/* Blender's easing curve (BLI_easing, "ease out", keyframe defaults) at `fraction` in 0..1, for
 * checking the engine's own copies. kind: 0 back, 1 bounce, 2 elastic; anything else returns
 * `fraction`. */
float ve_blender_ease(int32_t kind, float fraction);

void ve_solid_free(ve_solid* s);

/* Profiles: flat XY doubles, offsets[loop_count+1], even-odd holes. Max 64 loops,
 * 4096 points per loop and 16384 total points. Self intersections are rejected. */
ve_profile* ve_profile_new(const double* points, size_t point_count, const size_t* offsets,
                           size_t loop_count, ve_status* status);
void ve_profile_free(ve_profile* profile);
double ve_profile_area(const ve_profile* profile);
ve_status ve_profile_counts(const ve_profile* profile, size_t* points, size_t* loops);
ve_status ve_profile_copy(const ve_profile* profile, double* points, size_t point_capacity,
                          size_t* offsets, size_t offset_capacity);
/* Corner treatment of an offset. Round joins use 64 segments per full circle. */
typedef enum { VE_JOIN_MITER = 0, VE_JOIN_ROUND = 1 } ve_join;
/* Miter corners (limit 2) or round corners. Empty erosion is a valid empty profile. */
ve_profile* ve_profile_offset(const ve_profile* profile, double delta, int32_t join,
                              ve_status* status);
ve_profile* ve_profile_difference(const ve_profile* a, const ve_profile* b, ve_status* status);
/* Radians, Y-up output. Divisions count slabs (1..256); segments 3..512. */
ve_solid* ve_profile_extrude(const ve_profile* profile, double height, int32_t divisions,
                             double twist, double top_x, double top_y, ve_status* status);
ve_solid* ve_profile_revolve(const ve_profile* profile, int32_t segments, double angle,
                             ve_status* status);

/* Cancellation is permanent; use a fresh context to retry. Status evaluation observes it;
 * mesh extraction and unrelated queries do not. Operations are 0 union, 1 difference, 2 intersect.
 */
ve_context* ve_context_new(void);
ve_solid* ve_solid_batch_union_context(const ve_solid* const* solids, size_t count,
                                       const ve_context* context, ve_status* status);
void ve_context_free(ve_context* context);
void ve_context_cancel(ve_context* context);
int32_t ve_context_cancelled(const ve_context* context);
ve_solid* ve_solid_boolean_context(const ve_solid* a, const ve_solid* b, int32_t operation,
                                   const ve_context* context, ve_status* status);

#ifdef __cplusplus
}
#endif
#endif /* VE_KERNEL_H */
