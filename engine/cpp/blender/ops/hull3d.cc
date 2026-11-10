// Convex hull of a small point set (the frame corners around a branch of the Skin modifier, up to a few hundred points): an incremental hull in double precision.
// It implements the API Blender's bmesh `convex_hull` operator expects (shim/RBI_hull_api.h), in place of Bullet's btConvexHullComputer.
//
// Method: start from a tetrahedron of four well-spread points, then add every other point: the faces it can see (signed distance above a small tolerance) are removed and
// replaced by a fan from the point to the horizon edges (edges between a seen and an unseen face). A point lying in a face's plane is not "seen", so coplanar points that
// extend a face become hull vertices through their neighbours. Points that end up inside are simply not hull vertices. Deterministic: points are taken in input order.
#include <algorithm>
#include <array>
#include <cmath>
#include <map>
#include <utility>
#include <vector>

#include "RBI_hull_api.h"

namespace {

using Vec = std::array<double, 3>;

Vec sub(const Vec &a, const Vec &b) { return {a[0] - b[0], a[1] - b[1], a[2] - b[2]}; }
Vec cross(const Vec &a, const Vec &b) { return {a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]}; }
double dot(const Vec &a, const Vec &b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
double length(const Vec &a) { return std::sqrt(dot(a, a)); }

struct Face {
  std::array<int, 3> v;
  Vec normal;  // unit, outward
  double offset;  // dot(normal, point on the face)
  bool alive = true;
};

struct Hull {
  std::vector<Vec> points;           // input points
  std::vector<Face> faces;
  std::vector<int> vertex_of_input;  // input index -> hull vertex index, or -1
  std::vector<int> input_of_vertex;  // hull vertex -> input index
  std::vector<std::array<int, 3>> out_faces;  // in hull vertex indices
};

Face make_face(const std::vector<Vec> &pts, int a, int b, int c) {
  Face f;
  f.v = {a, b, c};
  Vec n = cross(sub(pts[size_t(b)], pts[size_t(a)]), sub(pts[size_t(c)], pts[size_t(a)]));
  const double len = length(n);
  f.normal = len > 0.0 ? Vec{n[0] / len, n[1] / len, n[2] / len} : Vec{0, 0, 0};
  f.offset = dot(f.normal, pts[size_t(a)]);
  return f;
}

}  // namespace

