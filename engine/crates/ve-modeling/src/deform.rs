//! Bounded vertex-space solid deformers with Manifold validation after displacement.
mod maps;
mod validate;

use crate::{Fail, cache::Value, fail, operations};
use maps::Map;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh, Solid};

/// Most triangles refinement may produce for one deformation.
const MAX_REFINED_TRIANGLES: usize = 100_000;

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    matches!(
        node.op.family().as_str(),
        "xform.twist"
            | "xform.taper"
            | "xform.bend"
            | "xform.shear"
            | "xform.wave"
            | "xform.noise"
            | "xform.lattice"
    )
    .then(|| build(node, inputs, context))
}

fn build(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "deformer needs a solid"))?;
    if input.mesh.is_empty() {
        return operations::inherited(node, inputs, Value::nothing());
    }
    if !input.frames.is_empty() {
        return Err(Box::new(
            ve_graph::Diagnostic::error(
                "socket.invalid_frame",
                "mesh deformations cannot preserve attached socket frames yet",
            )
            .at_node(&node.id),
        ));
    }
    let (minimum, maximum) = validate::bounds(&input.mesh)?;
    let map = Map::from_node(node)?;
    if let Some(angle) = map.bend_angle() {
        validate_bend(node, angle, minimum, maximum)?;
    }
    let tolerance = number(node, "tolerance")?;
    let mut mesh = refine(
        input.mesh.clone(),
        node,
        &map,
        minimum,
        maximum,
        tolerance,
        context,
    )?;
    let source = mesh.clone();
    for (index, point) in mesh.positions.iter_mut().enumerate() {
        if index % 512 == 0 && context.is_cancelled() {
            return Err(fail(
                "eval.cancelled",
                "deformation cancelled between vertices",
            ));
        }
        let source = point.map(f64::from);
        let output = map.apply(source, minimum, maximum);
        *point = to_f32(output)?;
    }
    validate::orientation(&source, &mesh, node, &map, minimum, maximum, context)?;
    if context.is_cancelled() {
        return Err(fail(
            "eval.cancelled",
            "deformation cancelled before solid validation",
        ));
    }
    let solid = Solid::from_mesh(&mesh).map_err(|error| operations::kernel_fail(node, &error))?;
    let mesh = solid
        .to_mesh()
        .map_err(|error| fail("kernel.failed", error.to_string()))?;
    operations::inherited(node, inputs, Value::solid(mesh, solid.volume()))
}

fn refine(
    mut mesh: Mesh,
    node: &Node,
    map: &Map,
    minimum: [f64; 3],
    maximum: [f64; 3],
    tolerance: f64,
    context: &ExecutionContext,
) -> Result<Mesh, Fail> {
    for _ in 0..map.passes() {
        let (edges, _) = refinement_edges(&mesh, map, minimum, maximum, tolerance, context)?;
        if edges.is_empty() {
            return Ok(mesh);
        }
        mesh = refine_local(mesh, &edges, context)?;
        if mesh.indices.len() / 3 > MAX_REFINED_TRIANGLES {
            return Err(Box::new(
                ve_graph::Diagnostic::error(
                    "budget.triangles",
                    format!("refining for this deformation passes {MAX_REFINED_TRIANGLES} triangles; raise tolerance or soften the deformation"),
                )
                .at_node(&node.id),
            ));
        }
    }
    let (edges, observed) = refinement_edges(&mesh, map, minimum, maximum, tolerance, context)?;
    if !edges.is_empty() {
        return Err(Box::new(
            ve_graph::Diagnostic::error(
                "geometry.deformation_tolerance",
                format!("bounded refinement leaves sampled chord deviation {observed:.6} above tolerance {tolerance:.6}; raise tolerance or soften the deformation"),
            )
            .at_node(&node.id)
            .with_limit(tolerance, observed),
        ));
    }
    Ok(mesh)
}

fn refine_local(
    mut mesh: Mesh,
    split_edges: &HashSet<(u32, u32)>,
    context: &ExecutionContext,
) -> Result<Mesh, Fail> {
    let mut next_indices = Vec::with_capacity(mesh.indices.len().saturating_mul(4));
    let mut midpoints = BTreeMap::new();
    for (triangle_index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
        if triangle_index % 1024 == 0 && context.is_cancelled() {
            return Err(fail("eval.cancelled", "deformation refinement cancelled"));
        }
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
        let mask = u8::from(split_edges.contains(&edge_key(a, b)))
            | (u8::from(split_edges.contains(&edge_key(b, c))) << 1)
            | (u8::from(split_edges.contains(&edge_key(c, a))) << 2);
        let ab = if mask & 1 != 0 {
            Some(edge_midpoint(&mut mesh.positions, &mut midpoints, a, b)?)
        } else {
            None
        };
        let bc = if mask & 2 != 0 {
            Some(edge_midpoint(&mut mesh.positions, &mut midpoints, b, c)?)
        } else {
            None
        };
        let ca = if mask & 4 != 0 {
            Some(edge_midpoint(&mut mesh.positions, &mut midpoints, c, a)?)
        } else {
            None
        };
        split_triangle(&mut next_indices, [a, b, c], [ab, bc, ca]);
    }
    mesh.indices = next_indices;
    Ok(mesh)
}

