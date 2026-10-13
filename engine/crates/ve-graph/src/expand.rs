//! Unfolding groups before evaluation (ADR 0013).
//!
//! [`expand`] turns a document with groups into a flat document without them, which is what the
//! planner and the evaluator work on. Top-level nodes keep their IDs. An instance is replaced by copies
//! of the members its group's result depends on: the copy of the result takes the instance's own ID, so
//! whatever reads or outputs the instance, and the part shown for it, keep their identity; the other
//! copies get synthetic IDs and a table says which instance and member each came from. Overrides are
//! applied to the copies, group inputs are bound to what the instance is linked to, and members the
//! result does not depend on are left out.

use std::collections::{BTreeMap, BTreeSet};

use crate::diag::Diagnostic;
use crate::doc::{Document, Flags, Node, PortRef};
use crate::group::{INPUT_OP, Instance};
use crate::group_check::structure;
use crate::id::NodeId;
use crate::limits::{MAX_EXPANDED_NODES, MAX_GROUP_DEPTH, count};
use crate::spec::OpId;
use crate::value::Value;

/// Where a node of the flat document came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The top-level instance whose expansion produced the node.
    pub instance: NodeId,
    /// The member of a group definition it is a copy of.
    pub member: NodeId,
}

/// A document with its groups unfolded.
#[derive(Clone, Debug)]
pub struct Expanded {
    /// The flat document: no groups, no instances, no members.
    pub doc: Document,
    /// The instance and member behind each node that came from a group.
    pub origins: BTreeMap<NodeId, Origin>,
}

type Fail = Box<Diagnostic>;

/// What a group's members are expanded against.
struct Frame {
    /// The top-level instance, for the origin table.
    outermost: NodeId,
    /// The flat node each `group.input` name is bound to.
    bindings: BTreeMap<String, NodeId>,
    /// Overridden parameter values by member and parameter.
    overrides: BTreeMap<(NodeId, String), Value>,
    /// The member whose copy takes `out_id`.
    output: NodeId,
    out_id: NodeId,
    depth: usize,
}

struct Builder<'a> {
    source: &'a Document,
    nodes: Vec<Node>,
    origins: BTreeMap<NodeId, Origin>,
    used: BTreeSet<NodeId>,
    next: u64,
}

/// Unfolds the groups of `doc`.
///
/// # Errors
/// A structure diagnostic (`group.unknown`, `group.recursive`, `budget.group_depth`), `group.outdated`
/// for an instance behind its group, `graph.missing_input` for a group input nothing is linked to, or
/// `budget.expanded_nodes` when the result would be too large.
pub fn expand(doc: &Document) -> Result<Expanded, Fail> {
    if doc.groups.is_empty()
        && doc
            .nodes
            .iter()
            .all(|n| n.group.is_none() && n.instance.is_none())
    {
        return Ok(Expanded {
            doc: doc.clone(),
            origins: BTreeMap::new(),
        });
    }
    structure(doc)?;
    let mut builder = Builder {
        source: doc,
        nodes: Vec::new(),
        origins: BTreeMap::new(),
        used: doc.nodes.iter().map(|n| n.id.clone()).collect(),
        next: doc.ids.next,
    };
    let top: Vec<&Node> = doc.nodes.iter().filter(|n| n.group.is_none()).collect();
    let roots: Vec<&NodeId> = top.iter().map(|n| &n.id).collect();
    let order = order_of(&top, &roots);
    builder.emit(&order, None)?;
    let mut flat = doc.clone();
    flat.nodes = builder.nodes;
    flat.groups = Vec::new();
    Ok(Expanded {
        doc: flat,
        origins: builder.origins,
    })
}

