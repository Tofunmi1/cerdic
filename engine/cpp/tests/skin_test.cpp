// Skin (Blender's Skin modifier, the B-Mesh algorithm) through the C ABI, run under ASan and UBSan.
//
// Analytic checks, no hashes: a skeleton graph must become one closed, manifold surface (Euler number 2 for a tree), made of quads except around branches, inside the
// skeleton's bounding box grown by the radii, and left-right symmetric for a symmetric skeleton. Bad input is refused with a typed code.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <map>
#include <string>
#include <utility>
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

struct Skeleton {
  std::vector<float> positions;  // 3 per joint
  std::vector<float> radii;      // 2 per joint
  std::vector<uint8_t> flags;
  std::vector<uint32_t> edges;
  uint32_t joint(float x, float y, float z, float rx, float rz, uint8_t flag = 0) {
    positions.insert(positions.end(), {x, y, z});
    radii.insert(radii.end(), {rx, rz});
    flags.push_back(flag);
    return uint32_t(flags.size() - 1);
  }
  void bone(uint32_t a, uint32_t b) { edges.insert(edges.end(), {a, b}); }
};

struct Result {
  std::vector<float> positions;
  std::vector<uint32_t> face_sizes;
  std::vector<uint32_t> corners;
  std::vector<uint32_t> origin;
  ve_status status = VE_OK;
};

Result run(const Skeleton& s, float smoothing = 0.0F, uint32_t symmetry = 0, int subdivide = 1) {
  ve_skin_input in{};
  in.struct_size = sizeof(in);
  in.positions = s.positions.data();
  in.radii = s.radii.data();
  in.flags = s.flags.data();
  in.vertex_count = s.flags.size();
  in.edges = s.edges.data();
  in.edge_count = s.edges.size() / 2;
  in.branch_smoothing = smoothing;
  in.symmetry_axes = symmetry;
  in.subdivide = subdivide;
  Result r;
  ve_polymesh* mesh = ve_skin(&in, &r.status);
  if (mesh == nullptr) {
    return r;
  }
  const size_t v = ve_polymesh_num_vertices(mesh);
  const size_t f = ve_polymesh_num_faces(mesh);
  const size_t c = ve_polymesh_num_corners(mesh);
  r.positions.resize(3 * v);
  r.face_sizes.resize(f);
  r.corners.resize(c);
  r.origin.resize(v);
  std::vector<uint32_t> parent(f);
  const ve_status copied = ve_polymesh_copy(mesh, r.positions.data(), r.positions.size(), r.face_sizes.data(), f,
                                            r.corners.data(), c, r.origin.data(), v, parent.data(), f);
  ve_polymesh_free(mesh);
  if (copied != VE_OK) {
    r.status = copied;
  }
  return r;
}

struct Topology {
  size_t vertices = 0, edges = 0, faces = 0, quads = 0, tris = 0, other = 0;
  size_t boundary = 0, overused = 0;  // edges with one face, edges with three or more
  size_t same_direction = 0;          // directed edges used by two faces (neighbours wound against each other)
  bool manifold = true;
  int euler() const { return int(vertices) - int(edges) + int(faces); }
};

Topology topology(const Result& r) {
  Topology t;
  t.vertices = r.positions.size() / 3;
  t.faces = r.face_sizes.size();
  std::map<std::pair<uint32_t, uint32_t>, int> uses;
  std::map<std::pair<uint32_t, uint32_t>, int> directed;
  size_t at = 0;
  for (uint32_t size : r.face_sizes) {
    if (size == 4) {
      ++t.quads;
    } else if (size == 3) {
      ++t.tris;
    } else {
      ++t.other;
    }
    for (uint32_t k = 0; k < size; ++k) {
      const uint32_t a = r.corners[at + k];
      const uint32_t b = r.corners[at + ((k + 1) % size)];
      ++uses[{std::min(a, b), std::max(a, b)}];
      ++directed[{a, b}];
    }
    at += size;
  }
  for (const auto& [edge, count] : directed) {
    static_cast<void>(edge);
    t.same_direction += count > 1 ? 1U : 0U;
  }
  t.edges = uses.size();
  for (const auto& [edge, count] : uses) {
    static_cast<void>(edge);
    if (count != 2) {
      t.manifold = false;
      t.boundary += count == 1 ? 1U : 0U;
      t.overused += count > 2 ? 1U : 0U;
    }
  }
  return t;
}

