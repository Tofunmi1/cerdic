//! The node, output, material and editor refusals each carry a suggestion naming the limit or port
//! to change (see `docs/modeling/diagnostics.md`).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and panic when a batch is not refused"
)]

use std::collections::BTreeMap;

use ve_graph::{
    Alias, Arity, Command, CommandBatch, Diagnostic, Document, InputRef, NodeRef, OperatorRegistry,
    OperatorSpec, OutRef, Outcome, OutputSpec, PortSpec, PortType, Readiness, Session,
    TransactionId, Value,
};

/// The one value type every test port accepts.
const SOLID: &[PortType] = &[PortType::Solid];

fn session() -> Session {
    Session::with_builtin(7)
}

fn alias(name: &str) -> Alias {
    Alias::parse(name).unwrap()
}

fn batch(session: &Session, transaction: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(transaction).unwrap(),
        label: None,
        commands,
    }
}

/// Adds one node with no parameters, optionally under an alias.
fn add(op: &str, as_alias: Option<&str>) -> Command {
    Command::AddNode {
        op: op.to_owned(),
        params: BTreeMap::new(),
        label: None,
        alias: as_alias.map(alias),
        group: None,
    }
}

fn commit(session: &mut Session, transaction: &str, commands: Vec<Command>) {
    let outcome = session.apply(&batch(session, transaction, commands));
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
}

/// Applies the commands, expecting a rejection, and returns its first diagnostic.
fn first_rejection(session: &mut Session, transaction: &str, commands: Vec<Command>) -> Diagnostic {
    let outcome = session.apply(&batch(session, transaction, commands));
    let Outcome::Rejected { mut diagnostics } = outcome else {
        panic!("expected a rejection, got {outcome:?}");
    };
    diagnostics.remove(0)
}

fn assert_suggested(diagnostic: &Diagnostic, code: &str, keyword: &str) {
    assert_eq!(diagnostic.code, code);
    let suggestion = diagnostic
        .suggestion
        .as_deref()
        .unwrap_or_else(|| panic!("no suggestion for {code}"));
    assert!(
        suggestion.to_lowercase().contains(&keyword.to_lowercase()),
        "suggestion {suggestion:?} does not name {keyword:?} for {code}"
    );
}

