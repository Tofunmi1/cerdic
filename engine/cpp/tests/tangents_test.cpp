// Tangent generation through the C ABI, run under ASan and UBSan.
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <string>

#include "ve_kernel.h"

namespace {

// Returns 1 for a failed check (after reporting it) and 0 for a passed one, so main can sum them.
int check(bool ok, const char* what) {
  if (!ok) {
    const std::string line = std::string("FAIL: ") + what + "\n";
    static_cast<void>(std::fputs(line.c_str(), stderr));
    return 1;
  }
  return 0;
}

constexpr std::array<float, 12> kPositions = {0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0};
constexpr std::array<float, 12> kNormals = {0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1};
constexpr std::array<float, 8> kUvs = {0, 0, 1, 0, 1, 1, 0, 1};
constexpr std::array<uint32_t, 6> kIndices = {0, 1, 2, 0, 2, 3};

int quad_follows_x() {
  int failures = 0;
  std::array<float, 24> out{};
  const ve_status status = ve_tangents(kPositions.data(), kNormals.data(), kUvs.data(), 4,
                                       kIndices.data(), kIndices.size(), out.data());
  failures += check(status == VE_OK, "a quad produces tangents");
  for (size_t corner = 0; corner < kIndices.size(); ++corner) {
    const float* tangent = out.data() + (4 * corner);
    const bool along_x = std::fabs(tangent[0] - 1.0F) < 1e-5F && std::fabs(tangent[1]) < 1e-5F &&
                         std::fabs(tangent[2]) < 1e-5F;
    failures += check(along_x, "tangent follows +x");
    failures +=
        check(std::fabs(tangent[3] - 1.0F) < 1e-5F, "orientation-preserving mapping has sign +1");
  }
  return failures;
}

int refusals() {
  int failures = 0;
  std::array<float, 24> out{};
  const std::array<uint32_t, 3> outside = {0, 1, 9};
  ve_status status = ve_tangents(kPositions.data(), kNormals.data(), kUvs.data(), 4, outside.data(),
                                 3, out.data());
  failures += check(status == VE_ERR_OUT_OF_BOUNDS, "an index past the vertices is refused");

  status = ve_tangents(kPositions.data(), kNormals.data(), kUvs.data(), 4, kIndices.data(), 5,
                       out.data());
  failures += check(status == VE_ERR_INVALID_ARG, "an index count that is not a multiple of three");

  status = ve_tangents(nullptr, kNormals.data(), kUvs.data(), 4, kIndices.data(), 6, out.data());
  failures += check(status == VE_ERR_INVALID_ARG, "a null pointer is refused");

  std::array<float, 12> broken = kPositions;
  std::get<3>(broken) = std::nanf("");
  status =
      ve_tangents(broken.data(), kNormals.data(), kUvs.data(), 4, kIndices.data(), 6, out.data());
  failures += check(status == VE_ERR_NON_FINITE, "a NaN position is refused");
  return failures;
}

int separate_triangles() {
  // Two triangles that share a position but not a UV: a split vertex, which MikkTSpace keeps apart.
  const std::array<float, 18> positions = {0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0};
  const std::array<float, 18> normals = {0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1};
  const std::array<float, 12> uvs = {0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0};
  const std::array<uint32_t, 6> indices = {0, 1, 2, 3, 4, 5};
  std::array<float, 24> out{};
  const ve_status status = ve_tangents(positions.data(), normals.data(), uvs.data(), 6,
                                       indices.data(), indices.size(), out.data());
  return check(status == VE_OK, "separate triangles produce tangents");
}

}  // namespace

int main() {
  const int failures = quad_follows_x() + refusals() + separate_triangles();
  if (failures == 0) {
    static_cast<void>(std::puts("tangents tests passed"));
  }
  return failures == 0 ? 0 : 1;
}
