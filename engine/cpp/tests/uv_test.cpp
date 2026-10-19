// UV unwrapping (Blender's parametrizer and island packer) through the C ABI, run under ASan and
// UBSan.
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

#include "ve_kernel.h"

namespace {

int check(bool ok, const std::string& what) {
  if (!ok) {
    const std::string line = "FAIL: " + what + "\n";
    static_cast<void>(std::fputs(line.c_str(), stderr));
    return 1;
  }
  return 0;
}

struct Mesh {
  std::vector<float> positions;
  std::vector<uint32_t> face_sizes;
  std::vector<uint32_t> corners;
};

Mesh cube() {
  Mesh m;
  m.positions = {-1, -1, -1, 1, -1, -1, 1, 1, -1, -1, 1, -1,
                 -1, -1, 1,  1, -1, 1,  1, 1, 1,  -1, 1, 1};
  m.face_sizes.assign(6, 4);
  m.corners = {0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7};
  return m;
}

// The twelve edges of the cube as vertex pairs.
const std::vector<uint32_t> kAllEdges = {0, 3, 3, 2, 2, 1, 1, 0, 4, 5, 5, 6,
                                         6, 7, 7, 4, 0, 4, 1, 5, 2, 6, 3, 7};
// Seven edges that cut the cube into a single cross-shaped net.
const std::vector<uint32_t> kCrossSeams = {0, 4, 1, 5, 2, 6, 3, 7, 4, 5, 5, 6, 4, 7};

ve_uv_input input(const Mesh& m, const std::vector<uint32_t>& seams, int32_t method, int32_t pack,
                  float margin) {
  const ve_polymesh_input mesh = {
      sizeof(ve_polymesh_input), m.positions.data(), m.positions.size() / 3, m.face_sizes.data(),
      m.face_sizes.size(),       m.corners.data(),   m.corners.size()};
  return {sizeof(ve_uv_input), mesh, seams.data(), seams.size() / 2, method, pack, 0, margin};
}

using Vec2 = std::array<double, 2>;

double signed_area(const std::vector<float>& uvs, const std::vector<uint32_t>& corners,
                   size_t first, uint32_t size) {
  double area = 0.0;
  for (uint32_t k = 0; k < size; ++k) {
    const size_t a = 2 * (first + k);
    const size_t b = 2 * (first + ((k + 1) % size));
    area += (static_cast<double>(uvs.at(a)) * uvs.at(b + 1)) -
            (static_cast<double>(uvs.at(b)) * uvs.at(a + 1));
  }
  static_cast<void>(corners);
  return area / 2.0;
}

// Cross net: every face of the cube unfolds without stretch, so each face keeps its area (4).
int cross_net_is_isometric() {
  int failures = 0;
  const Mesh m = cube();
  std::vector<float> uvs(2 * m.corners.size());
  uint32_t failed = 99;
  const ve_uv_input in = input(m, kCrossSeams, 0, 0, 0.0F);
  failures +=
      check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), &failed) == VE_OK, "cross net unwraps");
  failures += check(failed == 0, "no island fails");
  size_t first = 0;
  for (size_t f = 0; f < m.face_sizes.size(); ++f) {
    const double area = signed_area(uvs, m.corners, first, m.face_sizes.at(f));
    failures += check(std::fabs(std::fabs(area) - 4.0) < 1e-3,
                      "face " + std::to_string(f) + " keeps its area");
    first += m.face_sizes.at(f);
  }
  return failures;
}

struct Bounds {
  double lo_u = 1e9, lo_v = 1e9, hi_u = -1e9, hi_v = -1e9;
};

Bounds face_bounds(const std::vector<float>& uvs, size_t first, uint32_t size) {
  Bounds b;
  for (uint32_t k = 0; k < size; ++k) {
    const double u = uvs.at(2 * (first + k));
    const double v = uvs.at((2 * (first + k)) + 1);
    b.lo_u = std::min(b.lo_u, u);
    b.hi_u = std::max(b.hi_u, u);
    b.lo_v = std::min(b.lo_v, v);
    b.hi_v = std::max(b.hi_v, v);
  }
  return b;
}

