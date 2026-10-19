//! Focused tests for viewer grading.

use super::*;

#[test]
fn neutral_grading_is_the_identity() {
    let grading = Grading::neutral();
    for color in [[0.0, 0.0, 0.0], [0.5, 0.25, 0.75], [1.0, 1.0, 1.0]] {
        let out = grading.grade(color);
        for (before, after) in color.iter().zip(out.iter()) {
            assert!((before - after).abs() < 1e-6, "{color:?} -> {out:?}");
        }
    }
}

#[test]
fn mono_removes_colour_and_keeps_luminance_order() {
    let out = Grading::for_preset(GradePreset::Mono).grade([0.8, 0.2, 0.1]);
    assert!(
        (out[0] - out[1]).abs() < 1e-6 && (out[1] - out[2]).abs() < 1e-6,
        "grey output: {out:?}"
    );
    assert!(
        out[0] > 0.0 && out[0] < 1.0,
        "grey stays display-referred: {out:?}"
    );
}

#[test]
fn tint_scales_each_channel_independently() {
    let grading = Grading {
        tint: [1.2, 1.0, 0.8],
        ..Grading::neutral()
    };
    let out = grading.grade([0.4, 0.4, 0.4]);
    assert!(out[0] > out[1] && out[1] > out[2], "{out:?}");
}

#[test]
fn clamping_pins_every_control_to_its_range() {
    let wild = Grading {
        exposure: 99.0,
        contrast: -3.0,
        saturation: 5.0,
        tint: [2.0, 0.0, 0.5],
    }
    .clamped();
    assert!((wild.exposure - EXPOSURE_RANGE.1).abs() < f32::EPSILON);
    assert!((wild.contrast - CONTRAST_RANGE.0).abs() < f32::EPSILON);
    assert!((wild.saturation - SATURATION_RANGE.1).abs() < f32::EPSILON);
    assert!((wild.tint[0] - TINT_RANGE.1).abs() < f32::EPSILON);
    assert!((wild.tint[1] - TINT_RANGE.0).abs() < f32::EPSILON);
    assert!((wild.tint[2] - 0.5).abs() < f32::EPSILON);
}

#[test]
fn non_finite_controls_become_neutral() {
    let grading = Grading {
        exposure: f32::NAN,
        contrast: f32::INFINITY,
        saturation: f32::NEG_INFINITY,
        tint: [f32::NAN, f32::INFINITY, f32::NEG_INFINITY],
    }
    .clamped();
    assert_eq!(grading, Grading::neutral());
}

#[test]
fn every_preset_is_in_range_and_the_output_is_finite() {
    for preset in GradePreset::ALL {
        let grading = Grading::for_preset(preset);
        assert_eq!(grading, grading.clamped(), "{preset} is already in range");
        let out = grading.grade([10.0, -5.0, 250.0]);
        assert!(
            out.iter().all(|value| value.is_finite()),
            "{preset} -> {out:?}"
        );
    }
}

#[test]
fn presets_are_distinct_from_neutral_except_neutral() {
    for preset in GradePreset::ALL {
        let grading = Grading::for_preset(preset);
        if preset == GradePreset::Neutral {
            assert_eq!(grading, Grading::neutral());
        } else {
            assert_ne!(grading, Grading::neutral(), "{preset} differs from neutral");
        }
    }
}

#[test]
fn names_round_trip_and_unknown_names_are_rejected() {
    for preset in GradePreset::ALL {
        assert_eq!(preset.name().parse(), Ok(preset));
    }
    assert!("sepia".parse::<GradePreset>().is_err());
    assert!("".parse::<GradePreset>().is_err());
}