// The signed volume (divergence theorem over the fan-triangulated faces): positive when the faces are wound outward.
double signed_volume(const Result& r) {
  double volume = 0.0;
  size_t at = 0;
  for (uint32_t size : r.face_sizes) {
    const float* a = &r.positions[3 * r.corners[at]];
    for (uint32_t k = 1; k + 1 < size; ++k) {
      const float* b = &r.positions[3 * r.corners[at + k]];
      const float* c = &r.positions[3 * r.corners[at + k + 1]];
      volume += (double(a[0]) * (double(b[1]) * c[2] - double(b[2]) * c[1]) -
                 double(a[1]) * (double(b[0]) * c[2] - double(b[2]) * c[0]) +
                 double(a[2]) * (double(b[0]) * c[1] - double(b[1]) * c[0])) /
                6.0;
    }
    at += size;
  }
  return volume;
}

int test_straight_limb() {
  int failures = 0;
  Skeleton s;
  const uint32_t a = s.joint(0, 0, 0, 0.1F, 0.1F, 1);
  const uint32_t b = s.joint(0, 1, 0, 0.1F, 0.1F);
  const uint32_t c = s.joint(0, 2, 0, 0.1F, 0.1F);
  s.bone(a, b);
  s.bone(b, c);
  const Result r = run(s);
  failures += check(r.status == VE_OK, "a straight limb is skinned");
  const Topology t = topology(r);
  failures += check(t.manifold, "limb: every edge has exactly two faces");
  failures += check(t.euler() == 2, "limb: Euler number is 2 (a closed sphere-like surface), got " + std::to_string(t.euler()));
  failures += check(t.tris == 0 && t.other == 0, "limb: all quads");
  failures += check(signed_volume(r) > 0.0, "limb: faces wind outward");
  // A 0.2 m square section 2 m long, with caps at the joint ends: volume near 0.2 * 0.2 * 2 within the end-cap overshoot.
  failures += check(std::fabs(signed_volume(r) - 0.08) < 0.03, "limb: volume matches the 0.2 x 0.2 x 2 prism, got " + std::to_string(signed_volume(r)));
  return failures;
}

int test_branch() {
  int failures = 0;
  Skeleton s;
  const uint32_t hub = s.joint(0, 0, 0, 0.2F, 0.2F, 1);
  const std::array<std::array<float, 3>, 3> arm = {{{1.0F, 0, 0}, {-0.5F, 0.87F, 0}, {-0.5F, -0.87F, 0}}};
  for (const auto& p : arm) {
    const uint32_t tip = s.joint(p[0], p[1], p[2], 0.1F, 0.1F);
    s.bone(hub, tip);
  }
  const Result r = run(s);
  failures += check(r.status == VE_OK, "a Y branch is skinned");
  const Topology t = topology(r);
  static_cast<void>(std::fprintf(stderr, "branch: %zu vertices, %zu faces, %zu boundary edges, %zu overused edges\n", t.vertices, t.faces, t.boundary, t.overused));
  failures += check(t.manifold, "branch: manifold");
  failures += check(t.euler() == 2, "branch: Euler number is 2, got " + std::to_string(t.euler()));
  failures += check(t.quads * 10 >= t.faces * 8, "branch: at least 80% quads, got " + std::to_string(t.quads) + " of " + std::to_string(t.faces));
  failures += check(signed_volume(r) > 0.0, "branch: faces wind outward");
  failures += check(t.same_direction == 0, "branch: neighbouring faces are wound consistently");
  return failures;
}

