//! `bezier` interpolation for `anim.track`: smooth curves through all keys with Blender's automatic clamped
//! handles, so motion flows through a key instead of easing to a stop at every one.
//!
//! This is a port of the F-curve rules in Blender (`calchandleNurb_intern` for automatic animation handles with
//! no extra smoothing, the first and last key flattened for constant extrapolation, and `BKE_fcurve_correct_bezpart`
//! followed by solving the cubic for time), kept in single precision and in the same order of operations. The
//! reference values in the tests come from running Blender's own `evaluate_fcurve` on the same keys. Each of the
//! three axes is its own curve.
use crate::anim::Key;

/// Blender's handle length constant.
const HANDLE_SCALE: f32 = 2.5614;

/// A handle point: time, value.
type Point = [f32; 2];

/// The left and right handles of key `i` for one axis.
fn handles(keys: &[Key], axis: usize, i: usize) -> (Point, Point) {
    let at = |k: usize| -> Point { [keys[k].time, keys[k].value[axis]] };
    let p2 = at(i);
    let last = keys.len() - 1;
    let (prev, next) = ((i > 0).then(|| at(i - 1)), (i < last).then(|| at(i + 1)));
    let mirror = |other: Point| -> Point { [2.0 * p2[0] - other[0], 2.0 * p2[1] - other[1]] };
    // A missing neighbour is the other one mirrored through this key.
    let (p1, p3) = match (prev, next) {
        (Some(a), Some(b)) => (a, b),
        (None, Some(b)) => (mirror(b), b),
        (Some(a), None) => (a, mirror(a)),
        (None, None) => return ([p2[0], p2[1]], [p2[0], p2[1]]),
    };
    let dvec_a = [p2[0] - p1[0], p2[1] - p1[1]];
    let dvec_b = [p3[0] - p2[0], p3[1] - p2[1]];
    let mut len_a = if dvec_a[0] == 0.0 { 1.0 } else { dvec_a[0] };
    let mut len_b = if dvec_b[0] == 0.0 { 1.0 } else { dvec_b[0] };
    let tvec = [
        dvec_b[0] / len_b + dvec_a[0] / len_a,
        dvec_b[1] / len_b + dvec_a[1] / len_a,
    ];
    let len = tvec[0] * HANDLE_SCALE;
    let (mut left, mut right) = ([p2[0], p2[1]], [p2[0], p2[1]]);
    if len == 0.0 {
        return (left, right);
    }
    let flat_ends = i == 0 || i == last;
    len_a = len_a.min(5.0 * len_b);
    len_b = len_b.min(5.0 * len_a);
    let (mut left_violates, mut right_violates) = (false, false);
    // The clamping rules only apply to keys with a real neighbour on both sides.
    let both = prev.is_some() && next.is_some();
    let (ydiff1, ydiff2) = (p1[1] - p2[1], p3[1] - p2[1]);
    let extremum = (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0);

    len_a /= len;
    left = [p2[0] - tvec[0] * len_a, p2[1] - tvec[1] * len_a];
    if both {
        if extremum {
            left[1] = p2[1];
        } else if ydiff1 <= 0.0 {
            if p1[1] > left[1] {
                left[1] = p1[1];
                left_violates = true;
            }
        } else if p1[1] < left[1] {
            left[1] = p1[1];
            left_violates = true;
        }
    }

    len_b /= len;
    right = [p2[0] + tvec[0] * len_b, p2[1] + tvec[1] * len_b];
    if both {
        if extremum {
            right[1] = p2[1];
        } else if ydiff1 <= 0.0 {
            if p3[1] < right[1] {
                right[1] = p3[1];
                right_violates = true;
            }
        } else if p3[1] > right[1] {
            right[1] = p3[1];
            right_violates = true;
        }
    }

    if left_violates || right_violates {
        let h1_x = left[0] - p2[0];
        let h2_x = p2[0] - right[0];
        if left_violates {
            right[1] = p2[1] + ((p2[1] - left[1]) / h1_x) * h2_x;
        } else {
            left[1] = p2[1] + ((p2[1] - right[1]) / h2_x) * h1_x;
        }
    }
    // The first and last keys ease in and out: both handles are level with the key (constant extrapolation).
    if flat_ends {
        left[1] = p2[1];
        right[1] = p2[1];
    }
    (left, right)
}

