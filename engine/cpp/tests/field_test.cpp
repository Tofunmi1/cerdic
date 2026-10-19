// Native field kernel tests, run under ASan and UBSan.
#include <array>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <thread>

#include "ve_kernel.h"

namespace {

int& failures() {
  static int count = 0;
  return count;
}

void check(bool ok, const char* label) {
  if (!ok) {
    std::fprintf(stderr, "FAIL: %s\n", label);
    ++failures();
  }
}

struct TestContext {
  std::unique_ptr<ve_context, decltype(&ve_context_free)> context{ve_context_new(),
                                                                  ve_context_free};
  std::array<double, 3> clipped{};
};

ve_field_input input_for(const ve_field_instruction* instructions, size_t count, double edge,
                         ve_context* context) {
  ve_field_input input{};
  input.struct_size = sizeof(input);
  input.instructions = instructions;
  input.instruction_count = count;
  input.bounds[0] = -1.0;
  input.bounds[1] = -1.0;
  input.bounds[2] = -1.0;
  input.bounds[3] = 1.0;
  input.bounds[4] = 1.0;
  input.bounds[5] = 1.0;
  input.edge_length = edge;
  input.max_cells = 2'000'000;
  input.max_operations = 64'000'000;
  input.context = context;
  return input;
}

double sphere_volume(double edge) {
  const std::array<ve_field_instruction, 1> tape = {{{VE_FIELD_SPHERE, 0, 0, 0, 0, 0, 0.7}}};
  TestContext test;
  auto input = input_for(tape.data(), tape.size(), edge, test.context.get());
  ve_status status = VE_OK;
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> solid(
      ve_solid_from_field(&input, test.clipped.data(), &status), ve_solid_free);
  check(solid != nullptr && status == VE_OK, "sphere builds as a manifold solid");
  return solid == nullptr ? 0.0 : ve_solid_volume(solid.get());
}

void analytic_tests() {
  const double exact_sphere = 4.0 * M_PI * 0.7 * 0.7 * 0.7 / 3.0;
  const double coarse_error = std::abs(sphere_volume(0.08) - exact_sphere);
  const double fine_error = std::abs(sphere_volume(0.04) - exact_sphere);
  check(fine_error < coarse_error, "sphere volume error decreases with finer edge length");

  const std::array<ve_field_instruction, 1> torus = {{{VE_FIELD_TORUS, 0, 0, 0.5, 0.2, 0, 0}}};
  TestContext test;
  auto input = input_for(torus.data(), torus.size(), 0.04, test.context.get());
  ve_status status = VE_OK;
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> solid(
      ve_solid_from_field(&input, test.clipped.data(), &status), ve_solid_free);
  const double exact_torus = 2.0 * M_PI * M_PI * 0.5 * 0.2 * 0.2;
  check(solid != nullptr && status == VE_OK, "torus builds as a manifold solid");
  check(solid != nullptr && std::abs(ve_solid_volume(solid.get()) - exact_torus) < 0.015,
        "torus volume stays near its analytic value");
  check(solid != nullptr && ve_solid_num_components(solid.get()) == 1,
        "torus is one closed component");
}

void composition_tests() {
  const std::array<ve_field_instruction, 3> tape = {{{VE_FIELD_SPHERE, 0, 0, -0.3, 0, 0, 0.5},
                                                     {VE_FIELD_SPHERE, 0, 0, 0.3, 0, 0, 0.5},
                                                     {VE_FIELD_SMOOTH_UNION, 0, 1, 0.2, 0, 0, 0}}};
  TestContext test;
  auto input = input_for(tape.data(), tape.size(), 0.04, test.context.get());
  ve_status status = VE_OK;
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> solid(
      ve_solid_from_field(&input, test.clipped.data(), &status), ve_solid_free);
  check(solid != nullptr && status == VE_OK && ve_solid_volume(solid.get()) > 0.4,
        "smooth union produces a blended solid");
  const std::array<ve_field_instruction, 2> moved = {
      {{VE_FIELD_SPHERE, 0, 0, 0, 0, 0, 0.3}, {VE_FIELD_MOVE, 0, 0, 0.4, 0.1, 0.0, 0}}};
  auto moved_input = input_for(moved.data(), moved.size(), 0.04, test.context.get());
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> translated(
      ve_solid_from_field(&moved_input, test.clipped.data(), &status), ve_solid_free);
  std::array<double, 6> bounds{};
  check(translated != nullptr && ve_solid_bounds(translated.get(), bounds.data()) == VE_OK &&
            std::abs(bounds[0] - 0.1) < 0.03 && std::abs(bounds[3] - 0.7) < 0.03,
        "move evaluates its child at transformed coordinates");
}

void refusal_tests() {
  const std::array<ve_field_instruction, 1> sphere = {{{VE_FIELD_SPHERE, 0, 0, 0, 0, 0, 0.8}}};
  TestContext test;
  auto clipped = input_for(sphere.data(), sphere.size(), 0.04, test.context.get());
  clipped.bounds[0] = -0.5;
  clipped.bounds[1] = -0.5;
  clipped.bounds[2] = -0.5;
  clipped.bounds[3] = 0.5;
  clipped.bounds[4] = 0.5;
  clipped.bounds[5] = 0.5;
  ve_status status = VE_OK;
  check(ve_solid_from_field(&clipped, test.clipped.data(), &status) == nullptr &&
            status == VE_ERR_FIELD_CLIPPED && std::abs(test.clipped[0]) == 0.5,
        "boundary sampling refuses and names the clipped point");

  const std::array<ve_field_instruction, 1> distant = {{{VE_FIELD_SPHERE, 0, 0, 0, 0, 0, 0.1}}};
  auto empty = input_for(distant.data(), distant.size(), 0.08, test.context.get());
  empty.bounds[0] = 1.0;
  empty.bounds[1] = 1.0;
  empty.bounds[2] = 1.0;
  empty.bounds[3] = 2.0;
  empty.bounds[4] = 2.0;
  empty.bounds[5] = 2.0;
  check(ve_solid_from_field(&empty, test.clipped.data(), &status) == nullptr &&
            status == VE_ERR_FIELD_EMPTY,
        "surface outside the bounds is typed empty");

  auto over_budget = input_for(sphere.data(), sphere.size(), 0.04, test.context.get());
  over_budget.max_cells = 1;
  check(ve_solid_from_field(&over_budget, test.clipped.data(), &status) == nullptr &&
            status == VE_ERR_FIELD_BUDGET,
        "caller cannot exceed the bounded cell budget");

  test.context.reset(ve_context_new());
  ve_context_cancel(test.context.get());
  auto cancelled = input_for(sphere.data(), sphere.size(), 0.04, test.context.get());
  check(ve_solid_from_field(&cancelled, test.clipped.data(), &status) == nullptr &&
            status == VE_ERR_CANCELLED,
        "pre-cancelled field returns cancellation");
}

void in_sampling_cancellation_test() {
  std::array<ve_field_instruction, 64> tape{};
  tape.at(0) = {VE_FIELD_SPHERE, 0, 0, 0, 0, 0, 0.5};
  for (size_t index = 1; index < tape.size(); ++index) {
    tape.at(index) = {VE_FIELD_OFFSET, static_cast<uint32_t>(index - 1), 0, 0, 0, 0, 0};
  }
  TestContext test;
  auto input = input_for(tape.data(), tape.size(), 0.021, test.context.get());
  ve_status status = VE_OK;
  std::atomic<int64_t> cancel_requested_ns{0};
  std::thread canceller([&test, &cancel_requested_ns] {
    std::this_thread::sleep_for(std::chrono::milliseconds(250));
    const auto requested = std::chrono::steady_clock::now().time_since_epoch();
    cancel_requested_ns.store(
        std::chrono::duration_cast<std::chrono::nanoseconds>(requested).count());
    ve_context_cancel(test.context.get());
  });
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> solid(
      ve_solid_from_field(&input, test.clipped.data(), &status), ve_solid_free);
  canceller.join();
  const auto returned = std::chrono::steady_clock::now().time_since_epoch();
  const auto returned_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(returned).count();
  const auto requested_ns = cancel_requested_ns.load();
  const auto latency = std::chrono::duration_cast<std::chrono::microseconds>(
      std::chrono::nanoseconds(returned_ns - requested_ns));
  check(solid == nullptr && status == VE_ERR_CANCELLED,
        "cancellation during field sampling discards the result");
  check(requested_ns > 0 && latency < std::chrono::milliseconds(500),
        "field cancellation returns within 500 ms of the cancel request");
  std::fprintf(stdout, "field cancellation latency: %lld us\n",
               static_cast<long long>(latency.count()));
}

}  // namespace

int main() {
  analytic_tests();
  composition_tests();
  refusal_tests();
  in_sampling_cancellation_test();
  if (failures() != 0) return 1;
  std::puts("field tests passed");
  return 0;
}
