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
  VE_ERR_CANCELLED = 8
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

/* Booleans and transforms return a new solid; inputs are not consumed. */
ve_solid* ve_solid_union(const ve_solid* a, const ve_solid* b, ve_status* status);
ve_solid* ve_solid_difference(const ve_solid* a, const ve_solid* b, ve_status* status);
ve_solid* ve_solid_intersect(const ve_solid* a, const ve_solid* b, ve_status* status);
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

/* Mesh simplification (meshoptimizer). Writes at most `out_capacity` indices, returns the count in
 * *out_count. `target_ratio` in (0,1], `target_error` is relative to the mesh extent. */
ve_status ve_simplify(const float* positions, size_t vertex_count, const uint32_t* indices,
                      size_t index_count, double target_ratio, double target_error,
                      uint32_t* out_indices, size_t out_capacity, size_t* out_count);

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