/// An operator with the given input ports, no parameters and a solid result.
fn spec(id: &str, inputs: &[&'static str]) -> OperatorSpec {
    OperatorSpec {
        id: ve_graph::OpId::parse(id).unwrap(),
        doc: "a test operator",
        inputs: inputs
            .iter()
            .copied()
            .map(|name| PortSpec {
                name,
                accepts: SOLID,
                arity: Arity::One,
            })
            .collect(),
        output: OutputSpec::Fixed(PortType::Solid),
        params: Vec::new(),
        readiness: Readiness::Draft,
    }
}

#[test]
fn a_label_over_the_limit_names_the_byte_limit() {
    let mut s = session();
    commit(&mut s, "t_label_add", vec![add("solid.union@1", None)]);
    let id = s.document().nodes[0].id.clone();
    let diagnostic = first_rejection(
        &mut s,
        "t_label_set",
        vec![Command::SetLabel {
            node: NodeRef::Id(id),
            label: Some("x".repeat(300)),
        }],
    );
    assert_suggested(&diagnostic, "param.range", "256");
}

#[test]
fn too_many_nodes_names_the_node_limit() {
    let mut s = session();
    for batch_number in 0..5 {
        let commands: Vec<Command> = (0..1000).map(|_| add("solid.union@1", None)).collect();
        commit(&mut s, &format!("t_nodes_{batch_number}"), commands);
    }
    assert_eq!(s.document().nodes.len(), 5000);
    let diagnostic = first_rejection(&mut s, "t_nodes_over", vec![add("solid.union@1", None)]);
    assert_suggested(&diagnostic, "budget.nodes", "5000");
}

#[test]
fn a_zero_op_version_names_the_version_floor() {
    let mut s = session();
    let diagnostic = first_rejection(
        &mut s,
        "t_version_zero",
        vec![
            add("solid.union@1", Some("$n")),
            Command::SetOpVersion {
                node: NodeRef::Alias(alias("$n")),
                version: 0,
            },
        ],
    );
    assert_suggested(&diagnostic, "parse.unknown_op", "version");
}

#[test]
fn dropping_a_linked_input_port_lists_the_real_ports() {
    let mut registry = OperatorRegistry::new();
    registry.register(spec("test.src@1", &[])).unwrap();
    registry.register(spec("test.thing@1", &["in"])).unwrap();
    registry.register(spec("test.thing@2", &["other"])).unwrap();
    let mut s = Session::new(registry, Document::new(7));
    let diagnostic = first_rejection(
        &mut s,
        "t_version_port",
        vec![
            add("test.src@1", Some("$src")),
            add("test.thing@1", Some("$thing")),
            Command::Connect {
                from: OutRef::out(NodeRef::Alias(alias("$src"))),
                to: InputRef {
                    node: NodeRef::Alias(alias("$thing")),
                    port: "in".to_owned(),
                    index: None,
                },
            },
            Command::SetOpVersion {
                node: NodeRef::Alias(alias("$thing")),
                version: 2,
            },
        ],
    );
    assert_suggested(&diagnostic, "graph.unknown_port", "other");
}

#[test]
fn an_output_on_an_input_port_names_the_output_port() {
    let mut s = session();
    let diagnostic = first_rejection(
        &mut s,
        "t_output_port",
        vec![
            add("solid.union@1", Some("$n")),
            Command::SetOutput {
                name: "o".to_owned(),
                from: Some(OutRef {
                    node: NodeRef::Alias(alias("$n")),
                    port: "in".to_owned(),
                }),
            },
        ],
    );
    assert_suggested(&diagnostic, "graph.unknown_port", "out");
}

#[test]
fn too_many_outputs_names_the_output_limit() {
    let mut s = session();
    let mut commands = vec![add("solid.union@1", Some("$n"))];
    for index in 0..65 {
        commands.push(Command::SetOutput {
            name: format!("o{index}"),
            from: Some(OutRef::out(NodeRef::Alias(alias("$n")))),
        });
    }
    let diagnostic = first_rejection(&mut s, "t_outputs_over", commands);
    assert_suggested(&diagnostic, "budget.nodes", "64");
}

#[test]
fn too_many_materials_names_the_material_limit() {
    let mut s = session();
    let commands: Vec<Command> = (0..257)
        .map(|_| Command::DefineMaterial {
            def: BTreeMap::from([("albedo".to_owned(), Value::Num(1.0))]),
        })
        .collect();
    let diagnostic = first_rejection(&mut s, "t_materials_over", commands);
    assert_suggested(&diagnostic, "budget.nodes", "256");
}

#[test]
fn an_oversized_material_definition_names_the_entry_limit() {
    let mut s = session();
    let def: BTreeMap<String, Value> = (0..65)
        .map(|index| (format!("k{index}"), Value::Num(1.0)))
        .collect();
    let diagnostic = first_rejection(
        &mut s,
        "t_material_entries",
        vec![Command::DefineMaterial { def }],
    );
    assert_suggested(&diagnostic, "param.range", "64");
}

#[test]
fn a_position_past_the_editor_extent_names_the_coordinate_limit() {
    let mut s = session();
    let diagnostic = first_rejection(
        &mut s,
        "t_position_range",
        vec![
            add("solid.union@1", Some("$n")),
            Command::EditorSetPosition {
                node: NodeRef::Alias(alias("$n")),
                x: 2.0e6,
                y: 0.0,
            },
        ],
    );
    assert_suggested(&diagnostic, "param.range", "1000000");
}
