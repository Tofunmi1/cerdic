// Bounded Catmull-Clark point primvars and face-varying channels.
#include <opensubdiv/far/primvarRefiner.h>
#include <opensubdiv/far/topologyLevel.h>

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <vector>

#include "subdivision_internal.h"

namespace Far = OpenSubdiv::Far;

namespace {

template <size_t Width>
class PointValue {
 public:
  void Clear(void* client_data = nullptr) {
    static_cast<void>(client_data);
    values_.fill(0.0F);
  }

  void AddWithWeight(const PointValue& source, float weight) {
    for (size_t channel = 0; channel < Width; ++channel) {
      values_.at(channel) += source.values_.at(channel) * weight;
    }
  }

  void Set(size_t channel, float value) { values_.at(channel) = value; }

  [[nodiscard]] const std::array<float, Width>& Values() const { return values_; }

 private:
  std::array<float, Width> values_{};
};

class Uv {
 public:
  void Clear(void* client_data = nullptr) {
    static_cast<void>(client_data);
    values_ = {0.0F, 0.0F};
  }

  void AddWithWeight(const Uv& source, float weight) {
    for (size_t channel = 0; channel < values_.size(); ++channel) {
      values_.at(channel) += source.values_.at(channel) * weight;
    }
  }

  void Set(float u, float v) { values_ = {u, v}; }

  [[nodiscard]] const std::array<float, 2>& Values() const { return values_; }

 private:
  std::array<float, 2> values_{};
};

template <size_t Width>
ve_status interpolate_points(const ve_subdivide_input& input, const Far::TopologyRefiner& refiner,
                             ve_subdivision* output) {
  std::vector<PointValue<Width>> current(input.vertex_count);
  for (size_t vertex = 0; vertex < input.vertex_count; ++vertex) {
    for (size_t channel = 0; channel < Width; ++channel) {
      current.at(vertex).Set(channel, *(input.point_values + (vertex * Width) + channel));
    }
  }
  const Far::PrimvarRefiner primvar(refiner);
  for (uint32_t level = 1; level <= input.levels; ++level) {
    if (ve_context_cancelled(input.context) != 0) {
      return VE_ERR_CANCELLED;
    }
    const auto count =
        static_cast<size_t>(refiner.GetLevel(static_cast<int>(level)).GetNumVertices());
    std::vector<PointValue<Width>> next(count);
    auto* source = current.data();
    auto* destination = next.data();
    primvar.Interpolate(static_cast<int>(level), source, destination);
    current = std::move(next);
  }
  if (current.size() !=
      static_cast<size_t>(refiner.GetLevel(static_cast<int>(input.levels)).GetNumVertices())) {
    return VE_ERR_INTERNAL;
  }
  output->point_channels = Width;
  output->point_values.reserve(current.size() * Width);
  for (const auto& vertex : current) {
    const auto values = vertex.Values();
    output->point_values.insert(output->point_values.end(), values.begin(), values.end());
  }
  return VE_OK;
}

ve_status interpolate_points(const ve_subdivide_input& input, const Far::TopologyRefiner& refiner,
                             ve_subdivision* output) {
  switch (input.point_channels) {
    case 0:
      output->point_channels = 0;
      return VE_OK;
    case 1:
      return interpolate_points<1>(input, refiner, output);
    case 2:
      return interpolate_points<2>(input, refiner, output);
    case 3:
      return interpolate_points<3>(input, refiner, output);
    case 4:
      return interpolate_points<4>(input, refiner, output);
    case 5:
      return interpolate_points<5>(input, refiner, output);
    case 6:
      return interpolate_points<6>(input, refiner, output);
    case 7:
      return interpolate_points<7>(input, refiner, output);
    case 8:
      return interpolate_points<8>(input, refiner, output);
    default:
      return VE_ERR_ATTRIBUTE_LIMIT;
  }
}

ve_status interpolate_fvar(const ve_subdivide_input& input, const Far::TopologyRefiner& refiner,
                           ve_subdivision* output) {
  if (input.fvar_value_count == 0) {
    return VE_OK;
  }
  std::vector<Uv> current(input.fvar_value_count);
  for (size_t value = 0; value < input.fvar_value_count; ++value) {
    current.at(value).Set(*(input.fvar_values + (value * 2)),
                          *(input.fvar_values + (value * 2) + 1));
  }
  const Far::PrimvarRefiner primvar(refiner);
  for (uint32_t level = 1; level <= input.levels; ++level) {
    if (ve_context_cancelled(input.context) != 0) {
      return VE_ERR_CANCELLED;
    }
    const auto count =
        static_cast<size_t>(refiner.GetLevel(static_cast<int>(level)).GetNumFVarValues(0));
    std::vector<Uv> next(count);
    auto* source = current.data();
    auto* destination = next.data();
    primvar.InterpolateFaceVarying(static_cast<int>(level), source, destination, 0);
    current = std::move(next);
  }
  output->fvar_width = 2;
  output->fvar_values.reserve(current.size() * 2);
  for (const auto& value : current) {
    const auto components = value.Values();
    output->fvar_values.insert(output->fvar_values.end(), components.begin(), components.end());
  }
  const auto& final_level = refiner.GetLevel(static_cast<int>(input.levels));
  output->fvar_corner_values.reserve(output->corners.size());
  for (int face = 0; face < final_level.GetNumFaces(); ++face) {
    const auto values = final_level.GetFaceFVarValues(face, 0);
    for (const int value : values) {
      if (value < 0 || static_cast<size_t>(value) >= current.size()) {
        return VE_ERR_INTERNAL;
      }
      output->fvar_corner_values.push_back(static_cast<uint32_t>(value));
    }
  }
  return output->fvar_corner_values.size() == output->corners.size() ? VE_OK : VE_ERR_INTERNAL;
}

}  // namespace