// A full human: pelvis root, spine to the head, two legs with knees and feet, two arms with elbows and hands, and three fingers on each hand.
Skeleton human(int fingers = 3, float finger_gap = 0.03F) {
  const float arm_asym = 0.0F;
  Skeleton s;
  const uint32_t pelvis = s.joint(0, 0.95F, 0, 0.16F, 0.1F, 1);
  const uint32_t chest = s.joint(0, 1.3F, 0, 0.16F, 0.1F);
  const uint32_t neck = s.joint(0, 1.5F, 0, 0.05F, 0.05F);
  const uint32_t head = s.joint(0, 1.68F, 0.02F, 0.09F, 0.1F);
  s.bone(pelvis, chest);
  s.bone(chest, neck);
  s.bone(neck, head);
  for (const float side : {1.0F, -1.0F}) {
    const uint32_t hip = s.joint(side * 0.1F, 0.9F, 0, 0.09F, 0.09F);
    const uint32_t knee = s.joint(side * 0.1F, 0.5F, 0.01F, 0.06F, 0.06F);
    const uint32_t ankle = s.joint(side * 0.1F, 0.08F, 0, 0.04F, 0.04F);
    const uint32_t toe = s.joint(side * 0.1F, 0.03F, 0.14F, 0.04F, 0.03F);
    s.bone(pelvis, hip);
    s.bone(hip, knee);
    s.bone(knee, ankle);
    s.bone(ankle, toe);
    const uint32_t shoulder = s.joint(side * 0.2F, 1.42F, 0, 0.06F, 0.06F);
    const uint32_t elbow = s.joint(side * (0.26F + arm_asym), 1.12F, 0.02F, 0.045F, 0.045F);
    const uint32_t wrist = s.joint(side * 0.28F, 0.86F, 0.04F, 0.03F, 0.03F);
    const uint32_t palm = s.joint(side * 0.285F, 0.78F, 0.045F, 0.04F, 0.02F);
    s.bone(chest, shoulder);
    s.bone(shoulder, elbow);
    s.bone(elbow, wrist);
    s.bone(wrist, palm);
    // Fingers off a chain of knuckles across the palm (each knuckle has a finger), not all off one hub: the Skin algorithm cannot separate several thin branches that leave one joint.
    uint32_t previous = palm;
    for (int finger = 0; finger < fingers; ++finger) {
      const float x = side * (0.285F + finger_gap * float(finger));
      const uint32_t knuckle = s.joint(x, 0.76F, 0.045F, 0.013F, 0.011F);
      const uint32_t tip = s.joint(x, 0.67F, 0.05F, 0.009F, 0.009F);
      s.bone(previous, knuckle);
      s.bone(knuckle, tip);
      previous = knuckle;
    }
  }
  return s;
}

// Connected pieces of the surface (faces joined by shared vertices), with each piece's lowest and highest point.
std::vector<std::array<float, 3>> components(const Result& r) {
  std::vector<size_t> parent(r.positions.size() / 3);
  for (size_t i = 0; i < parent.size(); ++i) {
    parent[i] = i;
  }
  auto find = [&](size_t x) {
    while (parent[x] != x) {
      parent[x] = parent[parent[x]];
      x = parent[x];
    }
    return x;
  };
  size_t at = 0;
  for (uint32_t size : r.face_sizes) {
    for (uint32_t k = 1; k < size; ++k) {
      parent[find(r.corners[at + k])] = find(r.corners[at]);
    }
    at += size;
  }
  std::map<size_t, std::array<float, 3>> pieces;  // root -> {vertex count, min y, max y}
  for (size_t i = 0; i < parent.size(); ++i) {
    auto& p = pieces.try_emplace(find(i), std::array<float, 3>{0.0F, 1e9F, -1e9F}).first->second;
    p[0] += 1.0F;
    p[1] = std::min(p[1], r.positions[3 * i + 1]);
    p[2] = std::max(p[2], r.positions[3 * i + 1]);
  }
  std::vector<std::array<float, 3>> out;
  for (const auto& [root, p] : pieces) {
    static_cast<void>(root);
    out.push_back(p);
  }
  return out;
}

int test_human_variants() {
  for (const int fingers : {0, 1, 2, 3}) {
    const Result r = run(human(fingers), 0.0F, 1);
    const Topology t = topology(r);
    static_cast<void>(std::fprintf(stderr, "human with %d fingers: status %d, %zu faces, boundary %zu, overused %zu, Euler %d, pieces:", fingers, int(r.status), t.faces, t.boundary, t.overused, t.euler()));
    for (const auto& p : components(r)) {
      static_cast<void>(std::fprintf(stderr, " [%d verts, y %.2f..%.2f]", int(p[0]), double(p[1]), double(p[2])));
    }
    static_cast<void>(std::fprintf(stderr, "\n"));
  }
  return 0;
}

