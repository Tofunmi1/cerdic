#ifndef VE_EXTRUSION_HPP
#define VE_EXTRUSION_HPP
#include <array>
#include <cmath>

#include "manifold/mesh.h"
#include "manifold/polygon.h"
#include "solid_handle.hpp"

namespace ve_extrusion {
// Four triangles around the quad's centre average both diagonal choices. A consistently
// chosen diagonal adds a first-order twist-volume bias; this symmetric surface avoids it.
inline void vertices(manifold::MeshGL64& mesh, const manifold::Polygons& loops, double height,
                     int divisions, double twist, manifold::vec2 top) {
  for (int layer = 0; layer <= divisions; ++layer) {
    const double t = static_cast<double>(layer) / static_cast<double>(divisions);
    const double angle = t * twist;
    const double sx = 1.0 + (t * (top.x - 1.0));
    const double sy = 1.0 + (t * (top.y - 1.0));
    for (const auto& loop : loops) {
      for (const auto point : loop) {
        mesh.vertProperties.push_back(sx *
                                      ((std::cos(angle) * point.x) - (std::sin(angle) * point.y)));
        mesh.vertProperties.push_back(height * t);
        mesh.vertProperties.push_back(-sy *
                                      ((std::sin(angle) * point.x) + (std::cos(angle) * point.y)));
      }
    }
  }
}
inline void triangle(manifold::MeshGL64& mesh, uint64_t a, uint64_t b, uint64_t c) {
  mesh.triVerts.insert(mesh.triVerts.end(), {a, b, c});
}
inline void caps(manifold::MeshGL64& mesh, const manifold::Polygons& loops, size_t count,
                 int divisions) {
  for (const auto face : manifold::Triangulate(loops)) {
    const auto a = static_cast<uint64_t>(face.x);
    const auto b = static_cast<uint64_t>(face.y);
    const auto c = static_cast<uint64_t>(face.z);
    triangle(mesh, c, b, a);
    const auto offset = count * static_cast<size_t>(divisions);
    triangle(mesh, a + offset, b + offset, c + offset);
  }
}
inline void fan(manifold::MeshGL64& mesh, const std::array<uint64_t, 4>& quad) {
  const auto center = mesh.vertProperties.size() / 3;
  for (size_t axis = 0; axis < 3; ++axis) {
    double value = 0.0;
    for (const auto index : quad) {
      value += mesh.vertProperties.at((3 * index) + axis) * 0.25;
    }
    mesh.vertProperties.push_back(value);
  }
  for (size_t i = 0; i < 4; ++i) {
    triangle(mesh, quad.at(i), quad.at((i + 1) % 4), center);
  }
}
inline void sides(manifold::MeshGL64& mesh, const manifold::Polygons& loops, size_t count,
                  int divisions) {
  for (int layer = 0; layer < divisions; ++layer) {
    size_t start = 0;
    for (const auto& loop : loops) {
      for (size_t edge = 0; edge < loop.size(); ++edge) {
        const auto a = (count * static_cast<size_t>(layer)) + start + edge;
        const auto b = (count * static_cast<size_t>(layer)) + start + ((edge + 1) % loop.size());
        const std::array<uint64_t, 4> quad = {a, b, b + count, a + count};
        fan(mesh, quad);
      }
      start += loop.size();
    }
  }
}
inline manifold::Manifold balanced(const manifold::Polygons& loops, double height, int divisions,
                                   double twist, manifold::vec2 top) {
  manifold::MeshGL64 mesh;
  size_t count = 0;
  for (const auto& loop : loops) {
    count += loop.size();
  }
  vertices(mesh, loops, height, divisions, twist, top);
  caps(mesh, loops, count, divisions);
  sides(mesh, loops, count, divisions);
  return {mesh};
}
}  // namespace ve_extrusion
#endif
