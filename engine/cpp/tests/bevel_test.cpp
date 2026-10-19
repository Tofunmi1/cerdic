// Edge bevel (Blender's bmesh bevel) through the C ABI, run under ASan and UBSan.
//
// The expected counts and vertex hashes come from the feasibility spike, which ran the same cases
// natively and under WASI and found them identical bit for bit; `cargo xtask parity` keeps checking
// that for the engine build.
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
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

struct Box {
  std::vector<float> positions;
  std::vector<uint32_t> face_sizes;
  std::vector<uint32_t> corners;
};

// The same box the spike used: eight vertices, six quads, faces wound outward.
Box make_box(float sx, float sy, float sz) {
  const std::array<std::array<float, 3>, 8> p = {{{-sx, -sy, -sz},
                                                  {sx, -sy, -sz},
                                                  {sx, sy, -sz},
                                                  {-sx, sy, -sz},
                                                  {-sx, -sy, sz},
                                                  {sx, -sy, sz},
                                                  {sx, sy, sz},
                                                  {-sx, sy, sz}}};
  const std::array<uint32_t, 24> quads = {0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4,
                                          1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7};
  Box box;
  for (const auto& v : p) {
    box.positions.insert(box.positions.end(), v.begin(), v.end());
  }
  box.face_sizes.assign(6, 4);
  box.corners.assign(quads.begin(), quads.end());
  return box;
}

ve_polymesh_input as_input(const Box& box) {
  return {sizeof(ve_polymesh_input), box.positions.data(),  box.positions.size() / 3,
          box.face_sizes.data(),     box.face_sizes.size(), box.corners.data(),
          box.corners.size()};
}

struct Deleter {
  void operator()(ve_polymesh* mesh) const { ve_polymesh_free(mesh); }
};
using MeshPtr = std::unique_ptr<ve_polymesh, Deleter>;

struct Result {
  size_t vertices = 0;
  size_t faces = 0;
  size_t edges = 0;
  double volume = 0;
  uint64_t hash = 0;
  bool copied = false;
};

using Vec3 = std::array<double, 3>;

// Six times the signed volume of the tetrahedron (a, b, c, origin).
double triple(const Vec3& a, const Vec3& b, const Vec3& c) {
  const double bc_x = (b.at(1) * c.at(2)) - (b.at(2) * c.at(1));
  const double bc_y = (b.at(2) * c.at(0)) - (b.at(0) * c.at(2));
  const double bc_z = (b.at(0) * c.at(1)) - (b.at(1) * c.at(0));
  return (a.at(0) * bc_x) + (a.at(1) * bc_y) + (a.at(2) * bc_z);
}

// FNV-1a over the raw bits of every vertex coordinate, in vertex order.
Result measure(const ve_polymesh* mesh) {
  Result r;
  r.vertices = ve_polymesh_num_vertices(mesh);
  r.faces = ve_polymesh_num_faces(mesh);
  const size_t corner_count = ve_polymesh_num_corners(mesh);
  std::vector<float> positions(3 * r.vertices);
  std::vector<uint32_t> sizes(r.faces);
  std::vector<uint32_t> corners(corner_count);
  std::vector<uint32_t> origin(r.vertices);
  std::vector<uint32_t> parent(r.faces);
  const ve_status status = ve_polymesh_copy(
      mesh, positions.data(), positions.size(), sizes.data(), sizes.size(), corners.data(),
      corners.size(), origin.data(), origin.size(), parent.data(), parent.size());
  if (status != VE_OK) {
    return r;
  }
  r.copied = true;
  r.edges = corner_count / 2;  // closed manifold: every edge is shared by two corners
  r.hash = 0xcbf29ce484222325ULL;
  for (const float value : positions) {
    uint32_t bits = 0;
    std::memcpy(&bits, &value, sizeof(bits));
    r.hash = (r.hash ^ bits) * 0x100000001b3ULL;
  }
  const auto vertex = [&positions](uint32_t index) {
    const size_t base = 3 * static_cast<size_t>(index);
    return Vec3{positions.at(base), positions.at(base + 1), positions.at(base + 2)};
  };
  size_t offset = 0;
  for (const uint32_t size : sizes) {
    const Vec3 a = vertex(corners.at(offset));
    for (uint32_t k = 1; k + 1 < size; ++k) {
      r.volume +=
          triple(a, vertex(corners.at(offset + k)), vertex(corners.at(offset + k + 1))) / 6.0;
    }
    offset += size;
  }
  return r;
}

MeshPtr bevel(const Box& box, const std::vector<uint32_t>& edges, float width, int32_t segments,
              ve_status* status) {
  const ve_bevel_input input = {sizeof(ve_bevel_input),
                                as_input(box),
                                edges.data(),
                                edges.size() / 2,
                                width,
                                segments,
                                0,
                                0.5F};
  return MeshPtr(ve_bevel(&input, status));
}

const std::vector<uint32_t> kAllEdges = {0, 3, 3, 2, 2, 1, 1, 0, 4, 5, 5, 6,
                                         6, 7, 7, 4, 0, 4, 1, 5, 2, 6, 3, 7};

struct Expected {
  size_t vertices;
  size_t faces;
  double volume;
  uint64_t hash;
};

int expect(const std::string& name, const Result& r, const Expected& e) {
  int failures = 0;
  failures += check(r.copied, name + " copies out");
  failures += check(r.vertices == e.vertices, name + " vertex count");
  failures += check(r.faces == e.faces, name + " face count");
  failures += check(std::fabs(r.volume - e.volume) < 5e-6, name + " volume");
  failures += check(r.hash == e.hash, name + " vertex hash");
  // Euler: closed genus-0 surface.
  const bool sphere = r.vertices + r.faces == r.edges + 2;
  failures += check(sphere, name + " is a topological sphere");
  return failures;
}

