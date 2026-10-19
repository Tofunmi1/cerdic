//! Reading the parameters of polygon nodes.

use crate::Fail;
use ve_graph::{Node, Value as Param};

fn missing(node: &Node, name: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(
            "param.type",
            format!("parameter {name} is missing or has the wrong type"),
        )
        .at_node(&node.id),
    )
}

pub(super) fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| missing(node, name))
}

pub(super) fn int(node: &Node, name: &str) -> Result<i64, Fail> {
    match node.params.get(name) {
        Some(Param::Int(value)) => Ok(*value),
        _ => Err(missing(node, name)),
    }
}

pub(super) fn count(node: &Node, name: &str) -> Result<u32, Fail> {
    u32::try_from(int(node, name)?).map_err(|_| missing(node, name))
}

pub(super) fn flag(node: &Node, name: &str) -> Result<bool, Fail> {
    match node.params.get(name) {
        Some(Param::Bool(value)) => Ok(*value),
        _ => Err(missing(node, name)),
    }
}

pub(super) fn text<'a>(node: &'a Node, name: &str) -> Result<&'a str, Fail> {
    match node.params.get(name) {
        Some(Param::Str(value)) => Ok(value),
        _ => Err(missing(node, name)),
    }
}

pub(super) fn vector(node: &Node, name: &str) -> Result<[f64; 3], Fail> {
    match node.params.get(name) {
        Some(Param::List(items)) if items.len() == 3 => {
            let mut out = [0.0; 3];
            for (slot, item) in out.iter_mut().zip(items) {
                *slot = item.as_f64().ok_or_else(|| missing(node, name))?;
            }
            Ok(out)
        }
        _ => Err(missing(node, name)),
    }
}

pub(super) fn input_error(node: &Node, what: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error("graph.missing_input", format!("{} needs {what}", node.op))
            .at_node(&node.id),
    )
}
