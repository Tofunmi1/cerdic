// Native tests for the kernel C ABI, run under ASan and UBSan (cargo xtask cpp).
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <string>
#include <vector>

#include "ve_kernel.h"

namespace {

int& failures() {
  static int count = 0;
  return count;
}

void check(bool ok, const char* what) {
  if (!ok) {
    const std::string line = std::string("FAIL: ") + what + "\n";
    static_cast<void>(std::fputs(line.c_str(), stderr));
    ++failures();
  }
}

bool near(double a, double b, double tol) { return std::fabs(a - b) <= tol; }

void profile_test() {
  ve_status status = VE_OK;
  const std::array<double, 8> points = {0, 0, 1, 0, 1, 1, 0, 1};
  const std::array<size_t, 2> offsets = {0, 4};
  using Profile = std::unique_ptr<ve_profile, decltype(&ve_profile_free)>;
  using Solid = std::unique_ptr<ve_solid, decltype(&ve_solid_free)>;
  const Profile profile(ve_profile_new(points.data(), 4, offsets.data(), 1, &status),
                        ve_profile_free);
  check(profile != nullptr && status == VE_OK, "profile polygon builds");
  check(near(ve_profile_area(profile.get()), 1.0, 1e-9), "profile area");
  const Profile smaller(ve_profile_offset(profile.get(), -0.1, VE_JOIN_MITER, &status),
                        ve_profile_free);
  check(smaller != nullptr && near(ve_profile_area(smaller.get()), 0.64, 1e-6), "offset area");
  // Growing a unit square by r with round corners adds 4r + pi r^2 (the arcs are inscribed, so a
  // 64-segment circle falls just short of the full circle).
  const Profile rounded(ve_profile_offset(profile.get(), 0.1, VE_JOIN_ROUND, &status),
                        ve_profile_free);
  check(rounded != nullptr && near(ve_profile_area(rounded.get()), 1.0 + 0.4 + M_PI * 0.01, 2e-3),
        "round offset area");
  check(ve_profile_offset(profile.get(), 0.1, 7, &status) == nullptr, "unknown join rejected");
  const Profile hole(ve_profile_difference(profile.get(), smaller.get(), &status), ve_profile_free);
  check(hole != nullptr && near(ve_profile_area(hole.get()), 0.36, 1e-6), "profile difference");
  const Profile empty(ve_profile_offset(profile.get(), -1.0, VE_JOIN_MITER, &status),
                      ve_profile_free);
  check(empty != nullptr && ve_profile_area(empty.get()) == 0.0, "empty erosion valid");
  const Solid extruded(ve_profile_extrude(profile.get(), 2.0, 128, M_PI / 2.0, 1.0, 1.0, &status),
                       ve_solid_free);
  check(extruded != nullptr && near(ve_solid_volume(extruded.get()), 2.0, 0.002),
        "twisted extrusion volume");
  const Solid revolved(ve_profile_revolve(profile.get(), 64, 2.0 * M_PI, &status), ve_solid_free);
  check(revolved != nullptr && status == VE_OK, "revolve builds");
  size_t count = 0;
  size_t loops = 0;
  check(ve_profile_counts(profile.get(), &count, &loops) == VE_OK && count == 4 && loops == 1,
        "profile counts");
  std::array<double, 8> copied = {};
  std::array<size_t, 2> copied_offsets = {};
  check(ve_profile_copy(profile.get(), copied.data(), copied.size(), copied_offsets.data(), 2) ==
            VE_OK,
        "profile copy");
  check(ve_profile_copy(profile.get(), copied.data(), 1, copied_offsets.data(), 2) ==
            VE_ERR_BUFFER_TOO_SMALL,
        "short profile buffer");
  const std::array<size_t, 2> bad_offsets = {0, 5};
  check(ve_profile_new(points.data(), 4, bad_offsets.data(), 1, &status) == nullptr,
        "invalid offsets");
  check(ve_profile_new(nullptr, 4, offsets.data(), 1, &status) == nullptr, "null profile data");
  check(ve_profile_new(points.data(), 4, nullptr, 1, &status) == nullptr, "null offsets");
  const std::array<double, 8> crossed = {0, 0, 1, 1, 0, 1, 1, 0};
  check(ve_profile_new(crossed.data(), 4, offsets.data(), 1, &status) == nullptr,
        "self intersection rejected");
  check(ve_profile_extrude(profile.get(), 0, 1, 0, 1, 1, &status) == nullptr,
        "zero height rejected");
  check(ve_profile_extrude(profile.get(), 1, 257, 0, 1, 1, &status) == nullptr,
        "divisions bounded");
  check(ve_profile_revolve(profile.get(), 513, M_PI, &status) == nullptr,
        "revolve segments bounded");
  check(ve_profile_offset(profile.get(), NAN, VE_JOIN_MITER, &status) == nullptr,
        "nonfinite offset rejected");
}

void cancellation_test(const ve_solid* cube, const ve_solid* sphere) {
  ve_status status = VE_OK;
  const std::unique_ptr<ve_context, decltype(&ve_context_free)> context(ve_context_new(),
                                                                        ve_context_free);
  check(context != nullptr && ve_context_cancelled(context.get()) == 0,
        "fresh cancellation context");
  ve_context_cancel(context.get());
  check(ve_context_cancelled(context.get()) == 1, "cancellation signal observed");
  check(ve_solid_boolean_context(cube, sphere, 0, context.get(), &status) == nullptr &&
            status == VE_ERR_CANCELLED,
        "cancelled boolean returns no owned result");
  const std::array<const ve_solid*, 2> solids = {cube, sphere};
  check(ve_solid_batch_union_context(solids.data(), solids.size(), context.get(), &status) ==
                nullptr &&
            status == VE_ERR_CANCELLED,
        "cancelled batch tree returns no result");
  const std::unique_ptr<ve_context, decltype(&ve_context_free)> fresh(ve_context_new(),
                                                                      ve_context_free);
  const std::unique_ptr<ve_solid, decltype(&ve_solid_free)> result(
      ve_solid_batch_union_context(solids.data(), solids.size(), fresh.get(), &status),
      ve_solid_free);
  check(result != nullptr && status == VE_OK, "fresh context can retry batch union");
  check(ve_solid_batch_union_context(solids.data(), 1, fresh.get(), &status) == nullptr &&
            status == VE_ERR_INVALID_ARG,
        "invalid batch count rejected");
}

}  // namespace

