//! `ve_render::grass`: the look of grass blades (ADR 0070 port 4, the grass shading of RedGPU's landscape: `grassVertexCommon.wgsl`, `grassFragmentCommon.wgsl`,
//! `grassVertex.wgsl`). Every function here is mirrored one for one in `shaders/grass.wgsl`; the tests pin the formulas, the shader is validated by naga.
use ve_render::grass::{
    contact_ao, fade_scale, ground_blend, height_ratio, sink_correction, sky_occlusion,
    upward_normal,
};

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_height_ratio_runs_from_the_root_to_the_tip_and_is_clamped() {
    assert!(close(height_ratio(0.0, 0.0, 1.0 / 0.45), 0.0));
    assert!(close(height_ratio(0.45, 0.0, 1.0 / 0.45), 1.0));
    assert!(close(height_ratio(0.225, 0.0, 1.0 / 0.45), 0.5));
    assert!(close(height_ratio(-1.0, 0.0, 2.0), 0.0));
    assert!(close(height_ratio(9.0, 0.0, 2.0), 1.0));
    // A mesh whose lowest point is below the origin: the ratio counts from that point.
    assert!(close(height_ratio(0.0, -0.1, 1.0 / 0.5), 0.2));
}

#[test]
fn the_roots_take_the_ground_colour_and_the_upper_blade_keeps_its_own() {
    let base = [0.30, 0.50, 0.15];
    let ground = [0.50, 0.40, 0.25];
    // Full strength: the root (ratio 0) is entirely the ground colour; from 40% of the height up the blade is its own colour.
    let root = ground_blend(base, ground, 0.0, 1.0, 1.0);
    for k in 0..3 {
        assert!(close(root[k], ground[k]), "{root:?}");
    }
    let tip = ground_blend(base, ground, 1.0, 1.0, 1.0);
    let at_forty = ground_blend(base, ground, 0.40, 1.0, 1.0);
    for k in 0..3 {
        assert!(close(tip[k], base[k]), "{tip:?}");
        assert!(close(at_forty[k], base[k]), "{at_forty:?}");
    }
    // In between it is a straight mix: ratio 0.2 is halfway (factor = (0.4 - 0.2) * 2.5 = 0.5).
    let mid = ground_blend(base, ground, 0.2, 1.0, 1.0);
    for k in 0..3 {
        assert!(close(mid[k], 0.5 * (base[k] + ground[k])), "{mid:?}");
    }
}

#[test]
fn blend_strength_scales_the_effect_and_zero_or_tiny_turns_it_off() {
    let base = [0.3, 0.5, 0.15];
    let ground = [0.9, 0.1, 0.1];
    let off = ground_blend(base, ground, 0.0, 0.0, 1.0);
    let tiny = ground_blend(base, ground, 0.0, 0.005, 1.0);
    for k in 0..3 {
        assert!(
            close(off[k], base[k]) && close(tiny[k], base[k]),
            "{off:?} {tiny:?}"
        );
    }
    let half = ground_blend(base, ground, 0.0, 0.5, 1.0);
    for k in 0..3 {
        assert!(close(half[k], 0.5 * (base[k] + ground[k])), "{half:?}");
    }
}

#[test]
fn the_exposure_boost_scales_the_base_colour_and_is_floored_at_a_tenth() {
    let boosted = ground_blend([0.2, 0.2, 0.2], [0.0; 3], 1.0, 0.0, 2.0);
    assert!(close(boosted[0], 0.4));
    let floored = ground_blend([0.2, 0.2, 0.2], [0.0; 3], 1.0, 0.0, 0.0);
    assert!(close(floored[0], 0.02));
}

#[test]
fn the_roots_are_darker_sky_occlusion_and_contact_ao_rise_with_height() {
    assert!(close(sky_occlusion(0.0), 0.65));
    assert!(close(sky_occlusion(1.0), 1.0));
    assert!(close(
        sky_occlusion(0.7),
        0.65 + 0.35 * (0.7_f32 * 1.43).min(1.0)
    ));
    assert!(close(contact_ao(0.0), 0.40));
    assert!(close(contact_ao(0.25), 1.0));
    assert!(close(contact_ao(1.0), 1.0));
    let mut last = 0.0;
    for i in 0..=20 {
        let v = contact_ao(i as f32 / 20.0) * sky_occlusion(i as f32 / 20.0);
        assert!(v >= last - 1e-6, "darkening must fall toward the root");
        last = v;
    }
}

#[test]
fn normals_lean_toward_up_more_at_the_tip_and_stay_unit_length() {
    let side = [1.0, 0.0, 0.0];
    let root = upward_normal(side, 0.0);
    let tip = upward_normal(side, 1.0);
    // mix(0.55, 0.85, ratio) of the way to straight up.
    let expect = |t: f32| {
        let v = [1.0 - t, t, 0.0];
        let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
        [v[0] / len, v[1] / len, 0.0]
    };
    let (er, et) = (expect(0.55), expect(0.85));
    for k in 0..3 {
        assert!(
            close(root[k], er[k]) && close(tip[k], et[k]),
            "{root:?} {tip:?}"
        );
    }
    assert!(tip[1] > root[1]);
    for n in [
        root,
        tip,
        upward_normal([0.0, -1.0, 0.0], 0.5),
        upward_normal([0.0, 0.0, 0.0], 0.5),
    ] {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4, "{n:?}");
    }
}

#[test]
fn a_blade_that_dips_below_ground_is_lifted_most_at_the_root() {
    // sink = max(0, -y); y += sink * (1 - ratio * 0.7)
    assert!(close(sink_correction(-0.10, 0.0), 0.10));
    assert!(close(sink_correction(-0.10, 1.0), 0.10 * 0.3));
    assert!(
        close(sink_correction(0.20, 0.5), 0.0),
        "above ground nothing changes"
    );
}

#[test]
fn blades_shrink_with_distance_full_size_up_to_the_near_distance_and_gone_at_the_far_one() {
    assert!(close(fade_scale(5.0, 20.0, 60.0), 1.0));
    assert!(close(fade_scale(20.0, 20.0, 60.0), 1.0));
    assert!(close(fade_scale(40.0, 20.0, 60.0), 0.5));
    assert!(close(fade_scale(60.0, 20.0, 60.0), 0.0));
    assert!(close(fade_scale(500.0, 20.0, 60.0), 0.0));
    // Never shrinks to a wrong value when near is not below far.
    assert!(fade_scale(10.0, 50.0, 50.0).is_finite());
    assert!(fade_scale(f32::NAN, 20.0, 60.0).is_finite());
}

#[test]
fn the_shader_file_has_the_same_functions() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/shaders/grass.wgsl"
    ))
    .unwrap_or_default();
    for name in [
        "grass_height_ratio",
        "grass_ground_blend",
        "grass_sky_occlusion",
        "grass_contact_ao",
        "grass_upward_normal",
        "grass_sink_correction",
        "grass_fade_scale",
    ] {
        assert!(source.contains(&format!("fn {name}")), "missing fn {name}");
    }
}
