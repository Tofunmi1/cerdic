// Geometry kernel: implementation of ve_kernel.h over Manifold and meshoptimizer.
#include <algorithm>
#include <array>
#include <cmath>
#include <memory>
#include <new>
#include <vector>

#include "context_handle.hpp"
#include "manifold/manifold.h"
#include "meshoptimizer.h"
#include "solid_handle.hpp"
#include "ve_kernel.h"

namespace {

using manifold::Manifold;

constexpr size_t kMaxMeshElements = size_t{1} << 28U;

void set_status(ve_status* out, ve_status value) {
  if (out != nullptr) {
    *out = value;
  }
}

bool finite_positive(double v) { return std::isfinite(v) && v > 0.0; }

ve_status map_error(Manifold::Error e) {
  switch (e) {
    case Manifold::Error::NoError:
      return VE_OK;
    case Manifold::Error::NonFiniteVertex:
      return VE_ERR_NON_FINITE;
    case Manifold::Error::NotManifold:
      return VE_ERR_NOT_MANIFOLD;
    case Manifold::Error::VertexOutOfBounds:
    case Manifold::Error::MergeIndexOutOfBounds:
      return VE_ERR_OUT_OF_BOUNDS;
    case Manifold::Error::ResultTooLarge:
      return VE_ERR_TOO_LARGE;
    case Manifold::Error::Cancelled:
      return VE_ERR_CANCELLED;
    default:
      return VE_ERR_INVALID_ARG;
  }
}

// Takes ownership of a finished manifold; reports its status.
ve_solid* wrap(Manifold m, ve_status* status) {
  const ve_status st = map_error(m.Status());
  set_status(status, st);
  if (st != VE_OK) {
    return nullptr;
  }
  std::unique_ptr<ve_solid> solid(new (std::nothrow) ve_solid{std::move(m)});
  if (!solid) {
    set_status(status, VE_ERR_INTERNAL);
    return nullptr;
  }
  return solid.release();
}

ve_solid* boolean(const ve_solid* a, const ve_solid* b, manifold::OpType op, ve_status* status) {
  if (a == nullptr || b == nullptr) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(a->m.Boolean(b->m, op), status);
}

bool all_finite(const std::array<double, 3>& v) {
  return std::all_of(v.begin(), v.end(), [](double d) { return std::isfinite(d); });
}

bool valid_triangles(const uint32_t* indices, size_t index_count, size_t vertex_count) {
  if (index_count % 3 != 0 || index_count > kMaxMeshElements || vertex_count > kMaxMeshElements) {
    return false;
  }
  return std::all_of(indices, indices + index_count,
                     [vertex_count](uint32_t i) { return i < vertex_count; });
}

}  // namespace