ve_status ve_subdivision_prepare_channels(const ve_subdivide_input& input,
                                          ve_subdivision_prepared* prepared) {
  if (input.fvar_value_count != 0) {
    if (input.fvar_value_count > static_cast<size_t>(std::numeric_limits<int>::max())) {
      return VE_ERR_ATTRIBUTE_LIMIT;
    }
    prepared->fvar_corner_values.reserve(input.corner_count);
    for (size_t corner = 0; corner < input.corner_count; ++corner) {
      if (input.fvar_corner_values[corner] >= input.fvar_value_count) {
        return VE_ERR_OUT_OF_BOUNDS;
      }
      prepared->fvar_corner_values.push_back(static_cast<int>(input.fvar_corner_values[corner]));
    }
    if (std::any_of(input.fvar_values,
                    input.fvar_values + (input.fvar_value_count * input.fvar_width),
                    [](float value) { return !std::isfinite(value); })) {
      return VE_ERR_NON_FINITE;
    }
  }
  prepared->crease_pairs.reserve(input.crease_count * 2);
  for (size_t crease = 0; crease < input.crease_count; ++crease) {
    const size_t first = crease * 2;
    const uint32_t a = input.crease_edges[first];
    const uint32_t b = input.crease_edges[first + 1];
    if (!std::isfinite(input.crease_weights[crease]) || input.crease_weights[crease] < 0.0F ||
        a >= input.vertex_count || b >= input.vertex_count || a == b) {
      return VE_ERR_INVALID_ARG;
    }
    prepared->crease_pairs.push_back(static_cast<int>(a));
    prepared->crease_pairs.push_back(static_cast<int>(b));
  }
  prepared->corner_vertices.reserve(input.corner_count_values);
  for (size_t corner = 0; corner < input.corner_count_values; ++corner) {
    if (!std::isfinite(input.corner_weights[corner]) || input.corner_weights[corner] < 0.0F ||
        input.corner_vertices[corner] >= input.vertex_count) {
      return VE_ERR_INVALID_ARG;
    }
    prepared->corner_vertices.push_back(static_cast<int>(input.corner_vertices[corner]));
  }
  return VE_OK;
}

