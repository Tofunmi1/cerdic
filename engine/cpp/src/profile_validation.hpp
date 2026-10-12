#ifndef VE_PROFILE_VALIDATION_HPP
#define VE_PROFILE_VALIDATION_HPP
#include <algorithm>
#include <cmath>

#include "manifold/common.h"

namespace ve_profile_validation {
constexpr size_t kMaxPoints = 16384;
constexpr size_t kMaxLoops = 64;
constexpr double kExtent = 1.0e6;

inline double cross(manifold::vec2 a, manifold::vec2 b, manifold::vec2 c) {
  return ((b.x - a.x) * (c.y - a.y)) - ((b.y - a.y) * (c.x - a.x));
}
inline bool on_segment(manifold::vec2 a, manifold::vec2 b, manifold::vec2 p) {
  return cross(a, b, p) == 0.0 && p.x >= std::min(a.x, b.x) && p.x <= std::max(a.x, b.x) &&
         p.y >= std::min(a.y, b.y) && p.y <= std::max(a.y, b.y);
}
inline bool intersect(manifold::vec2 a, manifold::vec2 b, manifold::vec2 c, manifold::vec2 d) {
  const double ab_c = cross(a, b, c);
  const double ab_d = cross(a, b, d);
  const double cd_a = cross(c, d, a);
  const double cd_b = cross(c, d, b);
  return ((ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0)) || on_segment(a, b, c) ||
         on_segment(a, b, d) || on_segment(c, d, a) || on_segment(c, d, b);
}
inline bool valid_loop(const manifold::SimplePolygon& loop) {
  if (loop.size() < 3 || loop.size() > 4096) {
    return false;
  }
  double area = 0.0;
  for (size_t i = 0; i < loop.size(); ++i) {
    const auto a = loop.at(i);
    const auto b = loop.at((i + 1) % loop.size());
    if (!std::isfinite(a.x) || !std::isfinite(a.y) || std::abs(a.x) > kExtent ||
        std::abs(a.y) > kExtent || (a.x == b.x && a.y == b.y)) {
      return false;
    }
    area += (a.x * b.y) - (b.x * a.y);
    for (size_t j = i + 2; j < loop.size(); ++j) {
      if (i == 0 && j + 1 == loop.size()) {
        continue;
      }
      if (intersect(a, b, loop.at(j), loop.at((j + 1) % loop.size()))) {
        return false;
      }
    }
  }
  return std::abs(area) > 1.0e-18;
}
inline bool bounded(const manifold::Polygons& loops) {
  size_t total = 0;
  for (const auto& loop : loops) {
    total += loop.size();
    if (loop.size() > 4096) {
      return false;
    }
  }
  return loops.size() <= kMaxLoops && total <= kMaxPoints;
}
}  // namespace ve_profile_validation
#endif
