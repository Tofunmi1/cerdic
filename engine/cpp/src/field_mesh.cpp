#include <algorithm>
#include <array>
#include <cmath>
#include <memory>
#include <new>

#include "context_handle.hpp"
#include "field_tape.hpp"
#include "manifold/manifold.h"
#include "solid_handle.hpp"
#include "ve_kernel.h"

namespace {

constexpr size_t kMaxFieldCells = 2'000'000;
constexpr size_t kMaxFieldOps = 64'000'000;
constexpr size_t kMaxVoxelSlots = 2'097'152;

void set_status(ve_status* out, ve_status value) {
  if (out != nullptr) {
    *out = value;
  }
}

size_t next_power_of_two(size_t value) {
  size_t power = 1;
  while (power < value && power <= kMaxVoxelSlots) {
    power *= 2;
  }
  return power;
}

double component(const manifold::vec3& value, size_t axis) {
  switch (axis) {
    case 0:
      return value.x;
    case 1:
      return value.y;
    case 2:
      return value.z;
    default:
      return 0.0;
  }
}

struct GridInfo {
  manifold::vec3 minimum;
  manifold::vec3 maximum;
  manifold::vec3 extent;
  manifold::vec3 spacing;
  std::array<double, 6> bounds;
  std::array<size_t, 3> samples;
  size_t cells;
  size_t operations;
  size_t voxel_slots;
};

bool valid_request(const ve_field_input* input, ve_status* status) {
  if (input == nullptr || input->context == nullptr || input->instructions == nullptr ||
      input->struct_size < sizeof(ve_field_input) || input->instruction_count == 0 ||
      input->instruction_count > ve::field::kMaxFieldInstructions ||
      !std::isfinite(input->edge_length) || input->edge_length <= 0.0 || input->max_cells == 0 ||
      input->max_cells > kMaxFieldCells || input->max_operations == 0 ||
      input->max_operations > kMaxFieldOps) {
    set_status(status, VE_ERR_INVALID_ARG);
    return false;
  }
  if (!ve::field::valid_tape(input->instructions, input->instruction_count)) {
    set_status(status, VE_ERR_INVALID_ARG);
    return false;
  }
  return true;
}

bool prepare_grid(const ve_field_input* input, GridInfo& grid, ve_status* status) {
  grid.bounds = {input->bounds[0], input->bounds[1], input->bounds[2],
                 input->bounds[3], input->bounds[4], input->bounds[5]};
  for (const double bound : grid.bounds) {
    if (!std::isfinite(bound) || std::abs(bound) > 1.0e6) {
      set_status(status, VE_ERR_NON_FINITE);
      return false;
    }
  }
  grid.minimum = {grid.bounds.at(0), grid.bounds.at(1), grid.bounds.at(2)};
  grid.maximum = {grid.bounds.at(3), grid.bounds.at(4), grid.bounds.at(5)};
  grid.extent = grid.maximum - grid.minimum;
  if (grid.extent.x <= 0.0 || grid.extent.y <= 0.0 || grid.extent.z <= 0.0) {
    set_status(status, VE_ERR_INVALID_ARG);
    return false;
  }
  const std::array<double, 3> ratios = {grid.extent.x / input->edge_length,
                                        grid.extent.y / input->edge_length,
                                        grid.extent.z / input->edge_length};
  if (!std::all_of(ratios.begin(), ratios.end(),
                   [](double value) { return std::isfinite(value); })) {
    set_status(status, VE_ERR_NON_FINITE);
    return false;
  }
  if (std::any_of(ratios.begin(), ratios.end(), [](double value) { return value < 1.0; })) {
    set_status(status, VE_ERR_INVALID_ARG);
    return false;
  }
  if (std::any_of(ratios.begin(), ratios.end(),
                  [](double value) { return value > static_cast<double>(kMaxFieldCells); })) {
    set_status(status, VE_ERR_FIELD_BUDGET);
    return false;
  }
  for (size_t axis = 0; axis < grid.samples.size(); ++axis) {
    grid.samples.at(axis) = static_cast<size_t>(ratios.at(axis)) + 1;
  }
  if (grid.samples.at(0) > kMaxFieldCells / grid.samples.at(1) ||
      grid.samples.at(0) * grid.samples.at(1) > kMaxFieldCells / grid.samples.at(2)) {
    set_status(status, VE_ERR_FIELD_BUDGET);
    return false;
  }
  grid.cells = grid.samples.at(0) * grid.samples.at(1) * grid.samples.at(2);
  grid.operations = grid.cells * input->instruction_count;
  const std::array<size_t, 3> padded = {next_power_of_two(grid.samples.at(0) + 2),
                                        next_power_of_two(grid.samples.at(1) + 2),
                                        next_power_of_two(grid.samples.at(2) + 2)};
  const bool padded_overflow = padded.at(0) > kMaxVoxelSlots / padded.at(1) ||
                               padded.at(0) * padded.at(1) > kMaxVoxelSlots / padded.at(2);
  grid.voxel_slots =
      padded_overflow ? kMaxVoxelSlots + 1 : padded.at(0) * padded.at(1) * padded.at(2);
  grid.spacing = grid.extent / manifold::vec3(grid.samples.at(0) - 1, grid.samples.at(1) - 1,
                                              grid.samples.at(2) - 1);
  if (grid.cells > input->max_cells || grid.cells > kMaxFieldCells ||
      grid.operations > input->max_operations || grid.operations > kMaxFieldOps ||
      grid.voxel_slots > kMaxVoxelSlots) {
    set_status(status, VE_ERR_FIELD_BUDGET);
    return false;
  }
  return true;
}

template <typename Sample>
bool boundary_point_clips(const GridInfo& grid, const size_t axis, const size_t u, const size_t v,
                          const size_t i, const size_t j, const size_t side, Sample& sample,
                          double* clipped_point) {
  std::array<double, 3> point = {grid.minimum.x, grid.minimum.y, grid.minimum.z};
  point.at(axis) = grid.bounds.at(axis + (side * 3));
  point.at(u) += static_cast<double>(i) * component(grid.spacing, u);
  point.at(v) += static_cast<double>(j) * component(grid.spacing, v);
  if (sample({point.at(0), point.at(1), point.at(2)}) <= 0.0) {
    return false;
  }
  if (clipped_point != nullptr) {
    clipped_point[0] = point.at(0);
    clipped_point[1] = point.at(1);
    clipped_point[2] = point.at(2);
  }
  return true;
}

template <typename Sample>
bool boundary_clips(const GridInfo& grid, Sample& sample, double* clipped_point,
                    const bool& cancelled) {
  for (size_t axis = 0; axis < grid.samples.size(); ++axis) {
    const size_t u = (axis + 1) % 3;
    const size_t v = (axis + 2) % 3;
    for (size_t i = 0; i < grid.samples.at(u); ++i) {
      for (size_t j = 0; j < grid.samples.at(v); ++j) {
        for (size_t side = 0; side < 2; ++side) {
          if (boundary_point_clips(grid, axis, u, v, i, j, side, sample, clipped_point)) {
            return true;
          }
          if (cancelled) {
            return false;
          }
        }
      }
    }
  }
  return false;
}

}  // namespace

