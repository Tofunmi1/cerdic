//! Orientation checks for deformed mesh faces.
use crate::{Fail, fail};
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh};

pub(super) fn bounds(mesh: &Mesh) -> Result<([f64; 3], [f64; 3]), Fail> {
    let Some(first) = mesh.positions.first() else {
        return Err(fail(
            "graph.missing_input",
            "deformer needs a non-empty solid",
        ));
    };
    Ok(mesh.positions.iter().fold(
        (first.map(f64::from), first.map(f64::from)),
        |(mut minimum, mut maximum), point| {
            for axis in 0..3 {
                let value = f64::from(point[axis]);
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
            }
            (minimum, maximum)
        },
    ))
}

#[expect(
    clippy::many_single_char_names,
    reason = "The face-orientation calculation uses conventional vector labels."
)]
pub(super) fn orientation(
    source: &Mesh,
    mesh: &Mesh,
    node: &Node,
    map: &super::maps::Map,
    minimum: [f64; 3],
    maximum: [f64; 3],
    context: &ExecutionContext,
) -> Result<(), Fail> {
    // Positions are f32: the smallest width a face can have and still be told apart from a line.
    let reach = (0..3)
        .map(|axis| minimum[axis].abs().max(maximum[axis].abs()))
        .fold(1.0e-3_f64, f64::max);
    let resolution = 8.0 * f64::from(f32::EPSILON) * reach;
    for (index, triangle) in source.indices.chunks_exact(3).enumerate() {
        if index % 1024 == 0 && context.is_cancelled() {
            return Err(fail(
                "eval.cancelled",
                "deformation orientation check cancelled",
            ));
        }
        let source_points = [triangle[0], triangle[1], triangle[2]]
            .map(|id| source.positions[id as usize].map(f64::from));
        let [a, b, c] = source_points;
        let edge_a = sub(b, a);
        let edge_b = sub(c, a);
        let source_normal = cross(edge_a, edge_b);
        // A sliver thinner than the f32 grid of the mesh (boolean output can contain a few) has no
        // resolvable orientation and was not made by this deformation; the kernel validates the whole
        // solid afterwards.
        let longest = length(edge_a).max(length(edge_b)).max(length(sub(c, b)));
        if length(source_normal) <= 1.0e-12 || length(source_normal) / longest <= resolution {
            continue;
        }
        let center = [0, 1, 2].map(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
        let u =
            normalize(edge_a).ok_or_else(|| refusal(node, "deformation collapsed a mesh edge"))?;
        let n = normalize(source_normal).ok_or_else(|| {
            refusal(
                node,
                &format!(
                    "deformation collapsed mesh face {index} (normal length {:.3e}, edges {:.3e} and {:.3e})",
                    length(source_normal),
                    length(edge_a),
                    length(edge_b)
                ),
            )
        })?;
        let v = normalize(cross(n, u))
            .ok_or_else(|| refusal(node, "deformation collapsed a mesh face"))?;
        let epsilon = length(edge_a).min(length(edge_b)).clamp(1.0e-6, 1.0e-3) * 0.01;
        let map = |offset: [f64; 3]| map.apply(offset, minimum, maximum);
        let mapped_vertices = source_points.map(map);
        let expected_normal = cross(
            sub(mapped_vertices[1], mapped_vertices[0]),
            sub(mapped_vertices[2], mapped_vertices[0]),
        );
        let output_points = [triangle[0], triangle[1], triangle[2]]
            .map(|id| mesh.positions[id as usize].map(f64::from));
        let output_normal = cross(
            sub(output_points[1], output_points[0]),
            sub(output_points[2], output_points[0]),
        );
        if dot(expected_normal, output_normal) <= 0.0 {
            return Err(refusal(
                node,
                &format!(
                    "deformation flips or collapses mesh face {index} (normals {:.3e} and {:.3e}, dot {:.3e}, source twice-area {:.3e})",
                    length(expected_normal),
                    length(output_normal),
                    dot(expected_normal, output_normal),
                    length(source_normal)
                ),
            ));
        }
        let base = map(center);
        let mapped_u = sub(map(add(center, scale(u, epsilon))), base);
        let mapped_v = sub(map(add(center, scale(v, epsilon))), base);
        let mapped_n = sub(map(add(center, scale(n, epsilon))), base);
        // u, v and n are orthonormal and each step is `epsilon` long, so the triple product over
        // epsilon cubed is the local volume scale of the map: positive and not near zero means the map
        // does not fold, whatever the size of the face.
        let volume_scale = dot(cross(mapped_u, mapped_v), mapped_n) / epsilon.powi(3);
        if volume_scale <= 1.0e-6 {
            return Err(refusal(
                node,
                &format!(
                    "deformation map folds near mesh face {index} (local volume scale {volume_scale:.3e})"
                ),
            ));
        }
    }
    Ok(())
}

fn refusal(node: &Node, message: &str) -> Fail {
    Box::new(ve_graph::Diagnostic::error("geometry.deformation_fold", message).at_node(&node.id))
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: [f64; 3]) -> Option<[f64; 3]> {
    let len = length(a);
    (len > 1.0e-12).then(|| scale(a, 1.0 / len))
}
