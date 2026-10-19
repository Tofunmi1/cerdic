//! Moving, turning and scaling a polygon mesh. The solid kernel cannot do this to an open surface (a leaf card, a flag, a ground patch),
//! so a polygon value takes its own path: the faces are moved, then the triangles, UVs and volume are rebuilt from them.

use std::rc::Rc;

use glam::{DMat4, DVec3};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{convert::value_from_poly, errors::poly_fail};
use crate::{
    Fail,
    cache::Value,
    operations::{inherited, kernel_fail, vector},
};

/// Whether `family` on these inputs is a polygon mesh being moved, turned or scaled.
pub(super) fn applies(family: &str, inputs: &[Rc<Value>]) -> bool {
    matches!(family, "xform.move" | "xform.rotate" | "xform.scale")
        && inputs.first().is_some_and(|input| input.poly.is_some())
}

/// The transform a node asks for, as a matrix (rotation in radians, X then Y then Z, like the solid transform).
fn matrix(node: &Node, family: &str) -> Result<DMat4, Fail> {
    let read = |name: &str| vector(node, name).map_err(|error| kernel_fail(node, &error));
    Ok(match family {
        "xform.move" => DMat4::from_translation(DVec3::from(read("by")?)),
        "xform.rotate" => crate::instances::rotation(read("angles")?),
        _ => DMat4::from_scale(DVec3::from(read("by")?)),
    })
}

pub(super) fn run(
    node: &Node,
    family: &str,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let (Some(input), Some(poly)) = (inputs.first(), inputs.first().and_then(|i| i.poly.as_ref()))
    else {
        return Ok(Value::nothing());
    };
    let matrix = matrix(node, family)?;
    let moved = poly
        .transformed(|p| {
            let at = matrix.transform_point3(DVec3::new(
                f64::from(p[0]),
                f64::from(p[1]),
                f64::from(p[2]),
            ));
            #[expect(
                clippy::cast_possible_truncation,
                reason = "positions are f32 in the mesh"
            )]
            [at.x as f32, at.y as f32, at.z as f32]
        })
        .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    let mut value = value_from_poly(node, moved, input.material.clone(), context)?;
    // The same triangles in the same order, so the materials of its runs still fit.
    if value.mesh.triangle_count() == input.mesh.triangle_count() {
        value.regions.clone_from(&input.regions);
    }
    inherited(node, inputs, value)
}