/// The value of one axis at `time`, inside the segment from key `a` to key `a + 1`.
pub(crate) fn sample(keys: &[Key], a: usize, time: f32, axis: usize) -> f32 {
    let (k0, k1) = (&keys[a], &keys[a + 1]);
    let v1 = [k0.time, k0.value[axis]];
    let mut v2 = handles(keys, axis, a).1;
    let mut v3 = handles(keys, axis, a + 1).0;
    let v4 = [k1.time, k1.value[axis]];
    // On a key the curve is exactly the key's value.
    if time <= v1[0] {
        return v1[1];
    }
    if time >= v4[0] {
        return v4[1];
    }
    // Flat segment: the value is the shared value.
    if (v1[1] - v4[1]).abs() < f32::EPSILON
        && (v2[1] - v3[1]).abs() < f32::EPSILON
        && (v3[1] - v4[1]).abs() < f32::EPSILON
    {
        return v1[1];
    }
    // Handles may not reach past the neighbouring key's time (no looping or rewinding).
    let (h1, h2) = (
        [v1[0] - v2[0], v1[1] - v2[1]],
        [v4[0] - v3[0], v4[1] - v3[1]],
    );
    let span = v4[0] - v1[0];
    let (len1, len2) = (h1[0].abs(), h2[0].abs());
    if len1 + len2 != 0.0 {
        if len1 > span {
            let fac = span / len1;
            v2 = [v1[0] - fac * h1[0], v1[1] - fac * h1[1]];
        }
        if len2 > span {
            let fac = span / len2;
            v3 = [v4[0] - fac * h2[0], v4[1] - fac * h2[1]];
        }
    }
    // Solve x(t) = time by bisection (x is monotone once the handles are corrected), then read y(t).
    let cubic = |p0: f32, p1: f32, p2: f32, p3: f32, t: f64| -> f64 {
        let u = 1.0 - t;
        u * u * u * f64::from(p0)
            + 3.0 * u * u * t * f64::from(p1)
            + 3.0 * u * t * t * f64::from(p2)
            + t * t * t * f64::from(p3)
    };
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        if cubic(v1[0], v2[0], v3[0], v4[0], mid) < f64::from(time) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a curve value is stored as f32"
    )]
    let y = cubic(v1[1], v2[1], v3[1], v4[1], t) as f32;
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keys of Blender's reference run (`evaluate_fcurve` with automatic animation handles).
    fn reference_keys() -> Vec<Key> {
        [(1.0, 7.0), (2.0, 13.0), (4.0, 5.0), (6.0, 20.0)]
            .map(|(time, value)| Key {
                time,
                value: [value, 0.0, 0.0],
            })
            .to_vec()
    }

    fn value(keys: &[Key], time: f32) -> f32 {
        let a = keys
            .partition_point(|k| k.time <= time)
            .clamp(1, keys.len() - 1)
            - 1;
        sample(keys, a, time, 0)
    }

    #[test]
    fn matches_blenders_evaluation_of_the_same_keys() {
        // Printed by Blender's own `evaluate_fcurve` (see the F-curve spike), four decimals.
        let blender = [
            (1.2, 7.5333),
            (1.5, 10.0),
            (1.8, 12.4667),
            (2.1, 12.9559),
            (2.4, 12.2889),
            (2.7, 10.8807),
            (3.0, 9.0),
            (3.3, 7.1193),
            (3.6, 5.7111),
            (3.9, 5.0441),
            (4.2, 5.3325),
            (4.5, 7.0743),
            (4.8, 10.0889),
            (5.1, 13.7241),
            (5.4, 17.0395),
            (5.7, 19.2497),
        ];
        let keys = reference_keys();
        for (time, expected) in blender {
            let got = value(&keys, time);
            assert!(
                (got - expected).abs() < 2e-3,
                "t={time}: ours {got}, Blender {expected}"
            );
        }
    }

    #[test]
    fn passes_through_keys_flat_at_the_ends_and_without_overshoot() {
        let keys = reference_keys();
        for key in &keys[..3] {
            assert!(
                (value(&keys, key.time + 1e-4) - key.value[0]).abs() < 0.05,
                "through key at {}",
                key.time
            );
        }
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for i in 0..=500 {
            #[expect(clippy::cast_precision_loss, reason = "small loop index")]
            let v = value(&keys, 1.0 + 5.0 * i as f32 / 500.0);
            lowest = lowest.min(v);
            highest = highest.max(v);
        }
        assert!(
            lowest >= 5.0 - 0.15 && highest <= 20.0 + 1e-3,
            "clamped handles: {lowest}..{highest}"
        );
        // A curve that ends flat does not rush into the last key.
        assert!(value(&keys, 5.99) > 19.9);
    }
}
