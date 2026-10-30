//! Deterministic tree-to-register-tape lowering.
use crate::GeometryError;
use crate::field::MAX_TUBE_POINTS;
use crate::field::model::{Combine, Field, FieldExpr, Transform};
use ve_kernel::{FieldInstruction, FieldOp};

/// Kernel-supported maximum number of tape instructions.
pub const MAX_INSTRUCTIONS: usize = ve_kernel::FIELD_MAX_INSTRUCTIONS;

/// Lowers an expression to a topological register tape.
///
/// # Errors
/// Returns an error if lowering exceeds the bounded instruction count.
pub fn compile(field: &Field) -> Result<Vec<FieldInstruction>, GeometryError> {
    let mut tape = Vec::new();
    compile_expr(&field.expr, &mut tape)?;
    Ok(tape)
}

fn compile_expr(expr: &FieldExpr, tape: &mut Vec<FieldInstruction>) -> Result<u32, GeometryError> {
    if let Some(instruction) = compile_primitive(expr) {
        return append(tape, instruction);
    }
    let instruction = match expr {
        FieldExpr::Tube {
            points,
            radius,
            end_radius,
        } => return compile_tube(points, *radius, *end_radius, tape),
        FieldExpr::Sphere { .. }
        | FieldExpr::Ellipsoid { .. }
        | FieldExpr::Box { .. }
        | FieldExpr::Torus { .. }
        | FieldExpr::Capsule { .. }
        | FieldExpr::Cylinder { .. }
        | FieldExpr::Plane { .. } => unreachable!("primitive expressions are handled above"),
        FieldExpr::Repeat {
            period,
            count,
            child,
        } => {
            let child = compile_expr(child, tape)?;
            let mut instruction = FieldInstruction::unary(
                FieldOp::Repeat,
                child,
                [period[0], period[1], period[2], 0.0],
            );
            instruction.input_b =
                u32::from(count[0]) | (u32::from(count[1]) << 8) | (u32::from(count[2]) << 16);
            instruction
        }
        FieldExpr::Combine { op, a, b } => {
            let a = compile_expr(a, tape)?;
            let b = compile_expr(b, tape)?;
            let op = match op {
                Combine::Union => FieldOp::Union,
                Combine::Intersection => FieldOp::Intersection,
                Combine::Difference => FieldOp::Difference,
            };
            FieldInstruction::binary(op, a, b, [0.0; 4])
        }
        FieldExpr::SmoothUnion { a, b, radius } => {
            let a = compile_expr(a, tape)?;
            let b = compile_expr(b, tape)?;
            FieldInstruction::binary(FieldOp::SmoothUnion, a, b, [*radius, 0.0, 0.0, 0.0])
        }
        FieldExpr::Transform { op, child } => {
            let child = compile_expr(child, tape)?;
            match op {
                Transform::Move(delta) => FieldInstruction::unary(
                    FieldOp::Move,
                    child,
                    [delta[0], delta[1], delta[2], 0.0],
                ),
                Transform::Scale(factor) => {
                    FieldInstruction::unary(FieldOp::Scale, child, [*factor, 0.0, 0.0, 0.0])
                }
                Transform::Mirror(axes) => {
                    let mask =
                        u32::from(axes[0]) | (u32::from(axes[1]) << 1) | (u32::from(axes[2]) << 2);
                    FieldInstruction::unary(
                        FieldOp::Mirror,
                        child,
                        [f64::from(mask), 0.0, 0.0, 0.0],
                    )
                }
            }
        }
        FieldExpr::Offset { child, amount } => {
            let child = compile_expr(child, tape)?;
            FieldInstruction::unary(FieldOp::Offset, child, [*amount, 0.0, 0.0, 0.0])
        }
        FieldExpr::Shell { child, half_width } => {
            let child = compile_expr(child, tape)?;
            FieldInstruction::unary(FieldOp::Shell, child, [*half_width, 0.0, 0.0, 0.0])
        }
        FieldExpr::Noise {
            child,
            amplitude,
            scale,
            octaves,
            persistence,
            seed,
            ridged,
        } => {
            let child = compile_expr(child, tape)?;
            let mut instruction = FieldInstruction::unary(
                FieldOp::Noise,
                child,
                [
                    *amplitude,
                    *scale,
                    *persistence,
                    f64::from(*octaves) + if *ridged { 8.0 } else { 0.0 },
                ],
            );
            instruction.input_b = *seed;
            instruction
        }
        FieldExpr::Warp {
            child,
            kind,
            axis,
            toward,
            from,
            to,
            start,
            end,
        } => {
            let child = compile_expr(child, tape)?;
            let mut instruction =
                FieldInstruction::unary(FieldOp::Warp, child, [*from, *to, *start, *end]);
            instruction.input_b =
                u32::from(*kind) | (u32::from(*axis) << 8) | (u32::from(*toward) << 16);
            instruction
        }
    };
    append(tape, instruction)
}