extern "C" {

plConvexHull plConvexHullCompute(float (*coords)[3], int count) {
  auto *hull = new Hull();
  hull->vertex_of_input.assign(size_t(std::max(count, 0)), -1);
  for (int i = 0; i < count; ++i) {
    hull->points.push_back({double(coords[i][0]), double(coords[i][1]), double(coords[i][2])});
  }
  const std::vector<Vec> &p = hull->points;
  if (count >= 4) {
    // Scale of the point cloud, for the tolerances.
    Vec lo = p[0];
    Vec hi = p[0];
    for (const Vec &q : p) {
      for (size_t k = 0; k < 3; ++k) {
        lo[k] = std::min(lo[k], q[k]);
        hi[k] = std::max(hi[k], q[k]);
      }
    }
    const double scale = std::max({hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1e-12});
    const double eps = 1e-7 * scale;

    // Four points that span a tetrahedron: extreme in x, farthest from it, farthest from that line, farthest from that plane.
    int i0 = 0;
    for (int i = 1; i < count; ++i) {
      if (p[size_t(i)][0] < p[size_t(i0)][0]) {
        i0 = i;
      }
    }
    int i1 = -1;
    double best = eps;
    for (int i = 0; i < count; ++i) {
      const double d = length(sub(p[size_t(i)], p[size_t(i0)]));
      if (d > best) {
        best = d;
        i1 = i;
      }
    }
    int i2 = -1;
    best = eps;
    if (i1 >= 0) {
      const Vec axis = sub(p[size_t(i1)], p[size_t(i0)]);
      for (int i = 0; i < count; ++i) {
        const double d = length(cross(axis, sub(p[size_t(i)], p[size_t(i0)]))) / length(axis);
        if (d > best) {
          best = d;
          i2 = i;
        }
      }
    }
    int i3 = -1;
    best = eps;
    if (i2 >= 0) {
      Vec n = cross(sub(p[size_t(i1)], p[size_t(i0)]), sub(p[size_t(i2)], p[size_t(i0)]));
      const double nl = length(n);
      n = {n[0] / nl, n[1] / nl, n[2] / nl};
      for (int i = 0; i < count; ++i) {
        const double d = std::fabs(dot(n, sub(p[size_t(i)], p[size_t(i0)])));
        if (d > best) {
          best = d;
          i3 = i;
        }
      }
    }
    if (i3 >= 0) {
      // Orient the four faces outward: a face is outward when the fourth point is behind it.
      const std::array<int, 4> t = {i0, i1, i2, i3};
      const int tri[4][3] = {{0, 1, 2}, {0, 3, 1}, {1, 3, 2}, {0, 2, 3}};
      const int opp[4] = {3, 2, 0, 1};
      for (int k = 0; k < 4; ++k) {
        Face f = make_face(p, t[size_t(tri[k][0])], t[size_t(tri[k][1])], t[size_t(tri[k][2])]);
        if (dot(f.normal, p[size_t(t[size_t(opp[k])])]) - f.offset > 0.0) {
          f = make_face(p, t[size_t(tri[k][0])], t[size_t(tri[k][2])], t[size_t(tri[k][1])]);
        }
        hull->faces.push_back(f);
      }
      for (int i = 0; i < count; ++i) {
        if (i == i0 || i == i1 || i == i2 || i == i3) {
          continue;
        }
        std::vector<size_t> visible;
        for (size_t f = 0; f < hull->faces.size(); ++f) {
          const Face &face = hull->faces[f];
          if (face.alive && dot(face.normal, p[size_t(i)]) - face.offset > eps) {
            visible.push_back(f);
          }
        }
        if (visible.empty()) {
          continue;
        }
        // Directed edges of every live face, to find the horizon: an edge of a visible face whose reverse belongs to a face that is not visible.
        std::map<std::pair<int, int>, size_t> edge_face;
        for (size_t f = 0; f < hull->faces.size(); ++f) {
          if (!hull->faces[f].alive) {
            continue;
          }
          const auto &v = hull->faces[f].v;
          for (int e = 0; e < 3; ++e) {
            edge_face[{v[size_t(e)], v[size_t((e + 1) % 3)]}] = f;
          }
        }
        std::vector<bool> is_visible(hull->faces.size(), false);
        for (size_t f : visible) {
          is_visible[f] = true;
        }
        std::vector<std::pair<int, int>> horizon;
        for (size_t f : visible) {
          const auto &v = hull->faces[f].v;
          for (int e = 0; e < 3; ++e) {
            const int a = v[size_t(e)];
            const int b = v[size_t((e + 1) % 3)];
            const auto other = edge_face.find({b, a});
            if (other != edge_face.end() && !is_visible[other->second]) {
              horizon.emplace_back(a, b);
            }
          }
        }
        for (size_t f : visible) {
          hull->faces[f].alive = false;
        }
        for (const auto &[a, b] : horizon) {
          hull->faces.push_back(make_face(p, a, b, i));
        }
      }
      // Collect the vertices and faces that survived.
      for (const Face &face : hull->faces) {
        if (!face.alive) {
          continue;
        }
        std::array<int, 3> loop{};
        for (size_t k = 0; k < 3; ++k) {
          int &slot = hull->vertex_of_input[size_t(face.v[k])];
          if (slot < 0) {
            slot = int(hull->input_of_vertex.size());
            hull->input_of_vertex.push_back(face.v[k]);
          }
          loop[k] = slot;
        }
        hull->out_faces.push_back(loop);
      }
    }
  }
  return reinterpret_cast<plConvexHull>(hull);
}

void plConvexHullDelete(plConvexHull hull) { delete reinterpret_cast<Hull *>(hull); }

int plConvexHullNumVertices(plConvexHull hull) { return int(reinterpret_cast<Hull *>(hull)->input_of_vertex.size()); }

int plConvexHullNumFaces(plConvexHull hull) { return int(reinterpret_cast<Hull *>(hull)->out_faces.size()); }

void plConvexHullGetVertex(plConvexHull hull, int n, float coords[3], int *original_index) {
  const Hull *h = reinterpret_cast<Hull *>(hull);
  const int input = h->input_of_vertex[size_t(n)];
  for (size_t k = 0; k < 3; ++k) {
    coords[k] = float(h->points[size_t(input)][k]);
  }
  *original_index = input;
}

int plConvexHullGetFaceSize(plConvexHull, int) { return 3; }

void plConvexHullGetFaceVertices(plConvexHull hull, int n, int *r_vertices) {
  const auto &f = reinterpret_cast<Hull *>(hull)->out_faces[size_t(n)];
  r_vertices[0] = f[0];
  r_vertices[1] = f[1];
  r_vertices[2] = f[2];
}

}  // extern "C"
