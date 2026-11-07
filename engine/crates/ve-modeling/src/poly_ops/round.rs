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
            "no edge is sharper than the angle; lower angle (it is in radians: 0.5 is about 30 degrees)".to_owned(),
        ));
    }
    let requested = usize::try_from(count(node, "segments")?).unwrap_or(usize::MAX);
    let width = number(node, "width")?;
    // Where a cut meets a face (a doorway down to the floor) three rounded edges meet at a corner, and at more segments the corner patch
    // can fold into a polygon that cannot be triangulated. Try the requested number of segments, then fewer, and say so when it had to.
    let mut segments = requested;
    loop {
        let options = NetworkBevel { width, segments, profile: 0.5, offset: BevelOffset::Offset };
        let attempt = ve_geometry::bevel_network(poly, &hard, &options, context)
            .map_err(|e| poly_fail(node, &e, Some(poly)))
            .and_then(|result| value_from_poly(node, result, input.material.clone(), context));
        match attempt {
            Ok(mut value) => {
                if segments < requested {
                    value.warnings.push(crate::EvaluationWarning {
                        code: "poly.round_reduced".to_owned(),
                        node: node.id.to_string(),
                        related: Vec::new(),
                        message: format!(
                            "poly.round_edges used segments={segments} instead of {requested}: where three rounded edges meet at a cut the corner folded at more segments"
                        ),
                    });
                }
                return Ok(value);
            }
            Err(error) if segments > 1 && error.code != "eval.cancelled" && !context.is_cancelled() => segments -= 1,
            Err(error) => return Err(error),
        }
    }
}
