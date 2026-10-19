//! Closed-profile sweep mesh generation along an open sampled curve.
use super::super::cache::Value;
use super::{
    mesh_overlap, overlap,
    vec3::{cross, dot, normalize, sub},
};
use crate::{Fail, operations::kernel_fail};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh, Solid};

pub(super) fn build(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let curve = inputs
        .iter()
        .find_map(|value| value.curve.as_ref())
        .ok_or_else(|| crate::fail("graph.missing_input", "sweep needs a curve"))?;
    let profile = inputs
        .iter()
        .find(|value| !value.loops.is_empty())
        .ok_or_else(|| crate::fail("graph.missing_input", "sweep needs a profile"))?;
    if curve.points.len() < if curve.closed { 3 } else { 2 } || profile.loops.len() != 1 {
        return Err(node_fail(
            node,
            "geometry.unsupported_topology",
            "sweep requires a valid sampled path and one profile loop",
            "link a curve with enough points and one closed profile",
        ));
    }
    let loop_points = &profile.loops[0];
    if loop_points.len() < 3 || loop_points.len() > 4096 {
        return Err(node_fail(
            node,
            "geometry.degenerate_profile",
            "sweep profile needs at least three points",
            "give the profile at least three points",
        ));
    }
    if !convex(loop_points) {
        return Err(node_fail(
            node,
            "geometry.unsupported_profile",
            "sweep end caps currently require one strictly convex profile loop",
            "use one convex profile, for example profile.circle",
        ));
    }
    let number = |name: &str, default: f64| {
        node.params
            .get(name)
            .and_then(ve_graph::Value::as_f64)
            .unwrap_or(default)
    };
    let (scale_end, twist) = (number("scale_end", 1.0), number("twist", 0.0));
    if curve.closed && ((scale_end - 1.0).abs() > 1.0e-9 || twist.abs() > 1.0e-9) {
        return Err(node_fail(
            node,
            "geometry.unsupported_topology",
            "scale_end and twist need an open curve: a closed loop has no end to taper or turn toward",
            "use an open curve, or set scale_end=1 and twist=0",
        ));
    }
    // The widest the tube gets decides the curvature and path-crossing checks.
    let grow = scale_end.max(1.0);
    let widest: Vec<[f64; 2]> = loop_points
        .iter()
        .map(|point| [point[0] * grow, point[1] * grow])
        .collect();
    let tube_radius = widest
        .iter()
        .map(|point| (point[0] * point[0] + point[1] * point[1]).sqrt())
        .fold(0.0_f64, f64::max);
    if let Some(curvature_radius) = minimum_curvature_radius(&curve.points, curve.closed)
        && tube_radius >= curvature_radius
    {
        return Err(node_fail(
            node,
            "curve.sweep_curvature",
            &format!(
                "tube radius {tube_radius:.6} must be less than local curvature radius {curvature_radius:.6}"
            ),
            "shrink the profile or make the path less sharply curved",
        ));
    }
    if let Some(found) = overlap::crossing(&curve.points, &widest, curve.closed, context)? {
        return Err(node_fail(
            node,
            "curve.sweep_self_intersection",
            &format!(
                "the tube passes through itself: path segments {} and {} are {:.6} apart, less than the {:.6} the profile needs",
                found.first, found.second, found.distance, found.needed
            ),
            "widen the path or shrink the profile",
        ));
    }
    let mesh = build_mesh(
        node,
        &curve.points,
        loop_points,
        curve.closed,
        (scale_end, twist),
        context,
    )?;
    if let Some((first, second)) = mesh_overlap::intersects(
        &mesh,
        curve.points.len(),
        loop_points.len(),
        curve.closed,
        context,
    )? {
        return Err(node_fail(
            node,
            "curve.sweep_self_intersection",
            &format!("non-adjacent swept triangles {first} and {second} intersect"),
            "widen the path or shrink the profile",
        ));
    }
    let solid = Solid::from_mesh(&mesh).map_err(|error| kernel_fail(node, &error))?;
    if context.is_cancelled() {
        return Err(crate::fail("eval.cancelled", "evaluation was cancelled"));
    }
    let mesh = solid
        .to_mesh()
        .map_err(|error| crate::fail("kernel.failed", error.to_string()))?;
    Ok(Value::solid(mesh, solid.volume()))
}

