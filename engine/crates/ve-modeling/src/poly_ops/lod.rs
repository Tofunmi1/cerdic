//! Progressive levels of detail built from the polygon decimator.

use std::rc::Rc;

use ve_geometry::{MeshOp, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::{input_error, number},
};
use crate::{Fail, cache::Value};

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(source) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let levels = small_integer(node, "levels")?;
    let level = small_integer(node, "level")?;
    let ratio = number(node, "ratio")?;
    if super::params::flag(node, "protect_uv_seams")?
        || super::params::flag(node, "protect_boundary")?
    {
        return Err(refuse(
            node,
            "poly.lod_protection_unsupported",
            "the Blender decimator cannot protect UV seams or boundaries; disable the requested protection options or use a decimator that supports them".to_owned(),
        ));
    }
    if levels == 0 || levels > 4 || level > levels {
        return Err(refuse(
            node,
            "poly.lod_level",
            "level must be 0..levels, and levels must be 1..4".to_owned(),
        ));
    }
    if level == 0 {
        return value_from_poly(node, (**source).clone(), input.material.clone(), context);
    }
    let level_power = i32::try_from(level)
        .map_err(|_| refuse(node, "poly.lod_level", "level must be 0..4".to_owned()))?;
    let face_count = u32::try_from(source.face_sizes().len()).map_err(|_| {
        refuse(
            node,
            "poly.lod_too_large",
            "mesh has too many faces to estimate collapse".to_owned(),
        )
    })?;
    let cumulative = ratio.powi(level_power);
    if cumulative * f64::from(face_count) < 4.0 {
        return Err(refuse(
            node,
            "poly.lod_collapse",
            "this ratio would collapse the mesh; use a higher ratio or fewer levels".to_owned(),
        ));
    }
    // Repeated application preserves the requested per-level ratio and makes each output independently addressable.
    let mut result: PolyMesh = (**source).clone();
    for _ in 0..level {
        result = ve_geometry::blender_edit(
            &result,
            &[],
            &[],
            &[],
            &MeshOp::Decimate {
                ratio,
                symmetry: None,
                triangulate: true,
            },
            context,
        )
        .map_err(|e| poly_fail(node, &e, Some(&result)))?;
    }
    value_from_poly(node, result, input.material.clone(), context)
}

fn small_integer(node: &Node, name: &str) -> Result<u32, Fail> {
    match number(node, name)? {
        0.0 => Ok(0),
        1.0 => Ok(1),
        2.0 => Ok(2),
        3.0 => Ok(3),
        4.0 => Ok(4),
        _ => Err(refuse(
            node,
            "poly.lod_level",
            format!("{name} must be a whole number from 0 to 4"),
        )),
    }
}
