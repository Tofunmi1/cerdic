// CPU Catmull-Clark adapter over the pinned OpenSubdiv Far backend.
#include <opensubdiv/far/primvarRefiner.h>
#include <opensubdiv/far/topologyDescriptor.h>
#include <opensubdiv/far/topologyRefiner.h>
#include <opensubdiv/far/topologyRefinerFactory.h>
#include <opensubdiv/sdc/options.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <new>
#include <vector>

#include "subdivision_internal.h"

namespace Far = OpenSubdiv::Far;
namespace Sdc = OpenSubdiv::Sdc;

namespace {

constexpr size_t kMaxSubdivisionVertices = 2'000'000;
constexpr size_t kMaxSubdivisionFaces = 2'000'000;
constexpr size_t kMaxSubdivisionCorners = 8'000'000;
constexpr size_t kMaxSubdivisionPointFloats = 32'000'000;

struct Position {
  void Clear(void* client_data = nullptr) {
    static_cast<void>(client_data);
    x_ = y_ = z_ = 0.0F;
  }
  void AddWithWeight(const Position& source, float weight) {
    x_ += weight * source.x_;
    y_ += weight * source.y_;
    z_ += weight * source.z_;
  }
  void SetPosition(float px, float py, float pz) {
    x_ = px;
    y_ = py;
    z_ = pz;
  }
  [[nodiscard]] std::array<float, 3> coordinates() const { return {x_, y_, z_}; }