fn refinement_edges(
    mesh: &Mesh,
    map: &Map,
    minimum: [f64; 3],
    maximum: [f64; 3],
    tolerance: f64,
    context: &ExecutionContext,
) -> Result<(HashSet<(u32, u32)>, f64), Fail> {
    let mut split_edges = HashSet::new();
    let mut maximum_error: f64 = 0.0;
    for (triangle_index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
        if triangle_index % 1024 == 0 && context.is_cancelled() {
            return Err(fail(
                "eval.cancelled",
                "deformation tolerance check cancelled",
            ));
        }
        let ids = [triangle[0], triangle[1], triangle[2]];
        let points = ids.map(|id| mesh.positions[id as usize].map(f64::from));
        let mapped = points.map(|point| map.apply(point, minimum, maximum));
        let mut centroid = [0.0; 3];
        for axis in 0..3 {
            centroid[axis] = (points[0][axis] + points[1][axis] + points[2][axis]) / 3.0;
        }
        let mapped_centroid = map.apply(centroid, minimum, maximum);
        let centroid_error = (0..3)
            .map(|axis| {
                (mapped_centroid[axis]
                    - (mapped[0][axis] + mapped[1][axis] + mapped[2][axis]) / 3.0)
                    .powi(2)
            })
            .sum::<f64>()
            .sqrt();
        maximum_error = maximum_error.max(centroid_error);
        let mut errors = [0.0; 3];
        for (edge, [i, j]) in [[0, 1], [1, 2], [2, 0]].into_iter().enumerate() {
            let middle = [0, 1, 2].map(|axis| f64::midpoint(points[i][axis], points[j][axis]));
            let actual = map.apply(middle, minimum, maximum);
            errors[edge] = (0..3)
                .map(|axis| {
                    (actual[axis] - f64::midpoint(mapped[i][axis], mapped[j][axis])).powi(2)
                })
                .sum::<f64>()
                .sqrt();
            maximum_error = maximum_error.max(errors[edge]);
        }
        if centroid_error > tolerance {
            for edge in [[0, 1], [1, 2], [2, 0]] {
                split_edges.insert(edge_key(ids[edge[0]], ids[edge[1]]));
            }
        } else {
            for (edge, [i, j]) in [[0, 1], [1, 2], [2, 0]].into_iter().enumerate() {
                if errors[edge] > tolerance {
                    split_edges.insert(edge_key(ids[i], ids[j]));
                }
            }
        }
    }
    Ok((split_edges, maximum_error))
}

fn split_triangle(out: &mut Vec<u32>, [a, b, c]: [u32; 3], [ab, bc, ca]: [Option<u32>; 3]) {
    match (ab, bc, ca) {
        (None, None, None) => out.extend_from_slice(&[a, b, c]),
        (Some(ab), None, None) => out.extend_from_slice(&[a, ab, c, ab, b, c]),
        (None, Some(bc), None) => out.extend_from_slice(&[b, bc, a, bc, c, a]),
        (None, None, Some(ca)) => out.extend_from_slice(&[c, ca, b, ca, a, b]),
        (Some(ab), Some(bc), None) => out.extend_from_slice(&[b, bc, ab, a, ab, c, ab, bc, c]),
        (None, Some(bc), Some(ca)) => out.extend_from_slice(&[c, ca, bc, a, b, bc, a, bc, ca]),
        (Some(ab), None, Some(ca)) => out.extend_from_slice(&[a, ab, ca, b, c, ca, b, ca, ab]),
        (Some(ab), Some(bc), Some(ca)) => {
            out.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
        }
    }
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn edge_midpoint(
    positions: &mut Vec<[f32; 3]>,
    cache: &mut BTreeMap<(u32, u32), u32>,
    first: u32,
    second: u32,
) -> Result<u32, Fail> {
    let edge = (first.min(second), first.max(second));
    if let Some(index) = cache.get(&edge) {
        return Ok(*index);
    }
    let first = usize::try_from(edge.0)
        .map_err(|_| fail("budget.triangles", "refined vertex index overflow"))?;
    let second = usize::try_from(edge.1)
        .map_err(|_| fail("budget.triangles", "refined vertex index overflow"))?;
    let point =
        [0, 1, 2].map(|axis| f32::midpoint(positions[first][axis], positions[second][axis]));
    let index = u32::try_from(positions.len())
        .map_err(|_| fail("budget.triangles", "refined vertex index overflow"))?;
    positions.push(point);
    cache.insert(edge, index);
    Ok(index)
}

fn validate_bend(
    node: &Node,
    angle: f64,
    minimum: [f64; 3],
    maximum: [f64; 3],
) -> Result<(), Fail> {
    let length = maximum[1] - minimum[1];
    let half_width = (maximum[0] - minimum[0]) / 2.0;
    if angle.abs() > 1.0e-12 && length / angle.abs() <= half_width {
        return Err(Box::new(
            ve_graph::Diagnostic::error(
                "curve.bend_curvature",
                format!(
                    "bend radius {:.6} must exceed half-width {half_width:.6}",
                    length / angle.abs()
                ),
            )
            .at_node(&node.id),
        ));
    }
    Ok(())
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| {
            fail(
                "param.type",
                format!("deformer parameter {name} is not numeric"),
            )
        })
}

fn to_f32(point: [f64; 3]) -> Result<[f32; 3], Fail> {
    if point
        .iter()
        .any(|coordinate| !coordinate.is_finite() || coordinate.abs() > 1.0e6)
    {
        return Err(fail(
            "geometry.deformation_range",
            "deformation exceeds the supported extent",
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "deformed coordinates are bounded before conversion"
    )]
    Ok(point.map(|coordinate| coordinate as f32))
}
