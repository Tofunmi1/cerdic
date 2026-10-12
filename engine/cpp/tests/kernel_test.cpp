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
  for (ve_solid* s : {cube, sphere, diff, rebuilt}) {
    ve_solid_free(s);
  }
  ve_solid_free(nullptr);

  if (failures() == 0) {
    static_cast<void>(std::puts("kernel tests passed"));
  }
  return failures() == 0 ? 0 : 1;
}
