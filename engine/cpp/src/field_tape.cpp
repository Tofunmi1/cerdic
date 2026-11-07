#include "field_tape.hpp"

#include <algorithm>
#include <cmath>
#include <limits>

namespace ve::field {
namespace {

bool finite_instruction(const ve_field_instruction& instruction) {
  return std::isfinite(instruction.x) && std::isfinite(instruction.y) &&
         std::isfinite(instruction.z) && std::isfinite(instruction.w);
}

bool valid_reference(uint32_t reference, size_t current) { return reference < current; }

bool valid_instruction(const ve_field_instruction& instruction, size_t index) {
  if (!finite_instruction(instruction)) {
    return false;
  }
  switch (instruction.opcode) {
    case VE_FIELD_SPHERE:
      return instruction.w > 0.0;
    case VE_FIELD_ELLIPSOID:
      return instruction.x > 0.0 && instruction.y > 0.0 && instruction.z > 0.0;
    case VE_FIELD_BOX:
      return instruction.x > 0.0 && instruction.y > 0.0 && instruction.z > 0.0 &&
             instruction.w >= 0.0 &&
             instruction.w <= std::min({instruction.x, instruction.y, instruction.z});
    case VE_FIELD_TORUS:
      return instruction.x > 0.0 && instruction.y > 0.0 && instruction.x > instruction.y;
    case VE_FIELD_CAPSULE:
    case VE_FIELD_CYLINDER:
      return instruction.x > 0.0 && instruction.y >= 0.0;
    case VE_FIELD_PLANE:
      return std::abs(std::sqrt((instruction.x * instruction.x) + (instruction.y * instruction.y) +
                                (instruction.z * instruction.z)) -
                      1.0) <= 1.0e-12;
    case VE_FIELD_TUBE_POINT:
      return instruction.w == 0.0;
    case VE_FIELD_TUBE_SEGMENT:
      return valid_reference(instruction.input_a, index) &&
             valid_reference(instruction.input_b, index) && instruction.x > 0.0 &&
             instruction.y > 0.0;
    case VE_FIELD_REPEAT: {
      const std::array<uint32_t, 3> counts = {instruction.input_b & 0xffU,
                                              (instruction.input_b >> 8U) & 0xffU,
                                              (instruction.input_b >> 16U) & 0xffU};
      return valid_reference(instruction.input_a, index) && instruction.x > 0.0 &&
             instruction.y > 0.0 && instruction.z > 0.0 && instruction.w == 0.0 &&
             (instruction.input_b >> 24U) == 0U &&
             std::all_of(counts.begin(), counts.end(), [](uint32_t count) { return count <= 16U; });
    }
    case VE_FIELD_NOISE:
      return valid_reference(instruction.input_a, index) && instruction.x >= 0.0 &&
             instruction.y > 0.0 && instruction.z >= 0.0 && instruction.z <= 1.0 &&
             instruction.w >= 1.0 && instruction.w <= 14.0 &&
             std::floor(instruction.w) == instruction.w;
    case VE_FIELD_WARP:
      return valid_reference(instruction.input_a, index) && instruction.input_b < 196608U && instruction.y != instruction.x;
    case VE_FIELD_UNION:
    case VE_FIELD_INTERSECTION:
    case VE_FIELD_DIFFERENCE:
      return valid_reference(instruction.input_a, index) &&
             valid_reference(instruction.input_b, index);
    case VE_FIELD_SMOOTH_UNION:
      return valid_reference(instruction.input_a, index) &&
             valid_reference(instruction.input_b, index) && instruction.x > 0.0;
    case VE_FIELD_MOVE:
    case VE_FIELD_OFFSET:
    case VE_FIELD_SHELL:
      return valid_reference(instruction.input_a, index) &&
             (instruction.opcode != VE_FIELD_SHELL || instruction.x > 0.0);
    case VE_FIELD_MIRROR:
      return valid_reference(instruction.input_a, index) && instruction.x >= 0.0 &&
             instruction.x <= 7.0 && std::floor(instruction.x) == instruction.x;
    case VE_FIELD_SCALE:
      return valid_reference(instruction.input_a, index) && instruction.x > 0.0;
    default:
      return false;
  }
}

bool is_binary(uint32_t opcode) {
  return opcode == VE_FIELD_UNION || opcode == VE_FIELD_INTERSECTION ||
         opcode == VE_FIELD_DIFFERENCE || opcode == VE_FIELD_SMOOTH_UNION;
}

bool is_transform(uint32_t opcode) {
  return opcode == VE_FIELD_MOVE || opcode == VE_FIELD_SCALE || opcode == VE_FIELD_MIRROR ||
         opcode == VE_FIELD_REPEAT;
}

double repeat_axis(double coordinate, double period, uint32_t count) {
  const auto bound = static_cast<double>(count);
  const double cell = std::clamp(std::round(coordinate / period), -bound, bound);
  return coordinate - (cell * period);
}

manifold::vec3 transform_point(const ve_field_instruction& instruction, manifold::vec3 point) {
  if (instruction.opcode == VE_FIELD_MOVE) {
    return point - manifold::vec3(instruction.x, instruction.y, instruction.z);
  }
  if (instruction.opcode == VE_FIELD_SCALE) {
    return point / instruction.x;
  }
  if (instruction.opcode == VE_FIELD_REPEAT) {
    point.x = repeat_axis(point.x, instruction.x, instruction.input_b & 0xffU);
    point.y = repeat_axis(point.y, instruction.y, (instruction.input_b >> 8U) & 0xffU);
    point.z = repeat_axis(point.z, instruction.z, (instruction.input_b >> 16U) & 0xffU);
    return point;
  }
  const auto axes = static_cast<uint32_t>(instruction.x);
  if ((axes & 1U) != 0U) {
    point.x = std::abs(point.x);
  }
  if ((axes & 2U) != 0U) {
    point.y = std::abs(point.y);
  }
  if ((axes & 4U) != 0U) {
    point.z = std::abs(point.z);
  }
  return point;
}

double tube_segment_value(const ve_field_instruction& instruction,
                          const ve_field_instruction* instructions, manifold::vec3 point) {
  const auto& start = instructions[instruction.input_a];
  const auto& end = instructions[instruction.input_b];
  const manifold::vec3 from(start.x, start.y, start.z);
  const manifold::vec3 direction(end.x - start.x, end.y - start.y, end.z - start.z);
  const manifold::vec3 relative = point - from;
  const double denominator =
      (direction.x * direction.x) + (direction.y * direction.y) + (direction.z * direction.z);
  const double projection =
      ((relative.x * direction.x) + (relative.y * direction.y) + (relative.z * direction.z)) /
      denominator;
  const double t = std::clamp(projection, 0.0, 1.0);
  const manifold::vec3 delta = relative - direction * t;
  // x is the radius at the segment's start and y at its end; the radius changes linearly between
  // them.
  return instruction.x + ((instruction.y - instruction.x) * t) -
         std::sqrt((delta.x * delta.x) + (delta.y * delta.y) + (delta.z * delta.z));
}

double primitive_value(const ve_field_instruction& instruction,
                       const ve_field_instruction* instructions, manifold::vec3 point) {
  switch (instruction.opcode) {
    case VE_FIELD_SPHERE: {
      const double dx = point.x - instruction.x;
      const double dy = point.y - instruction.y;
      const double dz = point.z - instruction.z;
      return instruction.w - std::sqrt((dx * dx) + (dy * dy) + (dz * dz));
    }
    case VE_FIELD_ELLIPSOID: {
      const double minimum = std::min({instruction.x, instruction.y, instruction.z});
      const double dx = point.x / instruction.x;
      const double dy = point.y / instruction.y;
      const double dz = point.z / instruction.z;
      return minimum * (1.0 - std::sqrt((dx * dx) + (dy * dy) + (dz * dz)));
    }
    case VE_FIELD_BOX: {
      const double qx = std::abs(point.x) - instruction.x + instruction.w;
      const double qy = std::abs(point.y) - instruction.y + instruction.w;
      const double qz = std::abs(point.z) - instruction.z + instruction.w;
      const double ox = std::max(qx, 0.0);
      const double oy = std::max(qy, 0.0);
      const double oz = std::max(qz, 0.0);
      return instruction.w - std::sqrt((ox * ox) + (oy * oy) + (oz * oz)) -
             std::min(std::max({qx, qy, qz}), 0.0);
    }
    case VE_FIELD_TORUS: {
      const double ring = std::sqrt((point.x * point.x) + (point.z * point.z)) - instruction.x;
      return instruction.y - std::sqrt((ring * ring) + (point.y * point.y));
    }
    case VE_FIELD_CAPSULE: {
      const double cy = std::clamp(point.y, -instruction.y, instruction.y);
      const double dx = point.x;
      const double dy = point.y - cy;
      return instruction.x - std::sqrt((dx * dx) + (dy * dy) + (point.z * point.z));
    }
    case VE_FIELD_CYLINDER: {
      const double radial = std::sqrt((point.x * point.x) + (point.z * point.z)) - instruction.x;
      const double vertical = std::abs(point.y) - instruction.y;
      const double ox = std::max(radial, 0.0);
      const double oy = std::max(vertical, 0.0);
      return -(std::sqrt((ox * ox) + (oy * oy)) + std::min(std::max(radial, vertical), 0.0));
    }
    case VE_FIELD_PLANE:
      return (instruction.x * point.x) + (instruction.y * point.y) + (instruction.z * point.z) +
             instruction.w;
    case VE_FIELD_TUBE_SEGMENT:
      return tube_segment_value(instruction, instructions, point);
    default:
      return std::numeric_limits<double>::quiet_NaN();
  }
}

double combine_value(const ve_field_instruction& instruction, double left, double right) {
  switch (instruction.opcode) {
    case VE_FIELD_UNION:
      return std::max(left, right);
    case VE_FIELD_INTERSECTION:
      return std::min(left, right);
    case VE_FIELD_DIFFERENCE:
      return std::min(left, -right);
    case VE_FIELD_SMOOTH_UNION: {
      const double h = std::max(instruction.x - std::abs(left - right), 0.0) / instruction.x;
      return std::max(left, right) + (h * h * instruction.x * 0.25);
    }
    case VE_FIELD_SCALE:
      return right * instruction.x;
    case VE_FIELD_OFFSET:
      return right + instruction.x;
    case VE_FIELD_SHELL:
      return instruction.x - std::abs(right);
    default:
      return right;
  }
}

uint64_t noise_hash(int64_t x, int64_t y, int64_t z, uint32_t seed) {
  uint64_t h = static_cast<uint64_t>(seed) * 0x9E3779B97F4A7C15ULL;
  for (const int64_t coordinate : {x, y, z}) {
    h ^= static_cast<uint64_t>(coordinate);
    h *= 0xBF58476D1CE4E5B9ULL;
    h ^= h >> 31U;
  }
  h *= 0x94D049BB133111EBULL;
  h ^= h >> 29U;
  return h;
}

double noise_corner(int64_t x, int64_t y, int64_t z, uint32_t seed) {
  const double unit = static_cast<double>(noise_hash(x, y, z, seed) >> 40U) / 16777216.0;
  return (unit * 2.0) - 1.0;
}

double smooth_noise(manifold::vec3 point, uint32_t seed) {
  const std::array<double, 3> at = {point.x, point.y, point.z};
  const std::array<double, 3> base = {std::floor(point.x), std::floor(point.y),
                                      std::floor(point.z)};
  const std::array<int64_t, 3> cell = {static_cast<int64_t>(base.at(0)),
                                       static_cast<int64_t>(base.at(1)),
                                       static_cast<int64_t>(base.at(2))};
  const std::array<double, 3> fade = {
      (at.at(0) - base.at(0)) * (at.at(0) - base.at(0)) * (3.0 - (2.0 * (at.at(0) - base.at(0)))),
      (at.at(1) - base.at(1)) * (at.at(1) - base.at(1)) * (3.0 - (2.0 * (at.at(1) - base.at(1)))),
      (at.at(2) - base.at(2)) * (at.at(2) - base.at(2)) * (3.0 - (2.0 * (at.at(2) - base.at(2))))};
  double sum = 0.0;
  for (int dx = 0; dx < 2; ++dx) {
    for (int dy = 0; dy < 2; ++dy) {
      for (int dz = 0; dz < 2; ++dz) {
        const double weight = (dx != 0 ? fade.at(0) : 1.0 - fade.at(0)) *
                              (dy != 0 ? fade.at(1) : 1.0 - fade.at(1)) *
                              (dz != 0 ? fade.at(2) : 1.0 - fade.at(2));
        sum += weight * noise_corner(cell.at(0) + dx, cell.at(1) + dy, cell.at(2) + dz, seed);
      }
    }
  }
  return sum;
}

// The point of the straight input a point of the bent field comes from; the same formulas as
// `bend_inverse` in `ve-geometry/src/field/warp.rs` (the axis curves toward `toward` through
// `angle_degrees` between `from` and `to`; unchanged before `from`; a rigid motion past `to`).
manifold::vec3 bend_inverse(manifold::vec3 point, uint32_t axis, uint32_t toward, double from,
                            double to, double angle_degrees) {
  const double theta = angle_degrees * 0.017453292519943295;
  if (theta == 0.0) {
    return point;
  }
  const double radius = (to - from) / theta;
  const double sign = radius < 0.0 ? -1.0 : 1.0;
  const double ds = point[axis] - from;
  const double w = radius - point[toward];
  const double phi = std::atan2(ds * sign, w * sign);
  const double turned = phi * (theta < 0.0 ? -1.0 : 1.0);
  if (turned < 0.0) {
    return point;
  }
  manifold::vec3 out = point;
  if (turned > std::abs(theta)) {
    const double sine = std::sin(theta);
    const double cosine = std::cos(theta);
    const double end_axis = from + (radius * sine);
    const double end_toward = radius * (1.0 - cosine);
    const double x = point[axis] - end_axis;
    const double y = point[toward] - end_toward;
    out[axis] = to + (x * cosine) + (y * sine);
    out[toward] = (-x * sine) + (y * cosine);
    return out;
  }
  out[axis] = from + (radius * phi);
  out[toward] = radius - (sign * std::hypot(ds, w));
  return out;
}

double noise_value(const ve_field_instruction& instruction, manifold::vec3 point, double child) {
  if (instruction.x == 0.0) {
    return child;
  }
  const auto packed = static_cast<uint32_t>(instruction.w);
  const uint32_t octaves = packed & 7U;
  const bool ridged = (packed & 8U) != 0U;
  double sum = 0.0;
  double total = 0.0;
  double weight = 1.0;
  for (uint32_t octave = 0; octave < octaves; ++octave) {
    const auto frequency = static_cast<double>(1U << octave);
    const double value =
        smooth_noise({point.x / instruction.y * frequency, point.y / instruction.y * frequency,
                      point.z / instruction.y * frequency},
                     instruction.input_b + octave);
    sum += weight * (ridged ? 1.0 - (2.0 * std::abs(value)) : value);
    total += weight;
    weight *= instruction.z;
  }
  return child - (instruction.x * (sum / total));
}

}  // namespace

bool valid_tape(const ve_field_instruction* instructions, size_t count) {
  if (instructions == nullptr || count == 0 || count > kMaxFieldInstructions ||
      instructions[count - 1].opcode == VE_FIELD_TUBE_POINT) {
    return false;
  }
  for (size_t index = 0; index < count; ++index) {
    if (!valid_instruction(instructions[index], index)) {
      return false;
    }
    const auto& instruction = instructions[index];
    if (instruction.opcode == VE_FIELD_TUBE_SEGMENT) {
      const auto& start = instructions[instruction.input_a];
      const auto& end = instructions[instruction.input_b];
      const bool point_refs =
          start.opcode == VE_FIELD_TUBE_POINT && end.opcode == VE_FIELD_TUBE_POINT;
      const bool distinct = start.x != end.x || start.y != end.y || start.z != end.z;
      if (!point_refs || !distinct) {
        return false;
      }
    }
  }
  return true;
}

double TapeEvaluator::Evaluate(const ve_field_instruction* instructions, size_t root,
                               manifold::vec3 point) {
  frames_.clear();
  frames_.push_back({static_cast<uint32_t>(root), point});
  double returned = 0.0;
  while (!frames_.empty()) {
    Frame& frame = frames_.back();
    const auto& instruction = instructions[frame.instruction];
    if (is_binary(instruction.opcode) && frame.state < 2) {
      if (frame.state == 0) {
        frame.state = 1;
        frames_.push_back({instruction.input_a, frame.point});
      } else {
        frame.left = returned;
        frame.state = 2;
        frames_.push_back({instruction.input_b, frame.point});
      }
      continue;
    }
    if (is_transform(instruction.opcode) && frame.state == 0) {
      frame.state = 1;
      frames_.push_back({instruction.input_a, transform_point(instruction, frame.point)});
      continue;
    }
    if ((instruction.opcode == VE_FIELD_OFFSET || instruction.opcode == VE_FIELD_SHELL) &&
        frame.state == 0) {
      frame.state = 1;
      frames_.push_back({instruction.input_a, frame.point});
      continue;
    }
    if (instruction.opcode == VE_FIELD_NOISE && frame.state == 0) {
      frame.state = 1;
      frames_.push_back({instruction.input_a, frame.point});
      continue;
    }
    if (instruction.opcode == VE_FIELD_WARP && frame.state == 0) {
      frame.state = 1;
      auto point = frame.point;
      const auto kind = instruction.input_b & 0xffU;
      const auto axis = (instruction.input_b >> 8U) & 0xffU;
      const auto a = (axis + 1U) % 3U;
      const auto b = (axis + 2U) % 3U;
      const double t = std::clamp((point[axis] - instruction.x) / (instruction.y - instruction.x), 0.0, 1.0);
      const double value = instruction.z + (instruction.w - instruction.z) * t;
      if (kind == 0U) { point[a] /= value; point[b] /= value; }
      else if (kind == 1U) { const double angle = value * 0.017453292519943295; const double c = std::cos(angle), s = std::sin(angle); const double old_a = point[a], old_b = point[b]; point[a] = c * old_a + s * old_b; point[b] = -s * old_a + c * old_b; }
      else { point = bend_inverse(point, axis, (instruction.input_b >> 16U) & 0xffU, instruction.x, instruction.y, instruction.w); }
      frames_.push_back({instruction.input_a, point});
      continue;
    }
    if (frame.state == 0) {
      returned = primitive_value(instruction, instructions, frame.point);
      frames_.pop_back();
      continue;
    }
    returned = instruction.opcode == VE_FIELD_WARP && (instruction.input_b & 0xffU) == 0U
                   ? returned * (instruction.z + (instruction.w - instruction.z) * std::clamp((frame.point[(instruction.input_b >> 8U) & 0xffU] - instruction.x) / (instruction.y - instruction.x), 0.0, 1.0))
                   : instruction.opcode == VE_FIELD_NOISE
                   ? noise_value(instruction, frame.point, returned)
                   : combine_value(instruction, frame.left, returned);
    frames_.pop_back();
  }
  return returned;
}

}  // namespace ve::field
