// UV unwrapping: Blender's parametrizer (LSCM or ABF++) and island packer on a polygon mesh.
#include <algorithm>
#include <cmath>
#include <memory>
#include <unordered_set>
#include <vector>

#include "GEO_uv_pack.hh"
#include "GEO_uv_parametrizer.hh"
#include "polymesh.hh"

using namespace blender::geometry;
using namespace ve_blender;

// Blender's window-manager job system cancels a pack through `stop`; the engine runs a pack to the
// end (callers bound the work by input size), so nothing is ever cancelled here.
bool blender::geometry::UVPackIsland_Params::isCancelled() const { return false; }

namespace {

constexpr float kMaxMargin = 0.5F;

uint64_t edge_key(uint32_t a, uint32_t b) {
  const uint32_t low = std::min(a, b);
  const uint32_t high = std::max(a, b);
  return (static_cast<uint64_t>(low) << 32) | high;
}

// Every undirected edge of the mesh.
std::unordered_set<uint64_t> mesh_edges(const ve_polymesh_input& mesh) {
  std::unordered_set<uint64_t> edges;
  size_t offset = 0;
  for (size_t f = 0; f < mesh.face_count; ++f) {
    const uint32_t size = mesh.face_sizes[f];
    for (uint32_t k = 0; k < size; ++k) {
      edges.insert(edge_key(mesh.corners[offset + k], mesh.corners[offset + ((k + 1) % size)]));
    }
    offset += size;
  }
  return edges;
}

ve_status validate(const ve_uv_input& in, size_t uv_capacity, const float* uvs) {
  if (in.struct_size != sizeof(ve_uv_input) || uvs == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  if (const ve_status status = validate_mesh(in.mesh); status != VE_OK) {
    return status;
  }
  if (in.method < 0 || in.method > 1 || !std::isfinite(in.margin) || in.margin < 0.0F ||
      in.margin > kMaxMargin || (in.seam_count != 0 && in.seams == nullptr)) {
    return VE_ERR_INVALID_ARG;
  }
  if (uv_capacity < 2 * in.mesh.corner_count) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  const std::unordered_set<uint64_t> edges = mesh_edges(in.mesh);
  for (size_t i = 0; i < in.seam_count; ++i) {
    const uint32_t a = in.seams[2 * i];
    const uint32_t b = in.seams[(2 * i) + 1];
    if (a >= in.mesh.vertex_count || b >= in.mesh.vertex_count) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    if (edges.count(edge_key(a, b)) == 0) {
      return VE_ERR_INVALID_ARG;
    }
  }
  return VE_OK;
}

// Hands every face to the parametrizer; it keeps pointers into `uvs` and writes the result there.
void add_faces(ParamHandle* handle, const ve_uv_input& in, float* uvs) {
  const ve_polymesh_input& mesh = in.mesh;
  size_t offset = 0;
  std::vector<ParamKey> keys;
  std::vector<const float*> coords;
  std::vector<float*> outputs;
  std::vector<float> weights;
  for (size_t f = 0; f < mesh.face_count; ++f) {
    const uint32_t size = mesh.face_sizes[f];
    keys.assign(size, 0);
    coords.assign(size, nullptr);
    outputs.assign(size, nullptr);
    weights.assign(size, 1.0F);
    const auto pinned = std::make_unique<bool[]>(size);
    const auto selected = std::make_unique<bool[]>(size);
    for (uint32_t k = 0; k < size; ++k) {
      const uint32_t vertex = mesh.corners[offset + k];
      keys[k] = ParamKey(vertex);
      coords[k] = mesh.positions + (3 * static_cast<size_t>(vertex));
      outputs[k] = uvs + (2 * (offset + k));
      selected[k] = true;
    }
    uv_parametrizer_face_add(handle, ParamKey(f), static_cast<int>(size), keys.data(),
                             coords.data(), outputs.data(), weights.data(), pinned.get(),
                             selected.get());
    offset += size;
  }
  for (size_t i = 0; i < in.seam_count; ++i) {
    const ParamKey pair[2] = {ParamKey(in.seams[2 * i]), ParamKey(in.seams[(2 * i) + 1])};
    uv_parametrizer_edge_set_seam(handle, pair);
  }
}

ve_status run(const ve_uv_input& in, float* uvs, size_t uv_capacity, uint32_t* failed_islands) {
  if (const ve_status status = validate(in, uv_capacity, uvs); status != VE_OK) {
    return status;
  }
  std::fill(uvs, uvs + (2 * in.mesh.corner_count), 0.0F);
  const auto handle = std::make_unique<ParamHandle>();
  add_faces(handle.get(), in, uvs);
  int failed_fill = 0;
  uv_parametrizer_construct_end(handle.get(), false, false, &failed_fill);
  uv_parametrizer_lscm_begin(handle.get(), false, in.method == 1, false);
  int changed = 0;
  int failed = 0;
  uv_parametrizer_lscm_solve(handle.get(), &changed, &failed);
  uv_parametrizer_lscm_end(handle.get());
  if (in.pack != 0) {
    uv_parametrizer_average(handle.get(), true, false, false);
    UVPackIsland_Params params;
    params.rotate_method =
        in.rotate != 0 ? blender::ED_UVPACK_ROTATION_ANY : blender::ED_UVPACK_ROTATION_NONE;
    params.margin = in.margin;
    params.margin_method = blender::ED_UVPACK_MARGIN_FRACTION;
    uv_parametrizer_pack(handle.get(), params);
  }
  uv_parametrizer_flush(handle.get());
  if (failed_islands != nullptr) {
    *failed_islands = static_cast<uint32_t>(std::max(failed, 0));
  }
  return VE_OK;
}

}  // namespace

extern "C" ve_status ve_uv_unwrap(const ve_uv_input* input, float* uvs, size_t uv_capacity,
                                  uint32_t* failed_islands) {
  if (input == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  return run(*input, uvs, uv_capacity, failed_islands);
}

extern "C" ve_status ve_uv_pack(const ve_uv_pack_input* input, float* uvs,
                                size_t uv_capacity) {
  if (input == nullptr || input->struct_size != sizeof(ve_uv_pack_input) ||
      input->uvs == nullptr || uvs == nullptr) {
    return VE_ERR_INVALID_ARG;
  }
  if (const ve_status status = validate_mesh(input->mesh); status != VE_OK) {
    return status;
  }
  if (uv_capacity < 2 * input->mesh.corner_count) {
    return VE_ERR_BUFFER_TOO_SMALL;
  }
  if (!std::isfinite(input->margin) || input->margin < 0.0F || input->margin > kMaxMargin ||
      (input->shape != 0 && input->shape != 1)) {
    return VE_ERR_INVALID_ARG;
  }
  for (size_t i = 0; i < 2 * input->mesh.corner_count; ++i) {
    if (!std::isfinite(input->uvs[i])) {
      return VE_ERR_NON_FINITE;
    }
  }
  std::copy_n(input->uvs, 2 * input->mesh.corner_count, uvs);
  const auto handle = std::make_unique<ParamHandle>();
  const auto& mesh = input->mesh;
  size_t offset = 0;
  std::vector<ParamKey> keys;
  std::vector<const float*> coords;
  std::vector<float*> outputs;
  std::vector<float> weights;
  for (size_t f = 0; f < mesh.face_count; ++f) {
    const uint32_t size = mesh.face_sizes[f];
    keys.resize(size);
    coords.resize(size);
    outputs.resize(size);
    weights.assign(size, 1.0F);
    const auto pinned = std::make_unique<bool[]>(size);
    const auto selected = std::make_unique<bool[]>(size);
    for (uint32_t k = 0; k < size; ++k) {
      const uint32_t vertex = mesh.corners[offset + k];
      keys[k] = ParamKey(vertex);
      coords[k] = mesh.positions + (3 * static_cast<size_t>(vertex));
      outputs[k] = uvs + (2 * (offset + k));
      pinned[k] = false;
      selected[k] = true;
    }
    uv_parametrizer_face_add(handle.get(), ParamKey(f), static_cast<int>(size), keys.data(),
                             coords.data(), outputs.data(), weights.data(), pinned.get(),
                             selected.get());
    offset += size;
  }
  int failed = 0;
  uv_parametrizer_construct_end(handle.get(), false, true, &failed);
  UVPackIsland_Params params;
  params.rotate_method = input->rotate != 0 ? blender::ED_UVPACK_ROTATION_ANY
                                            : blender::ED_UVPACK_ROTATION_NONE;
  params.scale_to_fit = input->keep_density == 0;
  params.margin = input->margin;
  params.margin_method = blender::ED_UVPACK_MARGIN_FRACTION;
  params.shape_method = input->shape == 0 ? blender::ED_UVPACK_SHAPE_AABB
                                          : blender::ED_UVPACK_SHAPE_CONVEX;
  params.target_extent = 1.0F;
  uv_parametrizer_pack(handle.get(), params);
  uv_parametrizer_flush(handle.get());
  return failed == 0 ? VE_OK : VE_ERR_OP_REFUSED;
}
