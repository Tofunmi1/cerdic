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
  // x is the radius at the segment's start and y at its end; the radius changes linearly between them.
  return instruction.x + (instruction.y - instruction.x) * t -
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
    if (frame.state == 0) {
      returned = primitive_value(instruction, instructions, frame.point);
      frames_.pop_back();
      continue;
    }
    returned = combine_value(instruction, frame.left, returned);
    frames_.pop_back();
  }
  return returned;
}

}  // namespace ve::field
