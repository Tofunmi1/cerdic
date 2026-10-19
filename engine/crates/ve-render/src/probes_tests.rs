use super::{MAX_PROBES_PER_AXIS, ProbeVolume, f32_to_f16, texture_bytes};

/// A half float's value, for checking the conversion back against the original.
fn half_value(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x3ff);
    match exponent {
        0 => sign * mantissa * 2.0_f32.powi(-24),
        0x1f if mantissa == 0.0 => sign * f32::INFINITY,
        0x1f => f32::NAN,
        _ => sign * (1.0 + mantissa / 1024.0) * 2.0_f32.powi(exponent - 15),
    }
}

#[test]
fn well_known_values_convert_exactly() {
    for (value, bits) in [
        (0.0_f32, 0x0000_u16),
        (-0.0, 0x8000),
        (1.0, 0x3c00),
        (-1.0, 0xbc00),
        (0.5, 0x3800),
        (2.0, 0x4000),
        (-2.0, 0xc000),
        (65504.0, 0x7bff),
        (f32::INFINITY, 0x7c00),
        (f32::NEG_INFINITY, 0xfc00),
        // The smallest normal half, and the smallest denormal one.
        (2.0_f32.powi(-14), 0x0400),
        (2.0_f32.powi(-24), 0x0001),
    ] {
        assert_eq!(f32_to_f16(value), bits, "{value}");
    }
}

#[test]
fn overflow_underflow_and_nan_behave() {
    assert_eq!(
        f32_to_f16(65520.0),
        0x7c00,
        "past the largest half rounds to infinity"
    );
    assert_eq!(f32_to_f16(1.0e9), 0x7c00);
    assert_eq!(
        f32_to_f16(1.0e-10),
        0x0000,
        "too small for a denormal is zero"
    );
    assert_eq!(f32_to_f16(-1.0e-10), 0x8000);
    let nan = f32_to_f16(f32::NAN);
    assert_eq!(nan & 0x7c00, 0x7c00);
    assert_ne!(nan & 0x03ff, 0, "a NaN stays a NaN");
}

#[test]
fn rounding_goes_to_the_nearest_even_half() {
    // Halfway between 1.0 (0x3c00) and the next half (0x3c01): ties go to the even one, 0x3c00; just above goes up.
    let step = 2.0_f32.powi(-10);
    assert_eq!(f32_to_f16(1.0 + step / 2.0), 0x3c00);
    assert_eq!(f32_to_f16(1.0 + step / 2.0 + 1.0e-6), 0x3c01);
    // Halfway between 0x3c01 and 0x3c02 goes to the even 0x3c02.
    assert_eq!(f32_to_f16(1.0 + 1.5 * step), 0x3c02);
    // The largest mantissa carries into the exponent.
    assert_eq!(f32_to_f16(2.0 - 1.0e-4), 0x4000);
}

#[test]
fn every_finite_value_in_range_converts_within_half_precision() {
    // Over a sweep from deep denormals to the top of the range the result is within half a unit in the last place.
    let mut value = 3.0e-8_f32;
    while value < 60000.0 {
        for sign in [1.0, -1.0] {
            let original = sign * value;
            let back = half_value(f32_to_f16(original));
            let ulp = if value < 2.0_f32.powi(-14) {
                2.0_f32.powi(-24)
            } else {
                2.0_f32.powf(value.log2().floor() - 10.0)
            };
            assert!(
                (back - original).abs() <= ulp * 0.5 + f32::EPSILON * value,
                "{original} became {back} (half a step is {})",
                ulp * 0.5
            );
        }
        value *= 1.013;
    }
}

fn volume(dims: [u32; 3]) -> ProbeVolume {
    let count = dims.iter().map(|&d| d as usize).product();
    ProbeVolume {
        origin: [0.0; 3],
        spacing: 1.0,
        dims,
        coefficients: vec![[0.0; 12]; count],
    }
}

#[test]
fn a_volume_is_valid_only_when_its_numbers_agree() {
    assert!(volume([4, 3, 2]).is_valid());
    assert!(!ProbeVolume::default().is_valid(), "no probes");
    let mut wrong_count = volume([2, 2, 2]);
    wrong_count.coefficients.pop();
    assert!(!wrong_count.is_valid());
    let mut no_spacing = volume([2, 2, 2]);
    no_spacing.spacing = 0.0;
    assert!(!no_spacing.is_valid());
    let mut nan = volume([2, 2, 2]);
    nan.coefficients[3][5] = f32::NAN;
    assert!(!nan.is_valid());
    assert!(
        !volume([MAX_PROBES_PER_AXIS + 1, 1, 1]).is_valid(),
        "too long an axis"
    );
    assert!(volume([MAX_PROBES_PER_AXIS, MAX_PROBES_PER_AXIS, 16]).is_valid());
    assert!(
        !volume([MAX_PROBES_PER_AXIS, MAX_PROBES_PER_AXIS, 17]).is_valid(),
        "too many probes in all"
    );
}

#[test]
fn the_four_textures_hold_one_coefficient_each_in_probe_order() {
    let mut v = volume([2, 1, 1]);
    // Probe 0: dc (1, 2, 3), x (4, 5, 6), y (7, 8, 9), z (10, 11, 12). Probe 1 is the same plus 100.
    v.coefficients[0] = std::array::from_fn(|i| i as f32 + 1.0);
    v.coefficients[1] = std::array::from_fn(|i| i as f32 + 101.0);
    let bytes = texture_bytes(&v);
    let texel = |k: usize, probe: usize, channel: usize| {
        let at = probe * 8 + channel * 2;
        half_value(u16::from_le_bytes([bytes[k][at], bytes[k][at + 1]]))
    };
    for k in 0..4 {
        assert_eq!(bytes[k].len(), 16, "two probes of four half floats");
        for channel in 0..3 {
            assert_eq!(texel(k, 0, channel), (3 * k + channel + 1) as f32);
            assert_eq!(texel(k, 1, channel), (3 * k + channel + 101) as f32);
        }
        assert_eq!(texel(k, 0, 3), 1.0, "alpha is one");
    }
}
