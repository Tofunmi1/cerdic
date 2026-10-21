//! Deterministic vector math and bounded polynomial trigonometry.

pub(super) type V3 = [f32; 3];

pub(super) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scale(a: V3, factor: f32) -> V3 {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

pub(super) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

pub(super) fn unit(a: V3) -> V3 {
    let magnitude = length(a);
    if magnitude > 1.0e-8 {
        scale(a, 1.0 / magnitude)
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Polynomial sine and cosine after quadrant reduction; maximum error is below 1e-4.
pub(super) fn sin_cos(angle: f32) -> (f32, f32) {
    let tau = 6.283_185_5_f32;
    let mut x = angle - (angle / tau).floor() * tau;
    if x > core::f32::consts::PI {
        x -= tau;
    }
    let mut cosine_sign = 1.0;
    if x > core::f32::consts::FRAC_PI_2 {
        x = core::f32::consts::PI - x;
        cosine_sign = -1.0;
    } else if x < -core::f32::consts::FRAC_PI_2 {
        x = -core::f32::consts::PI - x;
        cosine_sign = -1.0;
    }
    let x2 = x * x;
    let sine = x
        * (1.0
            + x2 * (-0.166_666_67
                + x2 * (0.008_333_331
                    + x2 * (-0.000_198_409 + x2 * (0.000_002_752_6 - x2 * 0.000_000_025_05)))));
    let cosine = cosine_sign
        * (1.0
            + x2 * (-0.5
                + x2 * (0.041_666_638
                    + x2 * (-0.001_388_837_8 + x2 * (0.000_024_760_9 - x2 * 0.000_000_260_5)))));
    (sine, cosine)
}

/// Rotates a unit vector around a unit axis with Rodrigues' formula.
pub(super) fn rotate(vector: V3, axis: V3, angle: f32) -> V3 {
    let axis = unit(axis);
    let (sine, cosine) = sin_cos(angle);
    let cross = [
        axis[1] * vector[2] - axis[2] * vector[1],
        axis[2] * vector[0] - axis[0] * vector[2],
        axis[0] * vector[1] - axis[1] * vector[0],
    ];
    unit(add(
        add(scale(vector, cosine), scale(cross, sine)),
        scale(axis, dot(axis, vector) * (1.0 - cosine)),
    ))
}

pub(super) fn perpendicular(vector: V3) -> V3 {
    unit(if vector[1].abs() < 0.9 {
        [vector[2], 0.0, -vector[0]]
    } else {
        [1.0, 0.0, 0.0]
    })
}

/// Converts a bounded iteration counter to a floating-point fraction.
#[expect(
    clippy::cast_precision_loss,
    reason = "plant iteration counters are capped at 200,000"
)]
pub(super) fn count_f32(value: u32) -> f32 {
    value as f32
}

/// Converts a bounded node count to floating point for interpolation.
#[expect(
    clippy::cast_precision_loss,
    reason = "plant node counts are capped at 200,000"
)]
pub(super) fn length_f32(value: usize) -> f32 {
    value as f32
}

/// Converts a rounded, non-negative, bounded interpolation index to `usize`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value has already been rounded and bounded by the chain length"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "the value has already been clamped non-negative"
)]
pub(super) fn bounded_index(value: f32, upper: usize) -> usize {
    value.round().clamp(0.0, length_f32(upper)) as usize
}

/// Converts a positive segment estimate, capped above the skeleton limit.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the positive value is clamped to the skeleton limit before conversion"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "the positive value is clamped non-negative before conversion"
)]
pub(super) fn bounded_count(value: f32) -> u32 {
    value.ceil().clamp(1.0, 200_001.0) as u32
}

#[cfg(test)]
mod tests {
    use super::sin_cos;

    #[test]
    fn polynomial_sine_and_cosine_stay_within_tolerance() {
        let samples = [
            (0.0, 0.0, 1.0),
            (core::f32::consts::FRAC_PI_6, 0.5, 0.866_025_4),
            (core::f32::consts::FRAC_PI_4, 0.707_106_77, 0.707_106_77),
            (1.570_796_4, 1.0, 0.0),
            (core::f32::consts::PI, 0.0, -1.0),
            (3.0 * core::f32::consts::FRAC_PI_2, -1.0, 0.0),
        ];
        for (angle, expected_sine, expected_cosine) in samples {
            let (sine, cosine) = sin_cos(angle);
            assert!((sine - expected_sine).abs() < 1.0e-4);
            assert!((cosine - expected_cosine).abs() < 1.0e-4);
        }
    }
}
