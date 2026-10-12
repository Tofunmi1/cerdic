//! Planar profile operators, the solids built from them, and the wall constraint.
//!
//! Profiles travel between nodes as canonical f64 loops (never kernel handles); each operator
//! rebuilds a short-lived native profile, applies one kernel call and exports the loops again.
use crate::{Fail, cache::Value, operations::kernel_fail};
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_graph::{Diagnostic, Node, NodeId, Value as Param};
use ve_kernel::{KernelError, Profile, Solid};

/// Profile and check operators (`profile.*` and `check.min_wall`).
pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let family = node.op.family().as_str();
    let result = match family {
        "profile.polygon" => polygon(node),
        "profile.offset" => offset(node, inputs),
        "profile.difference" => difference(inputs),
        _ => return min_wall(node, inputs),
    };
    result
        .map(Value::profile)
        .map_err(|e| kernel_fail(node, &e))
}

/// `solid.extrude` and `solid.revolve`: one profile in, one solid out.
pub(crate) fn to_solid(node: &Node, inputs: &[Rc<Value>]) -> Result<Solid, KernelError> {
    let input = inputs
        .first()
        .ok_or(KernelError::InvalidArgument("missing profile"))?;
    let profile = Profile::from_loops(&input.loops)?;
    if node.op.family().as_str() == "solid.revolve" {
        return profile.revolve(whole(node, "segments")?, number(node, "angle")?);
    }
    let scale = pair(node, "top_scale")?;
    profile.extrude(
        number(node, "height")?,
        whole(node, "divisions")?,
        number(node, "twist")?,
        scale,
    )
}

fn polygon(node: &Node) -> Result<Vec<Vec<[f64; 2]>>, KernelError> {
    let Some(Param::List(items)) = node.params.get("points") else {
        return Err(KernelError::InvalidArgument("missing points"));
    };
    let points = items.iter().map(point).collect::<Result<Vec<_>, _>>()?;
    let profile = Profile::polygon(&points)?;
    profile.loops()
}

fn offset(node: &Node, inputs: &[Rc<Value>]) -> Result<Vec<Vec<[f64; 2]>>, KernelError> {
    let input = inputs
        .first()
        .ok_or(KernelError::InvalidArgument("missing profile"))?;
    let profile = Profile::from_loops(&input.loops)?;
    let delta = number(node, "delta")?;
    match node.params.get("join") {
        Some(Param::Str(join)) if join == "miter" => profile.offset(delta),
        Some(Param::Str(join)) if join == "round" => profile.offset_round(delta),
        _ => Err(KernelError::InvalidArgument("join is miter or round")),
    }?
    .loops()
}

fn difference(inputs: &[Rc<Value>]) -> Result<Vec<Vec<[f64; 2]>>, KernelError> {
    let (first, rest) = inputs
        .split_first()
        .ok_or(KernelError::InvalidArgument("missing profile"))?;
    let mut result = Profile::from_loops(&first.loops)?;
    for other in rest {
        result = result.difference(&Profile::from_loops(&other.loops)?)?;
    }
    result.loops()
}

/// Inputs grouped by port name, in the order `Node::inputs` lists them.
fn by_port<'a>(node: &'a Node, inputs: &'a [Rc<Value>]) -> BTreeMap<&'a str, &'a [Rc<Value>]> {
    let mut start = 0;
    let mut map = BTreeMap::new();
    for (port, links) in &node.inputs {
        let end = (start + links.len()).min(inputs.len());
        map.insert(port.as_str(), &inputs[start..end]);
        start = end;
    }
    map
}

fn source(node: &Node, port: &str) -> Option<NodeId> {
    node.inputs.get(port)?.first().map(|r| r.node.clone())
}

/// Largest wall for which opening the cross-section loses (almost) nothing. Opening by a disc of
/// diameter `w` removes every part of the wall thinner than `w`.
fn survives(wall: &Profile, w: f64) -> Result<bool, KernelError> {
    let opened = wall.offset(-w / 2.0)?.offset(w / 2.0)?;
    Ok(wall.area() - opened.area() <= 0.01 * (w / 2.0).powi(2))
}

fn min_wall(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let ports = by_port(node, inputs);
    let (Some(outer), Some(inner)) = (
        ports.get("outer").and_then(|v| v.first()),
        ports.get("inner").and_then(|v| v.first()),
    ) else {
        return Err(Box::new(Diagnostic::error(
            "graph.missing_input",
            "check.min_wall needs an outer and an inner profile",
        )));
    };
    let min = number(node, "min").map_err(|e| kernel_fail(node, &e))?;
    let wall = Profile::from_loops(&outer.loops)
        .and_then(|o| Profile::from_loops(&inner.loops).and_then(|i| o.difference(&i)))
        .map_err(|e| kernel_fail(node, &e))?;
    let mut lo = 0.0;
    // An empty wall (the inner profile covers the outer) is a wall of no thickness.
    if !wall.is_empty() {
        if survives(&wall, min).map_err(|e| kernel_fail(node, &e))? {
            return Ok(Value::nothing());
        }
        let mut hi = min;
        for _ in 0..12 {
            let mid = f64::midpoint(lo, hi);
            if survives(&wall, mid).map_err(|e| kernel_fail(node, &e))? {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    let related = [source(node, "outer"), source(node, "inner")]
        .into_iter()
        .flatten()
        .collect();
    Err(Box::new(
        Diagnostic::error(
            "constraint.min_wall",
            format!(
                "the wall is about {:.1} mm thick, below the required {:.1} mm",
                lo * 1000.0,
                min * 1000.0
            ),
        )
        .at_node(&node.id)
        .with_related(related)
        .with_limit(min, lo)
        .suggest("thicken the wall or lower the minimum"),
    ))
}

fn number(node: &Node, name: &str) -> Result<f64, KernelError> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or(KernelError::InvalidArgument("missing number"))
}

fn whole(node: &Node, name: &str) -> Result<u32, KernelError> {
    let value = number(node, name)?;
    if value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(KernelError::InvalidArgument("whole number"));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "range and integrality checked above"
    )]
    Ok(value as u32)
}

fn pair(node: &Node, name: &str) -> Result<[f64; 2], KernelError> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return Err(KernelError::InvalidArgument("missing pair"));
    };
    point(&Param::List(items.clone()))
}

fn point(value: &Param) -> Result<[f64; 2], KernelError> {
    let Param::List(items) = value else {
        return Err(KernelError::InvalidArgument("point"));
    };
    match items.as_slice() {
        [x, y] => Ok([
            x.as_f64().ok_or(KernelError::InvalidArgument("point"))?,
            y.as_f64().ok_or(KernelError::InvalidArgument("point"))?,
        ]),
        _ => Err(KernelError::InvalidArgument("point")),
    }
}