ve_status ve_subdivision_interpolate_channels(const ve_subdivide_input& input,
                                              const Far::TopologyRefiner& refiner,
                                              ve_subdivision* output) {
  const auto point_status = interpolate_points(input, refiner, output);
  return point_status == VE_OK ? interpolate_fvar(input, refiner, output) : point_status;
}

extern "C" {

size_t ve_subdivision_num_vertices(const ve_subdivision* subdivision) {
  return subdivision == nullptr ? 0 : subdivision->positions.size() / 3;
}
size_t ve_subdivision_num_faces(const ve_subdivision* subdivision) {
  return subdivision == nullptr ? 0 : subdivision->face_sizes.size();
}
size_t ve_subdivision_num_corners(const ve_subdivision* subdivision) {
  return subdivision == nullptr ? 0 : subdivision->corners.size();
}
size_t ve_subdivision_num_point_channels(const ve_subdivision* subdivision) {
  return subdivision == nullptr ? 0 : subdivision->point_channels;
}
size_t ve_subdivision_num_fvar_values(const ve_subdivision* subdivision) {
  return subdivision == nullptr || subdivision->fvar_width == 0
             ? 0
             : subdivision->fvar_values.size() / subdivision->fvar_width;
}
ve_status ve_subdivision_copy(const ve_subdivision* subdivision, float* positions,
                              size_t positions_capacity, uint32_t* face_sizes, size_t face_capacity,
                              uint32_t* corners, size_t corner_capacity,
                              uint32_t* vertex_origin_kind, uint32_t* vertex_origin_index,
                              size_t vertex_origin_capacity, uint32_t* face_parent,
                              size_t face_parent_capacity) {
  if (subdivision == nullptr || positions == nullptr || face_sizes == nullptr ||
      corners == nullptr || vertex_origin_kind == nullptr || vertex_origin_index == nullptr ||
      face_parent == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  if (positions_capacity < subdivision->positions.size() ||
      face_capacity < subdivision->face_sizes.size() ||
      corner_capacity < subdivision->corners.size() ||
      vertex_origin_capacity < subdivision->vertex_origin_kind.size() ||
      face_parent_capacity < subdivision->face_parent.size()) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  std::copy(subdivision->positions.begin(), subdivision->positions.end(), positions);
  std::copy(subdivision->face_sizes.begin(), subdivision->face_sizes.end(), face_sizes);
  std::copy(subdivision->corners.begin(), subdivision->corners.end(), corners);
  std::copy(subdivision->vertex_origin_kind.begin(), subdivision->vertex_origin_kind.end(),
            vertex_origin_kind);
  std::copy(subdivision->vertex_origin_index.begin(), subdivision->vertex_origin_index.end(),
            vertex_origin_index);
  std::copy(subdivision->face_parent.begin(), subdivision->face_parent.end(), face_parent);
  return VE_OK;
}
ve_status ve_subdivision_copy_channels(const ve_subdivision* subdivision, float* point_values,
                                       size_t point_values_capacity, float* fvar_values,
                                       size_t fvar_values_capacity, uint32_t* fvar_corner_values,
                                       size_t fvar_corner_capacity) {
  if (subdivision == nullptr || (!subdivision->point_values.empty() && point_values == nullptr) ||
      (!subdivision->fvar_values.empty() && fvar_values == nullptr) ||
      (!subdivision->fvar_corner_values.empty() && fvar_corner_values == nullptr)) {
    return VE_ERR_INVALID_ARG;
  }
  if (point_values_capacity < subdivision->point_values.size() ||
      fvar_values_capacity < subdivision->fvar_values.size() ||
      fvar_corner_capacity < subdivision->fvar_corner_values.size()) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  if (!subdivision->point_values.empty()) {
    std::copy(subdivision->point_values.begin(), subdivision->point_values.end(), point_values);
  }
  if (!subdivision->fvar_values.empty()) {
    std::copy(subdivision->fvar_values.begin(), subdivision->fvar_values.end(), fvar_values);
  }
  if (!subdivision->fvar_corner_values.empty()) {
    std::copy(subdivision->fvar_corner_values.begin(), subdivision->fvar_corner_values.end(),
              fvar_corner_values);
  }
  return VE_OK;
}

}  // extern "C"
