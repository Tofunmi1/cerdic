use std::collections::BTreeSet;

use super::{Kind, Node};
use crate::Registry;

pub(super) fn unknown_names(root: &Node, registry: &Registry) -> Vec<String> {
    let mut names = BTreeSet::new();
    collect_names(root, registry, &mut names);
    names.into_iter().collect()
}

fn collect_names(node: &Node, registry: &Registry, names: &mut BTreeSet<String>) {
    match &node.kind {
        Kind::Condition(name) if !registry.has_condition(name) => {
            names.insert(name.clone());
        }
        Kind::Action(name) if !registry.has_action(name) => {
            names.insert(name.clone());
        }
        Kind::Sequence(children) | Kind::Selector(children) | Kind::Parallel(_, children) => {
            for child in children {
                collect_names(child, registry, names);
            }
        }
        Kind::Inverter(child)
        | Kind::Succeeder(child)
        | Kind::Repeat(_, child)
        | Kind::Cooldown(_, child) => collect_names(child, registry, names),
        _ => {}
    }
}
