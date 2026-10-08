// Our own C++: weld duplicate vertices (exercises libc++ containers inside the shared module).
#include <cmath>
#include <cstdint>
#include <unordered_map>
#include <vector>

namespace {
struct Key {
  int64_t x, y, z;
  bool operator==(const Key& o) const { return x == o.x && y == o.y && z == o.z; }
};
struct KeyHash {
  size_t operator()(const Key& k) const {
    return std::hash<int64_t>()(k.x * 73856093 ^ k.y * 19349663 ^ k.z * 83492791);
  }
};
}  // namespace

extern "C" uint32_t gp_weld(const float* positions, uint32_t vertex_count, float epsilon,
                            uint32_t* remap) {
  std::unordered_map<Key, uint32_t, KeyHash> seen;
  std::vector<uint32_t> unique;
  unique.reserve(vertex_count);
  for (uint32_t i = 0; i < vertex_count; ++i) {
    const float* p = positions + i * 3;
    Key k{std::llround(p[0] / epsilon), std::llround(p[1] / epsilon), std::llround(p[2] / epsilon)};
    auto [it, inserted] = seen.emplace(k, static_cast<uint32_t>(unique.size()));
    if (inserted) unique.push_back(i);
    remap[i] = it->second;
  }
  return static_cast<uint32_t>(unique.size());
}
