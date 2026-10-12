// Checked, owned cross sections. No borrowed input pointers survive a call.
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <memory>
#include <new>

#include "extrusion.hpp"
#include "manifold/cross_section.h"
#include "profile_validation.hpp"
#include "solid_handle.hpp"
#include "ve_kernel.h"

struct ve_profile {
  manifold::CrossSection value;
};
namespace {
using manifold::CrossSection;
using manifold::Manifold;
namespace validation = ve_profile_validation;
void status_out(ve_status* out, ve_status status) {
  if (out != nullptr) {
    *out = status;
  }
}
ve_profile* wrap_profile(CrossSection value, ve_status* status) {
  if (!validation::bounded(value.ToPolygons())) {
    status_out(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  std::unique_ptr<ve_profile> profile(new (std::nothrow) ve_profile{std::move(value)});
  status_out(status, profile ? VE_OK : VE_ERR_INTERNAL);
  return profile.release();
}
ve_solid* wrap_solid(Manifold value, ve_status* status) {
  if (value.Status() != Manifold::Error::NoError) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  if (value.NumTri() > 2000000) {
    status_out(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  if (!value.IsEmpty()) {
    const auto box = value.BoundingBox();
    for (const double value : {box.min.x, box.min.y, box.min.z, box.max.x, box.max.y, box.max.z}) {
      if (!std::isfinite(value) || std::abs(value) > validation::kExtent) {
        status_out(status, VE_ERR_OUT_OF_BOUNDS);
        return nullptr;
      }
    }
  }
  std::unique_ptr<ve_solid> solid(new (std::nothrow) ve_solid{std::move(value)});
  status_out(status, solid ? VE_OK : VE_ERR_INTERNAL);
  return solid.release();
}
}  // namespace
extern "C" {
ve_profile* ve_profile_new(const double* points, size_t point_count, const size_t* offsets,
                           size_t loop_count, ve_status* status) {
  if (loop_count > validation::kMaxLoops || point_count > validation::kMaxPoints ||
      offsets == nullptr || offsets[0] != 0 || offsets[loop_count] != point_count ||
      (point_count > 0 && points == nullptr)) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  manifold::Polygons loops;
  for (size_t i = 0; i < loop_count; ++i) {
    if (offsets[i] > offsets[i + 1] || offsets[i + 1] > point_count ||
        offsets[i + 1] - offsets[i] > 4096) {
      status_out(status, VE_ERR_INVALID_ARG);
      return nullptr;
    }
    manifold::SimplePolygon loop;
    for (size_t j = offsets[i]; j < offsets[i + 1]; ++j) {
      loop.emplace_back(points[2 * j], points[(2 * j) + 1]);
    }
    if (!validation::valid_loop(loop)) {
      status_out(status, VE_ERR_INVALID_ARG);
      return nullptr;
    }
    loops.push_back(std::move(loop));
  }
  return wrap_profile(CrossSection(loops, CrossSection::FillRule::EvenOdd), status);
}
void ve_profile_free(ve_profile* profile) { const std::unique_ptr<ve_profile> owned(profile); }
double ve_profile_area(const ve_profile* profile) {
  return profile == nullptr ? 0.0 : profile->value.Area();
}
ve_status ve_profile_counts(const ve_profile* profile, size_t* points, size_t* loops) {
  if (profile == nullptr || points == nullptr || loops == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  *points = profile->value.NumVert();
  *loops = profile->value.NumContour();
  return VE_OK;
}
ve_status ve_profile_copy(const ve_profile* profile, double* points, size_t point_capacity,
                          size_t* offsets, size_t offset_capacity) {
  if (profile == nullptr || offsets == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  const auto loops = profile->value.ToPolygons();
  if (point_capacity < 2 * profile->value.NumVert() || offset_capacity < loops.size() + 1 ||
      (profile->value.NumVert() > 0 && points == nullptr)) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  size_t next = 0;
  offsets[0] = 0;
  for (size_t i = 0; i < loops.size(); ++i) {
    for (const auto point : loops.at(i)) {
      points[2 * next] = point.x;
      points[(2 * next) + 1] = point.y;
      ++next;
    }
    offsets[i + 1] = next;
  }
  return VE_OK;
}
ve_profile* ve_profile_offset(const ve_profile* profile, double delta, int32_t join,
                              ve_status* status) {
  if (profile == nullptr || !std::isfinite(delta) || std::abs(delta) > validation::kExtent ||
      (join != VE_JOIN_MITER && join != VE_JOIN_ROUND)) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  const auto type =
      join == VE_JOIN_ROUND ? CrossSection::JoinType::Round : CrossSection::JoinType::Miter;
  return wrap_profile(profile->value.Offset(delta, type, 2.0, 64), status);
}
ve_profile* ve_profile_difference(const ve_profile* a, const ve_profile* b, ve_status* status) {
  if (a == nullptr || b == nullptr) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  return wrap_profile(a->value.Boolean(b->value, manifold::OpType::Subtract), status);
}
ve_solid* ve_profile_extrude(const ve_profile* profile, double height, int32_t divisions,
                             double twist, double top_x, double top_y, ve_status* status) {
  if (profile == nullptr || !std::isfinite(height) || height <= 0.0 ||
      height > validation::kExtent || divisions < 1 || divisions > 256 || !std::isfinite(twist) ||
      std::abs(twist) > 10000.0 || !std::isfinite(top_x) || !std::isfinite(top_y) || top_x <= 0.0 ||
      top_y <= 0.0 || top_x > validation::kExtent || top_y > validation::kExtent) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  if (profile->value.IsEmpty()) {
    return wrap_solid(Manifold(), status);
  }
  if (profile->value.NumVert() * size_t(divisions + 1) * 4 > 2000000) {
    status_out(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  return wrap_solid(
      ve_extrusion::balanced(profile->value.ToPolygons(), height, divisions, twist, {top_x, top_y}),
      status);
}
ve_solid* ve_profile_revolve(const ve_profile* profile, int32_t segments, double angle,
                             ve_status* status) {
  if (profile == nullptr || segments < 3 || segments > 512 || !std::isfinite(angle) ||
      angle <= 0.0 || angle > 2.0 * std::acos(-1.0)) {
    status_out(status, VE_ERR_INVALID_ARG);
    return nullptr;
  }
  if (profile->value.IsEmpty()) {
    return wrap_solid(Manifold(), status);
  }
  if (profile->value.NumVert() * size_t(segments + 1) * 2 > 2000000) {
    status_out(status, VE_ERR_TOO_LARGE);
    return nullptr;
  }
  const auto loops = profile->value.ToPolygons();
  for (const auto& loop : loops) {
    for (const auto point : loop) {
      if (point.x < 0.0) {
        status_out(status, VE_ERR_INVALID_ARG);
        return nullptr;
      }
    }
  }
  const int slices =
      std::max(3, static_cast<int>(std::ceil(double(segments) * angle / (2.0 * std::acos(-1.0)))));
  return wrap_solid(
      Manifold::Revolve(loops, slices, angle * 180.0 / std::acos(-1.0)).Rotate(-90.0, 0.0, 0.0),
      status);
}
}