fn build_mesh(
    node: &Node,
    path: &[[f64; 3]],
    profile: &[[f64; 2]],
    closed: bool,
    (scale_end, twist): (f64, f64),
    context: &ExecutionContext,
) -> Result<Mesh, Fail> {
    let ring = profile.len();
    // How far along the path (0 to 1, by length) each ring sits, for the taper and the twist.
    let mut along = vec![0.0_f64];
    for pair in path.windows(2) {
        let step = ((pair[1][0] - pair[0][0]).powi(2)
            + (pair[1][1] - pair[0][1]).powi(2)
            + (pair[1][2] - pair[0][2]).powi(2))
        .sqrt();
        along.push(along.last().copied().unwrap_or(0.0) + step);
    }
    let total = along
        .last()
        .copied()
        .filter(|length| *length > 1.0e-12)
        .unwrap_or(1.0);
    let mut vertices = Vec::with_capacity(ring.saturating_mul(path.len()));
    let frames = (if closed {
        super::closed::frames(path)
    } else {
        frames(path)
    })
    .ok_or_else(|| {
        node_fail(
            node,
            "geometry.invalid_frame",
            "sweep path contains a repeated point or reversal",
            "remove repeated or reversing points from the path",
        )
    })?;
    for ((center, (normal, binormal)), travelled) in path.iter().zip(frames).zip(&along) {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sweep cancelled while building rings",
            ));
        }
        let t = travelled / total;
        let scale = 1.0 + (scale_end - 1.0) * t;
        let (sine, cosine) = (twist * t).sin_cos();
        for [px, py] in profile {
            let (x, y) = (
                scale * (px * cosine - py * sine),
                scale * (px * sine + py * cosine),
            );
            vertices.push([
                to_f32(center[0] + normal[0] * x + binormal[0] * y),
                to_f32(center[1] + normal[1] * x + binormal[1] * y),
                to_f32(center[2] + normal[2] * x + binormal[2] * y),
            ]);
        }
    }
    let mut indices = Vec::with_capacity(ring.saturating_mul(path.len()).saturating_mul(6));
    let segment_count = if closed { path.len() } else { path.len() - 1 };
    for path_index in 0..segment_count {
        let next_path = (path_index + 1) % path.len();
        for side in 0..ring {
            let next = (side + 1) % ring;
            let a = index(path_index * ring + side)?;
            let b = index(path_index * ring + next)?;
            let c = index(next_path * ring + side)?;
            let d = index(next_path * ring + next)?;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    if !closed {
        cap(&mut vertices, &mut indices, profile.len(), true)?;
        cap(&mut vertices, &mut indices, profile.len(), false)?;
    }
    Ok(Mesh {
        positions: vertices,
        indices,
    })
}

fn cap(
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    ring: usize,
    start: bool,
) -> Result<(), Fail> {
    let base = if start {
        0
    } else {
        vertices.len().saturating_sub(ring + 1)
    };
    let center_index = u32::try_from(vertices.len())
        .map_err(|_| crate::fail("budget.triangles", "sweep vertex index overflow"))?;
    let ring_f64 = f64::from(
        u32::try_from(ring)
            .map_err(|_| crate::fail("budget.triangles", "sweep ring count overflow"))?,
    );
    let center = vertices[base..base + ring]
        .iter()
        .fold([0.0_f64; 3], |mut sum, p| {
            for axis in 0..3 {
                sum[axis] += f64::from(p[axis]) / ring_f64;
            }
            sum
        })
        .map(to_f32);
    vertices.push(center);
    for side in 0..ring {
        let current = u32::try_from(base + side)
            .map_err(|_| crate::fail("budget.triangles", "sweep vertex index overflow"))?;
        let next = u32::try_from(base + (side + 1) % ring)
            .map_err(|_| crate::fail("budget.triangles", "sweep vertex index overflow"))?;
        if start {
            indices.extend_from_slice(&[center_index, next, current]);
        } else {
            indices.extend_from_slice(&[center_index, current, next]);
        }
    }
    Ok(())
}

fn frames(points: &[[f64; 3]]) -> Option<Vec<([f64; 3], [f64; 3])>> {
    let mut tangents = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let delta = if index == 0 {
            sub(points[1], points[0])
        } else if index + 1 == points.len() {
            sub(points[index], points[index - 1])
        } else {
            sub(points[index + 1], points[index - 1])
        };
        tangents.push(normalize(delta)?);
    }
    let reference = if tangents[0][1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut normal = normalize(cross(tangents[0], reference))?;
    let mut out = Vec::with_capacity(points.len());
    out.push((normal, normalize(cross(tangents[0], normal))?));
    for (index, tangent) in tangents.iter().enumerate().skip(1) {
        let previous_tangent = tangents[index - 1];
        if dot(previous_tangent, *tangent) < -0.9999 {
            return None;
        }
        let chord = sub(points[index], points[index - 1]);
        let tangent_reflection = reflect(previous_tangent, chord)?;
        let normal_reflection = reflect(normal, chord)?;
        let correction = sub(*tangent, tangent_reflection);
        normal = if dot(correction, correction) > 1.0e-24 {
            reflect(normal_reflection, correction)?
        } else {
            normal_reflection
        };
        let binormal = normalize(cross(*tangent, normal))?;
        normal = normalize(cross(binormal, *tangent))?;
        out.push((normal, binormal));
    }
    Some(out)
}

fn minimum_curvature_radius(points: &[[f64; 3]], closed: bool) -> Option<f64> {
    let mut minimum = f64::INFINITY;
    let triples = if closed {
        (0..points.len())
            .map(|index| {
                [
                    points[index],
                    points[(index + 1) % points.len()],
                    points[(index + 2) % points.len()],
                ]
            })
            .collect::<Vec<_>>()
    } else {
        points
            .windows(3)
            .map(|triple| [triple[0], triple[1], triple[2]])
            .collect()
    };
    for [first, middle, last] in triples {
        let a = sub(middle, first);
        let b = sub(last, middle);
        let chord = sub(last, first);
        let twice_area = dot(cross(a, chord), cross(a, chord)).sqrt();
        let denominator = 2.0 * twice_area;
        if denominator > 1.0e-12 {
            let radius =
                dot(a, a).sqrt() * dot(b, b).sqrt() * dot(chord, chord).sqrt() / denominator;
            minimum = minimum.min(radius);
        }
    }
    minimum.is_finite().then_some(minimum)
}

fn node_fail(node: &Node, code: &str, message: &str, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

fn reflect(vector: [f64; 3], plane_normal: [f64; 3]) -> Option<[f64; 3]> {
    let denominator = dot(plane_normal, plane_normal);
    if denominator <= 1.0e-24 {
        return None;
    }
    let factor = 2.0 * dot(vector, plane_normal) / denominator;
    normalize([
        vector[0] - factor * plane_normal[0],
        vector[1] - factor * plane_normal[1],
        vector[2] - factor * plane_normal[2],
    ])
}

fn index(value: usize) -> Result<u32, Fail> {
    u32::try_from(value).map_err(|_| crate::fail("budget.triangles", "sweep vertex index overflow"))
}

fn to_f32(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "geometry extents are bounded before mesh conversion"
    )]
    {
        value as f32
    }
}

fn convex(points: &[[f64; 2]]) -> bool {
    let mut orientation = None;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let c = points[(index + 2) % points.len()];
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if cross.abs() <= 1.0e-12 || orientation.is_some_and(|sign: f64| cross * sign < 0.0) {
            return false;
        }
        orientation = Some(cross.signum());
    }
    true
}

#[cfg(test)]
#[path = "sweep_tests.rs"]
mod tests;
