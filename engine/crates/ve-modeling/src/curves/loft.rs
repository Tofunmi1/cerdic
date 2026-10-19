//! Equal-correspondence lofts between convex planar profile sections.
use super::super::cache::Value;
use crate::{Fail, profile_ops};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh, Solid};

pub(super) fn build(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let sections = inputs.iter().collect::<Vec<_>>();
    if sections.len() < 2 || sections.len() > 64 {
        return Err(node_fail(
            node,
            "surface.section_count",
            "loft needs 2 to 64 profile sections",
            "link between 2 and 64 sections",
        ));
    }
    let ring = validate_sections(node, &sections)?;
    let spacing = profile_ops::number(node, "spacing")
        .map_err(|error| crate::operations::kernel_fail(node, &error))?;
    let steps = steps(node);
    let rows = super::loft_blend::rows(&sections, steps, context)?;
    check_rows(node, &rows)?;
    let step_count = f64::from(u32::try_from(steps).unwrap_or(1));
    build_mesh(node, &rows, ring, spacing / step_count, context)
}

/// Rows built per span: 1 keeps straight segments between sections, more blend them smoothly.
pub(super) fn steps(node: &Node) -> usize {
    match node.params.get("steps") {
        Some(ve_graph::Value::Int(value)) => usize::try_from(*value).unwrap_or(1).clamp(1, 8),
        _ => 1,
    }
}

fn area(points: &[[f64; 2]]) -> f64 {
    (0..points.len())
        .map(|index| {
            let (a, b) = (points[index], points[(index + 1) % points.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// A blended row can overshoot between sections; it must still be one convex loop wound like the first.
fn check_rows(node: &Node, rows: &[Vec<[f64; 2]>]) -> Result<(), Fail> {
    let winding = area(&rows[0]).signum();
    for (index, row) in rows.iter().enumerate() {
        if !convex(row) || area(row) * winding <= 0.0 {
            return Err(node_fail(
                node,
                "surface.unsupported_section",
                &format!(
                    "blended section {index} is not convex; lower steps or reduce the change between sections"
                ),
                "lower `steps`, or move the sections closer together",
            ));
        }
    }
    Ok(())
}

fn validate_sections(node: &Node, sections: &[&Rc<Value>]) -> Result<usize, Fail> {
    let Some(first) = sections.first().and_then(|value| value.loops.first()) else {
        return Err(node_fail(
            node,
            "surface.degenerate_section",
            "loft section has no closed loop",
            "give each section a closed profile, for example profile.circle or profile.polygon",
        ));
    };
    if first.len() < 3 || !convex(first) {
        return Err(node_fail(
            node,
            "surface.unsupported_section",
            "loft requires strictly convex sections",
            "use convex sections, for example profile.circle",
        ));
    }
    if sections.iter().any(|section| {
        section.loops.len() != 1
            || section.loops[0].len() != first.len()
            || !convex(&section.loops[0])
    }) {
        return Err(node_fail(
            node,
            "surface.incompatible_sections",
            "loft sections need one convex loop with matching point counts",
            "give every section the same point count, for example the same segments",
        ));
    }
    Ok(first.len())
}

fn build_mesh(
    node: &Node,
    rows: &[Vec<[f64; 2]>],
    ring: usize,
    spacing: f64,
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let mut vertices = Vec::with_capacity(rows.len().saturating_mul(ring));
    for (section_index, row) in rows.iter().enumerate() {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "loft cancelled while building sections",
            ));
        }
        let y = f64::from(
            u32::try_from(section_index)
                .map_err(|_| crate::fail("budget.triangles", "loft section index overflow"))?,
        ) * spacing;
        for point in row {
            vertices.push([to_f32(point[0]), to_f32(y), to_f32(point[1])]);
        }
    }
    let side_count = (rows.len() - 1).saturating_mul(ring).saturating_mul(6);
    let cap_count = ring.saturating_mul(6);
    let mut indices = Vec::with_capacity(side_count.saturating_add(cap_count));
    for section in 0..rows.len() - 1 {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "loft cancelled while stitching sections",
            ));
        }
        for side in 0..ring {
            let next = (side + 1) % ring;
            let a = index(section * ring + side)?;
            let b = index(section * ring + next)?;
            let c = index((section + 1) * ring + side)?;
            let d = index((section + 1) * ring + next)?;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    cap(&mut vertices, &mut indices, ring, true)?;
    cap(&mut vertices, &mut indices, ring, false)?;
    let solid = Solid::from_mesh(&Mesh {
        positions: vertices,
        indices,
    })
    .map_err(|error| crate::operations::kernel_fail(node, &error))?;
    let mesh = solid
        .to_mesh()
        .map_err(|error| crate::fail("kernel.failed", error.to_string()))?;
    Ok(Value::solid(mesh, solid.volume()))
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
    let center_index = index(vertices.len())?;
    let ring_size = f64::from(
        u32::try_from(ring)
            .map_err(|_| crate::fail("budget.triangles", "loft ring count overflow"))?,
    );
    let center = vertices[base..base + ring]
        .iter()
        .fold([0.0; 3], |mut sum, point| {
            for axis in 0..3 {
                sum[axis] += f64::from(point[axis]) / ring_size;
            }
            sum
        })
        .map(to_f32);
    vertices.push(center);
    for side in 0..ring {
        let current = index(base + side)?;
        let next = index(base + (side + 1) % ring)?;
        if start {
            indices.extend_from_slice(&[center_index, current, next]);
        } else {
            indices.extend_from_slice(&[center_index, next, current]);
        }
    }
    Ok(())
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

fn index(value: usize) -> Result<u32, Fail> {
    u32::try_from(value).map_err(|_| crate::fail("budget.triangles", "loft vertex index overflow"))
}

fn to_f32(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "kernel extent is bounded by graph parameter checks"
    )]
    {
        value as f32
    }
}

fn node_fail(node: &Node, code: &str, message: &str, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}
