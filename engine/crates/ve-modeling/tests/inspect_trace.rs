//! `Evaluator::trace`: what each node produced and what it changed. Written before the code was tuned; do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::inspect::NodeTrace;
use ve_modeling::{Evaluator, Quality};

fn trace(source: &str) -> Result<Vec<NodeTrace>, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_trace").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .trace(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

fn find<'a>(nodes: &'a [NodeTrace], label: &str) -> &'a NodeTrace {
    nodes
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no node {label}"))
}

const CHAIN: &str = "base = solid.box@1(size=[1, 1, 1])\nwide = xform.scale@1(base, by=[2, 1, 1])\nlifted = xform.move@1(wide, by=[0, 3, 0])\noutput part = lifted\n";

#[test]
fn every_node_is_recorded_in_run_order_with_its_inputs_and_what_it_made() {
    let nodes = trace(CHAIN).unwrap_or_else(|e| panic!("{e:?}"));
    let order: Vec<&str> = nodes.iter().filter_map(|n| n.label.as_deref()).collect();
    assert_eq!(order, vec!["base", "wide", "lifted"]);
    let base = find(&nodes, "base");
    assert_eq!(base.operator, "solid.box@1");
    assert_eq!(base.kind, "solid");
    assert_eq!(base.facts["triangles"], 12);
    assert_eq!(base.facts["size"], serde_json::json!([1.0, 1.0, 1.0]));
    assert!(base.inputs.is_empty());
    let wide = find(&nodes, "wide");
    assert_eq!(wide.inputs.len(), 1);
    assert_eq!(wide.inputs[0].from, "base");
    assert_eq!(wide.params["by"], serde_json::json!([2.0, 1.0, 1.0]));
}

#[test]
fn each_node_says_what_it_changed_from_its_main_input() {
    let nodes = trace(CHAIN).unwrap();
    let wide = find(&nodes, "wide");
    assert!(
        wide.effect.iter().any(|e| e == "size x 1 to 2 m"),
        "{:?}",
        wide.effect
    );
    assert!(
        wide.effect.iter().all(|e| !e.starts_with("triangles")),
        "scaling keeps the triangles: {:?}",
        wide.effect
    );
    let lifted = find(&nodes, "lifted");
    assert!(
        lifted.effect.iter().any(|e| e == "centre y 0 to 3 m"),
        "{:?}",
        lifted.effect
    );
    assert!(
        !lifted.effect.iter().any(|e| e.starts_with("size")),
        "moving does not resize: {:?}",
        lifted.effect
    );
    // The first node has no input to compare with.
    assert!(find(&nodes, "base").effect.is_empty());
}

#[test]
fn numbers_selections_and_polygon_meshes_are_described_by_what_they_are() {
    let program = "grid = poly.grid@1(width=1, height=1, columns=4, rows=4)\ntop = poly.select@1(grid, element=\"vertex\", by=\"extreme\", axis=\"y\", side=\"max\", tolerance=0.01)\nr = value.number@1(value=0.25)\nball = solid.sphere@1(radius=0.5)\n\
bigger = solid.sphere@1(ball, radius=0.5, radius=r)\noutput g = grid\n";
    // `radius=r` twice is not a thing; use a link for the number instead.
    let program = program.replace(
        "bigger = solid.sphere@1(ball, radius=0.5, radius=r)\n",
        "bigger = solid.sphere@1(radius=r)\n",
    );
    let nodes = trace(&program).unwrap_or_else(|e| panic!("{e:?}"));
    let grid = find(&nodes, "grid");
    assert_eq!(grid.kind, "poly");
    assert_eq!(grid.facts["faces"], 16);
    assert!(grid.summary.contains("polygon mesh"), "{}", grid.summary);
    let top = find(&nodes, "top");
    assert_eq!(top.kind, "selection");
    assert_eq!(top.facts["elements"], "vertices");
    assert_eq!(
        top.facts["count"], 5,
        "the 5 points along the top edge of a 4-column grid"
    );
    let number = find(&nodes, "r");
    assert_eq!(number.kind, "number");
    assert_eq!(number.facts["number"], 0.25);
    assert!(number.summary.contains("0.25"));
    // A number linked into a parameter shows as an input of the node that uses it.
    let bigger = find(&nodes, "bigger");
    assert!(
        bigger.facts["size"][0].as_f64().unwrap() > 0.4
            && bigger.facts["size"][0].as_f64().unwrap() < 0.6,
        "{:?}",
        bigger.facts
    );
}

#[test]
fn a_node_that_empties_the_selection_or_removes_geometry_says_so() {
    let nodes = trace("a = solid.box@1(size=[2, 2, 2])\nb = solid.box@1(size=[1, 1, 1])\ngone = solid.difference@1(b, a)\noutput x = gone\n").unwrap();
    let gone = find(&nodes, "gone");
    assert!(
        gone.effect.iter().any(|e| e == "removed all geometry"),
        "{:?}",
        gone.effect
    );
    assert_eq!(gone.kind, "nothing");
}

#[test]
fn a_failing_program_reports_the_failing_node_like_evaluate_does() {
    // A selection that chooses nothing refuses when it runs (selection.lost), so the trace fails at that node.
    let error = trace("g = poly.grid@1(width=1, height=1, columns=2, rows=2)\nnone = poly.select@1(g, element=\"face\", by=\"box\", box_min=[5, 5, 5], box_max=[6, 6, 6])\ndel = poly.delete_faces@1(g, none)\noutput t = del\n").err().unwrap();
    assert_eq!(error.0, "selection.lost");
    assert!(
        error.1.contains("none"),
        "the message names the node: {}",
        error.1
    );
}

#[test]
fn the_trace_is_a_pure_function_of_the_document() {
    let (a, b) = (trace(CHAIN).unwrap(), trace(CHAIN).unwrap());
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}
