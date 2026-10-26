//! One compact statement per stable node ID, including members of group definitions.

use std::collections::BTreeMap;

use crate::{Diagnostic, Document, Node, NodeId, OperatorRegistry};

/// Prints standalone compact node lines without folding pipe stages. Group members are qualified
/// by their group name so a revision difference can identify their scope.
///
/// # Errors
/// Returns the same unknown-operator and unsupported-instance diagnostics as `to_text`.
pub fn to_text_node_lines(
    registry: &OperatorRegistry,
    doc: &Document,
) -> Result<BTreeMap<NodeId, String>, Box<Diagnostic>> {
    let mut result = BTreeMap::new();
    for scope in std::iter::once(None).chain(doc.groups.iter().map(|group| Some(&group.id))) {
        let nodes: Vec<&Node> = doc
            .nodes
            .iter()
            .filter(|node| node.group.as_ref() == scope)
            .collect();
        let names = super::names(&nodes);
        for node in nodes {
            let line = if let Some(instance) = &node.instance {
                super::instance_statement(doc, registry, node, instance, &names, true)?
            } else {
                let spec = registry.get(&node.op).ok_or_else(|| {
                    Box::new(
                        Diagnostic::error(
                            "parse.unknown_op",
                            format!("{} is not registered", node.op),
                        )
                        .at_node(&node.id),
                    )
                })?;
                super::statement(node, spec, &names, true, registry)
            };
            let line = match scope.and_then(|id| doc.group(id)) {
                Some(group) => format!("{}: {line}", group.name),
                None => line,
            };
            result.insert(node.id.clone(), line);
        }
    }
    Ok(result)
}
