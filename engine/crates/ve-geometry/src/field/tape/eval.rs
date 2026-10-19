//! Pure Rust reference evaluator for the C++ field-tape interpreter.
use crate::field::model::{Combine, Field, FieldExpr, Transform};

/// Evaluates the expression in Rust as the numeric reference for the C++ tape interpreter.
#[must_use]
pub fn evaluate(field: &Field, point: [f64; 3]) -> f64 {
    evaluate_expr(&field.expr, point)
}

fn evaluate_expr(expr: &FieldExpr, point: [f64; 3]) -> f64 {
    if let Some(value) = evaluate_primitive(expr, point) {
        return value;
    }
    match expr {
        FieldExpr::Sphere { .. }
        | FieldExpr::Ellipsoid { .. }
        | FieldExpr::Box { .. }
        | FieldExpr::Torus { .. }
        | FieldExpr::Capsule { .. }
        | FieldExpr::Cylinder { .. }
        | FieldExpr::Plane { .. }
        | FieldExpr::Tube { .. } => unreachable!("primitive expressions are handled above"),
        FieldExpr::Combine { op, a, b } => {
            let left = evaluate_expr(a, point);
            let right = evaluate_expr(b, point);
            match op {
                Combine::Union => left.max(right),
                Combine::Intersection => left.min(right),
                Combine::Difference => left.min(-right),
            }
        }
        FieldExpr::SmoothUnion { a, b, radius } => {
            let left = evaluate_expr(a, point);
            let right = evaluate_expr(b, point);
            let h = (radius - (left - right).abs()).max(0.0) / radius;
            left.max(right) + h * h * radius * 0.25
        }
        FieldExpr::Transform { op, child } => match op {
            Transform::Move(delta) => evaluate_expr(child, sub(point, *delta)),
            Transform::Scale(factor) => {
                evaluate_expr(
                    child,
                    [point[0] / factor, point[1] / factor, point[2] / factor],
                ) * factor
            }
            Transform::Mirror(axes) => evaluate_expr(
                child,
                [
                    if axes[0] { point[0].abs() } else { point[0] },
                    if axes[1] { point[1].abs() } else { point[1] },
                    if axes[2] { point[2].abs() } else { point[2] },
                ],
            ),
        },
        FieldExpr::Repeat {
            period,
            count,
            child,
        } => evaluate_expr(child, repeated_point(point, *period, *count)),
        FieldExpr::Offset { child, amount } => evaluate_expr(child, point) + amount,
        FieldExpr::Shell { child, half_width } => half_width - evaluate_expr(child, point).abs(),
    }
}

fn evaluate_primitive(expr: &FieldExpr, point: [f64; 3]) -> Option<f64> {
    Some(match expr {
        FieldExpr::Sphere { center, radius } => radius - length(sub(point, *center)),
        FieldExpr::Ellipsoid { radii } => {
            let scaled = [
                point[0] / radii[0],
                point[1] / radii[1],
                point[2] / radii[2],
            ];
            radii[0].min(radii[1]).min(radii[2]) * (1.0 - length(scaled))
        }
        FieldExpr::Box { half, rounding } => {
            let q = [
                point[0].abs() - half[0] + rounding,
                point[1].abs() - half[1] + rounding,
                point[2].abs() - half[2] + rounding,
            ];
            let outside = length([q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)]);
            rounding - outside - q[0].max(q[1].max(q[2])).min(0.0)
        }
        FieldExpr::Torus { major, tube } => {
            let ring = (point[0] * point[0] + point[2] * point[2]).sqrt() - major;
            tube - (ring * ring + point[1] * point[1]).sqrt()
        }
        FieldExpr::Capsule {
            radius,
            half_length,
        } => {
            let y = point[1].clamp(-half_length, *half_length);
            radius - length([point[0], point[1] - y, point[2]])
        }
        FieldExpr::Cylinder {
            radius,
            half_height,
        } => {
            let radial = (point[0] * point[0] + point[2] * point[2]).sqrt() - radius;
            let vertical = point[1].abs() - half_height;
            let outside = length([radial.max(0.0), vertical.max(0.0), 0.0]);
            -(outside + radial.max(vertical).min(0.0))
        }
        FieldExpr::Plane { normal, offset } => {
            normal[0] * point[0] + normal[1] * point[1] + normal[2] * point[2] + offset
        }
        FieldExpr::Tube {
            points,
            radius,
            end_radius,
        } => evaluate_tube(points, *radius, *end_radius, point),
        _ => return None,
    })
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn length(point: [f64; 3]) -> f64 {
    (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt()
}

fn tube_segment(point: [f64; 3], start: [f64; 3], end: [f64; 3], radii: [f64; 2]) -> f64 {
    let direction = sub(end, start);
    let relative = sub(point, start);
    let denominator = dot(direction, direction);
    let t = (dot(relative, direction) / denominator).clamp(0.0, 1.0);
    let delta = [
        relative[0] - direction[0] * t,
        relative[1] - direction[1] * t,
        relative[2] - direction[2] * t,
    ];
    radii[0] + (radii[1] - radii[0]) * t - length(delta)
}

fn evaluate_tube(points: &[[f64; 3]], radius: f64, end_radius: f64, point: [f64; 3]) -> f64 {
    let radii = crate::field::model::tube_segment_radii(points, radius, end_radius);
    points
        .windows(2)
        .zip(radii)
        .map(|(segment, ends)| tube_segment(point, segment[0], segment[1], ends))
        .fold(f64::NEG_INFINITY, f64::max)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn repeated_point(mut point: [f64; 3], period: [f64; 3], count: [u8; 3]) -> [f64; 3] {
    for axis in 0..3 {
        let bound = f64::from(count[axis]);
        let cell = (point[axis] / period[axis]).round().clamp(-bound, bound);
        point[axis] -= cell * period[axis];
    }
    point
}
