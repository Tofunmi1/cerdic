//! Platform independent scalar transcendental approximations.

pub(crate) fn sin(value: f32) -> f32 {
    let x = reduce(value);
    let x2 = x * x;
    x * (1.0
        + x2 * (-0.166_666_67
            + x2 * (0.008_333_331 + x2 * (-0.000_198_409 + x2 * 0.000_002_752_6))))
}

pub(crate) fn cos(value: f32) -> f32 {
    sin(value + core::f32::consts::FRAC_PI_2)
}

fn reduce(value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    let mut x = value % core::f32::consts::TAU;
    if x > core::f32::consts::PI {
        x -= core::f32::consts::TAU;
    }
    if x < -core::f32::consts::PI {
        x += core::f32::consts::TAU;
    }
    if x > core::f32::consts::FRAC_PI_2 {
        x = core::f32::consts::PI - x;
    }
    if x < -core::f32::consts::FRAC_PI_2 {
        x = -core::f32::consts::PI - x;
    }
    x
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the input is clamped so the base-two exponent stays between -150 and 127"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "the clamped exponent magnitude is at most 150"
)]
pub(crate) fn exp(value: f32) -> f32 {
    if value.is_nan() {
        return 0.0;
    }
    if value >= 88.0 {
        return f32::MAX;
    }
    if value <= -104.0 {
        return 0.0;
    }
    let scaled = value * core::f32::consts::LOG2_E;
    let exponent = scaled.floor() as i32;
    let fraction = value - exponent as f32 * core::f32::consts::LN_2;
    let f2 = fraction * fraction;
    let mut result = 1.0
        + fraction
        + f2 * (0.5
            + fraction
                * (0.166_666_67
                    + fraction
                        * (0.041_666_668 + fraction * (0.008_333_334 + fraction * 0.001_388_889))));
    if exponent > 0 {
        for _ in 0..exponent {
            result *= 2.0;
        }
    }
    if exponent < 0 {
        for _ in exponent..0 {
            result *= 0.5;
        }
    }
    if result.is_finite() { result } else { f32::MAX }
}

pub(crate) fn log(value: f32) -> f32 {
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    let mut x = value;
    let mut exponent = 0.0;
    while x >= core::f32::consts::SQRT_2 {
        x *= 0.5;
        exponent += 1.0;
    }
    while x < 0.707_106_77 {
        x *= 2.0;
        exponent -= 1.0;
    }
    let z = (x - 1.0) / (x + 1.0);
    let z2 = z * z;
    let mut term = z;
    let mut sum = term;
    for denominator in [3.0, 5.0, 7.0, 9.0, 11.0, 13.0, 15.0, 17.0] {
        term *= z2;
        sum += term / denominator;
    }
    2.0 * sum + exponent * core::f32::consts::LN_2
}

pub(crate) fn atan2(y: f32, x: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let ax = x.abs();
    let ay = y.abs();
    let (ratio, invert) = if ax >= ay {
        (ay / ax, false)
    } else {
        (ax / ay, true)
    };
    let r2 = ratio * ratio;
    let mut angle = ratio
        * (0.999_866
            + r2 * (-0.330_299_5 + r2 * (0.180_141 + r2 * (-0.085_133 + r2 * 0.020_835_1))));
    if invert {
        angle = core::f32::consts::FRAC_PI_2 - angle;
    }
    if x < 0.0 {
        angle = core::f32::consts::PI - angle;
    }
    if y < 0.0 {
        angle = -angle;
    }
    angle
}

pub(crate) fn pow(base: f32, exponent: f32) -> f32 {
    if base < 0.0 || !base.is_finite() || !exponent.is_finite() {
        return 0.0;
    }
    if base == 0.0 {
        return 0.0;
    }
    let result = exp(exponent * log(base));
    if result.is_finite() { result } else { f32::MAX }
}
