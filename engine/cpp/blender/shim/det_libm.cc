// Deterministic replacements for the libm functions Blender's UV and bevel code calls.
//
// Only + - * / and sqrt are used (all exactly rounded), so every platform produces the same bits.
// The Blender sources are compiled with sin, cos, ... renamed to these functions (see
// CMakeLists.txt), so the rest of the engine keeps the platform libm. Accurate to about 1e-15 for
// |x| < 1e5; this is a determinism tool, not a replacement for a full libm.
#include <cmath>

namespace {

constexpr double kPio2Hi = 1.57079632673412561417e+00;
constexpr double kPio2Mid = 6.07710050630396597660e-11;
constexpr double kPio2Lo = 2.02226624871116645580e-21;
constexpr double kTwoOverPi = 6.36619772367581382433e-01;
constexpr double kPio2 = 1.57079632679489661923e+00;
constexpr double kPi = 3.14159265358979323846e+00;

// sin and cos of r for |r| <= pi/4 (fdlibm minimax kernels).
double sin_kernel(double r) {
  const double z = r * r;
  const double p =
      -1.66666666666666324348e-01 +
      (z * (8.33333333332248946124e-03 +
            (z * (-1.98412698298579493134e-04 +
                  (z * (2.75573137070700676789e-06 + (z * (-2.50507602534068634195e-08 +
                                                           (z * 1.58969099521155010221e-10)))))))));
  return r + (r * z * p);
}

double cos_kernel(double r) {
  const double z = r * r;
  const double p =
      4.16666666666666019037e-02 +
      (z *
       (-1.38888888888741095749e-03 +
        (z * (2.48015872894767294178e-05 +
              (z * (-2.75573143513906633035e-07 +
                    (z * (2.08757232129817482790e-09 + (z * -1.13596475577881948265e-11)))))))));
  return (1.0 - (0.5 * z)) + (z * z * p);
}

// Reduces x to r in [-pi/4, pi/4] with x = n * pi/2 + r; returns n modulo 4.
int reduce(double x, double* r) {
  const double n = std::floor((x * kTwoOverPi) + 0.5);
  *r = ((x - (n * kPio2Hi)) - (n * kPio2Mid)) - (n * kPio2Lo);
  const double m = n - (4.0 * std::floor(n * 0.25));
  return static_cast<int>(m);
}

double det_sin(double x) {
  double r = 0.0;
  switch (reduce(x, &r)) {
    case 0:
      return sin_kernel(r);
    case 1:
      return cos_kernel(r);
    case 2:
      return -sin_kernel(r);
    default:
      return -cos_kernel(r);
  }
}

double det_cos(double x) {
  double r = 0.0;
  switch (reduce(x, &r)) {
    case 0:
      return cos_kernel(r);
    case 1:
      return -sin_kernel(r);
    case 2:
      return -cos_kernel(r);
    default:
      return sin_kernel(r);
  }
}

double det_tan(double x) {
  double r = 0.0;
  const int n = reduce(x, &r);
  const double s = sin_kernel(r);
  const double c = cos_kernel(r);
  return n % 2 == 0 ? s / c : -c / s;
}

// atan for any x: halve the angle twice (atan x = 2 atan(x / (1 + sqrt(1 + x^2)))), then a Taylor
// series.
double det_atan(double x) {
  const bool negative = x < 0.0;
  double a = negative ? -x : x;
  const bool invert = a > 1.0;
  if (invert) {
    a = 1.0 / a;
  }
  a = a / (1.0 + std::sqrt(1.0 + (a * a)));
  a = a / (1.0 + std::sqrt(1.0 + (a * a)));
  const double z = a * a;
  double sum = 0.0;
  for (int k = 14; k >= 0; --k) {
    sum = ((k % 2 == 0 ? 1.0 : -1.0) / static_cast<double>((2 * k) + 1)) + (z * sum);
  }
  double result = 4.0 * a * sum;
  if (invert) {
    result = kPio2 - result;
  }
  return negative ? -result : result;
}

double det_atan2(double y, double x) {
  if (x == 0.0 && y == 0.0) {
    return 0.0;
  }
  if (x == 0.0) {
    return y > 0.0 ? kPio2 : -kPio2;
  }
  const double a = det_atan(y / x);
  if (x > 0.0) {
    return a;
  }
  return y >= 0.0 ? a + kPi : a - kPi;
}

double det_acos(double x) {
  if (x >= 1.0) {
    return 0.0;
  }
  if (x <= -1.0) {
    return kPi;
  }
  return det_atan2(std::sqrt((1.0 - x) * (1.0 + x)), x);
}

double det_asin(double x) {
  if (x >= 1.0) {
    return kPio2;
  }
  if (x <= -1.0) {
    return -kPio2;
  }
  return det_atan2(x, std::sqrt((1.0 - x) * (1.0 + x)));
}

}  // namespace

extern "C" {
double ve_det_sin(double x) { return det_sin(x); }
double ve_det_cos(double x) { return det_cos(x); }
double ve_det_tan(double x) { return det_tan(x); }
double ve_det_atan(double x) { return det_atan(x); }
double ve_det_atan2(double y, double x) { return det_atan2(y, x); }
double ve_det_acos(double x) { return det_acos(x); }
double ve_det_asin(double x) { return det_asin(x); }
float ve_det_sinf(float x) { return static_cast<float>(det_sin(double(x))); }
float ve_det_cosf(float x) { return static_cast<float>(det_cos(double(x))); }
float ve_det_tanf(float x) { return static_cast<float>(det_tan(double(x))); }
float ve_det_atanf(float x) { return static_cast<float>(det_atan(double(x))); }
float ve_det_atan2f(float y, float x) {
  return static_cast<float>(det_atan2(double(y), double(x)));
}
float ve_det_acosf(float x) { return static_cast<float>(det_acos(double(x))); }
float ve_det_asinf(float x) { return static_cast<float>(det_asin(double(x))); }
}