/// The nodes reachable from `roots` among `scope`, each after the nodes it reads.
fn order_of<'a>(scope: &[&'a Node], roots: &[&NodeId]) -> Vec<&'a Node> {
    let by_id: BTreeMap<&NodeId, &Node> = scope.iter().map(|n| (&n.id, *n)).collect();
    let mut done: BTreeSet<&NodeId> = BTreeSet::new();
    let mut out = Vec::new();
    for root in roots {
        let mut stack: Vec<(&NodeId, Vec<&NodeId>)> = Vec::new();
        if done.insert(root)
            && let Some(node) = by_id.get(root)
        {
            stack.push((root, reads(node)));
        }
        while let Some((id, pending)) = stack.last_mut() {
            if let Some(next) = pending.pop() {
                if by_id.contains_key(next) && done.insert(next) {
                    let children = by_id.get(next).map(|n| reads(n)).unwrap_or_default();
                    stack.push((next, children));
                }
            } else {
                if let Some(node) = by_id.get(*id) {
                    out.push(*node);
                }
                stack.pop();
            }
        }
    }
    out
}

fn reads(node: &Node) -> Vec<&NodeId> {
    let mut sources: Vec<&NodeId> = node.inputs.values().flatten().map(|r| &r.node).collect();
    sources.reverse(); // popped from the end, so the first input is visited first
    sources
}

