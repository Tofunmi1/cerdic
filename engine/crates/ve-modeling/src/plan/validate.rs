//! Checks node parameters and ports before geometry evaluation.
use crate::{Fail, fail, fail_suggest};
use std::collections::BTreeMap;
use ve_graph::{Arity, Node, NodeId, PortType};

pub(super) fn validate(
    node: &Node,
    spec: &ve_graph::OperatorSpec,
    types: &BTreeMap<NodeId, PortType>,
) -> Result<(), Fail> {
    for (name, value) in &node.params {
        let param = spec
            .param(name)
            .ok_or_else(|| fail("parse.unknown_param", name))?;
        param
            .ty
            .check(value)
            .map_err(|e| Box::new(ve_graph::Diagnostic::from_issue(e, Some(&node.id), name)))?;
    }
    for param in &spec.params {
        // A param added to an operator later has a default; a document written before it (a saved one, one parsed by an older build) does not carry it.
        if !node.params.contains_key(param.name)
            && param.default.is_none()
            && !spec.has_input_default(param.name)
        {
            return Err(fail("parse.missing_param", param.name));
        }
    }
    for (name, links) in &node.inputs {
        if spec.input(name).is_some() {
            continue;
        }
        // A number socket: a link into a number parameter, which must come from a value node.
        if spec.socket(name).is_none() {
            return Err(fail("graph.unknown_port", name));
        }
        for link in links {
            if link.port != "out" || types.get(&link.node) != Some(&PortType::Number) {
                return Err(fail_suggest(
                    "graph.port_type",
                    format!(
                        "{} gets {name} from a node that does not produce a number",
                        node.id
                    ),
                    "link a value.number, value.math, value.size or value.point node, or type the number in the parameter",
                ));
            }
        }
    }
    if node.op.family().as_str() == "xform.align" {
        crate::assembly::alignment(node).map_err(|e| crate::operations::kernel_fail(node, &e))?;
    }
    for port in &spec.inputs {
        let links = node.inputs.get(port.name).map_or(&[][..], Vec::as_slice);
        let valid = match port.arity {
            Arity::One => links.len() == 1,
            Arity::Optional => links.len() <= 1,
            Arity::Variadic { min } => (usize::from(min)..=64).contains(&links.len()),
        };
        if !valid {
            return Err(fail_suggest(
                "graph.missing_input",
                format!("{} requires {}", node.id, port.name),
                format!("link the {} input of {}", port.name, node.id),
            ));
        }
        for link in links {
            if link.port != "out"
                || !types
                    .get(&link.node)
                    .is_some_and(|kind| port.accepts.contains(kind))
            {
                return Err(fail_suggest(
                    "graph.port_type",
                    format!("incompatible input on {}", node.id),
                    format!("link a value the {} input accepts", port.name),
                ));
            }
        }
    }
    Ok(())
}
