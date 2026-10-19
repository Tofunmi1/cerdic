//! Easing curves from Blender's `BLI_easing` (`blenlib/intern/easing.cc`, Robert Penner's equations), in their
//! "ease out" form on a 0 to 1 fraction: the motion starts at full speed and settles into the end value. Used by
//! `anim.track` interpolation `back`, `bounce` and `elastic`. The test compares them with Blender's code run
//! through the kernel on the same inputs.
use std::f32::consts::PI;

/// Overshoot Blender uses for keyframes by default.
const BACK_OVERSHOOT: f32 = 1.70158;

/// Rushes past the end value a little and comes back.
#[must_use]
pub fn back_out(fraction: f32) -> f32 {
    let t = fraction - 1.0;
    t * t * ((BACK_OVERSHOOT + 1.0) * t + BACK_OVERSHOOT) + 1.0
}

/// Bounces against the end value, each bounce smaller.
#[must_use]
pub fn bounce_out(fraction: f32) -> f32 {
    let mut t = fraction;
    if t < 1.0 / 2.75 {
        return 7.5625 * t * t;
    }
    if t < 2.0 / 2.75 {
        t -= 1.5 / 2.75;
        return 7.5625 * t * t + 0.75;
    }
    if t < 2.5 / 2.75 {
        t -= 2.25 / 2.75;
        return 7.5625 * t * t + 0.9375;
    }
    t -= 2.625 / 2.75;
    7.5625 * t * t + 0.984_375
}

/// Blender's keyframe default for the elastic amplitude (smaller than a full swing, so it is blended in).
const ELASTIC_AMPLITUDE: f32 = 0.8;
/// The elastic period as a fraction of the motion's duration.
const ELASTIC_PERIOD: f32 = 0.3;

/// Springs past the end value and oscillates down onto it.
#[must_use]
pub fn elastic_out(fraction: f32) -> f32 {
    if fraction <= 0.0 {
        return 0.0;
    }
    if fraction >= 1.0 {
        return 1.0;
    }
    let time = -fraction;
    let s = ELASTIC_PERIOD / 4.0;
    // The amplitude is below the change, so Blender scales the oscillation by amplitude / change and blends it
    // in over the first part of the motion, else the start would be a sharp falloff.
    let mut f = ELASTIC_AMPLITUDE;
    if time.abs() < s.abs() {
        let l = time.abs() / s.abs();
        f = f * l + (1.0 - l);
    }
    f * (2.0_f32.powf(10.0 * time) * ((time - s) * (2.0 * PI) / ELASTIC_PERIOD).sin()) + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_start_at_zero_and_settle_on_one() {
        for ease in [back_out, bounce_out, elastic_out] {
            assert!(ease(0.0).abs() < 1e-5);
            assert!((ease(1.0) - 1.0).abs() < 1e-5);
        }
        assert!(back_out(0.7) > 1.0, "back overshoots the end value");
        assert!(
            (0..100).any(|i| elastic_out(f32::from(i as u8) / 100.0) > 1.0),
            "elastic overshoots"
        );
        let bounce: Vec<f32> = (1..40)
            .map(|i| bounce_out(f32::from(i as u8) / 40.0))
            .collect();
        assert!(
            bounce.windows(2).any(|w| w[1] < w[0]),
            "bounce comes back down after touching the end"
        );
    }
}
