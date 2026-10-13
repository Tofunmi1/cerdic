//! Proper rigid alignment about an explicit pivot.
use crate::operations::vector;
use glam::{DMat4, DQuat, DVec3};
use ve_graph::Node;
use ve_kernel::KernelError;

pub(crate) fn alignment(node: &Node) -> Result<[f64; 12], KernelError> {
    let direction = |name| {
        DVec3::from_array(vector(node, name)?)
            .try_normalize()
            .ok_or(KernelError::InvalidArgument(
                "alignment direction must be nonzero",
            ))
    };
    let from = direction("from")?;
    let to = direction("to")?;
    let cross = from.cross(to);
    let dot = from.dot(to).clamp(-1.0, 1.0);
    // atan2 retains small angles that acos(dot) loses; exact collinearity is handled separately.
    let rotation = if let Some(axis) = cross.try_normalize() {
        DQuat::from_axis_angle(axis, cross.length().atan2(dot))
    } else if dot >= 0.0 {
        DQuat::IDENTITY
    } else {
        let smallest = if from.x.abs() <= from.y.abs() && from.x.abs() <= from.z.abs() {
            DVec3::X
        } else if from.y.abs() <= from.z.abs() {
            DVec3::Y
        } else {
            DVec3::Z
        };
        DQuat::from_axis_angle(from.cross(smallest).normalize(), std::f64::consts::PI)
    };
    let pivot = DVec3::from_array(vector(node, "pivot")?);
    let matrix = DMat4::from_rotation_translation(rotation, pivot - rotation * pivot);
    Ok(rows(matrix))
}

pub(crate) fn rows(matrix: DMat4) -> [f64; 12] {
    let c = matrix.to_cols_array_2d();
    [
        c[0][0], c[1][0], c[2][0], c[3][0], c[0][1], c[1][1], c[2][1], c[3][1], c[0][2], c[1][2],
        c[2][2], c[3][2],
    ]
}
