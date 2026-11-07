//! Domain warps of a field along an axis: the point to look up in the input field for a point of the warped one.
//!
//! A taper divides the cross-section coordinates by a scale that runs linearly along the axis; a twist turns the cross-section by an angle
//! that runs linearly along the axis; a bend curves the axis into a circular arc. The C++ evaluator (`cpp/src/field_tape.cpp`) repeats these
//! formulas operation for operation, so meshing agrees with this code up to the platform's `sin`, `cos` and `atan2`.

/// Position along the range `from..to` clamped to 0..1 (held at the ends outside the range).
pub(super) fn along(point: [f64; 3], axis: usize, from: f64, to: f64) -> f64 {
    ((point[axis] - from) / (to - from)).clamp(0.0, 1.0)
}

/// The scale (taper) or angle in degrees (twist) at a point: `start` at `from`, `end` at `to`.
pub(super) fn value_at(point: [f64; 3], axis: usize, from: f64, to: f64, start: f64, end: f64) -> f64 {
    start + (end - start) * along(point, axis, from, to)
}

/// The point of the straight input that a point of the bent field comes from. The axis `axis` curves toward the axis `toward` through
/// `angle_degrees` between `from` and `to` (positive toward +`toward`); the point at `from` stays fixed. Before `from` the input is
/// unchanged; past `to` the input continues straight along the end tangent (a rigid motion of the end). Beyond 180 degrees the bent shape
/// overlaps itself and the inverse takes the nearer turn.
pub(super) fn bend_inverse(point: [f64; 3], axis: usize, toward: usize, from: f64, to: f64, angle_degrees: f64) -> [f64; 3] {
    let theta = angle_degrees.to_radians();
    if theta == 0.0 {
        return point;
    }
    // Signed radius: the centre of the arc is `radius` away from the axis toward `toward`, at the position `from`.
    let radius = (to - from) / theta;
    let sign = if radius < 0.0 { -1.0 } else { 1.0 };
    let (ds, w) = (point[axis] - from, radius - point[toward]);
    // The angle around the centre, measured so that it grows from 0 at `from` to `theta` at `to` (forward: s = from + radius * phi).
    let phi = (ds * sign).atan2(w * sign);
    let turned = phi * theta.signum();
    let mut out = point;
    if turned < 0.0 {
        return out;
    }
    if turned > theta.abs() {
        // Past the end: undo the rigid motion that carries the end cross-section (to, 0) to its place on the arc.
        let (sin, cos) = theta.sin_cos();
        let end = (from + radius * sin, radius * (1.0 - cos));
        let (x, y) = (point[axis] - end.0, point[toward] - end.1);
        out[axis] = to + x * cos + y * sin;
        out[toward] = -x * sin + y * cos;
        return out;
    }
    let rho = sign * ds.hypot(w);
    out[axis] = from + radius * phi;
    out[toward] = radius - rho;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the straight point `(s, u)` goes when the axis is bent (the forward map of the same arc), for the round-trip test.
    fn bend_forward(s: f64, u: f64, from: f64, to: f64, angle_degrees: f64) -> (f64, f64) {
        let theta = angle_degrees.to_radians();
        let radius = (to - from) / theta;
        let t = ((s - from) / (to - from)).clamp(0.0, 1.0);
        let phi = theta * t;
        let (sin, cos) = phi.sin_cos();
        let arc = (from + (radius - u) * sin, radius - (radius - u) * cos);
        // beyond the ends the end cross-section's tangent continues
        let extra = s - from - (to - from) * t;
        let tangent = (phi.cos(), phi.sin());
        (arc.0 + extra * tangent.0, arc.1 + extra * tangent.1)
    }

    #[test]
    fn a_ninety_degree_bend_over_a_metre_ends_at_the_analytic_point() {
        // radius 1 / (pi / 2); the bar's end (s = 0.5, u = 0) sits at (0.137, 0.637) in (axis, toward) coordinates.
        let (s, u) = bend_forward(0.5, 0.0, -0.5, 0.5, 90.0);
        let radius = 1.0 / std::f64::consts::FRAC_PI_2;
        assert!((s - (-0.5 + radius)).abs() < 1e-12 && (u - radius).abs() < 1e-12, "{s} {u}");
        assert!((s - 0.1366).abs() < 1e-3 && (u - 0.6366).abs() < 1e-3);
    }

    #[test]
    fn the_inverse_undoes_the_forward_map_inside_and_beyond_the_range_for_both_directions() {
        // (A wide bend folds the bar and its straight extension over each other, so a point can come from two places; the inverse
        // picks one. Up to 90 degrees every point has one source.)
        for angle in [90.0, -90.0, 30.0, -60.0] {
            for &s in &[-0.9, -0.5, -0.2, 0.0, 0.3, 0.5, 0.8] {
                for &u in &[-0.15, 0.0, 0.1] {
                    let (fs, fu) = bend_forward(s, u, -0.5, 0.5, angle);
                    // lay the forward point out as a 3d point on axis 1 (y) and toward 0 (x)
                    let back = bend_inverse([fu, fs, 0.25], 1, 0, -0.5, 0.5, angle);
                    assert!((back[1] - s).abs() < 1e-9 && (back[0] - u).abs() < 1e-9 && back[2] == 0.25, "angle {angle} ({s}, {u}) came back as ({}, {})", back[1], back[0]);
                }
            }
        }
    }

    #[test]
    fn a_zero_angle_changes_nothing_bit_for_bit() {
        let p = [0.3, -0.2, 0.7];
        assert_eq!(bend_inverse(p, 1, 0, -0.5, 0.5, 0.0), p);
    }

    #[test]
    fn a_range_given_backwards_bends_the_other_end_of_the_axis() {
        let (from, to) = (0.5, -0.5);
        // `from` stays fixed, so the point at s = 0.5 is not moved
        let back = bend_inverse([0.0, 0.5, 0.0], 1, 0, from, to, 90.0);
        assert!((back[1] - 0.5).abs() < 1e-12 && back[0].abs() < 1e-12, "{back:?}");
    }

    #[test]
    fn taper_and_twist_values_run_linearly_and_are_held_outside_the_range() {
        let p = |y: f64| [0.0, y, 0.0];
        assert!((value_at(p(0.0), 1, -0.5, 0.5, 1.0, 0.5) - 0.75).abs() < 1e-12);
        assert_eq!(value_at(p(-5.0), 1, -0.5, 0.5, 1.0, 0.5), 1.0);
        assert_eq!(value_at(p(5.0), 1, -0.5, 0.5, 1.0, 0.5), 0.5);
        assert!((value_at(p(0.0), 1, -0.5, 0.5, 0.0, 90.0) - 45.0).abs() < 1e-12);
    }
}