extern "C" {

ve_solid* ve_solid_transform(const ve_solid* solid, const double* matrix, size_t count,
                             ve_status* status) {
  if (solid == nullptr || matrix == nullptr || count != 12) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  const manifold::vec3 x = {matrix[0], matrix[4], matrix[8]};
  const manifold::vec3 y = {matrix[1], matrix[5], matrix[9]};
  const manifold::vec3 z = {matrix[2], matrix[6], matrix[10]};
  if (!std::all_of(matrix, matrix + 12,
                   [](double v) { return std::isfinite(v) && std::abs(v) <= 1.0e6; }) ||
      std::abs(manifold::la::dot(x, x) - 1.0) > 1e-9 ||
      std::abs(manifold::la::dot(y, y) - 1.0) > 1e-9 ||
      std::abs(manifold::la::dot(z, z) - 1.0) > 1e-9 || std::abs(manifold::la::dot(x, y)) > 1e-9 ||
      std::abs(manifold::la::dot(x, z)) > 1e-9 || std::abs(manifold::la::dot(y, z)) > 1e-9 ||
      std::abs(manifold::la::dot(x, manifold::la::cross(y, z)) - 1.0) > 1e-9) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  const manifold::mat3x4 transform(x, y, z, {matrix[3], matrix[7], matrix[11]});
  return wrap(solid->m.Transform(transform), status);
}

ve_solid* ve_solid_cube(double x, double y, double z, int32_t center, ve_status* status) {
  if (!finite_positive(x) || !finite_positive(y) || !finite_positive(z)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(Manifold::Cube({x, y, z}, center != 0), status);
}

ve_solid* ve_solid_sphere(double radius, int32_t segments, ve_status* status) {
  if (!finite_positive(radius) || segments < 3 || segments > VE_MAX_SEGMENTS) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(Manifold::Sphere(radius, segments), status);
}

ve_solid* ve_solid_cylinder(double height, double radius_low, double radius_high, int32_t segments,
                            int32_t center, ve_status* status) {
  const bool radii_ok = std::isfinite(radius_low) && std::isfinite(radius_high) &&
                        radius_low >= 0.0 && radius_high >= 0.0 &&
                        (radius_low > 0.0 || radius_high > 0.0);
  if (!finite_positive(height) || !radii_ok || segments < 3 || segments > VE_MAX_SEGMENTS) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  // Manifold builds cylinders along +Z; the engine is Y-up, so turn the axis onto +Y here, the one
  // place the convention is applied. A quarter turn about X takes +Z to +Y, so the low end stays at
  // y = 0 (or -height/2 when centred) and the high end at the top.
  return wrap(Manifold::Cylinder(height, radius_low, radius_high, segments, center != 0)
                  .Rotate(-90.0, 0.0, 0.0),
              status);
}

ve_solid* ve_solid_from_mesh(const float* positions, size_t vertex_count, const uint32_t* indices,
                             size_t index_count, ve_status* status) {
  if (positions == nullptr || indices == nullptr || vertex_count == 0 || index_count == 0 ||
      !valid_triangles(indices, index_count, vertex_count)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  manifold::MeshGL mesh;
  mesh.numProp = 3;
  mesh.vertProperties.assign(positions, positions + (vertex_count * 3));
  mesh.triVerts.assign(indices, indices + index_count);
  mesh.Merge();  // weld coincident vertices so independently listed triangles connect
  return wrap(Manifold(mesh), status);
}

uint32_t ve_reserve_ids(uint32_t count) { return Manifold::ReserveIDs(count); }

ve_solid* ve_solid_from_mesh_runs(const float* positions, size_t vertex_count,
                                  const uint32_t* indices, size_t index_count,
                                  const size_t* run_first_triangle, const uint32_t* run_ids,
                                  size_t run_count, ve_status* status) {
  const size_t triangles = index_count / 3;
  bool runs_ok = run_first_triangle != nullptr && run_ids != nullptr && run_count > 0 &&
                 run_count <= triangles && run_first_triangle[0] == 0;
  for (size_t r = 1; runs_ok && r < run_count; ++r) {
    runs_ok = run_first_triangle[r] > run_first_triangle[r - 1] &&
              run_first_triangle[r] < triangles && run_ids[r] >= run_ids[r - 1];
  }
  if (positions == nullptr || indices == nullptr || vertex_count == 0 || index_count == 0 ||
      !runs_ok || !valid_triangles(indices, index_count, vertex_count)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  manifold::MeshGL mesh;
  mesh.numProp = 3;
  mesh.vertProperties.assign(positions, positions + (vertex_count * 3));
  mesh.triVerts.assign(indices, indices + index_count);
  for (size_t r = 0; r < run_count; ++r) {
    mesh.runIndex.push_back(static_cast<uint32_t>(run_first_triangle[r] * 3));
    mesh.runOriginalID.push_back(run_ids[r]);
  }
  mesh.runIndex.push_back(static_cast<uint32_t>(index_count));
  mesh.Merge();
  return wrap(Manifold(mesh), status);
}

size_t ve_solid_num_runs(const ve_solid* s) { return s == nullptr ? 0 : s->m.GetMeshGL().NumRun(); }

ve_status ve_solid_copy_runs(const ve_solid* s, size_t* first_triangle, uint32_t* ids,
                             size_t capacity) {
  if (s == nullptr || (capacity != 0 && (first_triangle == nullptr || ids == nullptr))) {
    return VE_ERR_INVALID_ARG;
  }
  const manifold::MeshGL mesh = s->m.GetMeshGL();
  const size_t runs = mesh.NumRun();
  if (capacity < runs) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  std::transform(mesh.runIndex.data(), mesh.runIndex.data() + runs, first_triangle,
                 [](uint32_t index) { return static_cast<size_t>(index / 3); });
  std::copy_n(mesh.runOriginalID.data(), runs, ids);
  return VE_OK;
}

ve_solid* ve_solid_union(const ve_solid* a, const ve_solid* b, ve_status* status) {
  return boolean(a, b, manifold::OpType::Add, status);
}

ve_solid* ve_solid_difference(const ve_solid* a, const ve_solid* b, ve_status* status) {
  return boolean(a, b, manifold::OpType::Subtract, status);
}

ve_solid* ve_solid_intersect(const ve_solid* a, const ve_solid* b, ve_status* status) {
  return boolean(a, b, manifold::OpType::Intersect, status);
}

ve_solid* ve_solid_round(const ve_solid* solid, double radius, int32_t segments,
                         ve_status* status) {
  constexpr size_t kMaxRoundTriangles = 20'000;
  constexpr size_t kMaxRoundResultTriangles = 250'000;
  if (solid == nullptr || !finite_positive(radius) || segments < 6 || segments > 24) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  if (solid->m.NumTri() > kMaxRoundTriangles) {
    set_status(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  const Manifold ball = Manifold::Sphere(radius, segments);
  const Manifold core = solid->m.MinkowskiDifference(ball);
  if (core.Status() != Manifold::Error::NoError || core.IsEmpty()) {
    set_status(status, core.Status() == Manifold::Error::NoError ? VE_ERR_INVALID_ARG
                                                                 : map_error(core.Status()));
    return nullptr;
  }
  Manifold rounded = core.MinkowskiSum(ball);
  if (rounded.NumTri() > kMaxRoundResultTriangles) {
    set_status(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  return wrap(std::move(rounded), status);
}

ve_solid* ve_solid_chamfer(const ve_solid* solid, double width, ve_status* status) {
  constexpr size_t kMaxChamferTriangles = 20'000;
  constexpr size_t kMaxChamferResultTriangles = 250'000;
  if (solid == nullptr || !finite_positive(width)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  if (solid->m.NumTri() > kMaxChamferTriangles) {
    set_status(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  // The octahedron is the L1 unit ball: eroding and dilating by it cuts a convex edge with a flat
  // 45-degree facet rather than the circular arc a sphere leaves.
  const Manifold octahedron = Manifold::Hull(std::vector<manifold::vec3>{{width, 0.0, 0.0},
                                                                         {-width, 0.0, 0.0},
                                                                         {0.0, width, 0.0},
                                                                         {0.0, -width, 0.0},
                                                                         {0.0, 0.0, width},
                                                                         {0.0, 0.0, -width}});
  if (octahedron.Status() != Manifold::Error::NoError) {
    set_status(status, map_error(octahedron.Status()));
    return nullptr;
  }
  const Manifold core = solid->m.MinkowskiDifference(octahedron);
  if (core.Status() != Manifold::Error::NoError || core.IsEmpty()) {
    set_status(status, core.Status() == Manifold::Error::NoError ? VE_ERR_INVALID_ARG
                                                                 : map_error(core.Status()));
    return nullptr;
  }
  Manifold chamfered = core.MinkowskiSum(octahedron);
  if (chamfered.NumTri() > kMaxChamferResultTriangles) {
    set_status(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  return wrap(std::move(chamfered), status);
}

ve_solid* ve_solid_translate(const ve_solid* s, double x, double y, double z, ve_status* status) {
  const std::array<double, 3> v = {x, y, z};
  if (s == nullptr || !all_finite(v)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(s->m.Translate({x, y, z}), status);
}

ve_solid* ve_solid_rotate(const ve_solid* s, double x_deg, double y_deg, double z_deg,
                          ve_status* status) {
  const std::array<double, 3> v = {x_deg, y_deg, z_deg};
  if (s == nullptr || !all_finite(v)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(s->m.Rotate(x_deg, y_deg, z_deg), status);
}

ve_solid* ve_solid_scale(const ve_solid* s, double x, double y, double z, ve_status* status) {
  const std::array<double, 3> v = {x, y, z};
  if (s == nullptr || !all_finite(v) || x == 0.0 || y == 0.0 || z == 0.0) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap(s->m.Scale({x, y, z}), status);
}

double ve_solid_volume(const ve_solid* s) { return s == nullptr ? 0.0 : s->m.Volume(); }

double ve_solid_surface_area(const ve_solid* s) { return s == nullptr ? 0.0 : s->m.SurfaceArea(); }

size_t ve_solid_num_verts(const ve_solid* s) { return s == nullptr ? 0 : s->m.NumVert(); }

size_t ve_solid_num_tris(const ve_solid* s) { return s == nullptr ? 0 : s->m.NumTri(); }

size_t ve_solid_num_components(const ve_solid* s) {
  return s == nullptr ? 0 : s->m.Decompose().size();
}

int32_t ve_solid_is_empty(const ve_solid* s) { return s == nullptr || s->m.IsEmpty() ? 1 : 0; }

ve_status ve_solid_bounds(const ve_solid* s, double out[6]) {
  if (s == nullptr || out == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  const manifold::Box box = s->m.BoundingBox();
  out[0] = box.min.x;
  out[1] = box.min.y;
  out[2] = box.min.z;
  out[3] = box.max.x;
  out[4] = box.max.y;
  out[5] = box.max.z;
  return VE_OK;
}

ve_status ve_solid_copy_mesh(const ve_solid* s, float* positions, size_t positions_capacity,
                             uint32_t* indices, size_t indices_capacity) {
  if (s == nullptr || (positions == nullptr && positions_capacity != 0) ||
      (indices == nullptr && indices_capacity != 0)) {
    return VE_ERR_INVALID_ARG;
  }
  const manifold::MeshGL mesh = s->m.GetMeshGL();
  const size_t verts = mesh.NumVert();
  const size_t tris = mesh.NumTri();
  if (positions_capacity < verts * 3 || indices_capacity < tris * 3) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  const size_t stride = mesh.numProp;
  for (size_t v = 0; v < verts; ++v) {
    std::copy_n(mesh.vertProperties.data() + (v * stride), 3, positions + (v * 3));
  }
  std::copy_n(mesh.triVerts.data(), tris * 3, indices);
  return VE_OK;
}

ve_status ve_simplify(const float* positions, size_t vertex_count, const uint32_t* indices,
                      size_t index_count, double target_ratio, double target_error,
                      uint32_t* out_indices, size_t out_capacity, size_t* out_count) {
  if (positions == nullptr || indices == nullptr || out_indices == nullptr ||
      out_count == nullptr || !std::isfinite(target_ratio) || target_ratio <= 0.0 ||
      target_ratio > 1.0 || !std::isfinite(target_error) || target_error < 0.0 ||
      !valid_triangles(indices, index_count, vertex_count)) {
    return VE_ERR_INVALID_ARG;
  }
  if (out_capacity < index_count) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  const size_t target = std::max<size_t>(
      3, static_cast<size_t>(static_cast<double>(index_count) * target_ratio) / 3 * 3);
  float result_error = 0.0F;
  *out_count = meshopt_simplify(out_indices, indices, index_count, positions, vertex_count,
                                sizeof(float) * 3, target, static_cast<float>(target_error), 0,
                                &result_error);
  return VE_OK;
}

void ve_solid_free(ve_solid* s) { const std::unique_ptr<ve_solid> owner(s); }

ve_context* ve_context_new(void) {
  auto context = std::make_unique<ve_context>();
  return context.release();
}

ve_solid* ve_solid_batch_union_context(const ve_solid* const* solids, size_t count,
                                       const ve_context* context, ve_status* status) {
  if (solids == nullptr || context == nullptr || count < 2 || count > 64) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  std::vector<Manifold> values;
  values.reserve(count);
  for (size_t i = 0; i < count; ++i) {
    const auto* solid = *(solids + i);
    if (solid == nullptr) {
      set_status(status, VE_ERR_INVALID_ARG);
      return nullptr;
    }
    values.push_back(solid->m);
  }
  return wrap(Manifold::BatchBoolean(values, manifold::OpType::Add).WithContext(context->value),
              status);
}

void ve_context_free(ve_context* context) { const std::unique_ptr<ve_context> owner(context); }
void ve_context_cancel(ve_context* context) {
  if (context != nullptr) {
    context->value.Cancel();
  }
}
int32_t ve_context_cancelled(const ve_context* context) {
  return context != nullptr && context->value.Cancelled() ? 1 : 0;
}

ve_solid* ve_solid_boolean_context(const ve_solid* a, const ve_solid* b, int32_t operation,
                                   const ve_context* context, ve_status* status) {
  if (a == nullptr || b == nullptr || context == nullptr || operation < 0 || operation > 2) {
    set_status(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  auto op = manifold::OpType::Intersect;
  if (operation == 0) {
    op = manifold::OpType::Add;
  } else if (operation == 1) {
    op = manifold::OpType::Subtract;
  }
  auto result = a->m.Boolean(b->m, op).WithContext(context->value);
  return wrap(std::move(result), status);
}

}  // extern "C"
