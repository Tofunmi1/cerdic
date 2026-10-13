//! Bounded frame propagation through transforms and unioning patterns.
use super::{Frame, Frames, MAX_FRAMES};
use crate::{
    Fail, budget, fail,
    operations::{kernel_fail, vector},
};
use glam::{DMat3, DMat4, DQuat, DVec3};
use ve_graph::Node;

pub(super) fn orientation(matrix: DMat4) -> Result<DQuat, Fail> {
    let mut basis = DMat3::from_mat4(matrix);
    if !basis.is_finite() || basis.determinant() <= 0.0 {
        return Err(fail(
            "socket.invalid_frame",
            "socket orientation must have positive determinant",
        ));
    }
    // Polar iteration strips nonuniform scale/shear to the nearest proper rotation.
    for _ in 0..64 {
        let next = (basis + basis.inverse().transpose()) * 0.5;
        let error = (next - basis)
            .to_cols_array()
            .into_iter()
            .map(f64::abs)
            .fold(0.0, f64::max);
        basis = next;
        if error < 1e-12 {
            break;
        }
    }
    let check = basis.transpose() * basis - DMat3::IDENTITY;
    if !basis.is_finite() || check.to_cols_array().into_iter().any(|n| n.abs() > 1e-8) {
        return Err(fail(
            "socket.invalid_frame",
            "socket orientation cannot be made rigid",
        ));
    }
    Ok(DQuat::from_mat3(&basis).normalize())
}

pub(crate) fn placed(frames: &Frames, matrix: DMat4) -> Result<Frames, Fail> {
    // A pure mirror conjugates by the same reflection. General reflected placement batches use
    // the renderer's canonical X correction to keep prototype socket orientations proper.
    let right = if matrix.determinant() < 0.0 {
        let diagonal = DMat4::from_scale(DVec3::new(
            matrix.x_axis.x,
            matrix.y_axis.y,
            matrix.z_axis.z,
        ));
        if (matrix - diagonal)
            .to_cols_array()
            .into_iter()
            .all(|v| v.abs() < 1e-12)
        {
            diagonal
        } else {
            DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0))
        }
    } else {
        DMat4::IDENTITY
    };
    frames
        .iter()
        .map(|(name, frame)| {
            Ok((
                name.clone(),
                Frame::from_matrix(matrix * frame.matrix() * right)?,
            ))
        })
        .collect()
}

pub(crate) fn reflected(frames: &Frames, reflection: DMat4) -> Result<Frames, Fail> {
    let mut result = Frames::new();
    for (name, frame) in placed(frames, reflection)? {
        let name = format!("mirror.{name}");
        if result.insert(name.clone(), frame).is_some() {
            return Err(fail(
                "socket.duplicate",
                format!("generated socket {name:?} collides"),
            ));
        }
    }
    Ok(result)
}

pub(crate) fn copies(frames: &Frames, matrices: &[DMat4], mirror: bool) -> Result<Frames, Fail> {
    let count = frames
        .len()
        .checked_mul(matrices.len())
        .ok_or_else(|| fail("budget.sockets", "socket product overflow"))?;
    if count > MAX_FRAMES {
        return Err(budget::exceeded("budget.sockets", MAX_FRAMES, count));
    }
    let mut result = Frames::new();
    for (i, matrix) in matrices.iter().enumerate() {
        for (name, frame) in placed(frames, *matrix)? {
            let name = if i == 0 {
                name
            } else if mirror {
                format!("mirror.{name}")
            } else {
                format!("copy{i}.{name}")
            };
            if result.insert(name.clone(), frame).is_some() {
                return Err(fail(
                    "socket.duplicate",
                    format!("generated socket {name:?} collides with an existing name"),
                ));
            }
        }
    }
    Ok(result)
}

pub(crate) fn propagate(node: &Node, frames: &Frames) -> Result<Frames, Fail> {
    if frames.is_empty() {
        return Ok(Frames::new());
    }
    let vector = |name| vector(node, name).map_err(|e| kernel_fail(node, &e));
    let kind = node.op.family().as_str();
    let matrix = match kind {
        "xform.move" => DMat4::from_translation(DVec3::from_array(vector("by")?)),
        "xform.rotate" => crate::instances::rotation(vector("angles")?),
        "xform.scale" => DMat4::from_scale(DVec3::from_array(vector("by")?)),
        "xform.align" => {
            let r = crate::assembly::alignment(node).map_err(|e| kernel_fail(node, &e))?;
            DMat4::from_cols_array(&[
                r[0], r[4], r[8], 0.0, r[1], r[5], r[9], 0.0, r[2], r[6], r[10], 0.0, r[3], r[7],
                r[11], 1.0,
            ])
        }
        "xform.mirror" | "xform.array" | "xform.radial" | "xform.repeat" => {
            let matrices = crate::instances::placements(node).map_err(|e| kernel_fail(node, &e))?;
            return copies(frames, &matrices, kind == "xform.mirror");
        }
        _ => return Ok(frames.clone()),
    };
    placed(frames, matrix)
}
