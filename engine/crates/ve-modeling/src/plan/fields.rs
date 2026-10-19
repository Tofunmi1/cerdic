//! Field tape and lattice costs checked before constructing any geometry.
use crate::{Fail, Quality, budget};
use std::collections::BTreeMap;
use ve_graph::{Node, NodeId, Value};

const MAX_CELLS: usize = 2_000_000;
const MAX_TAPE: usize = 4_096;
const MAX_WORK: usize = 64_000_000;
const MAX_PADDED: usize = 2_097_152;
const MAX_CELLS_F64: f64 = 2_000_000.0;
const TRIANGLES_PER_PRIMITIVE_CELL: usize = 10;

#[derive(Clone, Copy, Default)]
pub(super) struct Complexity {
    instructions: usize,
    primitives: usize,
    /// Upper bound of the field's surface area in square metres, or infinity when it is not bounded (a plane).
    area: f64,
}

/// Surface area of one primitive, or infinity when it has none that is bounded.
fn primitive_area(node: &Node, family: &str) -> f64 {
    let num = |name: &str| node.params.get(name).and_then(Value::as_f64);
    let vec3 = |name: &str| match node.params.get(name) {
        Some(Value::List(values)) if values.len() == 3 => {
            let parts: Vec<f64> = values.iter().filter_map(Value::as_f64).collect();
            (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
        }
        _ => None,
    };
    let pi = std::f64::consts::PI;
    let area = match family {
        "field.sphere" => num("radius").map(|r| 4.0 * pi * r * r),
        "field.ellipsoid" => vec3("radii").map(|[a, b, c]| {
            // Thomsen's approximation, within about 1% for any ellipsoid.
            let p = 1.6075;
            4.0 * pi
                * ((a.powf(p) * b.powf(p) + a.powf(p) * c.powf(p) + b.powf(p) * c.powf(p)) / 3.0)
                    .powf(1.0 / p)
        }),
        "field.box" => vec3("half").map(|[a, b, c]| 8.0 * (a * b + a * c + b * c)),
        "field.torus" => num("major")
            .zip(num("tube"))
            .map(|(big, small)| 4.0 * pi * pi * big * small),
        "field.capsule" => num("radius")
            .zip(num("half_length"))
            .map(|(r, h)| 4.0 * pi * r * r + 4.0 * pi * r * h),
        "field.cylinder" => num("radius")
            .zip(num("half_height"))
            .map(|(r, h)| 4.0 * pi * r * h + 2.0 * pi * r * r),
        "field.tube" => match (node.params.get("points"), num("radius")) {
            (Some(Value::List(points)), Some(radius)) => {
                let end = num("end_radius").filter(|end| *end > 0.0).unwrap_or(radius);
                let mean = f64::midpoint(radius, end);
                let big = radius.max(end);
                let coordinates: Vec<[f64; 3]> = points
                    .iter()
                    .filter_map(|point| match point {
                        Value::List(c) if c.len() == 3 => {
                            Some([c[0].as_f64()?, c[1].as_f64()?, c[2].as_f64()?])
                        }
                        _ => None,
                    })
                    .collect();
                let length: f64 = coordinates
                    .windows(2)
                    .map(|p| {
                        ((p[1][0] - p[0][0]).powi(2)
                            + (p[1][1] - p[0][1]).powi(2)
                            + (p[1][2] - p[0][2]).powi(2))
                        .sqrt()
                    })
                    .sum();
                // The side, plus a cap at each end and slack for joints between segments.
                let joints = f64::from(u32::try_from(coordinates.len()).unwrap_or(u32::MAX));
                Some(
                    2.0 * pi * mean * length + 4.0 * pi * big * big + 2.0 * pi * big * big * joints,
                )
            }
            _ => None,
        },
        _ => None,
    };
    area.filter(|area| area.is_finite())
        .unwrap_or(f64::INFINITY)
}

pub(super) fn complexity(
    node: &Node,
    previous: &BTreeMap<NodeId, Complexity>,
) -> Option<Complexity> {
    let family = node.op.family().as_str();
    if !family.starts_with("field.") || family == "field.to_solid" {
        return None;
    }
    let (instructions, primitives) = if family == "field.tube" {
        let points = match node.params.get("points") {
            Some(Value::List(points)) => points.len(),
            _ => 2,
        };
        (
            points.saturating_mul(3).saturating_sub(3),
            points.saturating_sub(1),
        )
    } else {
        (
            1,
            usize::from(matches!(
                family,
                "field.sphere"
                    | "field.ellipsoid"
                    | "field.plane"
                    | "field.box"
                    | "field.torus"
                    | "field.capsule"
                    | "field.cylinder"
            )),
        )
    };
    let mut value = Complexity {
        instructions,
        primitives,
        // Only primitives have a surface of their own; combiners and transforms add up their inputs' below.
        area: if matches!(
            family,
            "field.sphere"
                | "field.ellipsoid"
                | "field.plane"
                | "field.box"
                | "field.torus"
                | "field.capsule"
                | "field.cylinder"
                | "field.tube"
        ) {
            primitive_area(node, family)
        } else {
            0.0
        },
    };
    let mut inputs_area = 0.0;
    for link in node.inputs.values().flatten() {
        if let Some(input) = previous.get(&link.node) {
            value.instructions = value.instructions.saturating_add(input.instructions);
            value.primitives = value.primitives.saturating_add(input.primitives);
            inputs_area += input.area;
        }
    }
    // Combiners and transforms: the surface of a union or blend is at most the sum of its inputs' (a blend's small
    // extra is inside the margin of the final estimate); a scale grows it by the factor squared, a mirror doubles it
    // per axis, offset and shell may double it.
    let factor = |name: &str| node.params.get(name).and_then(Value::as_f64).unwrap_or(1.0);
    let transform = match family {
        "field.scale" => factor("factor").max(1.0).powi(2),
        "field.offset" | "field.shell" => 2.0,
        "field.mirror" => match node.params.get("axes") {
            Some(Value::List(axes)) => 2.0_f64.powi(
                i32::try_from(
                    axes.iter()
                        .filter_map(Value::as_f64)
                        .filter(|a| *a > 0.5)
                        .count(),
                )
                .unwrap_or(3),
            ),
            _ => 8.0,
        },
        _ => 1.0,
    };
    value.area += inputs_area * transform;
    if family == "field.repeat" {
        value.primitives = value.primitives.saturating_mul(repeat_multiplicity(node));
        value.area *= f64::from(u32::try_from(repeat_multiplicity(node)).unwrap_or(u32::MAX));
    }
    Some(value)
}

fn repeat_multiplicity(node: &Node) -> usize {
    const MAX_COPIES_PER_AXIS: usize = 33;
    let Some(Value::List(counts)) = node.params.get("count") else {
        return MAX_COPIES_PER_AXIS.pow(3);
    };
    if counts.len() != 3 {
        return MAX_COPIES_PER_AXIS.pow(3);
    }
    counts.iter().fold(1_usize, |copies, value| {
        let Some(count) = value.as_f64() else {
            return copies.saturating_mul(MAX_COPIES_PER_AXIS);
        };
        if !count.is_finite()
            || !(0.0..=16.0).contains(&count)
            || count.fract().to_bits() != 0.0_f64.to_bits()
        {
            return copies.saturating_mul(MAX_COPIES_PER_AXIS);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "repeat count is validated as an integer from 0 to 16"
        )]
        let axis_copies = (count as usize).saturating_mul(2).saturating_add(1);
        copies.saturating_mul(axis_copies)
    })
}

