// Tangent-space generation (MikkTSpace, vendored from Blender) behind the kernel's C ABI.
//
// MikkTSpace is the standard tangent basis for baked normal maps, so a normal map made here shades
// the same in any game engine. The vendored headers expect their includer to supply `uint`, the
// branch-hint macros and the standard headers they use without including; this file does that.
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <type_traits>
#include <unordered_map>
#include <utility>
#include <vector>

#include "ve_kernel.h"

using uint = unsigned int;
#define LIKELY(x) (x)
#define UNLIKELY(x) (x)
#include "mikktspace.hh"

namespace {

// Two million triangles is the largest Final-quality mesh the engine allows.
constexpr size_t kMaxTangentTriangles = 2'000'000;

// The mesh MikkTSpace reads: triangles over shared vertices, tangents written per corner. The
// method names are the interface the vendored code calls.
class CornerMesh {
 public:
  CornerMesh(const float* positions, const float* normals, const float* uvs,
             const uint32_t* indices, size_t triangles, float* out)
      : positions_(positions),
        normals_(normals),
        uvs_(uvs),
        indices_(indices),
        triangles_(triangles),
        out_(out) {}

  [[nodiscard]] size_t GetNumFaces() const { return triangles_; }
  [[nodiscard]] static size_t GetNumVerticesOfFace(size_t /*face*/) { return 3; }
  [[nodiscard]] static bool has_uv() { return true; }

  [[nodiscard]] mikk::float3 GetPosition(size_t face, size_t corner) const {
    return {positions_ + (3 * vertex(face, corner))};
  }
  [[nodiscard]] mikk::float3 GetNormal(size_t face, size_t corner) const {
    return {normals_ + (3 * vertex(face, corner))};
  }
  [[nodiscard]] mikk::float3 GetTexCoord(size_t face, size_t corner) const {
    const float* uv = uvs_ + (2 * vertex(face, corner));
    return {uv[0], uv[1], 0.0F};
  }
  // The fourth component is the bitangent sign, +1 when the UV mapping preserves orientation.
  void SetTangentSpace(size_t face, size_t corner, const mikk::float3& tangent,
                       bool orient_preserving) {
    float* out = out_ + (4 * ((3 * face) + corner));
    out[0] = tangent.x;
    out[1] = tangent.y;
    out[2] = tangent.z;
    out[3] = orient_preserving ? 1.0F : -1.0F;
  }

 private:
  [[nodiscard]] size_t vertex(size_t face, size_t corner) const {
    return indices_[(3 * face) + corner];
  }

  const float* positions_;
  const float* normals_;
  const float* uvs_;
  const uint32_t* indices_;
  size_t triangles_;
  float* out_;
};

bool all_finite(const float* values, size_t count) {
  return std::all_of(values, values + count, [](float value) { return std::isfinite(value); });
}

}  // namespace

extern "C" ve_status ve_tangents(const float* positions, const float* normals, const float* uvs,
                                 size_t vertex_count, const uint32_t* indices, size_t index_count,
                                 float* out_tangents) {
  if (positions == nullptr || normals == nullptr || uvs == nullptr || indices == nullptr ||
      out_tangents == nullptr || vertex_count == 0 || index_count == 0 || index_count % 3 != 0) {
    return VE_ERR_INVALID_ARG;
  }
  if (index_count / 3 > kMaxTangentTriangles || vertex_count > (index_count * 4) + 4) {
    return VE_ERR_TOO_LARGE;
  }
  if (std::any_of(indices, indices + index_count,
                  [vertex_count](uint32_t index) { return index >= vertex_count; })) {
    return VE_ERR_OUT_OF_BOUNDS;
  }
  if (!all_finite(positions, 3 * vertex_count) || !all_finite(normals, 3 * vertex_count) ||
      !all_finite(uvs, 2 * vertex_count)) {
    return VE_ERR_NON_FINITE;
  }
  CornerMesh mesh(positions, normals, uvs, indices, index_count / 3, out_tangents);
  mikk::Mikktspace<CornerMesh>(mesh).genTangSpace();
  return VE_OK;
}