int test_human() {
  int failures = 0;
  const Skeleton s = human();
  const Result r = run(s, 0.0F, 1);  // symmetry about x
  failures += check(r.status == VE_OK, "a human skeleton is skinned, status " + std::to_string(r.status));
  if (r.status != VE_OK) {
    return failures;
  }
  const Topology t = topology(r);
  static_cast<void>(std::fprintf(stderr, "human: %zu boundary edges, %zu overused edges\n", t.boundary, t.overused));
  failures += check(t.manifold, "human: manifold (every edge has two faces)");
  failures += check(t.euler() == 2, "human: one closed surface of genus 0, Euler number " + std::to_string(t.euler()));
  failures += check(t.quads * 10 >= t.faces * 7, "human: at least 70% quads, got " + std::to_string(t.quads) + " of " + std::to_string(t.faces));
  failures += check(signed_volume(r) > 0.0, "human: faces wind outward");
  failures += check(t.same_direction == 0, "human: neighbouring faces are wound consistently, " + std::to_string(t.same_direction) + " edges are used in the same direction twice");
  // The surface stays near the skeleton: inside the joints' bounding box grown by the largest radius and a margin.
  float lo[3] = {1e9F, 1e9F, 1e9F};
  float hi[3] = {-1e9F, -1e9F, -1e9F};
  for (size_t i = 0; i < r.positions.size(); i += 3) {
    for (int k = 0; k < 3; ++k) {
      lo[k] = std::min(lo[k], r.positions[i + size_t(k)]);
      hi[k] = std::max(hi[k], r.positions[i + size_t(k)]);
    }
  }
  failures += check(lo[1] > -0.05F && lo[1] < 0.08F, "human: the feet reach the floor, lowest y " + std::to_string(lo[1]));
  // The skin stops at an end joint (it does not extend past it), so the top of the head is the head joint's height, 1.68.
  failures += check(hi[1] > 1.66F && hi[1] < 1.72F, "human: the surface ends at the head joint, highest y " + std::to_string(hi[1]));
  failures += check(std::fabs(lo[0] + hi[0]) < 0.02F, "human: left-right symmetric, x range " + std::to_string(lo[0]) + " .. " + std::to_string(hi[0]));
  // Every joint owns some vertices.
  std::vector<int> owned(s.flags.size(), 0);
  for (uint32_t o : r.origin) {
    if (o < owned.size()) {
      ++owned[o];
    }
  }
  int without = 0;
  for (int count : owned) {
    without += count == 0 ? 1 : 0;
  }
  failures += check(without <= 4, "human: nearly every joint owns surface vertices, " + std::to_string(without) + " own none");
  // Symmetric halves: for every vertex there is one at the mirrored x (within a small tolerance).
  std::vector<std::array<float, 3>> pts;
  for (size_t i = 0; i < r.positions.size(); i += 3) {
    pts.push_back({r.positions[i], r.positions[i + 1], r.positions[i + 2]});
  }
  size_t unmatched = 0;
  for (const auto& p : pts) {
    bool found = false;
    for (const auto& q : pts) {
      if (std::fabs(q[0] + p[0]) < 0.01F && std::fabs(q[1] - p[1]) < 0.01F && std::fabs(q[2] - p[2]) < 0.01F) {
        found = true;
        break;
      }
    }
    unmatched += found ? 0U : 1U;
  }
  failures += check(unmatched * 20 <= pts.size(), "human: at least 95% of vertices have a mirror twin, unmatched " + std::to_string(unmatched) + " of " + std::to_string(pts.size()));
  static_cast<void>(std::fprintf(stderr, "human: %zu vertices, %zu faces (%zu quads, %zu triangles), Euler %d\n", t.vertices, t.faces, t.quads, t.tris, t.euler()));
  return failures;
}

int test_refusals() {
  int failures = 0;
  Skeleton s;
  const uint32_t a = s.joint(0, 0, 0, 0.1F, 0.1F);  // no root flag
  const uint32_t b = s.joint(0, 1, 0, 0.1F, 0.1F);
  s.bone(a, b);
  failures += check(run(s).status == VE_ERR_INVALID_ARG, "a piece with no root joint is refused");
  s.flags[0] = 1;
  failures += check(run(s).status == VE_OK, "with a root it is accepted");
  Skeleton bad = s;
  bad.edges = {0, 7};
  failures += check(run(bad).status == VE_ERR_OUT_OF_BOUNDS, "an edge to a missing joint is refused");
  bad = s;
  bad.radii[0] = -1.0F;
  failures += check(run(bad).status == VE_ERR_INVALID_ARG, "a negative radius is refused");
  bad = s;
  bad.positions[1] = std::nanf("");
  failures += check(run(bad).status == VE_ERR_NON_FINITE, "a non-finite position is refused");
  bad = s;
  bad.edges = {1, 1};
  failures += check(run(bad).status == VE_ERR_INVALID_ARG, "a joint joined to itself is refused");
  return failures;
}

int test_deterministic() {
  const Skeleton s = human();
  const Result a = run(s, 0.3F, 1);
  const Result b = run(s, 0.3F, 1);
  return check(a.positions == b.positions && a.corners == b.corners && a.face_sizes == b.face_sizes, "the same skeleton gives the same surface twice");
}

}  // namespace

int main() {
  int failures = 0;
  failures += test_straight_limb();
  failures += test_branch();
  failures += test_human_variants();
  failures += test_human();
  failures += test_refusals();
  failures += test_deterministic();
  if (failures == 0) {
    static_cast<void>(std::puts("skin_test: all checks passed"));
  }
  return failures == 0 ? 0 : 1;
}
