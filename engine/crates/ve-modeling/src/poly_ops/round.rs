//! `poly.round_edges`: one node that rounds every hard edge, so a model need not select the edges and bevel them in two steps.

use std::rc::Rc;

use ve_geometry::{NetworkBevel, Topology};
use ve_graph::Node;
use ve_kernel::{BevelOffset, ExecutionContext};

use super::convert::value_from_poly;
use super::errors::poly_fail;
use super::params::{count, input_error, number};
use super::select::{dot, face_data};
use crate::{Fail, cache::Value};

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(poly) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let fail = |e: ve_geometry::GeometryError| poly_fail(node, &e, Some(poly));
    let topology = Topology::build(poly, context).map_err(fail)?;
    let data = face_data(poly);
    let threshold = number(node, "angle")?;
    let hard: Vec<(u32, u32)> = topology
        .edges()
        .iter()
        .filter(|(_, info)| match info.uses() {
            [first, second] => {
                dot(data[first.face].0, data[second.face].0)
                    .clamp(-1.0, 1.0)
                    .acos()
                    >= threshold
            }
            _ => false,
        })
        .map(|(&edge, _)| edge)
        .collect();
    if hard.is_empty() {
        return Err(super::errors::refuse(
            node,
            "selection.lost",
            "no edge is sharper than the angle; lower angle, or merge coplanar triangles first with poly.merge_coplanar".to_owned(),
        ));
    }
    let options = NetworkBevel {
        width: number(node, "width")?,
        segments: usize::try_from(count(node, "segments")?).unwrap_or(usize::MAX),
        profile: 0.5,
        offset: BevelOffset::Offset,
    };
    let result = ve_geometry::bevel_network(poly, &hard, &options, context)
        .map_err(|e| poly_fail(node, &e, Some(poly)))?;
    value_from_poly(node, result, input.material.clone(), context)
}