struct AllEdgesCase {
  std::string name;
  int32_t segments;
  float width;
  Expected expected;
};

int all_edges_cases() {
  int failures = 0;
  const Box cube = make_box(1, 1, 1);
  const std::array<AllEdgesCase, 4> cases = {{
      {"cube-all-seg1", 1, 0.15F, {24, 26, 7.748000, 0x97ebc958e10f33d5ULL}},
      {"cube-all-seg2", 2, 0.15F, {56, 54, 7.849585, 0x447bf004cb14b0ddULL}},
      {"cube-all-seg4", 4, 0.15F, {152, 150, 7.878235, 0x866d2a48a3797085ULL}},
      {"cube-all-seg3", 3, 0.2F, {96, 98, 7.759928, 0xbe5774df48cb5125ULL}},
  }};
  for (const auto& c : cases) {
    ve_status status = VE_ERR_INTERNAL;
    const MeshPtr mesh = bevel(cube, kAllEdges, c.width, c.segments, &status);
    failures += check(mesh != nullptr && status == VE_OK, c.name);
    if (mesh != nullptr) {
      failures += expect(c.name, measure(mesh.get()), c.expected);
    }
  }
  return failures;
}

int selected_edge_cases() {
  int failures = 0;
  ve_status status = VE_ERR_INTERNAL;
  {
    const Box box = make_box(1, 0.5F, 1.5F);
    const MeshPtr mesh = bevel(box, {2, 3, 6, 7, 2, 6, 3, 7}, 0.1F, 3, &status);
    failures += check(mesh != nullptr, "top loop bevels");
    if (mesh != nullptr) {
      failures +=
          expect("box-top-loop", measure(mesh.get()), {20, 18, 5.975488, 0xdb7ea428a01aa095ULL});
    }
  }
  const Box cube = make_box(1, 1, 1);
  {
    const MeshPtr mesh = bevel(cube, {1, 0, 0, 3}, 0.2F, 2, &status);
    failures += check(mesh != nullptr, "two edges bevel");
    if (mesh != nullptr) {
      failures +=
          expect("cube-two-edges", measure(mesh.get()), {14, 10, 7.954376, 0xe828709002a4aa92ULL});
    }
  }
  {
    const MeshPtr mesh = bevel(cube, {4, 5}, 0.25F, 3, &status);
    failures += check(mesh != nullptr, "one edge bevels");
    if (mesh != nullptr) {
      failures +=
          expect("cube-one-edge", measure(mesh.get()), {14, 9, 7.968750, 0xa2b8b27eda002abdULL});
    }
  }
  return failures;
}

int refusals() {
  int failures = 0;
  ve_status status = VE_OK;
  const Box cube = make_box(1, 1, 1);
  // Not an edge of the mesh (a space diagonal).
  failures +=
      check(bevel(cube, {0, 6}, 0.1F, 2, &status) == nullptr && status == VE_ERR_INVALID_ARG,
            "a pair that is not an edge is refused");
  failures +=
      check(bevel(cube, {0, 99}, 0.1F, 2, &status) == nullptr && status == VE_ERR_OUT_OF_BOUNDS,
            "an out-of-range vertex is refused");
  failures +=
      check(bevel(cube, {0, 3}, -1.0F, 2, &status) == nullptr && status == VE_ERR_INVALID_ARG,
            "a negative width is refused");
  failures +=
      check(bevel(cube, {0, 3}, 0.1F, 0, &status) == nullptr && status == VE_ERR_INVALID_ARG,
            "zero segments are refused");
  failures += check(bevel(cube, {0, 3}, NAN, 2, &status) == nullptr && status == VE_ERR_INVALID_ARG,
                    "a NaN width is refused");
  failures += check(bevel(cube, {}, 0.1F, 2, &status) == nullptr && status == VE_ERR_INVALID_ARG,
                    "an empty edge list is refused");
  // An open quad: its boundary edges have one face, so they cannot be beveled.
  Box quad;
  quad.positions = {0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0};
  quad.face_sizes = {4};
  quad.corners = {0, 1, 2, 3};
  failures +=
      check(bevel(quad, {0, 1}, 0.1F, 2, &status) == nullptr && status == VE_ERR_NOT_MANIFOLD,
            "a boundary edge is refused as non-manifold");
  // A face that repeats a corner.
  Box bad = make_box(1, 1, 1);
  bad.corners.at(1) = bad.corners.at(0);
  failures += check(bevel(bad, {0, 3}, 0.1F, 2, &status) == nullptr && status == VE_ERR_INVALID_ARG,
                    "a face with a repeated corner is refused");
  Box nan_box = make_box(1, 1, 1);
  nan_box.positions.at(4) = NAN;
  failures +=
      check(bevel(nan_box, {0, 3}, 0.1F, 2, &status) == nullptr && status == VE_ERR_NON_FINITE,
            "a non-finite position is refused");
  failures += check(ve_bevel(nullptr, &status) == nullptr && status == VE_ERR_INVALID_ARG,
                    "a null input is refused");
  return failures;
}

}  // namespace

int main() {
  int failures = 0;
  failures += all_edges_cases();
  failures += selected_edge_cases();
  failures += refusals();
  if (failures != 0) {
    static_cast<void>(std::fputs("bevel_test: FAILED\n", stderr));
    return 1;
  }
  static_cast<void>(std::puts("bevel_test: ok"));
  return 0;
}
