use super::{ERROR_LIMIT, EV_LIMIT, GAIN, SPEED, TARGET, adaptation, next_exposure};

/// The tonemapper of `forward.wgsl`, so the controller is tested against the response it actually steers.
fn aces(x: f32) -> f32 {
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// Runs the controller on a scene of constant radiance `scene` for `frames` frames of `delta` seconds, from exposure 1.
fn settle(scene: f32, frames: usize, delta: f32) -> (f32, f32) {
    let adapt = adaptation(delta);
    let mut exposure = 1.0;
    for _ in 0..frames {
        let displayed = aces(scene * exposure).clamp(0.001, 1.0);
        exposure = next_exposure(exposure, displayed, TARGET, adapt);
    }
    (exposure, aces(scene * exposure))
}

/// The value of `const NAME: f32 = ...;` in the exposure shader.
fn shader_constant(name: &str) -> f32 {
    let source = include_str!("shaders/exposure.wgsl");
    let line = source
        .lines()
        .find(|line| line.starts_with(&format!("const {name}:")))
        .unwrap_or_else(|| panic!("exposure.wgsl has no constant {name}"));
    line.split('=')
        .nth(1)
        .and_then(|value| value.trim().trim_end_matches(';').parse().ok())
        .unwrap_or_else(|| panic!("cannot read {name} from {line}"))
}

#[test]
fn the_shader_uses_the_same_constants_as_the_controller() {
    assert!((shader_constant("EV_LIMIT") - EV_LIMIT).abs() < f32::EPSILON);
    assert!((shader_constant("ERROR_LIMIT") - ERROR_LIMIT).abs() < f32::EPSILON);
    assert!((shader_constant("GAIN") - GAIN).abs() < f32::EPSILON);
}

#[test]
fn a_correct_picture_is_left_alone() {
    // A picture already at the target does not move the exposure.
    for exposure in [0.3, 1.0, 3.0] {
        let next = next_exposure(exposure, TARGET, TARGET, 0.05);
        assert!((next - exposure).abs() < 1.0e-6, "{exposure} became {next}");
    }
}

#[test]
fn a_dark_picture_brightens_and_a_bright_one_darkens() {
    assert!(next_exposure(1.0, 0.02, TARGET, 0.05) > 1.0);
    assert!(next_exposure(1.0, 0.9, TARGET, 0.05) < 1.0);
}

#[test]
fn scenes_of_very_different_brightness_settle_on_the_same_picture_brightness() {
    // Scene radiance from deep shade to bright sun, within what a factor of 16 either way can correct.
    for scene in [0.02, 0.05, 0.18, 0.5, 1.5, 4.0] {
        let (exposure, shown) = settle(scene, 600, 1.0 / 60.0);
        assert!(
            (shown - TARGET).abs() < 0.01,
            "scene {scene}: exposure {exposure} shows {shown}, wanted {TARGET}"
        );
    }
}

#[test]
fn the_exposure_never_leaves_its_range() {
    let limit = EV_LIMIT.exp2();
    // A black scene wants an enormous exposure, a blinding one a tiny one.
    let (dark, _) = settle(1.0e-6, 5000, 1.0 / 60.0);
    let (bright, _) = settle(1.0e6, 5000, 1.0 / 60.0);
    assert!(dark <= limit + 1.0e-4 && dark > limit * 0.9, "{dark}");
    assert!(
        bright >= 1.0 / limit - 1.0e-4 && bright < 1.1 / limit,
        "{bright}"
    );
}

#[test]
fn adaptation_takes_the_same_time_at_any_frame_rate() {
    // One second of adaptation, in 60 steps or in 15, ends at nearly the same exposure.
    let run = |frames: usize, delta: f32| {
        let adapt = adaptation(delta);
        (0..frames).fold(1.0, |exposure, _| {
            next_exposure(exposure, aces(0.05 * exposure), TARGET, adapt)
        })
    };
    let (fast, slow) = (run(60, 1.0 / 60.0), run(15, 1.0 / 15.0));
    assert!(
        (fast.log2() - slow.log2()).abs() < 0.15,
        "{fast} against {slow}"
    );
    // And it is not instant: after a tenth of a second most of the way is still to go.
    let early = run(6, 1.0 / 60.0);
    let (settled, _) = settle(0.05, 2000, 1.0 / 60.0);
    assert!(
        early < settled * 0.8,
        "{early} after 0.1 s, {settled} settled"
    );
}

#[test]
fn a_huge_frame_time_cannot_snap_the_exposure() {
    // A stall (a tab in the background) gives a delta of seconds; the share applied is capped.
    assert!(adaptation(10.0) <= 1.0 - (-0.5 * SPEED).exp() + 1.0e-6);
    assert!(adaptation(0.0).abs() < 1.0e-9);
}