fn compile_primitive(expr: &FieldExpr) -> Option<FieldInstruction> {
    Some(match expr {
        FieldExpr::Sphere { center, radius } => {
            FieldInstruction::leaf(FieldOp::Sphere, [center[0], center[1], center[2], *radius])
        }
        FieldExpr::Ellipsoid { radii } => {
            FieldInstruction::leaf(FieldOp::Ellipsoid, [radii[0], radii[1], radii[2], 0.0])
        }
        FieldExpr::Box { half, rounding } => {
            FieldInstruction::leaf(FieldOp::Box, [half[0], half[1], half[2], *rounding])
        }
        FieldExpr::Torus { major, tube } => {
            FieldInstruction::leaf(FieldOp::Torus, [*major, *tube, 0.0, 0.0])
        }
        FieldExpr::Capsule {
            radius,
            half_length,
        } => FieldInstruction::leaf(FieldOp::Capsule, [*radius, *half_length, 0.0, 0.0]),
        FieldExpr::Cylinder {
            radius,
            half_height,
        } => FieldInstruction::leaf(FieldOp::Cylinder, [*radius, *half_height, 0.0, 0.0]),
        FieldExpr::Plane { normal, offset } => {
            FieldInstruction::leaf(FieldOp::Plane, [normal[0], normal[1], normal[2], *offset])
        }
        _ => return None,
    })
}

fn compile_tube(
    points: &[[f64; 3]],
    radius: f64,
    end_radius: f64,
    tape: &mut Vec<FieldInstruction>,
) -> Result<u32, GeometryError> {
    let radii = crate::field::model::tube_segment_radii(points, radius, end_radius);
    if points.len() > MAX_TUBE_POINTS {
        return Err(GeometryError::FieldTapeLimit(points.len()));
    }
    let mut point_ids = Vec::with_capacity(points.len());
    for point in points {
        point_ids.push(append(
            tape,
            FieldInstruction::leaf(FieldOp::TubePoint, [point[0], point[1], point[2], 0.0]),
        )?);
    }
    let mut result = None;
    for (pair, ends) in point_ids.windows(2).zip(&radii) {
        let segment = append(
            tape,
            FieldInstruction::binary(
                FieldOp::TubeSegment,
                pair[0],
                pair[1],
                [ends[0], ends[1], 0.0, 0.0],
            ),
        )?;
        result = Some(if let Some(previous) = result {
            append(
                tape,
                FieldInstruction::binary(FieldOp::Union, previous, segment, [0.0; 4]),
            )?
        } else {
            segment
        });
    }
    result.ok_or(GeometryError::FieldTapeLimit(tape.len()))
}

fn append(
    tape: &mut Vec<FieldInstruction>,
    instruction: FieldInstruction,
) -> Result<u32, GeometryError> {
    if tape.len() >= MAX_INSTRUCTIONS {
        return Err(GeometryError::FieldTapeLimit(tape.len() + 1));
    }
    let index = u32::try_from(tape.len()).map_err(|_| GeometryError::FieldTapeLimit(tape.len()))?;
    tape.push(instruction);
    Ok(index)
}