impl Builder<'_> {
    fn fresh(&mut self) -> NodeId {
        loop {
            let id = NodeId::generated(self.next, self.source.ids.salt);
            self.next += 1;
            if self.used.insert(id.clone()) {
                return id;
            }
        }
    }

    fn push(&mut self, node: Node, origin: Option<(&NodeId, &NodeId)>) -> Result<(), Fail> {
        if self.nodes.len() >= MAX_EXPANDED_NODES {
            return Err(Box::new(
                Diagnostic::error(
                    "budget.expanded_nodes",
                    "the groups unfold into too many nodes",
                )
                .with_limit(count(MAX_EXPANDED_NODES), count(self.nodes.len() + 1)),
            ));
        }
        if let Some((instance, member)) = origin {
            self.origins.insert(
                node.id.clone(),
                Origin {
                    instance: instance.clone(),
                    member: member.clone(),
                },
            );
        }
        self.nodes.push(node);
        Ok(())
    }

    /// Emits the nodes of one scope in order: the top level when `frame` is `None`, else the members of
    /// one instance of a group.
    fn emit(&mut self, order: &[&Node], frame: Option<&Frame>) -> Result<(), Fail> {
        let mut flat: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        for node in order {
            let id = match frame {
                None => node.id.clone(),
                Some(f) if node.id == f.output => f.out_id.clone(),
                Some(_) => self.fresh(),
            };
            let inputs = rewire(node, &flat);
            if node.op.family().as_str() == INPUT_OP {
                let name = match node.params.get("name") {
                    Some(Value::Str(name)) => name.clone(),
                    _ => String::new(),
                };
                let bound = frame.and_then(|f| f.bindings.get(&name)).ok_or_else(|| {
                    Box::new(
                        Diagnostic::error(
                            "graph.missing_input",
                            format!("the group input {name:?} has nothing linked to it"),
                        )
                        .at_node(frame.map_or(&node.id, |f| &f.outermost)),
                    )
                })?;
                flat.insert(node.id.clone(), bound.clone());
            } else if let Some(instance) = &node.instance {
                flat.insert(node.id.clone(), id.clone());
                let depth = frame.map_or(1, |f| f.depth + 1);
                let outermost = frame.map_or(&node.id, |f| &f.outermost);
                self.instance(node, instance, &id, &inputs, outermost, depth)?;
            } else {
                flat.insert(node.id.clone(), id.clone());
                let mut copy = Node {
                    id: id.clone(),
                    group: None,
                    instance: None,
                    inputs,
                    ..(*node).clone()
                };
                if let Some(f) = frame {
                    for (param, value) in &mut copy.params {
                        if let Some(over) = f.overrides.get(&(node.id.clone(), param.clone())) {
                            value.clone_from(over);
                        }
                    }
                    self.push(copy, Some((&f.outermost, &node.id)))?;
                } else {
                    self.push(copy, None)?;
                }
            }
        }
        Ok(())
    }

    /// Replaces an instance by copies of what its group's result depends on, the result taking `out_id`.
    fn instance(
        &mut self,
        node: &Node,
        instance: &Instance,
        out_id: &NodeId,
        inputs: &BTreeMap<String, Vec<PortRef>>,
        outermost: &NodeId,
        depth: usize,
    ) -> Result<(), Fail> {
        let source = self.source;
        let group = source.group(&instance.group).ok_or_else(|| {
            Box::new(Diagnostic::error("group.unknown", "the group is missing").at_node(outermost))
        })?;
        if instance.version != group.version {
            return Err(Box::new(
                Diagnostic::error(
                    "group.outdated",
                    format!(
                        "{} was made against version {} of its group, which is now version {}",
                        node.id, instance.version, group.version
                    ),
                )
                .at_node(outermost)
                .with_related(vec![node.id.clone()])
                .suggest("upgrade the instance"),
            ));
        }
        if depth > MAX_GROUP_DEPTH {
            return Err(Box::new(
                Diagnostic::error("budget.group_depth", "groups are nested too deeply")
                    .with_limit(count(MAX_GROUP_DEPTH), count(depth)),
            ));
        }
        let bindings: BTreeMap<String, NodeId> = inputs
            .iter()
            .filter_map(|(port, refs)| refs.first().map(|r| (port.clone(), r.node.clone())))
            .collect();
        if node.flags.muted || node.flags.bypass {
            return self.placeholder(node, out_id, &bindings, outermost);
        }
        let overrides: BTreeMap<(NodeId, String), Value> = instance
            .overrides
            .iter()
            .filter_map(|(name, value)| {
                let exposed = group.params.iter().find(|p| &p.name == name)?;
                Some(((exposed.node.clone(), exposed.param.clone()), value.clone()))
            })
            .collect();
        let members: Vec<&Node> = source.members(&group.id).collect();
        let order = order_of(&members, &[&group.output]);
        let frame = Frame {
            outermost: outermost.clone(),
            bindings,
            overrides,
            output: group.output.clone(),
            out_id: out_id.clone(),
            depth,
        };
        self.emit(&order, Some(&frame))
    }

    /// A muted instance has no result; a bypassed one passes its first input through.
    fn placeholder(
        &mut self,
        node: &Node,
        out_id: &NodeId,
        bindings: &BTreeMap<String, NodeId>,
        outermost: &NodeId,
    ) -> Result<(), Fail> {
        let through = node
            .flags
            .bypass
            .then(|| bindings.values().next())
            .flatten()
            .filter(|_| !node.flags.muted);
        let (op, params, muted, inputs) = match through {
            Some(source) => (
                "xform.move@1",
                BTreeMap::from([("by".to_owned(), Value::List(vec![Value::Num(0.0); 3]))]),
                false,
                BTreeMap::from([(
                    "in".to_owned(),
                    vec![PortRef {
                        node: source.clone(),
                        port: crate::doc::OUT_PORT.to_owned(),
                    }],
                )]),
            ),
            None => (
                "solid.box@1",
                BTreeMap::from([("size".to_owned(), Value::List(vec![Value::Num(1.0); 3]))]),
                true,
                BTreeMap::new(),
            ),
        };
        let op = OpId::parse(op).map_err(|e| {
            Box::new(Diagnostic::error("parse.unknown_op", e.to_string()).at_node(outermost))
        })?;
        let copy = Node {
            id: out_id.clone(),
            op,
            label: None,
            params,
            inputs,
            flags: Flags {
                muted,
                bypass: false,
            },
            group: None,
            instance: None,
        };
        self.push(copy, Some((outermost, &node.id)))
    }
}

/// The node's links with their sources replaced by the flat nodes they became.
fn rewire(node: &Node, flat: &BTreeMap<NodeId, NodeId>) -> BTreeMap<String, Vec<PortRef>> {
    node.inputs
        .iter()
        .map(|(port, refs)| {
            let mapped = refs
                .iter()
                .map(|r| PortRef {
                    node: flat.get(&r.node).cloned().unwrap_or_else(|| r.node.clone()),
                    port: r.port.clone(),
                })
                .collect();
            (port.clone(), mapped)
        })
        .collect()
}
