// Edge bevel: Blender's bmesh bevel on a polygon mesh, selected by vertex pairs.
#include <cmath>

#include "bmesh.hh"
#include "bmesh_bevel.hh"
#include "polymesh.hh"

using namespace blender;
using namespace ve_blender;

namespace {

constexpr int kMaxSegments = 64;
// Edges times segments bounds the output size before any work is done.
constexpr size_t kMaxBevelWork = 2'000'000;
constexpr float kMinProfile = 0.1F;

ve_status validate_params(const ve_bevel_input& in) {
  if (in.struct_size != sizeof(ve_bevel_input) || in.edges == nullptr || in.edge_count == 0) {
    return VE_ERR_INVALID_ARG;
  }
  if (!std::isfinite(in.width) || !(in.width > 0.0F) || !std::isfinite(in.profile) ||
      in.profile < kMinProfile || in.profile > 1.0F || in.segments < 1 ||
      in.segments > kMaxSegments || in.offset_type < 0 || in.offset_type > 3) {
    return VE_ERR_INVALID_ARG;
  }
  if (in.edge_count > kMaxBevelWork / size_t(in.segments)) {
    return VE_ERR_TOO_LARGE;
  }
  return VE_OK;
}

// Tags the requested edges and their vertices. Blender requires every tagged edge to be manifold.
ve_status tag_edges(const ve_bevel_input& in, const LoadedMesh& mesh) {
  for (size_t i = 0; i < in.edge_count; ++i) {
    const uint32_t a = in.edges[2 * i];
    const uint32_t b = in.edges[(2 * i) + 1];
    if (a >= mesh.verts.size() || b >= mesh.verts.size() || a == b) {
      return VE_ERR_OUT_OF_BOUNDS;
    }
    BMEdge* edge = BM_edge_exists(mesh.verts[a], mesh.verts[b]);
    if (edge == nullptr) {
      return VE_ERR_INVALID_ARG;
    }
    if (!BM_edge_is_manifold(edge)) {
      return VE_ERR_NOT_MANIFOLD;
    }
    BM_elem_flag_enable(edge, BM_ELEM_TAG);
    BM_elem_flag_enable(edge->v1, BM_ELEM_TAG);
    BM_elem_flag_enable(edge->v2, BM_ELEM_TAG);
  }
  return VE_OK;
}

ve_polymesh* run_bevel(const ve_bevel_input& in, ve_status* status) {
  if (*status = validate_params(in); *status != VE_OK) {
    return nullptr;
  }
  LoadedMesh mesh;
  if (*status = load_mesh(in.mesh, &mesh); *status != VE_OK) {
    return nullptr;
  }
  if (*status = tag_edges(in, mesh); *status != VE_OK) {
    return nullptr;
  }
  BM_mesh_bevel(mesh.bm.get(), in.width, in.offset_type, 0, in.segments, in.profile, true, false,
                true, nullptr, -1, -1, true, false, false, false, 0, 0, 0, 0.1F, nullptr, 0, -1,
                -1);
  auto result = std::make_unique<ve_polymesh>();
  if (*status = store_mesh(mesh, result.get()); *status != VE_OK) {
    return nullptr;
  }
  *status = VE_OK;
  return result.release();
}

}  // namespace

extern "C" ve_polymesh* ve_bevel(const ve_bevel_input* input, ve_status* status) {
  ve_status local = VE_ERR_INVALID_ARG;
  ve_status* out = status != nullptr ? status : &local;
  *out = VE_ERR_INVALID_ARG;
  if (input == nullptr) {
    return nullptr;
  }
  return run_bevel(*input, out);
}
