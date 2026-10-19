//! Authored curves and deterministic sweep mesh construction.
mod closed;
mod loft;
mod loft_blend;
mod mesh_overlap;
mod overlap;
mod rings;
mod sample;
mod sweep;
mod tools;
mod tube;
mod vec3;

use crate::{
    Fail,
    cache::{Curve3, Value},
    fail,
};
use ve_graph::{Node, Value as Param};

pub(crate) fn evaluate(node: &Node) -> Result<Value, Fail> {
    let family = node.op.family().as_str();
    if family == "curve.circle" {
        return Ok(Value::curve(Curve3 {
            points: sample::circle(
                vector(node, "center", [0.0; 3])?,
                number(node, "radius", 1.0)?,
                count(node, "segments"),
            )?,
            closed: true,
        }));
    }
    let closed = matches!(node.params.get("closed"), Some(Param::Bool(true)));
    let controls = points(node)?;
    if closed && family == "curve.bezier" && distance(controls[0], controls[3]) > 1.0e-9 {
        return Err(curve_error(
            node,
            "geometry.invalid_curve",
            "a closed cubic Bézier must end where it starts",
            "make the first and last points equal, or set closed=false",
        ));
    }
    let mut sampled = if family == "curve.bezier" {
        sample::bezier(&controls, count(node, "steps"))
    } else {
        Ok(controls)
    }?;
    if closed && family == "curve.polyline" {
        if sampled.len() > 2
            && distance(sampled[0], *sampled.last().unwrap_or(&sampled[0])) <= 1.0e-9
        {
            sampled.pop();
        }
    } else if closed {
        sampled.pop();
    }
    if closed && sampled.len() < 3 {
        return Err(curve_error(
            node,
            "geometry.invalid_curve",
            "a closed curve needs at least three unique samples",
            "add more points, or set closed=false",
        ));
    }
    if sampled
        .windows(2)
        .any(|pair| distance(pair[0], pair[1]) <= 1.0e-9)
        || (closed && distance(sampled[0], *sampled.last().unwrap_or(&sampled[0])) <= 1.0e-9)
    {
        return Err(curve_error(
            node,
            "geometry.degenerate_curve",
            "curve has a repeated consecutive point",
            "remove the repeated point from `points`",
        ));
    }
    Ok(Value::curve(Curve3 {
        points: sampled,
        closed,
    }))
}

fn curve_error(node: &Node, code: &str, message: &str, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

pub(crate) fn run(
    node: &Node,
    inputs: &[std::rc::Rc<Value>],
    context: &ve_kernel::ExecutionContext,
) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "curve.polyline" | "curve.bezier" | "curve.circle" => Some(evaluate(node)),
        family if tools::handles(family) => Some(tools::build(node, inputs)),
        "solid.sweep" => Some(sweep(node, inputs, context)),
        "curve.tube" => Some(tube::build(node, inputs, context)),
        "solid.loft" => Some(loft(node, inputs, context)),
        _ => None,
    }
}

pub(crate) fn sweep(
    node: &Node,
    inputs: &[std::rc::Rc<Value>],
    context: &ve_kernel::ExecutionContext,
) -> Result<Value, Fail> {
    sweep::build(node, inputs, context)
}

/// Rows a loft builds per span (1 to 8), for cost estimates.
pub(crate) fn loft_steps(node: &Node) -> usize {
    loft::steps(node)
}

pub(crate) fn loft(
    node: &Node,
    inputs: &[std::rc::Rc<Value>],
    context: &ve_kernel::ExecutionContext,
) -> Result<Value, Fail> {
    loft::build(node, inputs, context)
}

/// Whether the node is one of the curve generators and modifiers in `tools`.
pub(crate) fn is_tool(family: &str) -> bool {
    tools::handles(family)
}

/// Samples such a node makes, given the samples of its input curve.
pub(crate) fn estimate_tool(node: &Node, input_samples: usize) -> usize {
    tools::estimate(node, input_samples)
}

pub(crate) fn estimate_curve(node: &Node) -> usize {
    if node.op.family().as_str() == "curve.bezier" {
        count(node, "steps").saturating_add(1)
    } else if node.op.family().as_str() == "curve.circle" {
        count(node, "segments")
    } else {
        match node.params.get("points") {
            Some(Param::List(points)) => points.len(),
            _ => 0,
        }
    }
}

fn number(node: &Node, name: &str, default: f64) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| fail("param.type", "circle radius is not numeric"))
        .or_else(|_| {
            if node.params.contains_key(name) {
                Err(fail("param.type", "circle radius is not numeric"))
            } else {
                Ok(default)
            }
        })
}

fn vector(node: &Node, name: &str, default: [f64; 3]) -> Result<[f64; 3], Fail> {
    let Some(Param::List(values)) = node.params.get(name) else {
        return Ok(default);
    };
    if values.len() != 3 {
        return Err(fail(
            "param.type",
            "circle center must have three components",
        ));
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .ok_or_else(|| fail("param.type", "circle center component is not numeric"))?;
    }
    Ok(result)
}

fn points(node: &Node) -> Result<Vec<[f64; 3]>, Fail> {
    let Some(Param::List(points)) = node.params.get("points") else {
        return Err(fail("param.type", "curve points are missing"));
    };
    points
        .iter()
        .map(|point| {
            let Param::List(coordinates) = point else {
                return Err(fail("param.type", "expected 3D point"));
            };
            if coordinates.len() != 3 {
                return Err(fail("param.type", "expected 3D point"));
            }
            let mut out = [0.0; 3];
            for (index, component) in coordinates.iter().enumerate() {
                out[index] = component
                    .as_f64()
                    .ok_or_else(|| fail("param.type", "point component is not numeric"))?;
            }
            Ok(out)
        })
        .collect()
}

fn count(node: &Node, name: &str) -> usize {
    match node.params.get(name) {
        Some(Param::Int(value)) => usize::try_from(*value).unwrap_or(12),
        _ => 12,
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