 private:
  float x_{};
  float y_{};
  float z_{};
};

void set_status(ve_status* output, ve_status status) {
  if (output != nullptr) {
    *output = status;
  }
}

bool checked_add(size_t a, size_t b, size_t* output) {
  if (b > std::numeric_limits<size_t>::max() - a) {
    return false;
  }
  *output = a + b;
  return true;
}

ve_status validate_edges(size_t faces, const uint32_t* face_sizes, const uint32_t* indices,
                         size_t* edge_count) {
  std::map<std::pair<uint32_t, uint32_t>, std::pair<size_t, int>> edges;
  size_t offset = 0;
  for (size_t face = 0; face < faces; ++face) {
    for (size_t local = 0; local < face_sizes[face]; ++local) {
      const uint32_t a = indices[offset + local];
      const uint32_t b = indices[offset + ((local + 1) % face_sizes[face])];
      if (a == b) {
        return VE_ERR_NOT_MANIFOLD;
      }
      const auto key = std::minmax(a, b);
      auto& incidence = edges[key];
      ++incidence.first;
      incidence.second += (a < b) ? 1 : -1;
      if (incidence.first > 2) {
        return VE_ERR_NOT_MANIFOLD;
      }
    }
    offset += face_sizes[face];
  }
  for (const auto& edge : edges) {
    if (edge.second.first == 2 && edge.second.second != 0) {
      return VE_ERR_NOT_MANIFOLD;
    }
  }
  *edge_count = edges.size();
  return VE_OK;
}

ve_status growth_preflight(size_t vertices, size_t faces, size_t corners, size_t edges,
                           uint32_t levels, size_t* final_vertices, size_t* final_faces,
                           size_t* final_corners) {
  size_t current_vertices = vertices;
  size_t current_faces = faces;
  size_t current_corners = corners;
  size_t current_edges = edges;
  size_t total_vertices = vertices;
  for (uint32_t level = 0; level < levels; ++level) {
    size_t next_faces = 0;
    size_t next_corners = 0;
    size_t next_vertices = 0;
    size_t next_edges = 0;
    if (level == 0) {
      next_faces = corners;
    } else if (current_faces > std::numeric_limits<size_t>::max() / 4) {
      return VE_ERR_TOO_LARGE;
    } else {
      next_faces = current_faces * 4;
    }
    if (next_faces > std::numeric_limits<size_t>::max() / 4) {
      return VE_ERR_TOO_LARGE;
    }
    next_corners = next_faces * 4;
    if (next_faces > kMaxSubdivisionFaces || next_corners > kMaxSubdivisionCorners) {
      return VE_ERR_TOO_LARGE;
    }
    if (!checked_add(current_vertices, current_edges, &next_vertices) ||
        !checked_add(next_vertices, current_faces, &next_vertices) ||
        !checked_add(current_edges, current_edges, &next_edges) ||
        !checked_add(next_edges, current_corners, &next_edges) ||
        !checked_add(total_vertices, next_vertices, &total_vertices) ||
        next_vertices > kMaxSubdivisionVertices || next_faces > kMaxSubdivisionFaces ||
        next_corners > kMaxSubdivisionCorners || total_vertices > 2 * kMaxSubdivisionVertices ||
        next_vertices > static_cast<size_t>(std::numeric_limits<int>::max())) {
      return VE_ERR_TOO_LARGE;
    }
    current_vertices = next_vertices;
    current_edges = next_edges;
    current_faces = next_faces;
    current_corners = next_corners;
  }
  *final_vertices = current_vertices;
  *final_faces = current_faces;
  *final_corners = current_corners;
  return VE_OK;
}

ve_status preflight(size_t vertices, size_t faces, size_t corners, const uint32_t* face_sizes,
                    const uint32_t* indices, uint32_t levels, size_t* final_vertices,
                    size_t* final_faces, size_t* final_corners) {
  size_t edge_count = 0;
  const ve_status topology_status = validate_edges(faces, face_sizes, indices, &edge_count);
  if (topology_status != VE_OK) {
    return topology_status;
  }
  return growth_preflight(vertices, faces, corners, edge_count, levels, final_vertices, final_faces,
                          final_corners);
}

bool cancelled(const ve_context* context) { return ve_context_cancelled(context) != 0; }

using Input = ve_subdivide_input;
using Prepared = ve_subdivision_prepared;

bool valid_arguments(const Input& input) {
  if (input.struct_size < sizeof(ve_subdivide_input)) {
    return false;
  }
  const bool point_valid = input.point_channels == 0 || input.point_values != nullptr;
  const bool fvar_valid = (input.fvar_value_count == 0 && input.fvar_width == 0 &&
                           input.fvar_values == nullptr && input.fvar_corner_values == nullptr) ||
                          (input.fvar_value_count != 0 && input.fvar_width == 2 &&
                           input.fvar_values != nullptr && input.fvar_corner_values != nullptr);
  return input.positions != nullptr && input.face_sizes != nullptr && input.corners != nullptr &&
         point_valid && fvar_valid && input.vertex_count != 0 && input.face_count != 0 &&
         input.corner_count != 0 && input.levels != 0 && input.levels <= 4 &&
         input.context != nullptr && input.boundary_rule >= 0 && input.boundary_rule <= 2 &&
         input.crease_method >= 0 && input.crease_method <= 1 &&
         (input.crease_count == 0 ||
          (input.crease_edges != nullptr && input.crease_weights != nullptr)) &&
         (input.corner_count_values == 0 ||
          (input.corner_vertices != nullptr && input.corner_weights != nullptr)) &&
         (input.hole_count == 0 || input.hole_faces != nullptr);
}

ve_status prepare_input(const Input& input, Prepared* output) {
  if (!valid_arguments(input)) {
    return VE_ERR_INVALID_ARG;
  }
  if (input.vertex_count > kMaxSubdivisionVertices || input.face_count > kMaxSubdivisionFaces ||
      input.corner_count > kMaxSubdivisionCorners || input.crease_count > kMaxSubdivisionCorners ||
      input.corner_count_values > kMaxSubdivisionVertices) {
    return VE_ERR_TOO_LARGE;
  }
  if (input.point_channels > 8) {
    return VE_ERR_ATTRIBUTE_LIMIT;
  }
  if (input.point_channels != 0 &&
      std::any_of(input.point_values,
                  input.point_values + (input.vertex_count * input.point_channels),
                  [](float value) { return !std::isfinite(value); })) {
    return VE_ERR_NON_FINITE;
  }
  if (cancelled(input.context)) {
    return VE_ERR_CANCELLED;
  }
  size_t sum = 0;
  output->face_sizes.reserve(input.face_count);
  for (size_t face = 0; face < input.face_count; ++face) {
    const uint32_t size = input.face_sizes[face];
    if (size < 3 || size > static_cast<uint32_t>(std::numeric_limits<int>::max()) ||
        !checked_add(sum, size, &sum)) {
      return VE_ERR_INVALID_ARG;
    }
    output->face_sizes.push_back(static_cast<int>(size));
  }
  if (sum != input.corner_count) {
    return VE_ERR_INVALID_ARG;
  }
  if (std::any_of(input.positions, input.positions + (input.vertex_count * 3),
                  [](float value) { return !std::isfinite(value); })) {
    return VE_ERR_NON_FINITE;
  }
  if (std::any_of(input.corners, input.corners + input.corner_count,
                  [&input](uint32_t index) { return index >= input.vertex_count; })) {
    return VE_ERR_OUT_OF_BOUNDS;
  }
  if (input.hole_count != 0) {
    return VE_ERR_INVALID_ARG;
  }
  const ve_status growth =
      preflight(input.vertex_count, input.face_count, input.corner_count, input.face_sizes,
                input.corners, input.levels, &output->predicted_vertices, &output->predicted_faces,
                &output->predicted_corners);
  if (growth != VE_OK) {
    return growth;
  }
  if (input.point_channels != 0 &&
      output->predicted_vertices > kMaxSubdivisionPointFloats / input.point_channels) {
    return VE_ERR_ATTRIBUTE_LIMIT;
  }
  output->corners.reserve(input.corner_count);
  for (size_t corner = 0; corner < input.corner_count; ++corner) {
    output->corners.push_back(static_cast<int>(input.corners[corner]));
  }
  return ve_subdivision_prepare_channels(input, output);
}

std::unique_ptr<Far::TopologyRefiner> make_refiner(const Input& input, Prepared& prepared) {
  Far::TopologyDescriptor descriptor;
  descriptor.numVertices = static_cast<int>(input.vertex_count);
  descriptor.numFaces = static_cast<int>(input.face_count);
  descriptor.numVertsPerFace = prepared.face_sizes.data();
  descriptor.vertIndicesPerFace = prepared.corners.data();
  descriptor.numCreases = static_cast<int>(input.crease_count);
  descriptor.creaseVertexIndexPairs = prepared.crease_pairs.data();
  descriptor.creaseWeights = input.crease_weights;
  descriptor.numCorners = static_cast<int>(input.corner_count_values);
  descriptor.cornerVertexIndices = prepared.corner_vertices.data();
  descriptor.cornerWeights = input.corner_weights;
  descriptor.numFVarChannels = input.fvar_value_count == 0 ? 0 : 1;
  if (descriptor.numFVarChannels != 0) {
    prepared.fvar_channel.numValues = static_cast<int>(input.fvar_value_count);
    prepared.fvar_channel.valueIndices = prepared.fvar_corner_values.data();
    descriptor.fvarChannels = &prepared.fvar_channel;
  } else {
    descriptor.fvarChannels = nullptr;
  }
  descriptor.numHoles = 0;
  descriptor.holeIndices = nullptr;
  Sdc::Options options;
  options.SetVtxBoundaryInterpolation(
      static_cast<Sdc::Options::VtxBoundaryInterpolation>(input.boundary_rule));
  options.SetCreasingMethod(static_cast<Sdc::Options::CreasingMethod>(input.crease_method));
  options.SetFVarLinearInterpolation(Sdc::Options::FVAR_LINEAR_BOUNDARIES);
  using Factory = Far::TopologyRefinerFactory<Far::TopologyDescriptor>;
  return std::unique_ptr<Far::TopologyRefiner>(
      Factory::Create(descriptor, Factory::Options(Sdc::SCHEME_CATMARK, options)));
}

void set_vertex_origins(const Far::TopologyLevel& parent, std::vector<uint32_t>* kinds,
                        std::vector<uint32_t>* indices) {
  for (int vertex = 0; vertex < parent.GetNumVertices(); ++vertex) {
    const auto output = static_cast<size_t>(parent.GetVertexChildVertex(vertex));
    kinds->at(output) = 0;
    indices->at(output) = static_cast<uint32_t>(vertex);
  }
  std::map<std::pair<uint32_t, uint32_t>, uint32_t> edge_ordinals;
  for (int edge = 0; edge < parent.GetNumEdges(); ++edge) {
    const auto vertices = parent.GetEdgeVertices(edge);
    std::array<int, 2> endpoints{};
    std::copy_n(vertices.begin(), endpoints.size(), endpoints.begin());
    const auto first = static_cast<uint32_t>(endpoints.front());
    const auto second = static_cast<uint32_t>(endpoints.back());
    const std::pair<uint32_t, uint32_t> key{std::min(first, second), std::max(first, second)};
    edge_ordinals.emplace(key, 0);
  }
  uint32_t ordinal = 0;
  for (auto& [edge, index] : edge_ordinals) {
    static_cast<void>(edge);
    index = ordinal;
    ++ordinal;
  }
  for (int edge = 0; edge < parent.GetNumEdges(); ++edge) {
    const auto output = static_cast<size_t>(parent.GetEdgeChildVertex(edge));
    const auto vertices = parent.GetEdgeVertices(edge);
    std::array<int, 2> endpoints{};
    std::copy_n(vertices.begin(), endpoints.size(), endpoints.begin());
    const auto first = static_cast<uint32_t>(endpoints.front());
    const auto second = static_cast<uint32_t>(endpoints.back());
    const std::pair<uint32_t, uint32_t> key{std::min(first, second), std::max(first, second)};
    kinds->at(output) = 1;
    indices->at(output) = edge_ordinals.at(key);
  }
  for (int face = 0; face < parent.GetNumFaces(); ++face) {
    const auto output = static_cast<size_t>(parent.GetFaceChildVertex(face));
    kinds->at(output) = 2;
    indices->at(output) = static_cast<uint32_t>(face);
  }
}

ve_status interpolate_positions(const Input& input, const Far::TopologyRefiner& refiner,
                                size_t final_vertex_count, std::vector<Position>* values,
                                size_t* final_offset, std::vector<uint32_t>* origin_kinds,
                                std::vector<uint32_t>* origin_indices) {
  values->resize(static_cast<size_t>(refiner.GetNumVerticesTotal()));
  for (size_t vertex = 0; vertex < input.vertex_count; ++vertex) {
    values->at(vertex).SetPosition(input.positions[(vertex * 3)], input.positions[(vertex * 3) + 1],
                                   input.positions[(vertex * 3) + 2]);
  }
  origin_kinds->resize(final_vertex_count);
  origin_indices->resize(final_vertex_count);
  const Far::PrimvarRefiner primvar(refiner);
  size_t source_offset = 0;
  size_t destination_offset = input.vertex_count;
  for (uint32_t level = 1; level <= input.levels; ++level) {
    if (cancelled(input.context)) {
      return VE_ERR_CANCELLED;
    }
    Position* source = values->data() + source_offset;
    Position* destination = values->data() + destination_offset;
    primvar.Interpolate(static_cast<int>(level), source, destination);
    const auto& parent = refiner.GetLevel(static_cast<int>(level - 1));
    const auto& child = refiner.GetLevel(static_cast<int>(level));
    *final_offset = destination_offset;
    source_offset = destination_offset;
    destination_offset += static_cast<size_t>(child.GetNumVertices());
    if (level == input.levels) {
      set_vertex_origins(parent, origin_kinds, origin_indices);
    }
  }
  return cancelled(input.context) ? VE_ERR_CANCELLED : VE_OK;
}

void set_face_parents(const Far::TopologyRefiner& refiner, size_t face_count, uint32_t levels,
                      std::vector<uint32_t>* output) {
  std::vector<uint32_t> parents(face_count);
  for (size_t face = 0; face < face_count; ++face) {
    parents.at(face) = static_cast<uint32_t>(face);
  }
  for (uint32_t level = 1; level <= levels; ++level) {
    const auto& parent = refiner.GetLevel(static_cast<int>(level - 1));
    const auto& child = refiner.GetLevel(static_cast<int>(level));
    std::vector<uint32_t> next(static_cast<size_t>(child.GetNumFaces()));
    for (int face = 0; face < parent.GetNumFaces(); ++face) {
      const auto children = parent.GetFaceChildFaces(face);
      for (const int child_face : children) {
        next.at(static_cast<size_t>(child_face)) = parents.at(static_cast<size_t>(face));
      }
    }
    parents = std::move(next);
  }
  *output = std::move(parents);
}

std::unique_ptr<ve_subdivision> make_output(
    const Far::TopologyLevel& final_level, const std::vector<Position>& values, size_t final_offset,
    size_t final_vertex_count, size_t final_face_count, size_t final_corner_count,
    std::vector<uint32_t> origin_kinds, std::vector<uint32_t> origin_indices,
    const Far::TopologyRefiner& refiner, size_t input_faces, uint32_t levels) {
  std::unique_ptr<ve_subdivision> output(new (std::nothrow) ve_subdivision{});
  if (output == nullptr) {
    return nullptr;
  }
  output->positions.reserve(final_vertex_count * 3);
  for (size_t vertex = 0; vertex < final_vertex_count; ++vertex) {
    const auto coordinates = values.at(final_offset + vertex).coordinates();
    output->positions.insert(output->positions.end(), coordinates.begin(), coordinates.end());
  }
  output->face_sizes.reserve(final_face_count);
  output->corners.reserve(final_corner_count);
  for (int face = 0; face < final_level.GetNumFaces(); ++face) {
    const auto vertices = final_level.GetFaceVertices(face);
    output->face_sizes.push_back(static_cast<uint32_t>(vertices.size()));
    for (const int vertex : vertices) {
      output->corners.push_back(static_cast<uint32_t>(vertex));
    }
  }
  output->vertex_origin_kind = std::move(origin_kinds);
  output->vertex_origin_index = std::move(origin_indices);
  set_face_parents(refiner, input_faces, levels, &output->face_parent);
  return output;
}

std::unique_ptr<ve_subdivision> run_catmark(const Input& input, ve_status* status) {
  Prepared prepared;
  set_status(status, prepare_input(input, &prepared));
  if (*status != VE_OK) {
    return nullptr;
  }
  auto refiner = make_refiner(input, prepared);
  if (refiner == nullptr || !refiner->GetLevel(0).ValidateTopology()) {
    set_status(status, VE_ERR_NOT_MANIFOLD);
    return nullptr;
  }
  refiner->RefineUniform(Far::TopologyRefiner::UniformOptions(static_cast<int>(input.levels)));
  if (cancelled(input.context)) {
    set_status(status, VE_ERR_CANCELLED);
    return nullptr;
  }
  const auto& final_level = refiner->GetLevel(static_cast<int>(input.levels));
  const auto final_vertex_count = static_cast<size_t>(final_level.GetNumVertices());
  const auto final_face_count = static_cast<size_t>(final_level.GetNumFaces());
  const auto final_corner_count = static_cast<size_t>(final_level.GetNumFaceVertices());
  if (final_vertex_count != prepared.predicted_vertices ||
      final_face_count != prepared.predicted_faces ||
      final_corner_count != prepared.predicted_corners) {
    set_status(status, VE_ERR_INTERNAL);
    return nullptr;
  }
  std::vector<Position> values;
  std::vector<uint32_t> origin_kinds;
  std::vector<uint32_t> origin_indices;
  size_t final_offset = 0;
  const ve_status interpolation = interpolate_positions(
      input, *refiner, final_vertex_count, &values, &final_offset, &origin_kinds, &origin_indices);
  if (interpolation != VE_OK) {
    set_status(status, interpolation);
    return nullptr;
  }
  auto result = make_output(final_level, values, final_offset, final_vertex_count, final_face_count,
                            final_corner_count, std::move(origin_kinds), std::move(origin_indices),
                            *refiner, input.face_count, input.levels);
  if (result != nullptr) {
    const auto channel_status = ve_subdivision_interpolate_channels(input, *refiner, result.get());
    if (channel_status != VE_OK) {
      set_status(status, channel_status);
      return nullptr;
    }
  }
  if (cancelled(input.context)) {
    set_status(status, VE_ERR_CANCELLED);
    return nullptr;
  }
  set_status(status, result == nullptr ? VE_ERR_INTERNAL : VE_OK);
  return result;
}

}  // namespace

extern "C" {

ve_subdivision* ve_subdivide_catmark(const ve_subdivide_input* input, ve_status* status) {
  set_status(status, VE_ERR_INVALID_ARG);
  if (input == nullptr) {
    return nullptr;
  }
  ve_status result_status = VE_ERR_INVALID_ARG;
  auto result = run_catmark(*input, &result_status);
  set_status(status, result_status);
  return result.release();
}

void ve_subdivision_free(ve_subdivision* const subdivision) {
  const std::unique_ptr<ve_subdivision> owner(subdivision);
}

}  // extern "C"