pub(super) fn mesh_estimate(
    node: &Node,
    complexity: Complexity,
    quality: Quality,
) -> Result<usize, Fail> {
    if complexity.instructions > MAX_TAPE {
        return Err(budget::exceeded(
            "budget.field_tape",
            MAX_TAPE,
            complexity.instructions,
        ));
    }
    let minimum = vector(node, "min")?;
    let maximum = vector(node, "max")?;
    let edge = number(node, "edge_length")? * quality.field_edge_scale();
    let mut samples = [0_usize; 3];
    for axis in 0..3 {
        let ratio = (maximum[axis] - minimum[axis]) / edge;
        if !ratio.is_finite() || ratio < 1.0 {
            return Err(crate::fail(
                "field.invalid_parameter",
                "invalid field grid ratio",
            ));
        }
        if ratio > MAX_CELLS_F64 {
            return Err(budget::exceeded(
                "budget.field_cells",
                MAX_CELLS,
                MAX_CELLS + 1,
            ));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the positive ratio is bounded by the field sample cap"
        )]
        let axis_samples = ratio as usize + 1;
        samples[axis] = axis_samples;
    }
    let cells = samples[0]
        .checked_mul(samples[1])
        .and_then(|value| value.checked_mul(samples[2]))
        .unwrap_or(usize::MAX);
    if cells > MAX_CELLS {
        return Err(budget::exceeded("budget.field_cells", MAX_CELLS, cells));
    }
    let work = cells.saturating_mul(complexity.instructions);
    if work > MAX_WORK {
        return Err(budget::exceeded("budget.field_ops", MAX_WORK, work));
    }
    let padded = samples.map(|count| (count + 2).next_power_of_two());
    let padded_slots = padded[0]
        .checked_mul(padded[1])
        .and_then(|value| value.checked_mul(padded[2]))
        .unwrap_or(usize::MAX);
    if padded_slots > MAX_PADDED {
        return Err(budget::exceeded(
            "budget.field_cells",
            MAX_PADDED,
            padded_slots,
        ));
    }
    let max_axis = samples.into_iter().max().unwrap_or(0);
    let by_primitives = complexity
        .primitives
        .saturating_mul(TRIANGLES_PER_PRIMITIVE_CELL)
        .saturating_mul(max_axis)
        .saturating_mul(max_axis);
    // A bounded surface of area A meshed on a grid of spacing e has about 2 A / e^2 triangles (marching tetrahedra);
    // 4 A / e^2 leaves headroom. An unbounded one (a plane) keeps the per-primitive bound.
    let by_area = complexity.area * 4.0 / (edge * edge);
    if by_area.is_finite() {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the area bound is finite and non-negative, and the result is clamped by the larger bound"
        )]
        let bound = by_area.ceil() as usize;
        return Ok(bound.min(by_primitives));
    }
    Ok(by_primitives)
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Value::as_f64)
        .ok_or_else(|| crate::fail("field.invalid_parameter", format!("missing {name}")))
}

fn vector(node: &Node, name: &str) -> Result<[f64; 3], Fail> {
    let Some(Value::List(values)) = node.params.get(name) else {
        return Err(crate::fail(
            "field.invalid_parameter",
            format!("missing {name}"),
        ));
    };
    let components = values.iter().map(Value::as_f64).collect::<Option<Vec<_>>>();
    let Some(components) = components.filter(|values| values.len() == 3) else {
        return Err(crate::fail(
            "field.invalid_parameter",
            format!("invalid {name}"),
        ));
    };
    Ok([components[0], components[1], components[2]])
}