int main() {
  ve_status st = VE_OK;

  ve_solid* cube = ve_solid_cube(1, 1, 1, 1, &st);
  check(cube != nullptr && st == VE_OK, "cube builds");
  check(near(ve_solid_volume(cube), 1.0, 1e-9), "cube volume is 1");
  check(ve_solid_num_tris(cube) == 12, "cube has 12 triangles");
  check(ve_solid_num_components(cube) == 1, "cube is one component");

  ve_solid* sphere = ve_solid_sphere(0.65, 48, &st);
  check(sphere != nullptr, "sphere builds");
  const double exact = 4.0 / 3.0 * M_PI * 0.65 * 0.65 * 0.65;
  check(near(ve_solid_volume(sphere), exact, exact * 0.02), "sphere volume within 2%");

  ve_solid* diff = ve_solid_difference(cube, sphere, &st);
  check(diff != nullptr && st == VE_OK, "difference builds");
  check(ve_solid_volume(diff) < 1.0 && ve_solid_volume(diff) > 0.0, "difference shrinks volume");

  std::vector<float> pos(ve_solid_num_verts(diff) * 3);
  std::vector<uint32_t> idx(ve_solid_num_tris(diff) * 3);
  check(ve_solid_copy_mesh(diff, pos.data(), pos.size(), idx.data(), idx.size()) == VE_OK,
        "copy mesh");
  check(ve_solid_copy_mesh(diff, pos.data(), pos.size() - 1, idx.data(), idx.size()) ==
            VE_ERR_BUFFER_TOO_SMALL,
        "short buffer rejected");

  ve_solid* rebuilt = ve_solid_from_mesh(pos.data(), pos.size() / 3, idx.data(), idx.size(), &st);
  check(rebuilt != nullptr, "mesh round trip builds");
  check(near(ve_solid_volume(rebuilt), ve_solid_volume(diff), 1e-4), "round trip keeps volume");

  std::vector<uint32_t> simplified(idx.size());
  size_t out_count = 0;
  check(ve_simplify(pos.data(), pos.size() / 3, idx.data(), idx.size(), 0.5, 0.05,
                    simplified.data(), simplified.size(), &out_count) == VE_OK,
        "simplify runs");
  check(out_count > 0 && out_count <= idx.size(), "simplify reduces or keeps indices");

  // Invalid input is rejected, never a crash.
  check(ve_solid_cube(-1, 1, 1, 0, &st) == nullptr && st == VE_ERR_INVALID_ARG,
        "negative cube rejected");
  check(ve_solid_cube(NAN, 1, 1, 0, &st) == nullptr, "NaN cube rejected");
  check(ve_solid_sphere(1.0, 2, &st) == nullptr, "too few segments rejected");
  check(ve_solid_sphere(1.0, VE_MAX_SEGMENTS + 1, &st) == nullptr, "too many segments rejected");
  check(ve_solid_scale(cube, 0, 1, 1, &st) == nullptr, "zero scale rejected");
  check(ve_solid_union(nullptr, cube, &st) == nullptr, "null operand rejected");
  const std::array<uint32_t, 3> bad_idx = {0, 1, 7};
  const std::array<float, 9> tri = {0, 0, 0, 1, 0, 0, 0, 1, 0};
  check(ve_solid_from_mesh(tri.data(), 3, bad_idx.data(), 3, &st) == nullptr,
        "out-of-range index rejected");
  const std::array<uint32_t, 3> open_idx = {0, 1, 2};
  check(ve_solid_from_mesh(tri.data(), 3, open_idx.data(), 3, &st) == nullptr,
        "open mesh rejected");

  cancellation_test(cube, sphere);
  profile_test();
  for (ve_solid* s : {cube, sphere, diff, rebuilt}) {
    ve_solid_free(s);
  }
  ve_solid_free(nullptr);

  if (failures() == 0) {
    static_cast<void>(std::puts("kernel tests passed"));
  }
  return failures() == 0 ? 0 : 1;
}
