#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <vector>

#include "ve_kernel.h"

namespace {
void check(bool condition, const char* message, int* failures) {
  if (!condition) {
    static_cast<void>(std::fputs("FAIL: ", stderr));
    static_cast<void>(std::fputs(message, stderr));
    static_cast<void>(std::fputc('\n', stderr));
    ++(*failures);
  }
}
}  // namespace

int main() {
  int failures = 0;
  const std::array<float, 24> positions = {0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0,
                                           0, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1};
  const std::array<uint32_t, 6> sizes = {4, 4, 4, 4, 4, 4};
  const std::array<uint32_t, 24> corners = {0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4,
                                            3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5};
  const std::unique_ptr<ve_context, decltype(&ve_context_free)> context(ve_context_new(),
                                                                        ve_context_free);
  ve_subdivide_input input{};
  input.struct_size = sizeof(input);
  input.positions = positions.data();
  input.vertex_count = 8;
  input.face_sizes = sizes.data();
  input.face_count = sizes.size();
  input.corners = corners.data();
  input.corner_count = corners.size();
  std::array<float, 8> point_values = {0, 1, 1, 0, 0, 1, 1, 0};
  std::vector<float> fvar_values(48);
  std::vector<uint32_t> fvar_indices(24);
  for (size_t corner = 0; corner < fvar_indices.size(); ++corner) {
    fvar_indices.at(corner) = static_cast<uint32_t>(corner);
    fvar_values.at(corner * 2) = static_cast<float>(corner);
    fvar_values.at((corner * 2) + 1) = static_cast<float>(corner % 3);
  }
  input.point_values = point_values.data();
  input.point_channels = 1;
  input.fvar_values = fvar_values.data();
  input.fvar_value_count = fvar_indices.size();
  input.fvar_width = 2;
  input.fvar_corner_values = fvar_indices.data();
  input.levels = 1;
  input.context = context.get();
  ve_status status = VE_OK;
  const std::unique_ptr<ve_subdivision, decltype(&ve_subdivision_free)> result(
      ve_subdivide_catmark(&input, &status), ve_subdivision_free);
  check(result != nullptr && status == VE_OK, "closed cube refines", &failures);
  check(ve_subdivision_num_vertices(result.get()) == 26, "one level creates 26 vertices",
        &failures);
  check(ve_subdivision_num_faces(result.get()) == 24, "one level creates 24 faces", &failures);
  check(ve_subdivision_num_corners(result.get()) == 96, "one level creates 96 corners", &failures);
  const auto fvar_count = ve_subdivision_num_fvar_values(result.get());
  check(ve_subdivision_num_point_channels(result.get()) == 1, "one point channel is returned",
        &failures);
  check(fvar_count > fvar_indices.size(), "face-varying values refine", &failures);
  std::vector<float> out_positions(26 * 3);
  std::vector<uint32_t> out_sizes(24);
  std::vector<uint32_t> out_corners(96);
  std::vector<uint32_t> kind(26);
  std::vector<uint32_t> origin(26);
  std::vector<uint32_t> parents(24);
  check(ve_subdivision_copy(result.get(), out_positions.data(), out_positions.size(),
                            out_sizes.data(), out_sizes.size(), out_corners.data(),
                            out_corners.size(), kind.data(), origin.data(), kind.size(),
                            parents.data(), parents.size()) == VE_OK,
        "copies refined output", &failures);
  std::vector<float> out_point_values(26);
  std::vector<float> out_fvar_values(fvar_count * 2);
  std::vector<uint32_t> out_fvar_indices(96);
  check(ve_subdivision_copy_channels(result.get(), out_point_values.data(), out_point_values.size(),
                                     out_fvar_values.data(), out_fvar_values.size(),
                                     out_fvar_indices.data(), out_fvar_indices.size()) == VE_OK,
        "copies point and face-varying channels", &failures);
  for (size_t vertex = 0; vertex < out_point_values.size(); ++vertex) {
    check(out_point_values.at(vertex) == out_positions.at(vertex * 3),
          "point channel follows the refined x coordinate", &failures);
  }
  check(std::all_of(out_fvar_indices.begin(), out_fvar_indices.end(),
                    [fvar_count](uint32_t index) { return index < fvar_count; }),
        "every refined corner refers to a UV value", &failures);
  check(
      ve_subdivision_copy(result.get(), out_positions.data(), 1, out_sizes.data(), out_sizes.size(),
                          out_corners.data(), out_corners.size(), kind.data(), origin.data(),
                          kind.size(), parents.data(), parents.size()) == VE_ERR_BUFFER_TOO_SMALL,
      "short output buffers rejected", &failures);
  ve_context_cancel(context.get());
  check(ve_subdivide_catmark(&input, &status) == nullptr && status == VE_ERR_CANCELLED,
        "pre-cancelled subdivision returns cancellation", &failures);
  static_cast<void>(
      std::puts(failures == 0 ? "subdivision tests passed" : "subdivision tests failed"));
  return failures == 0 ? 0 : 1;
}
