//! Small deterministic helpers used by the gait cycle.

pub(super) fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

pub(super) fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}

pub(super) fn sine_arc(progress: f64) -> f64 {
    (std::f64::consts::PI * progress).sin()
}

#[cfg(test)]
mod tests {
    use super::{sine_arc, smoothstep};

    #[test]
    fn phase_smoothstep_has_stationary_endpoints() {
        assert_eq!(smoothstep(0.0).to_bits(), 0.0_f64.to_bits());
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-15);
        assert!((smoothstep(1.0) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn swing_arc_is_zero_at_ends_and_one_at_midpoint() {
        assert!(sine_arc(0.0).abs() < 1e-15);
        assert!((sine_arc(0.5) - 1.0).abs() < 1e-15);
        assert!(sine_arc(1.0).abs() < 1e-15);
    }
}