extern "C" ve_solid* ve_solid_from_field(const ve_field_input* input, double clipped_point[3],
                                         ve_status* status) {
  if (!valid_request(input, status)) {
    return nullptr;
  }
  GridInfo grid{};
  if (!prepare_grid(input, grid, status)) {
    return nullptr;
  }
  if (input->context->value.Cancelled()) {
    set_status(status, VE_ERR_CANCELLED);
    return nullptr;
  }
  ve::field::TapeEvaluator evaluator;
  size_t sampled_points = 0;
  bool cancelled_during_sampling = false;
  bool levelset_started = false;
  bool cancelled_in_levelset = false;
  const auto sample = [&evaluator, input, &sampled_points, &cancelled_during_sampling,
                       &levelset_started, &cancelled_in_levelset](manifold::vec3 point) {
    if (cancelled_during_sampling) {
      return 0.0;
    }
    if ((sampled_points++ & 4095U) == 0U && input->context->value.Cancelled()) {
      cancelled_during_sampling = true;
      cancelled_in_levelset = levelset_started;
      return 0.0;
    }
    return evaluator.Evaluate(input->instructions, input->instruction_count - 1, point);
  };
  if (boundary_clips(grid, sample, clipped_point, cancelled_during_sampling)) {
    set_status(status, VE_ERR_FIELD_CLIPPED);
    return nullptr;
  }
  if (cancelled_during_sampling || input->context->value.Cancelled()) {
    set_status(status, VE_ERR_CANCELLED);
    return nullptr;
  }
  const manifold::Box bounds(grid.minimum, grid.maximum);
  levelset_started = true;
  auto solid = input->context->value.LevelSet(sample, bounds, input->edge_length, 0.0, -1.0, false);
  if (cancelled_in_levelset || solid.Status() == manifold::Manifold::Error::Cancelled) {
    set_status(status, VE_ERR_CANCELLED);
    return nullptr;
  }
  if (solid.Status() != manifold::Manifold::Error::NoError) {
    set_status(status, VE_ERR_NOT_MANIFOLD);
    return nullptr;
  }
  if (solid.IsEmpty()) {
    set_status(status, VE_ERR_FIELD_EMPTY);
    return nullptr;
  }
  std::unique_ptr<ve_solid> handle(new (std::nothrow) ve_solid{std::move(solid)});
  if (!handle) {
    set_status(status, VE_ERR_INTERNAL);
    return nullptr;
  }
  set_status(status, VE_OK);
  return handle.release();
}

extern "C" ve_status ve_field_sample(const ve_field_instruction* instructions,
                                     size_t instruction_count, const double point[3],
                                     double* out_value) {
  if (instructions == nullptr || point == nullptr || out_value == nullptr ||
      instruction_count == 0 || instruction_count > ve::field::kMaxFieldInstructions ||
      !std::isfinite(point[0]) || !std::isfinite(point[1]) || !std::isfinite(point[2])) {
    return VE_ERR_INVALID_ARG;
  }
  if (!ve::field::valid_tape(instructions, instruction_count)) {
    return VE_ERR_INVALID_ARG;
  }
  ve::field::TapeEvaluator evaluator;
  *out_value =
      evaluator.Evaluate(instructions, instruction_count - 1, {point[0], point[1], point[2]});
  return std::isfinite(*out_value) ? VE_OK : VE_ERR_NON_FINITE;
}