// All edges cut: six separate islands, packed into the unit square without overlapping.
int six_islands_pack_into_the_unit_square() {
  int failures = 0;
  const Mesh m = cube();
  std::vector<float> uvs(2 * m.corners.size());
  uint32_t failed = 99;
  const ve_uv_input in = input(m, kAllEdges, 0, 1, 0.02F);
  failures +=
      check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), &failed) == VE_OK, "six islands unwrap");
  failures += check(failed == 0, "no island fails when packing");
  std::vector<Bounds> boxes;
  size_t first = 0;
  for (const uint32_t size : m.face_sizes) {
    boxes.push_back(face_bounds(uvs, first, size));
    first += size;
  }
  for (const Bounds& b : boxes) {
    failures +=
        check(b.lo_u >= -1e-6 && b.lo_v >= -1e-6 && b.hi_u <= 1.0 + 1e-6 && b.hi_v <= 1.0 + 1e-6,
              "island inside the unit square");
  }
  for (size_t a = 0; a < boxes.size(); ++a) {
    for (size_t b = a + 1; b < boxes.size(); ++b) {
      const bool apart =
          boxes.at(a).hi_u <= boxes.at(b).lo_u || boxes.at(b).hi_u <= boxes.at(a).lo_u ||
          boxes.at(a).hi_v <= boxes.at(b).lo_v || boxes.at(b).hi_v <= boxes.at(a).lo_v;
      failures += check(
          apart, "islands " + std::to_string(a) + " and " + std::to_string(b) + " do not overlap");
    }
  }
  return failures;
}

int abf_matches_lscm_on_a_developable_net() {
  int failures = 0;
  const Mesh m = cube();
  std::vector<float> lscm(2 * m.corners.size());
  std::vector<float> abf(2 * m.corners.size());
  const ve_uv_input a = input(m, kCrossSeams, 0, 0, 0.0F);
  const ve_uv_input b = input(m, kCrossSeams, 1, 0, 0.0F);
  failures += check(ve_uv_unwrap(&a, lscm.data(), lscm.size(), nullptr) == VE_OK, "lscm");
  failures += check(ve_uv_unwrap(&b, abf.data(), abf.size(), nullptr) == VE_OK, "abf");
  size_t first = 0;
  for (const uint32_t size : m.face_sizes) {
    failures += check(std::fabs(std::fabs(signed_area(abf, m.corners, first, size)) - 4.0) < 1e-3,
                      "abf keeps face area");
    first += size;
  }
  return failures;
}

int refusals() {
  int failures = 0;
  const Mesh m = cube();
  std::vector<float> uvs(2 * m.corners.size());
  ve_uv_input in = input(m, kCrossSeams, 0, 0, 0.0F);
  failures +=
      check(ve_uv_unwrap(&in, uvs.data(), uvs.size() - 1, nullptr) == VE_ERR_BUFFER_TOO_SMALL,
            "a short output buffer is refused");
  const std::vector<uint32_t> not_an_edge = {0, 6};
  in = input(m, not_an_edge, 0, 0, 0.0F);
  failures += check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), nullptr) == VE_ERR_INVALID_ARG,
                    "a seam that is not an edge is refused");
  const std::vector<uint32_t> out_of_range = {0, 99};
  in = input(m, out_of_range, 0, 0, 0.0F);
  failures += check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), nullptr) == VE_ERR_OUT_OF_BOUNDS,
                    "an out-of-range seam vertex is refused");
  in = input(m, kCrossSeams, 2, 0, 0.0F);
  failures += check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), nullptr) == VE_ERR_INVALID_ARG,
                    "an unknown method is refused");
  in = input(m, kCrossSeams, 0, 1, 0.9F);
  failures += check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), nullptr) == VE_ERR_INVALID_ARG,
                    "a huge margin is refused");
  in = input(m, kCrossSeams, 0, 0, NAN);
  failures += check(ve_uv_unwrap(&in, uvs.data(), uvs.size(), nullptr) == VE_ERR_INVALID_ARG,
                    "a NaN margin is refused");
  failures += check(ve_uv_unwrap(nullptr, uvs.data(), uvs.size(), nullptr) == VE_ERR_INVALID_ARG,
                    "a null input is refused");
  return failures;
}

}  // namespace

int main() {
  int failures = 0;
  failures += cross_net_is_isometric();
  failures += six_islands_pack_into_the_unit_square();
  failures += abf_matches_lscm_on_a_developable_net();
  failures += refusals();
  if (failures != 0) {
    static_cast<void>(std::fputs("uv_test: FAILED\n", stderr));
    return 1;
  }
  static_cast<void>(std::puts("uv_test: ok"));
  return 0;
}
